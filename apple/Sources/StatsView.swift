import Charts
import SwiftUI

/// How the stats screen buckets results.
enum StatsPeriod: String, CaseIterable, Identifiable {
    case week = "Weekly"
    case month = "Four-weekly"
    case year = "Yearly"

    var id: String { rawValue }
}

/// One bucket of a company's results: a week, a four-week period or a year, with balances at its
/// close when the engine kept them.
struct ResultBucket: Identifiable {
    let label: String
    let revenue: Int64
    let costOfGoods: Int64
    let wages: Int64
    let operating: Int64
    let depreciation: Int64
    let interest: Int64
    let tax: Int64
    let netIncome: Int64
    var cash: Int64?
    var equity: Int64?
    var liabilities: Int64?
    var price: Int64?
    var id: String { label }

    var operatingProfit: Int64 { revenue - costOfGoods - wages - operating - depreciation }
    var margin: Double? { revenue > 0 ? Double(netIncome) / Double(revenue) : nil }
}

enum ResultBuckets {
    /// Game weeks to a calendar label: the game year is exactly 52 weeks, thirteen periods of four.
    static func weekLabel(_ week: UInt32) -> String {
        let w = max(Int(week), 1)
        return "W\(String(format: "%02d", (w - 1) % 52 + 1)) \(2000 + (w - 1) / 52)"
    }

    static func make(_ period: StatsPeriod, weeks: [WeekDto], history: [PeriodDto]) -> [ResultBucket] {
        switch period {
        case .week:
            return weeks.map { w in
                ResultBucket(
                    label: weekLabel(w.week),
                    revenue: w.revenueCents,
                    costOfGoods: w.costOfGoodsCents,
                    wages: w.wagesCents,
                    operating: w.operatingExpenseCents,
                    depreciation: w.depreciationCents,
                    interest: w.netInterestCents,
                    tax: w.taxCents,
                    netIncome: w.netIncomeCents
                )
            }
        case .month:
            return history.suffix(13).map { p in
                let w = max(Int(p.firstWeek), 1)
                return bucket(label: "P\((w - 1) % 52 / 4 + 1) \(2000 + (w - 1) / 52)", [p])
            }
        case .year:
            let years = Dictionary(grouping: history) { (max(Int($0.firstWeek), 1) - 1) / 52 }
            return years.keys.sorted().map { year in
                bucket(label: String(2000 + year), years[year] ?? [])
            }
        }
    }

    private static func bucket(label: String, _ periods: [PeriodDto]) -> ResultBucket {
        func sum(_ value: (PeriodDto) -> Int64) -> Int64 { periods.reduce(0) { $0 + value($1) } }
        let last = periods.last
        return ResultBucket(
            label: label,
            revenue: sum(\.revenueCents),
            costOfGoods: sum(\.costOfGoodsCents),
            wages: sum(\.wagesCents),
            operating: sum(\.operatingExpenseCents),
            depreciation: sum(\.depreciationCents),
            interest: sum(\.netInterestCents),
            tax: sum(\.taxCents),
            netIncome: sum(\.netIncomeCents),
            cash: last?.cashCents,
            equity: last?.equityCents,
            liabilities: last?.liabilitiesCents,
            price: last?.sharePriceCents
        )
    }
}

/// One line of the income statement.
struct StatementRow: Identifiable {
    let title: String
    let total: Bool
    let value: (ResultBucket) -> Int64
    var id: String { title }
}

/// The selected company's numbers in depth: results by week, period or year, the income
/// statement, the balance sheet over time, the share price and market value, and who owns it.
struct StatsView: View {
    @EnvironmentObject private var store: AppStore
    @State private var period: StatsPeriod = .month

    private var firm: FirmDto? { store.selectedFirmDto }
    private var stock: StockDto? { firm.flatMap { f in store.stocks.first { $0.firm == f.handle } } }

