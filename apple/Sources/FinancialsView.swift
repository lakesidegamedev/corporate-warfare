import SwiftUI

/// Everything the financial tabs show for one company, read from the engine in one go.
struct FinancialData {
    let name: String
    let weeks: [StatementDto]
    let quarters: [StatementDto]
    let years: [StatementDto]
    let share: [PricePointDto]
    let index: [PricePointDto]
    /// The two largest rivals in the same sector whose statements you can see, by quarter.
    let rivals: [(name: String, statements: [StatementDto])]

    func of(_ period: PlotPeriod) -> [StatementDto] {
        switch period {
        case .weekly: return weeks
        case .quarterly: return quarters
        case .yearly: return years
        }
    }

    /// Read one company's financials on the engine. Rivals are every other firm in the sector,
    /// listed or not, whose statements the engine shows you; the two with the most revenue in
    /// their latest quarter are compared. A firm with nothing visible is skipped rather than
    /// taking a place.
    static func load(_ h: SessionHandle, firm: UInt64, name: String, industry: String) -> FinancialData {
        let rivals = h.firms()
            .filter { $0.handle != firm && $0.industry == industry }
            .map { (name: $0.name, statements: h.statements(firm: $0.handle, period: "quarter")) }
            .filter { !$0.statements.isEmpty }
            .sorted { ($0.statements.last?.revenueCents ?? 0) > ($1.statements.last?.revenueCents ?? 0) }
            .prefix(2)
        return FinancialData(
            name: name,
            weeks: h.statements(firm: firm, period: "week"),
            quarters: h.statements(firm: firm, period: "quarter"),
            years: h.statements(firm: firm, period: "year"),
            share: h.sharePriceHistory(firm: firm),
            index: h.corridorIndexHistory(),
            rivals: Array(rivals)
        )
    }
}

/// The financial tabs of a company page.
enum FinanceTab: String, CaseIterable {
    case overview = "Overview", income = "Income statement", balance = "Balance sheet", cash = "Cash flow"
    case divisions = "Divisions", ratios = "Ratios"
}

/// Shared figures for the financial and systems views.
enum Figures {
    static func pct(_ bps: Int64?) -> String { bps.map { String(format: "%.1f%%", Double($0) / 100) } ?? "n/a" }
    static func times(_ bps: Int64?) -> String { bps.map { String(format: "%.2fx", Double($0) / 10_000) } ?? "n/a" }
    static func count(_ value: Int64) -> String { value.formatted(.number.locale(Locale(identifier: "en_US"))) }
    static func points(_ rows: [StatementDto], _ value: (StatementDto) -> Int64) -> [PlotPoint] {
        rows.map { PlotPoint(week: Int($0.firstWeek), value: value($0)) }
    }
    static func points(_ rows: [PricePointDto]) -> [PlotPoint] {
        rows.map { PlotPoint(week: Int($0.week), value: $0.cents) }
    }
    /// Whether the latest statement covers fewer weeks than a full period.
    static func partial(_ rows: [StatementDto], _ period: PlotPeriod) -> Bool {
        rows.last.map { Int($0.weeks) < period.weeks } ?? false
    }
}

/// A row whose cells share the width by weight, like Compose's `Modifier.weight`.
struct WeightRow: Layout {
    let weights: [CGFloat]

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 320
        let total = max(weights.reduce(0, +), 1)
        var height: CGFloat = 0
        for (i, view) in subviews.enumerated() {
            let w = width * weight(i) / total
            height = max(height, view.sizeThatFits(ProposedViewSize(width: w, height: nil)).height)
        }
        return CGSize(width: width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let total = max(weights.reduce(0, +), 1)
        var x = bounds.minX
        for (i, view) in subviews.enumerated() {
            let w = bounds.width * weight(i) / total
            view.place(at: CGPoint(x: x, y: bounds.minY), proposal: ProposedViewSize(width: w, height: bounds.height))
            x += w
        }
    }

    private func weight(_ i: Int) -> CGFloat { i < weights.count ? weights[i] : 1 }
}

/// Load a company's financials and show them; reloads every week the clock moves.
struct CompanyFinancialsSection: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    let firm: UInt64
    let name: String
    let industry: String
    var wideColumns = 4
    @State private var data: FinancialData?

    var body: some View {
        Group {
            if let data {
                CompanyFinancials(data: data, columns: sizeClass == .compact ? 3 : wideColumns)
            } else {
                Text("Loading").font(.caption).foregroundStyle(Theme.secondary)
            }
        }
        .task(id: "\(firm)-\(store.overview?.revision ?? 0)") {
            let firm = firm, name = name, industry = industry
            data = await store.read { FinancialData.load($0, firm: firm, name: name, industry: industry) }
        }
    }
}

