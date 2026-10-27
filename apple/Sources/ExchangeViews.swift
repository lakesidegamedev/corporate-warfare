import Charts
import SwiftUI

enum MarketSection: String, CaseIterable, Identifiable {
    case stocks = "Stocks"
    case commodities = "Commodities"
    case regions = "Regions"
    var id: String { rawValue }
}

/// Markets: the corridor's stock exchange, its commodity board, and each region's own board.
struct MarketsView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var section: MarketSection

    init(initialSection: MarketSection = .stocks) {
        _section = State(initialValue: initialSection)
    }

    var body: some View {
        TabScreen(title: "Markets", subtitle: store.overview?.weekLabel) {
            EconomyCard(collapsible: sizeClass != .regular)
            Picker("Section", selection: $section) {
                ForEach(MarketSection.allCases) { option in
                    Text(option.rawValue).tag(option)
                }
            }
            .pickerStyle(.segmented)
            switch section {
            case .stocks: StocksSection()
            case .commodities: CommoditiesSection()
            case .regions: RegionMarketsSection()
            }
        }
        .onAppear { store.tutorial.visited(.markets) }
    }
}

// MARK: - Shared pieces

enum Change {
    static func text(_ bps: Int64) -> String {
        String(format: "%+.1f%%", Double(bps) / 100)
    }

    static func tint(_ bps: Int64) -> Color {
        bps > 0 ? Theme.positive : bps < 0 ? Theme.negative : Theme.secondary
    }
}

/// A small line of closing prices, scaled to its own range.
struct Sparkline: View {
    let values: [Int64]
    var tint: Color = Theme.gold

    var body: some View {
        GeometryReader { geo in
            Path { path in
                guard values.count > 1, let low = values.min(), let high = values.max() else { return }
                let span = Double(max(high - low, 1))
                for (index, value) in values.enumerated() {
                    let point = CGPoint(
                        x: geo.size.width * Double(index) / Double(values.count - 1),
                        y: geo.size.height * (1 - Double(value - low) / span)
                    )
                    if index == 0 {
                        path.move(to: point)
                    } else {
                        path.addLine(to: point)
                    }
                }
            }
            .stroke(tint, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
        }
        .accessibilityHidden(true)
    }
}

/// Weekly closes as a chart, dollars on the axis.
struct PriceChart: View {
    let values: [Int64]
    var tint: Color = Theme.gold

    var body: some View {
        Chart(Array(values.enumerated()), id: \.offset) { item in
            LineMark(
                x: .value("Week", item.offset),
                y: .value("Price", Double(item.element) / 100)
            )
            .foregroundStyle(tint)
            .interpolationMethod(.monotone)
        }
        .chartYScale(domain: .automatic(includesZero: false))
        .chartXAxis(.hidden)
        .frame(height: 150)
        .accessibilityLabel("Price over the last \(values.count) weeks")
    }
}

// MARK: - Stocks

private enum StockSort: String, CaseIterable, Identifiable {
    case movers = "Top movers"
    case size = "Largest"
    case yours = "Yours"
    var id: String { rawValue }
}

/// Every operating company's shares: a corridor index, then one line per stock.
struct StocksSection: View {
    @EnvironmentObject private var store: AppStore
    @State private var sort: StockSort = .movers
    @State private var selected: StockDto?

    /// A market-cap weighted index of the companies with a full price history, 1,000 at the
    /// start of that history.
    private var index: [Int64] {
        let length = store.stocks.map(\.historyCents.count).max() ?? 0
        let members = store.stocks.filter { $0.historyCents.count == length && length > 0 }
        guard let first = members.first, !first.historyCents.isEmpty else { return [] }
        let base = members.reduce(Int64(0)) { $0 + $1.historyCents[0] * $1.sharesIssued }
        guard base > 0 else { return [] }
        return (0..<length).map { week in
            members.reduce(Int64(0)) { $0 + $1.historyCents[week] * $1.sharesIssued } * 1_000 / base
        }
    }