    private var buckets: [ResultBucket] {
        ResultBuckets.make(period, weeks: store.finance?.recentWeeks ?? [], history: store.history)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Picker("Period", selection: $period) {
                    ForEach(StatsPeriod.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)

                if buckets.isEmpty {
                    EmptyNote(text: "Results appear after the first week closes.")
                } else {
                    resultsChart
                    statement
                    if period != .week {
                        balanceChart
                    }
                }
                if let stock {
                    shares(stock)
                }
                if let ownership = store.ownership, ownership.firm == firm?.handle {
                    owners(ownership)
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .navigationTitle(firm.map { "\($0.name) stats" } ?? "Stats")
        .navigationBarTitleDisplayMode(.inline)
        .task { await store.loadHistory() }
        .onAppear { store.tutorial.visited(.stats) }
    }

    private var resultsChart: some View {
        SectionCard(title: "Revenue and profit") {
            Chart {
                ForEach(buckets) { b in
                    BarMark(x: .value("Period", b.label), y: .value("Revenue", Double(b.revenue) / 100))
                        .foregroundStyle(Theme.gold.opacity(0.55))
                        .position(by: .value("Series", "Revenue"))
                    BarMark(x: .value("Period", b.label), y: .value("Net income", Double(b.netIncome) / 100))
                        .foregroundStyle(b.netIncome >= 0 ? Theme.positive : Theme.negative)
                        .position(by: .value("Series", "Net income"))
                }
            }
            .chartXAxis {
                AxisMarks(values: .automatic(desiredCount: 6)) { _ in
                    AxisValueLabel().font(.caption2)
                }
            }
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine().foregroundStyle(Theme.stroke)
                    AxisValueLabel {
                        if let dollars = value.as(Double.self) {
                            Text(Format.moneyCompact(Int64(dollars * 100))).font(.caption2)
                        }
                    }
                }
            }
            .frame(height: 200)
            HStack(spacing: 14) {
                legend(Theme.gold.opacity(0.55), "Revenue")
                legend(Theme.positive, "Net income")
                legend(Theme.negative, "Loss")
            }
        }
    }