/// A company's statements in depth: an overview with charts, the income statement, balance
/// sheet and cash flow as tables over the last several periods, each division's results, and
/// ratios. `columns` is how many periods the tables show side by side.
struct CompanyFinancials: View {
    let data: FinancialData
    var columns = 4
    @State var tab: FinanceTab = .overview
    @State var period: PlotPeriod = .quarterly
    @State private var sharePeriod: PlotPeriod = .weekly

    private var periods: [PlotPeriod] { PlotPeriod.allCases }

    var body: some View {
        let rows = data.of(period)
        VStack(alignment: .leading, spacing: 8) {
            ChipFlow(spacing: 4) {
                ForEach(FinanceTab.allCases, id: \.self) { t in
                    PlotChips(options: [t.rawValue], selected: t == tab ? t.rawValue : "") { _ in tab = t }
                }
            }
            HStack(spacing: 8) {
                Text("Period").font(.caption).foregroundStyle(Theme.secondary)
                PlotChips(options: periods.map(\.rawValue), selected: period.rawValue) { t in
                    if let p = periods.first(where: { $0.rawValue == t }) { period = p }
                }
            }
            if rows.isEmpty {
                EmptyNote(text: "Statements build up as weeks close. A career saved before this version starts its history now.")
            } else {
                content(rows)
            }
        }
    }

    private func chart(_ series: [PlotSeries], title: String, kind: PlotKind = .line, rows: [StatementDto],
                       format: @escaping (Int64) -> String = { Format.moneyCompact($0) }, scale: PlotScale = .zero) -> some View {
        Panel(padding: 12) {
            PixelChart(series, title: title, initialKind: kind, period: period, periods: periods, onPeriod: { period = $0 },
                       format: format, partialLast: Figures.partial(rows, period), scale: scale)
        }
    }