    private var rows: [StockDto] {
        switch sort {
        case .movers: return store.stocks.sorted { abs($0.changeWeekBps) > abs($1.changeWeekBps) }
        case .size: return store.stocks.sorted { $0.marketCapCents > $1.marketCapCents }
        case .yours: return store.stocks.filter { $0.yourShares > 0 || $0.controlledByYou }
        }
    }

    var body: some View {
        let series = index
        VStack(alignment: .leading, spacing: 12) {
            if let last = series.last {
                let week = series.count > 1 ? (last - series[series.count - 2]) * 10_000 / max(series[series.count - 2], 1) : 0
                Panel(padding: 12) {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Eyebrow(text: "Market index")
                            Text(last.formatted())
                                .font(.title2.weight(.bold).monospacedDigit())
                                .foregroundStyle(Theme.primaryText)
                            Text("\(Change.text(week)) this week · \(store.stocks.count) companies")
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(Change.tint(week))
                        }
                        Sparkline(values: series, tint: Change.tint(last - (series.first ?? last)))
                            .frame(height: 44)
                    }
                }
            }
            HStack(spacing: 6) {
                ForEach(StockSort.allCases) { option in
                    Button { sort = option } label: {
                        Text(option.rawValue)
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .foregroundStyle(sort == option ? Theme.ink : Theme.primaryText)
                            .background(sort == option ? Theme.gold : Theme.panel, in: Capsule())
                            .overlay(Capsule().stroke(Theme.stroke, lineWidth: sort == option ? 0 : 1))
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(sort == option ? .isSelected : [])
                }
            }
            if rows.isEmpty {
                CalloutRow(icon: "trend_up", text: "You hold no shares yet. Open any company to buy.", tint: Theme.secondary)
            } else {
                Panel(padding: 12) {
                    VStack(spacing: 0) {
                        ForEach(Array(rows.enumerated()), id: \.element.firm) { position, stock in
                            if position > 0 { Divider().overlay(Theme.stroke) }
                            Button { selected = stock } label: { StockRow(stock: stock) }
                                .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
        .sheet(item: $selected) { stock in
            StockSheet(stock: stock)
                .environmentObject(store)
                .presentationDetents([.large])
                .presentationBackground(Theme.panel)
        }
    }
}

extension StockDto: Identifiable {
    public var id: UInt64 { firm }
}

private struct StockRow: View {
    let stock: StockDto

    var body: some View {
        HStack(spacing: 10) {
            SectorBadge(sector: stock.industry, size: 26)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(stock.name).font(.callout.weight(.medium)).foregroundStyle(Theme.primaryText).lineLimit(1)
                    if stock.controlledByYou {
                        PixelIcon("star", size: 12).foregroundStyle(Theme.gold)
                    }
                }
                Text("\(Sector.name(stock.industry)) · cap \(Format.moneyCompact(stock.marketCapCents))")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(Theme.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 6)
            Sparkline(values: stock.historyCents, tint: Change.tint(stock.changeQuarterBps))
                .frame(width: 54, height: 22)
            VStack(alignment: .trailing, spacing: 2) {
                Text(Format.price(stock.priceCents)).font(.callout.weight(.semibold).monospacedDigit()).foregroundStyle(Theme.primaryText)
                SignalPill(text: Change.text(stock.changeWeekBps), tint: Change.tint(stock.changeWeekBps))
            }
        }
        .padding(.vertical, 8)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }
}

/// One stock: its chart, its valuation and your position, and an order ticket.
struct StockSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let stock: StockDto
    @State private var buy = true
    @State private var quantityText = "1000"
    @State private var limitText = ""

    private var annualEarnings: Int64? {
        guard stock.earningsWeeks > 0 else { return nil }
        return stock.earningsCents * 52 / Int64(stock.earningsWeeks)
    }

    private var priceEarnings: String {
        guard let earnings = annualEarnings, earnings > 0 else { return "Loss" }
        let ratio = Double(stock.marketCapCents) / Double(earnings)
        // Thin early earnings make the ratio meaningless rather than informative.
        return ratio > 200 ? "n/m" : String(format: "%.1f", ratio)
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    StatGroup(
                        headline: Stat(label: "Share price", value: Format.price(stock.priceCents),
                                       note: "\(Change.text(stock.changeWeekBps)) this week"),
                        stats: [
                            Stat(label: "Quarter", value: Change.text(stock.changeQuarterBps), tone: Change.tint(stock.changeQuarterBps)),
                            Stat(label: "Market cap", value: Format.moneyCompact(stock.marketCapCents)),
                            Stat(label: "P/E", value: priceEarnings),
                            Stat(label: "Fair value", value: Format.price(stock.fairCents)),
                            Stat(label: "Free float", value: Format.percentBps(Int32(clamping: stock.floatBps))),
                            Stat(label: "You hold", value: stock.yourShares.formatted(),
                                 note: Format.moneyCompact(stock.yourShares * stock.priceCents)),
                        ]
                    )
                    if stock.historyCents.count > 1 {
                        Panel(padding: 12) {
                            VStack(alignment: .leading, spacing: 6) {
                                Eyebrow(text: "Last \(stock.historyCents.count) weeks")
                                PriceChart(values: stock.historyCents, tint: Change.tint(stock.changeQuarterBps))
                            }
                        }
                    }
                    Panel(padding: 12) {
                        VStack(alignment: .leading, spacing: 10) {
                            Eyebrow(text: "Order")
                            Picker("Side", selection: $buy) {
                                Text("Buy").tag(true)
                                Text("Sell").tag(false)
                            }
                            .pickerStyle(.segmented)
                            HStack {
                                Text("Shares").foregroundStyle(Theme.secondary)
                                Spacer()
                                TextField("Shares", text: $quantityText)
                                    .keyboardType(.numberPad)
                                    .multilineTextAlignment(.trailing)
                                    .frame(maxWidth: 140)
                            }
                            HStack {
                                Text("Limit per share ($)").foregroundStyle(Theme.secondary)
                                Spacer()
                                TextField(String(format: "%.2f", Double(stock.priceCents) / 100), text: $limitText)
                                    .keyboardType(.decimalPad)
                                    .multilineTextAlignment(.trailing)
                                    .frame(maxWidth: 140)
                            }
                            Text("Orders fill at the weekly close, best price first, up to 2% of the freely held shares a week. Unfilled orders lapse after a week.")
                                .font(.caption)
                                .foregroundStyle(Theme.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                            Button {
                                Task { await place() }
                            } label: {
                                Text(buy ? "Place buy order" : "Place sell order")
                                    .font(.callout.weight(.semibold))
                                    .frame(maxWidth: .infinity)
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(Theme.gold)
                            .foregroundStyle(Theme.ink)
                            .disabled(quantity == nil || store.isBusy)
                        }
                        .foregroundStyle(Theme.primaryText)
                    }
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle(stock.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
    }

    private var quantity: Int64? {
        guard let value = Int64(quantityText.trimmingCharacters(in: .whitespaces)), value > 0 else { return nil }
        return value
    }

    private var limit: Int64 {
        if let cents = parseDollarsToCents(limitText), cents > 0 { return cents }
        // Default limits sit two percent through the current price so the order is marketable.
        return buy ? stock.priceCents * 102 / 100 : stock.priceCents * 98 / 100
    }

    private func place() async {
        guard let quantity else { return }
        await store.orderShares(firm: stock.firm, buy: buy, quantity: quantity, limit: max(limit, 1))
        dismiss()
    }
}

// MARK: - Commodities

/// Every traded good across the corridor: its price, how it moved, and supply against demand.
struct CommoditiesSection: View {
    @EnvironmentObject private var store: AppStore
    @State private var selected: CommodityDto?

    private var groups: [(sector: String, rows: [CommodityDto])] {
        let bySector = Dictionary(grouping: store.commodities, by: \.sector)
        return (Sector.order + ["other"]).compactMap { sector in
            guard let rows = bySector[sector], !rows.isEmpty else { return nil }
            return (sector, rows.sorted { $0.name < $1.name })
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Market prices per lot: the median asking price of the companies that make each good, or the import price where nobody does.")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ForEach(groups, id: \.sector) { group in
                Panel(padding: 12) {
                    VStack(alignment: .leading, spacing: 0) {
                        HStack(spacing: 8) {
                            if group.sector != "other" { SectorBadge(sector: group.sector, size: 20) }
                            Eyebrow(text: group.sector == "other" ? "Other goods" : Sector.name(group.sector))
                        }
                        .padding(.bottom, 4)
                        ForEach(Array(group.rows.enumerated()), id: \.element.product) { position, line in
                            if position > 0 { Divider().overlay(Theme.stroke) }
                            Button { selected = line } label: { CommodityRow(line: line) }
                                .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
        .sheet(item: $selected) { line in
            CommoditySheet(line: line)
                .environmentObject(store)
                .presentationDetents([.medium, .large])
                .presentationBackground(Theme.panel)
        }
    }
}

extension CommodityDto: Identifiable {
    public var id: UInt32 { product }
}

private struct CommodityRow: View {
    let line: CommodityDto

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 8) {
                Text(line.name).font(.callout.weight(.medium)).foregroundStyle(Theme.primaryText).lineLimit(1)
                if line.yours {
                    PixelIcon("star", size: 12).foregroundStyle(Theme.gold)
                }
                Spacer(minLength: 6)
                Sparkline(values: line.historyCents, tint: Change.tint(line.changeQuarterBps))
                    .frame(width: 54, height: 20)
                Text(Format.money(line.priceCents)).font(.callout.weight(.semibold).monospacedDigit()).foregroundStyle(Theme.primaryText)
                SignalPill(text: Change.text(line.changeWeekBps), tint: Change.tint(line.changeWeekBps))
            }
            Text(balance)
                .font(.caption2.monospacedDigit())
                .foregroundStyle(Theme.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.85)
        }
        .padding(.vertical, 7)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }

    private var balance: String {
        var parts = ["made \(line.madeLots.formatted())", "sold \(line.soldLots.formatted())"]
        if line.importedLots > 0 { parts.append("imported \(line.importedLots.formatted())") }
        if line.exportedLots > 0 { parts.append("exported \(line.exportedLots.formatted())") }
        return parts.joined(separator: " · ") + " a week"
    }
}

private struct CommoditySheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let line: CommodityDto
    @State private var pricing: ProductLineDto?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    StatGroup(
                        headline: Stat(label: "Market price per lot", value: Format.money(line.priceCents),
                                       note: "\(Change.text(line.changeWeekBps)) this week"),
                        stats: [
                            Stat(label: "Quarter", value: Change.text(line.changeQuarterBps), tone: Change.tint(line.changeQuarterBps)),
                            Stat(label: "Reference", value: Format.money(line.referenceCents)),
                            Stat(label: "Import at", value: line.importCents > 0 ? Format.money(line.importCents) : "n/a"),
                            Stat(label: "Export at", value: line.exportCents > 0 ? Format.money(line.exportCents) : "n/a"),
                            Stat(label: "Made / wk", value: line.madeLots.formatted()),
                            Stat(label: "Sold / wk", value: line.soldLots.formatted()),
                            Stat(label: "Imported / wk", value: line.importedLots.formatted()),
                            Stat(label: "Exported / wk", value: line.exportedLots.formatted()),
                            Stat(label: "In stock", value: line.stockLots.formatted()),
                        ]
                    )
                    if line.historyCents.count > 1 {
                        Panel(padding: 12) {
                            VStack(alignment: .leading, spacing: 6) {
                                Eyebrow(text: "Last \(line.historyCents.count) weeks")
                                PriceChart(values: line.historyCents, tint: Change.tint(line.changeQuarterBps))
                            }
                        }
                    }
                    StoreGoodPanel(line: line)
                    if let mine = store.productLines.first(where: { $0.product == line.product }) {
                        Button {
                            pricing = mine
                        } label: {
                            PixelLabel("Your price: \(Format.money(mine.priceCents)) · \(PostureText.label(mine))", icon: "tag")
                                .font(.callout.weight(.semibold))
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.gold)
                        .foregroundStyle(Theme.ink)
                    }
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle(line.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
            .sheet(item: $pricing) { mine in
                PostureSheet(line: mine)
                    .environmentObject(store)
                    .presentationDetents([.medium, .large])
                    .presentationBackground(Theme.panel)
            }
        }
    }
}