    private func legend(_ color: Color, _ text: String) -> some View {
        HStack(spacing: 5) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 10, height: 10)
            Text(text).font(.caption2).foregroundStyle(Theme.secondary)
        }
    }

    /// The latest bucket's income statement beside the one before it.
    private var statement: some View {
        let latest = buckets[buckets.count - 1]
        let previous = buckets.count > 1 ? buckets[buckets.count - 2] : nil
        let rows = [
            StatementRow(title: "Revenue", total: false) { $0.revenue },
            StatementRow(title: "Cost of goods", total: false) { -$0.costOfGoods },
            StatementRow(title: "Wages", total: false) { -$0.wages },
            StatementRow(title: "Operating costs", total: false) { -$0.operating },
            StatementRow(title: "Depreciation", total: false) { -$0.depreciation },
            StatementRow(title: "Operating profit", total: true) { $0.operatingProfit },
            StatementRow(title: "Interest", total: false) { $0.interest },
            StatementRow(title: "Tax", total: false) { -$0.tax },
            StatementRow(title: "Net income", total: true) { $0.netIncome },
        ]
        return SectionCard(title: "Income statement") {
            Grid(alignment: .trailing, horizontalSpacing: 12, verticalSpacing: 6) {
                GridRow {
                    Text("").gridColumnAlignment(.leading)
                    Text(latest.label).font(.caption.weight(.semibold)).foregroundStyle(Theme.secondary)
                    if let previous {
                        Text(previous.label).font(.caption.weight(.semibold)).foregroundStyle(Theme.secondary)
                    }
                }
                ForEach(rows) { row in
                    GridRow {
                        Text(row.title)
                            .font(row.total ? .callout.weight(.semibold) : .callout)
                            .gridColumnAlignment(.leading)
                        amount(row.value(latest), bold: row.total)
                        if let previous {
                            amount(row.value(previous), bold: row.total).opacity(0.7)
                        }
                    }
                    if row.total { Divider().gridCellUnsizedAxes(.horizontal) }
                }
            }
            if let margin = latest.margin {
                Text("Net margin \(String(format: "%.1f", margin * 100))%")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(margin >= 0 ? Theme.positive : Theme.negative)
            }
        }
    }

    private func amount(_ cents: Int64, bold: Bool) -> some View {
        Text(Format.money(cents))
            .font((bold ? Font.callout.weight(.semibold) : .callout).monospacedDigit())
            .foregroundStyle(bold ? (cents >= 0 ? Theme.positive : Theme.negative) : Theme.primaryText)
    }

    private var balanceChart: some View {
        let points = buckets.filter { $0.cash != nil }
        return SectionCard(title: "Balance sheet at each close") {
            Chart {
                ForEach(points) { b in
                    LineMark(x: .value("Period", b.label), y: .value("Cash", Double(b.cash ?? 0) / 100))
                        .foregroundStyle(by: .value("Series", "Cash"))
                    LineMark(x: .value("Period", b.label), y: .value("Equity", Double(b.equity ?? 0) / 100))
                        .foregroundStyle(by: .value("Series", "Equity"))
                    LineMark(x: .value("Period", b.label), y: .value("Debts", Double(b.liabilities ?? 0) / 100))
                        .foregroundStyle(by: .value("Series", "Liabilities"))
                }
            }
            .chartForegroundStyleScale([
                "Cash": Theme.gold, "Equity": Theme.positive, "Liabilities": Theme.negative,
            ])
            .chartXAxis {
                AxisMarks(values: .automatic(desiredCount: 6)) { _ in AxisValueLabel().font(.caption2) }
            }
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine().foregroundStyle(Theme.stroke)
                    AxisValueLabel {
                        if let dollars = value.as(Double.self) {
                            Text(Format.moneyCompact(Int64(dollars * 100))).font(.caption2)
                        }
                    }
                }
            }
            .frame(height: 180)
        }
    }

    private func shares(_ stock: StockDto) -> some View {
        let annualEarnings = stock.earningsWeeks > 0 ? stock.earningsCents * 52 / Int64(stock.earningsWeeks) : 0
        let pe = annualEarnings > 0 ? String(format: "%.1f", Double(stock.marketCapCents) / Double(annualEarnings)) : "n/a"
        let floatShares = stock.sharesIssued * stock.floatBps / 10_000
        return VStack(alignment: .leading, spacing: 12) {
            StatGroup(
                title: "Shares",
                headline: Stat(label: "Market value", value: Format.money(stock.marketCapCents),
                               note: "\(Format.money(stock.priceCents)) a share, \(Change.text(stock.changeQuarterBps)) this quarter",
                               tone: Change.tint(stock.changeQuarterBps)),
                stats: [
                    Stat(label: "Shares issued", value: stock.sharesIssued.formatted()),
                    Stat(label: "Free float", value: floatShares.formatted(), note: Format.percentBps(Int32(clamping: stock.floatBps))),
                    Stat(label: "You hold", value: stock.yourShares.formatted()),
                    Stat(label: "Fair value", value: Format.money(stock.fairCents)),
                    Stat(label: "Price to earnings", value: pe),
                    Stat(label: "Earnings a year", value: Format.moneyCompact(annualEarnings)),
                ]
            )
            if stock.historyCents.count > 1 {
                SectionCard(title: "Share price, last \(stock.historyCents.count) weeks") {
                    PriceChart(values: stock.historyCents, tint: Change.tint(stock.changeQuarterBps))
                }
            }
        }
    }

    private func owners(_ ownership: OwnershipDto) -> some View {
        SectionCard(title: "Who owns it") {
            ForEach(ownership.holders.sorted { $0.quantity > $1.quantity }.prefix(8), id: \.holder) { holder in
                let share = ownership.issued > 0 ? Double(holder.quantity) / Double(ownership.issued) : 0
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        Text(holder.holder).font(.callout)
                        Spacer()
                        Text(String(format: "%.1f%%", share * 100)).font(.callout.monospacedDigit())
                    }
                    GeometryReader { proxy in
                        ZStack(alignment: .leading) {
                            Capsule().fill(Theme.stroke)
                            Capsule().fill(Theme.gold).frame(width: proxy.size.width * share)
                        }
                    }
                    .frame(height: 5)
                }
            }
            if let controller = ownership.controller {
                Text("Controlled by \(controller)").font(.caption).foregroundStyle(Theme.secondary)
            }
        }
    }
}