    @ViewBuilder
    private func content(_ rows: [StatementDto]) -> some View {
        let recent = Array(rows.suffix(columns).reversed())
        let open = Figures.partial(rows, period)
        switch tab {
        case .overview:
            let latest = rows[rows.count - 1]
            StatGroup(
                headline: Stat(label: "Revenue", value: Format.moneyCompact(latest.revenueCents),
                               note: "\(latest.label)\(open ? " to date" : ""), growth \(Figures.pct(latest.revenueGrowthBps))"),
                stats: [
                    Stat(label: "Net income", value: Format.moneyCompact(latest.netIncomeCents),
                         tone: latest.netIncomeCents < 0 ? Theme.negative : Theme.positive),
                    Stat(label: "Net margin", value: Figures.pct(latest.netMarginBps)),
                    Stat(label: "Cash", value: Format.moneyCompact(latest.cashCents)),
                    Stat(label: "Equity", value: Format.moneyCompact(latest.equityCents)),
                    Stat(label: "Debt", value: Format.moneyCompact(latest.debtCents)),
                    Stat(label: "Return on equity", value: Figures.pct(latest.roeBps)),
                ]
            )
            chart([
                PlotSeries(name: "Revenue", color: Theme.gold, points: Figures.points(rows) { $0.revenueCents }),
                PlotSeries(name: "Net income", color: Theme.positive, points: Figures.points(rows) { $0.netIncomeCents }),
            ], title: "Revenue and profit", kind: .bar, rows: rows)
            if !data.rivals.isEmpty {
                let colours = [Theme.gold, PlotColors.blue, PlotColors.violet]
                let all = [(name: data.name, statements: data.quarters)] + data.rivals
                Panel(padding: 12) {
                    PixelChart(
                        all.enumerated().map { i, r in
                            PlotSeries(name: r.name, color: colours[i % colours.count], points: Figures.points(r.statements) { $0.revenueCents })
                        },
                        title: "Revenue against the largest rivals, by quarter", period: .quarterly,
                        partialLast: Figures.partial(data.quarters, .quarterly)
                    )
                }
            }
            if data.share.count > 1 {
                shareChart
            }
        case .income:
            StatementTable(columns: recent, lines: [
                StatementLine("Revenue") { $0.revenueCents },
                StatementLine("Cost of goods") { -$0.costOfGoodsCents },
                StatementLine("Gross profit", total: true) { $0.grossProfitCents },
                StatementLine("Wages") { -$0.wagesCents },
                StatementLine("Operating costs") { -$0.operatingExpenseCents },
                StatementLine("Other income") { $0.otherCents },
                StatementLine("Depreciation") { -$0.depreciationCents },
                StatementLine("Operating profit", total: true) { $0.ebitCents },
                StatementLine("Interest paid") { -$0.interestExpenseCents },
                StatementLine("Interest earned") { $0.interestIncomeCents },
                StatementLine("Tax") { -$0.taxCents },
                StatementLine("Net income", total: true) { $0.netIncomeCents },
            ])
            chart([
                PlotSeries(name: "Revenue", color: Theme.gold, points: Figures.points(rows) { $0.revenueCents }),
                PlotSeries(name: "Gross profit", color: PlotColors.blue, points: Figures.points(rows) { $0.grossProfitCents }),
                PlotSeries(name: "Net income", color: Theme.positive, points: Figures.points(rows) { $0.netIncomeCents }),
            ], title: "Income", rows: rows)
        case .balance:
            StatementTable(columns: recent, lines: [
                StatementLine("Cash") { $0.cashCents },
                StatementLine("Inventory") { $0.inventoryCents },
                StatementLine("Current assets", total: true) { $0.currentAssetsCents },
                StatementLine("Plant and construction") { $0.fixedAssetsCents },
                StatementLine("Total assets", total: true) { $0.assetsCents },
                StatementLine("Current liabilities") { $0.currentLiabilitiesCents },
                StatementLine("Debt") { $0.debtCents },
                StatementLine("Total liabilities", total: true) { $0.liabilitiesCents },
                StatementLine("Equity", total: true) { $0.equityCents },
                StatementLine("Shares issued", text: { Figures.count($0.sharesIssued) }),
                StatementLine("Share price", text: { Format.price($0.sharePriceCents) }),
            ])
            chart([
                PlotSeries(name: "Assets", color: Theme.gold, points: Figures.points(rows) { $0.assetsCents }),
                PlotSeries(name: "Liabilities", color: Theme.negative, points: Figures.points(rows) { $0.liabilitiesCents }),
                PlotSeries(name: "Equity", color: Theme.positive, points: Figures.points(rows) { $0.equityCents }),
            ], title: "Balance sheet at each close", kind: .area, rows: rows, scale: .level)
        case .cash:
            StatementTable(columns: recent, lines: [
                StatementLine("Opening cash") { $0.cashOpeningCents },
                StatementLine("Operating") { $0.cashOperatingCents },
                StatementLine("Investing") { $0.cashInvestingCents },
                StatementLine("Financing") { $0.cashFinancingCents },
                StatementLine("Tax") { $0.cashTaxCents },
                StatementLine("Closing cash", total: true) { $0.cashClosingCents },
            ])
            Text("Operating covers sales, wages, materials and interest. Investing is plants, construction and holdings. Financing is loans, deposits, shares and dividends.")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            chart([
                PlotSeries(name: "Operating", color: Theme.positive, points: Figures.points(rows) { $0.cashOperatingCents }),
                PlotSeries(name: "Investing", color: Theme.negative, points: Figures.points(rows) { $0.cashInvestingCents }),
                PlotSeries(name: "Financing", color: PlotColors.blue, points: Figures.points(rows) { $0.cashFinancingCents }),
                PlotSeries(name: "Tax", color: Theme.warning, points: Figures.points(rows) { $0.cashTaxCents }),
            ], title: "Cash flow by activity", kind: .bar, rows: rows)
        case .divisions:
            divisions(rows)
        case .ratios:
            StatementTable(columns: recent, lines: [
                StatementLine("Gross margin", text: { Figures.pct($0.grossMarginBps) }),
                StatementLine("Net margin", text: { Figures.pct($0.netMarginBps) }),
                StatementLine("Return on equity, a year", text: { Figures.pct($0.roeBps) }),
                StatementLine("Return on assets, a year", text: { Figures.pct($0.roaBps) }),
                StatementLine("Debt to equity", text: { Figures.times($0.debtToEquityBps) }),
                StatementLine("Current ratio", text: { Figures.times($0.currentRatioBps) }),
                StatementLine("Interest cover", text: { Figures.times($0.interestCoverBps) }),
                StatementLine("Revenue growth", text: { Figures.pct($0.revenueGrowthBps) }),
            ])
            chart([
                PlotSeries(name: "Gross margin", color: Theme.gold, points: rows.compactMap { s in s.grossMarginBps.map { PlotPoint(week: Int(s.firstWeek), value: $0) } }),
                PlotSeries(name: "Net margin", color: Theme.positive, points: rows.compactMap { s in s.netMarginBps.map { PlotPoint(week: Int(s.firstWeek), value: $0) } }),
                PlotSeries(name: "Return on equity", color: PlotColors.blue, points: rows.compactMap { s in s.roeBps.map { PlotPoint(week: Int(s.firstWeek), value: $0) } }),
            ], title: "Margins and returns", rows: rows, format: { Figures.pct($0) })
        }
    }

    /// The share and the corridor index on one scale: both rebased to 100 at the first close shown.
    private var shareChart: some View {
        let base = data.share[0]
        let indexBase = data.index.first { $0.week >= base.week }
        let share = data.share.map { PlotPoint(week: Int($0.week), value: $0.cents * 10_000 / max(base.cents, 1)) }
        let index: [PlotPoint] = indexBase.map { b in
            data.index.filter { $0.week >= base.week }.map { PlotPoint(week: Int($0.week), value: $0.cents * 10_000 / max(b.cents, 1)) }
        } ?? []
        return Panel(padding: 12) {
            PixelChart(
                [
                    PlotSeries(name: "Share", color: Theme.gold, points: GameDates.lastClose(share, sharePeriod)),
                    PlotSeries(name: "Corridor index", color: Theme.secondary, points: GameDates.lastClose(index, sharePeriod)),
                ],
                title: "Share price against the index (100 at start)", period: sharePeriod, periods: periods,
                onPeriod: { sharePeriod = $0 }, format: { String(format: "%.1f", Double($0) / 100) },
                partialLast: GameDates.partial(share.last?.week ?? 0, sharePeriod), scale: .level
            )
        }
    }

    @ViewBuilder
    private func divisions(_ rows: [StatementDto]) -> some View {
        let source = period == .weekly ? data.quarters : rows
        if let latest = source.last, !latest.divisions.isEmpty {
            let shown: PlotPeriod = period == .weekly ? .quarterly : period
            let ordered = latest.divisions.sorted { $0.revenueCents > $1.revenueCents }
            Text("\(latest.label)\(Figures.partial(source, shown) ? " to date" : ""). Estimated from each division's plants; weekly view shows quarters.")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SystemsTable(
                headers: ["Division", "Revenue", "Costs", "Profit", "Plants", "Share"],
                rows: ordered.map { d in
                    ["\(Sector.name(d.industry)) · \(d.regionName)", Format.moneyCompact(d.revenueCents), Format.moneyCompact(d.costsCents),
                     Format.moneyCompact(d.profitCents), "\(d.plants)", Figures.pct(d.shareBps)]
                },
                weights: [2.2, 1, 1, 1, 0.8, 0.9]
            )
            let colours = [Theme.gold, Theme.positive, PlotColors.blue, PlotColors.violet]
            Panel(padding: 12) {
                PixelChart(
                    ordered.prefix(4).enumerated().map { i, d in
                        PlotSeries(
                            name: "\(Sector.name(d.industry)) · \(d.regionName)", color: colours[i],
                            points: source.compactMap { s in
                                s.divisions.first { $0.industry == d.industry && $0.regionName == d.regionName }
                                    .map { PlotPoint(week: Int(s.firstWeek), value: $0.profitCents) }
                            }
                        )
                    },
                    title: "Division profit", initialKind: .bar, period: shown, partialLast: Figures.partial(source, shown)
                )
            }
        } else {
            EmptyNote(text: "Division results are kept by quarter once a quarter has closed weeks.")
        }
    }
}

/// One row of a statement table: a money line or a preformatted figure.
struct StatementLine: Identifiable {
    let title: String
    var total = false
    var value: ((StatementDto) -> Int64)?
    var text: ((StatementDto) -> String)?
    var id: String { title }

    init(_ title: String, total: Bool = false, value: @escaping (StatementDto) -> Int64) {
        self.title = title
        self.total = total
        self.value = value
    }

    init(_ title: String, text: @escaping (StatementDto) -> String) {
        self.title = title
        self.text = text
    }
}

/// Statement lines down the side, the latest periods across, newest first.
struct StatementTable: View {
    let columns: [StatementDto]
    let lines: [StatementLine]

    var body: some View {
        let weights: [CGFloat] = [1.6] + Array(repeating: 1, count: columns.count)
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 3) {
                WeightRow(weights: weights) {
                    Color.clear.frame(height: 1)
                    ForEach(columns, id: \.firstWeek) { s in
                        Text(s.label)
                            .font(Pixel.font(9))
                            .foregroundStyle(Theme.secondary)
                            .lineLimit(1)
                            .minimumScaleFactor(0.7)
                            .frame(maxWidth: .infinity, alignment: .trailing)
                    }
                }
                ForEach(lines) { line in
                    WeightRow(weights: weights) {
                        Text(line.title)
                            .font(line.total ? .caption.weight(.semibold) : .caption)
                            .foregroundStyle(Theme.primaryText)
                            .lineLimit(2)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        ForEach(columns, id: \.firstWeek) { s in
                            let v = line.value?(s)
                            let tone = line.total && v != nil ? ((v ?? 0) >= 0 ? Theme.positive : Theme.negative) : Theme.primaryText
                            Text(line.text?(s) ?? v.map { Format.moneyCompact($0) } ?? "")
                                .font((line.total ? Font.caption.weight(.semibold) : .caption).monospacedDigit())
                                .foregroundStyle(tone)
                                .lineLimit(1)
                                .minimumScaleFactor(0.7)
                                .frame(maxWidth: .infinity, alignment: .trailing)
                        }
                    }
                }
            }
        }
    }
}
