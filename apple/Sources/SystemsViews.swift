import SwiftUI

// The deeper systems displays, the same as Compose's Systems.kt: a good's market depth, a
// region's dossier, a division's operations, a company's freight, the rivals board and the
// economy history. Every figure is read from the engine; the views only lay it out and chart it.

private let systemsTierNames = ["Low skill", "Mid skill", "High skill"]

extension SeriesDto {
    func points(_ scale: (Int64) -> Int64 = { $0 }) -> [PlotPoint] {
        zip(weeks, values).map { PlotPoint(week: Int($0), value: scale($1)) }
    }
}

extension Array where Element == SeriesDto {
    func of(_ key: String, _ scale: (Int64) -> Int64 = { $0 }) -> [PlotPoint] {
        first { $0.key == key }?.points(scale) ?? []
    }

    var hasHistory: Bool { contains { $0.weeks.count >= 2 } }
}

private func pctOf(_ bps: Int64) -> String { String(format: "%.1f%%", Double(bps) / 100) }
private func milliLots(_ milli: Int64) -> String { Figures.count(milli / 1_000) }

/// A solid tag, no translucency, in the 8-bit style.
struct SolidTag: View {
    let text: String
    let tint: Color
    var body: some View {
        Text(text)
            .font(Pixel.font(10, bold: true))
            .foregroundStyle(Theme.ink)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 5)
            .padding(.vertical, 1)
            .background(tint, in: Pixel.small)
    }
}

/// A compact table: the first column left, the rest right. Rows flagged `yours` are gold. A
/// tap on a header calls `onSort` with its column; a tap on a row calls `onRow`.
struct SystemsTable: View {
    let headers: [String]
    let rows: [[String]]
    var weights: [CGFloat]?
    var yours: (Int) -> Bool = { _ in false }
    var sort = -1
    var onSort: ((Int) -> Void)?
    var onRow: ((Int) -> Void)?
    var selected = -1

    var body: some View {
        let weights = self.weights ?? ([2.2] + Array(repeating: 1, count: max(headers.count - 1, 0)))
        VStack(spacing: 0) {
            WeightRow(weights: weights) {
                ForEach(Array(headers.enumerated()), id: \.offset) { i, h in
                    Text(i == sort ? "\(h) *" : h)
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(i == sort ? Theme.gold : Theme.secondary)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: i == 0 ? .leading : .trailing)
                        .contentShape(Rectangle())
                        .onTapGesture { onSort?(i) }
                }
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
            .background(Theme.panelRaised)
            ForEach(Array(rows.enumerated()), id: \.offset) { r, cells in
                let tint = yours(r) ? Theme.gold : Theme.primaryText
                WeightRow(weights: weights) {
                    ForEach(Array(cells.enumerated()), id: \.offset) { i, c in
                        Text(c)
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(tint)
                            .lineLimit(1)
                            .truncationMode(.tail)
                            .frame(maxWidth: .infinity, alignment: i == 0 ? .leading : .trailing)
                    }
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(r == selected ? Theme.stroke : r % 2 == 1 ? Theme.panel : Theme.ink)
                .contentShape(Rectangle())
                .onTapGesture { onRow?(r) }
            }
        }
        .frame(maxWidth: .infinity)
    }
}

/// Two columns side by side when there is room, one under the other on a phone.
struct SystemsSplit<Left: View, Right: View>: View {
    @Environment(\.horizontalSizeClass) private var sizeClass
    @ViewBuilder var left: Left
    @ViewBuilder var right: Right

    var body: some View {
        if sizeClass == .regular {
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 8) { left }.frame(maxWidth: .infinity, alignment: .topLeading)
                VStack(alignment: .leading, spacing: 8) { right }.frame(maxWidth: .infinity, alignment: .topLeading)
            }
        } else {
            VStack(alignment: .leading, spacing: 8) {
                left
                right
            }
        }
    }
}

/// One chart for a chart grid or a column.
struct ChartSpec {
    let title: String
    let series: [PlotSeries]
    var kind: PlotKind = .line
    var format: (Int64) -> String = { Format.moneyCompact($0) }
    var scale: PlotScale = .zero

    var drawable: Bool { series.contains { $0.points.count >= 2 } }
}

/// A chart in a panel, a year shown at first. Nothing when no series has two points.
struct ChartPanel: View {
    let spec: ChartSpec
    var body: some View {
        if spec.drawable {
            Panel(padding: 12) {
                PixelChart(spec.series, title: spec.title, initialKind: spec.kind, initialRange: .y1, height: 130,
                           format: spec.format, scale: spec.scale)
            }
        }
    }
}

/// Charts in rows of two when there is room, so the columns stay level; one per row on a phone.
struct ChartGrid: View {
    @Environment(\.horizontalSizeClass) private var sizeClass
    let specs: [ChartSpec]

    var body: some View {
        let shown = specs.filter(\.drawable)
        let perRow = sizeClass == .regular ? 2 : 1
        let rows = stride(from: 0, to: shown.count, by: perRow).map { Array(shown[$0..<min($0 + perRow, shown.count)]) }
        VStack(spacing: 8) {
            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                HStack(alignment: .top, spacing: 10) {
                    ForEach(Array(row.enumerated()), id: \.offset) { _, spec in
                        ChartPanel(spec: spec).frame(maxWidth: .infinity)
                    }
                    if row.count < perRow { Color.clear.frame(maxWidth: .infinity, maxHeight: 1) }
                }
            }
        }
    }
}

private struct EmptyHistory: View {
    var body: some View {
        EmptyNote(text: "History builds up as weeks close. A career saved before this version starts it now.")
    }
}

private func shareTable(_ rows: [ShareRowDto]) -> [[String]] {
    rows.map { [$0.name, Figures.count($0.lots), pctOf($0.shareBps)] }
}

/// Load from the engine every time the week or a command moves, then show it.
struct EngineSection<T, Content: View>: View {
    @EnvironmentObject private var store: AppStore
    let key: String
    let load: (SessionHandle) -> T?
    @ViewBuilder let content: (T) -> Content
    @State private var value: T? = nil

    var body: some View {
        Group {
            if let value { content(value) }
        }
        .task(id: "\(key)-\(store.overview?.revision ?? 0)") {
            let load = self.load
            value = await store.read { load($0) } ?? nil
        }
    }
}

// MARK: - 1. Market depth per good

struct GoodDepthView: View {
    let d: GoodDepthDto

    var body: some View {
        let s = d.series
        VStack(alignment: .leading, spacing: 8) {
            StatGroup(stats: [
                Stat(label: "Weeks of cover", value: d.coverX100.map { String(format: "%.1f wk", Double($0) / 100) } ?? "no sales", note: "stock over sales"),
                Stat(label: "Unsold on shelves", value: Figures.count(d.unsoldLots)),
                Stat(label: "Your share of sales", value: pctOf(d.yourShareBps), tone: d.yourShareBps > 0 ? Theme.gold : Theme.primaryText),
                Stat(label: "Imported / wk", value: Figures.count(d.importedLots)),
                Stat(label: "Exported / wk", value: Figures.count(d.exportedLots)),
                Stat(label: "Sales-weighted ask", value: Format.money(d.askCents)),
            ])
            Panel(padding: 12) {
                VStack(alignment: .leading, spacing: 6) {
                    Eyebrow(text: "Why the price is moving")
                    ForEach(Array(d.drivers.enumerated()), id: \.offset) { _, driver in
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            switch driver.push {
                            case 1: SolidTag(text: "UP", tint: Theme.warning)
                            case -1: SolidTag(text: "DOWN", tint: PlotColors.blue)
                            default: SolidTag(text: "NOTE", tint: Theme.secondary)
                            }
                            Text(driver.text)
                                .font(.caption)
                                .foregroundStyle(Theme.primaryText)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
            }
            if !s.hasHistory { EmptyHistory() }
            ChartGrid(specs: specs)
            SystemsSplit {
                Eyebrow(text: "Top sellers last week")
                if d.sellers.isEmpty {
                    EmptyNote(text: "Nobody sold it last week.")
                } else {
                    SystemsTable(headers: ["Company", "Lots", "Share"], rows: shareTable(d.sellers), yours: { d.sellers[$0].yours })
                }
            } right: {
                Eyebrow(text: "Top buyers (weekly use)")
                if d.buyers.isEmpty {
                    EmptyNote(text: "No plant uses it and households do not buy it.")
                } else {
                    SystemsTable(headers: ["Buyer", "Lots", "Share"], rows: shareTable(d.buyers), yours: { d.buyers[$0].yours })
                }
            }
        }
    }

    private var specs: [ChartSpec] {
        let s = d.series
        var specs = [
            ChartSpec(title: "Made, sold and trade per week", series: [
                PlotSeries(name: "Made", color: Theme.gold, points: s.of("made")),
                PlotSeries(name: "Sold", color: Theme.positive, points: s.of("sold")),
                PlotSeries(name: "Imported", color: Theme.negative, points: s.of("imported")),
                PlotSeries(name: "Exported", color: PlotColors.blue, points: s.of("exported")),
            ], format: { Figures.count($0) }),
            ChartSpec(title: "Stock held", series: [PlotSeries(name: "Stock", color: PlotColors.violet, points: s.of("stock"))],
                      kind: .area, format: { Figures.count($0) }, scale: .level),
            ChartSpec(title: "Weeks of cover", series: [PlotSeries(name: "Cover", color: PlotColors.blue, points: s.of("cover_x100"))],
                      format: { String(format: "%.1f", Double($0) / 100) }, scale: .level),
        ]
        if s.of("your_share_bps").contains(where: { $0.value > 0 }) {
            specs.append(ChartSpec(title: "Your share of sales", series: [PlotSeries(name: "You", color: Theme.gold, points: s.of("your_share_bps"))],
                                   kind: .area, format: { pctOf($0) }, scale: .level))
        }
        return specs
    }
}

/// A good's weekly corridor price for up to ten years against its reference price.
struct GoodHistoryChart: View {
    let price: [PlotPoint]
    let referenceCents: Int64
    @State private var period: PlotPeriod = .weekly

    var body: some View {
        if price.count >= 2 {
            PixelChart(
                [
                    PlotSeries(name: "Price", color: Theme.gold, points: GameDates.lastClose(price, period)),
                    PlotSeries(name: "Reference", color: Theme.secondary,
                               points: GameDates.lastClose(price.map { PlotPoint(week: $0.week, value: referenceCents) }, period)),
                ],
                title: "Price per lot", initialRange: .y1, period: period, periods: PlotPeriod.allCases, onPeriod: { period = $0 },
                height: 150, format: { Format.money($0) }, partialLast: GameDates.partial(price.last?.week ?? 0, period), scale: .level
            )
        }
    }
}

/// A share's weekly closes for up to ten years against the corridor index, both 100 at the
/// first close shown.
struct StockHistoryChart: View {
    let share: [PlotPoint]
    let index: [PlotPoint]
    var height: CGFloat = 150
    @State private var period: PlotPeriod = .weekly

    private func rebase(_ points: [PlotPoint]) -> [PlotPoint] {
        guard let base = points.first else { return [] }
        return points.map { PlotPoint(week: $0.week, value: $0.value * 10_000 / max(base.value, 1)) }
    }

    var body: some View {
        if share.count >= 2, let start = share.first?.week {
            PixelChart(
                [
                    PlotSeries(name: "Share", color: Theme.gold, points: GameDates.lastClose(rebase(share), period)),
                    PlotSeries(name: "Corridor index", color: Theme.secondary, points: GameDates.lastClose(rebase(index.filter { $0.week >= start }), period)),
                ],
                title: "Price history (100 at start)", initialRange: .y1, period: period, periods: PlotPeriod.allCases,
                onPeriod: { period = $0 }, height: height, format: { String(format: "%.1f", Double($0) / 100) },
                partialLast: GameDates.partial(share.last?.week ?? 0, period), scale: .level
            )
        }
    }
}

/// A good's price history and market depth, loaded for the commodity sheet.
struct GoodDepthSection: View {
    let product: UInt32
    let referenceCents: Int64

    var body: some View {
        let product = self.product
        EngineSection(key: "good-\(product)", load: { h in (Figures.points(h.goodPriceHistory(product: product)), h.goodDepth(product: product)) }) { loaded in
            VStack(alignment: .leading, spacing: 12) {
                if loaded.0.count >= 2 {
                    Panel(padding: 12) { GoodHistoryChart(price: loaded.0, referenceCents: referenceCents) }
                }
                if let depth = loaded.1 { GoodDepthView(d: depth) }
            }
        }
    }
}

/// A share's long price history against the index, loaded for a stock or company page.
struct StockHistorySection: View {
    let firm: UInt64
    var height: CGFloat = 150

    var body: some View {
        let firm = self.firm
        EngineSection(key: "share-\(firm)", load: { h in (Figures.points(h.sharePriceHistory(firm: firm)), Figures.points(h.corridorIndexHistory())) }) { loaded in
            if loaded.0.count >= 2 {
                Panel(padding: 12) { StockHistoryChart(share: loaded.0, index: loaded.1, height: height) }
            }
        }
    }
}

// MARK: - 2. Region dossier

struct RegionDossierSection: View {
    @EnvironmentObject private var store: AppStore
    let region: UInt32

    var body: some View {
        let region = self.region
        let firm = store.selectedFirm ?? store.overview?.primaryFirm ?? 0
        EngineSection(key: "region-\(region)-\(firm)", load: { $0.regionDossier(region: region, firm: firm) }) { d in
            RegionDossierView(d: d) { industry in
                store.openDivision = store.divisions.first { $0.industry == industry && $0.regionName == d.name }
            }
        }
    }
}

struct RegionDossierView: View {
    let d: RegionDossierDto
    var onDivision: (String) -> Void = { _ in }

    var body: some View {
        let idle = d.population - d.employed
        let s = d.series
        VStack(alignment: .leading, spacing: 8) {
            StatGroup(stats: [
                Stat(label: "Workforce", value: Figures.count(d.population)),
                Stat(label: "Without a job", value: Figures.count(idle), note: d.population > 0 ? pctOf(idle * 10_000 / d.population) : nil),
                Stat(label: "Average wage / wk", value: Format.money(d.wageCents)),
                Stat(label: "Land price", value: "\(d.landIndexBps / 100)%", note: "of the corridor"),
                Stat(label: "Demand index", value: "\(d.demandIndexBps / 100)%"),
                Stat(label: "Spend / wk", value: Format.moneyCompact(d.spendCents), note: "households"),
            ])
            SystemsSplit {
                Eyebrow(text: "Labour pool")
                SystemsTable(
                    headers: ["Tier", "Workers", "Idle", "Wage"],
                    rows: d.tiers.enumerated().map { i, t in
                        [i < systemsTierNames.count ? systemsTierNames[i] : "Tier \(i)", Figures.count(t.population), Figures.count(t.idle), Format.money(t.wageCents)]
                    }
                )
                Eyebrow(text: "What households buy here")
                SystemsTable(headers: ["Good", "Spend / wk", "Share"],
                             rows: d.demand.map { [$0.name, Format.moneyCompact($0.spendCents), pctOf($0.shareBps)] })
            } right: {
                Eyebrow(text: "Who competes here")
                if d.competitors.isEmpty {
                    EmptyNote(text: "No company operates here yet.")
                } else {
                    ForEach(industries, id: \.self) { industry in
                        let rows = d.competitors.filter { $0.industry == industry }
                        SystemsTable(
                            headers: [Sector.name(industry), "Plants", "Share", "Profit"],
                            rows: rows.map { [$0.name, "\($0.blocks)", pctOf($0.shareBps), $0.yours ? Format.moneyCompact($0.profitCents) : ""] },
                            weights: [2.4, 0.8, 1, 1.1],
                            yours: { rows[$0].yours },
                            onRow: { r in if rows[r].yours { onDivision(rows[r].industry) } }
                        )
                    }
                }
            }
            if !s.hasHistory { EmptyHistory() }
            ChartGrid(specs: specs)
        }
    }

    private var specs: [ChartSpec] {
        let s = d.series
        let shares = s.filter { $0.key.hasPrefix("share:") }
        let colours = [Theme.gold, Theme.positive, PlotColors.blue, PlotColors.violet, Theme.warning]
        var specs = [
            ChartSpec(title: "Workers and jobs", series: [
                PlotSeries(name: "Workforce", color: PlotColors.blue, points: s.of("population")),
                PlotSeries(name: "Employed", color: Theme.positive, points: s.of("employed")),
            ], format: { Figures.count($0) }, scale: .level),
            ChartSpec(title: "Average wage", series: [PlotSeries(name: "Wage", color: Theme.gold, points: s.of("wage"))],
                      format: { Format.money($0) }, scale: .level),
            ChartSpec(title: "Land price (100 is the corridor)", series: [PlotSeries(name: "Land", color: Theme.warning, points: s.of("land_index"))],
                      format: { String(format: "%.0f", Double($0) / 100) }, scale: .level),
            ChartSpec(title: "Household spending per week", series: [PlotSeries(name: "Spend", color: PlotColors.violet, points: s.of("spend"))], kind: .bar),
        ]
        if !shares.isEmpty {
            var lines: [PlotSeries] = []
            for (i, x) in shares.enumerated() {
                lines.append(PlotSeries(name: Sector.name(String(x.key.dropFirst("share:".count))), color: colours[i % colours.count], points: x.points()))
            }
            specs.append(ChartSpec(title: "Your share by sector", series: lines, format: { pctOf($0) }, scale: .level))
        }
        return specs
    }

    /// Sectors in the order their first competitor appears, as Compose's groupBy keeps them.
    private var industries: [String] {
        var seen: [String] = []
        for c in d.competitors where !seen.contains(c.industry) { seen.append(c.industry) }
        return seen
    }
}

// MARK: - 3. Division operations

struct DivisionOpsSection: View {
    @EnvironmentObject private var store: AppStore
    let division: DivisionDto

    var body: some View {
        if let firm = store.selectedFirm ?? store.overview?.primaryFirm {
            let region = division.region
            let industry = division.industry
            EngineSection(key: "ops-\(firm)-\(region)-\(industry)", load: { $0.divisionOps(firm: firm, region: region, industry: industry) }) { d in
                DivisionOpsView(d: d)
            }
        }
    }
}

private let costNames = [
    "materials": "Inputs", "wages": "Labour", "maintenance": "Upkeep", "land": "Land and rent",
    "depreciation": "Depreciation", "freight": "Freight",
]

/// A cost against the largest of cost and price, as a solid bar.
private struct CostBar: View {
    let label: String
    let cents: Int64
    let scale: Double
    let tint: Color

    var body: some View {
        HStack(spacing: 8) {
            Text(label).font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1).frame(width: 96, alignment: .leading)
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Rectangle().fill(Theme.panelRaised)
                    Rectangle().fill(tint).frame(width: max(geo.size.width * min(max(Double(cents) / scale, 0), 1), 2))
                }
            }
            .frame(height: 10)
            Text(Format.money(cents)).font(.caption.monospacedDigit()).foregroundStyle(Theme.primaryText).lineLimit(1)
                .frame(width: 78, alignment: .trailing)
        }
    }
}

struct DivisionOpsView: View {
    let d: DivisionOpsDto

    var body: some View {
        let s = d.series
        VStack(alignment: .leading, spacing: 8) {
            Eyebrow(text: "Operations")
            StatGroup(stats: [
                Stat(label: "Utilisation", value: pctOf(d.utilisationBps), note: "of this week's full run",
                     tone: d.utilisationBps < 6_000 ? Theme.warning : Theme.primaryText),
                Stat(label: "Made / wk", value: milliLots(d.madeMilli)),
                Stat(label: "Sold / wk", value: milliLots(d.soldMilli), tone: d.soldMilli < d.madeMilli * 9 / 10 ? Theme.warning : Theme.primaryText),
                Stat(label: "Capacity / wk", value: milliLots(d.capacityMilli)),
                Stat(label: "Cost per lot", value: Format.money(d.unitCostCents)),
                Stat(label: "Margin per lot", value: Format.signedMoney(d.marginPerLotCents),
                     tone: d.marginPerLotCents < 0 ? Theme.negative : Theme.positive),
            ])
            Eyebrow(text: "What a lot costs against its price")
            if d.madeMilli <= 0 {
                EmptyNote(text: "Nothing made last week, so there is no cost per lot.")
            } else {
                let scale = Double(max(d.priceCents, d.unitCostCents, 1))
                Panel(padding: 12) {
                    VStack(spacing: 5) {
                        ForEach(Array(d.unitCosts.enumerated()), id: \.offset) { _, part in
                            CostBar(label: costNames[part.key] ?? part.key, cents: part.cents, scale: scale, tint: Theme.negative)
                        }
                        CostBar(label: "Total cost", cents: d.unitCostCents, scale: scale, tint: Theme.warning)
                        CostBar(label: "Price", cents: d.priceCents, scale: scale, tint: Theme.gold)
                    }
                }
            }
            if !s.hasHistory { EmptyHistory() }
            ChartGrid(specs: [
                ChartSpec(title: "Output against sales", series: [
                    PlotSeries(name: "Made", color: Theme.gold, points: s.of("made") { $0 / 1_000 }),
                    PlotSeries(name: "Sold", color: Theme.positive, points: s.of("sold") { $0 / 1_000 }),
                    PlotSeries(name: "Capacity", color: Theme.secondary, points: s.of("capacity") { $0 / 1_000 }),
                ], format: { Figures.count($0) }),
                ChartSpec(title: "Utilisation", series: [PlotSeries(name: "Used", color: PlotColors.blue, points: s.of("utilisation_bps"))],
                          kind: .area, format: { pctOf($0) }),
                ChartSpec(title: "Cost per lot", series: [PlotSeries(name: "Cost", color: Theme.negative, points: s.of("unit_cost"))],
                          format: { Format.money($0) }, scale: .level),
                ChartSpec(title: "Revenue and costs", series: [
                    PlotSeries(name: "Revenue", color: Theme.gold, points: s.of("revenue")),
                    PlotSeries(name: "Costs", color: Theme.negative, points: s.of("costs")),
                ], kind: .bar),
            ])
        }
    }
}

// MARK: - 4. Logistics

struct LogisticsStatsSection: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        if let firm = store.selectedFirm ?? store.overview?.primaryFirm {
            EngineSection(key: "freight-\(firm)", load: { $0.logisticsStats(firm: firm) }) { d in
                LogisticsStatsView(d: d)
            }
        }
    }
}

struct LogisticsStatsView: View {
    let d: LogisticsStatsDto

    var body: some View {
        let s = d.series
        let nothing = d.lots == 0 && d.carrierCap == 0 && s.of("lots").allSatisfy { $0.value == 0 }
        VStack(alignment: .leading, spacing: 8) {
            Eyebrow(text: "Freight stats")
            StatGroup(stats: [
                Stat(label: "Lots moved / wk", value: Figures.count(d.lots)),
                Stat(label: "Cost per lot", value: Format.money(d.centsPerLot)),
                Stat(label: "Overflow to outside", value: Figures.count(d.outsideLots), tone: d.outsideLots > 0 ? Theme.warning : Theme.primaryText),
                Stat(label: "Own carriers", value: Figures.count(d.ownLots)),
                Stat(label: "Own fleet used", value: d.carrierCap > 0 ? "\(Figures.count(d.carrierUsed)) of \(Figures.count(d.carrierCap))" : "none"),
                Stat(label: "Idle capacity", value: Figures.count(d.carrierIdle),
                     tone: d.carrierCap > 0 && d.carrierFillBps < 6_000 ? Theme.warning : Theme.primaryText),
            ])
            if nothing {
                EmptyNote(text: "No freight moved yet: everything you sell is sold where it is made. Shipping starts when you sell into another region or buy inputs from one.")
            } else {
                if !s.hasHistory { EmptyHistory() }
                SystemsSplit {
                    ChartPanel(spec: ChartSpec(title: "Who carried your freight", series: [
                        PlotSeries(name: "Own", color: Theme.gold, points: s.of("own")),
                        PlotSeries(name: "Other carriers", color: PlotColors.blue, points: s.of("other")),
                        PlotSeries(name: "Outside", color: Theme.negative, points: s.of("outside")),
                    ], kind: .bar, format: { Figures.count($0) }))
                    ChartPanel(spec: ChartSpec(title: "Cost per lot", series: [PlotSeries(name: "Per lot", color: Theme.warning, points: s.of("cents_per_lot"))],
                                               format: { Format.money($0) }, scale: .level))
                } right: {
                    if s.of("carrier_cap").contains(where: { $0.value > 0 }) {
                        ChartPanel(spec: ChartSpec(title: "Your carriers: capacity and use", series: [
                            PlotSeries(name: "Capacity", color: Theme.secondary, points: s.of("carrier_cap")),
                            PlotSeries(name: "Used", color: Theme.positive, points: s.of("carrier_used")),
                        ], format: { Figures.count($0) }))
                        ChartPanel(spec: ChartSpec(title: "Carrier fill", series: [PlotSeries(name: "Fill", color: PlotColors.blue, points: s.of("fill_bps"))],
                                                   kind: .area, format: { pctOf($0) }))
                    }
                    ChartPanel(spec: ChartSpec(title: "Freight bill per week", series: [PlotSeries(name: "Paid", color: Theme.negative, points: s.of("cents"))], kind: .bar))
                }
            }
        }
    }
}

// MARK: - 5. Rivals board

enum RivalSort: String, CaseIterable {
    case revenue = "Revenue", profit = "Profit", cash = "Cash", debt = "Debt", price = "Share", divisions = "Divs", margin = "Net margin"

    func key(_ r: RivalRowDto) -> Int64 {
        switch self {
        case .revenue: return r.revenueCents
        case .profit: return r.profitCents
        case .cash: return r.cashCents
        case .debt: return r.debtCents
        case .price: return r.sharePriceCents
        case .divisions: return Int64(r.divisions)
        // No revenue sorts below every margin, losses included.
        case .margin: return r.netMarginBps ?? Int64.min
        }
    }

    func cell(_ r: RivalRowDto) -> String {
        switch self {
        case .divisions: return "\(r.divisions)"
        case .margin: return r.netMarginBps.map { pctOf($0) } ?? "n/a"
        case .price: return Format.price(r.sharePriceCents)
        default: return Format.moneyCompact(key(r))
        }
    }
}

/// What the rival comparison shows: both companies by quarter and their goods head to head.
struct RivalCompare {
    let you: [StatementDto]
    let them: [StatementDto]
    let goods: [HeadToHeadDto]
}

/// The rivals board as a sheet: loads the board, then the comparison for the picked rival.
struct RivalsBoardSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var rows: [RivalRowDto]?
    @State private var picked: UInt64?
    @State private var compare: RivalCompare?

    var body: some View {
        let you = store.selectedFirm ?? store.overview?.primaryFirm
        NavigationStack {
            ScrollView {
                Group {
                    if let rows {
                        RivalsBoardView(rows: rows, picked: picked, onPick: { picked = $0 }, compare: compare)
                    } else {
                        Text("Loading.").foregroundStyle(Theme.secondary)
                    }
                }
                .padding(16)
                .frame(maxWidth: 980)
                .frame(maxWidth: .infinity)
            }
            .background(Theme.panel)
            .navigationTitle("Rivals board")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
        .task(id: store.overview?.revision ?? 0) { rows = await store.read { $0.rivalsBoard() } }
        .task(id: "\(picked ?? 0)-\(you ?? 0)-\(store.overview?.revision ?? 0)") {
            guard let p = picked, let me = you else { compare = nil; return }
            compare = await store.read {
                RivalCompare(you: $0.statements(firm: me, period: "quarter"), them: $0.statements(firm: p, period: "quarter"),
                             goods: $0.headToHead(you: me, rival: p))
            }
        }
    }
}

struct RivalsBoardView: View {
    @Environment(\.horizontalSizeClass) private var sizeClass
    let rows: [RivalRowDto]
    let picked: UInt64?
    let onPick: (UInt64) -> Void
    let compare: RivalCompare?
    @State var sort: RivalSort = .revenue

    var body: some View {
        let sorted = rows.sorted { sort.key($0) > sort.key($1) }
        let wide = sizeClass == .regular
        // The phone keeps four columns with the net margin; the pad shows them all.
        let columns: [RivalSort] = wide ? RivalSort.allCases : [.revenue, .profit, .margin, .price]
        VStack(alignment: .leading, spacing: 8) {
            Text("Last 13 weeks. Tap a heading to sort, a company to compare it with yours.")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SystemsTable(
                headers: ["Company"] + columns.map(\.rawValue),
                rows: sorted.map { r in [r.name] + columns.map { $0.cell(r) } },
                weights: [wide ? 2.4 : 1.8] + columns.map { _ in 1 },
                yours: { sorted[$0].yours },
                sort: (columns.firstIndex(of: sort) ?? -2) + 1,
                onSort: { i in if i > 0 { sort = columns[i - 1] } },
                onRow: { i in onPick(sorted[i].firm) },
                selected: sorted.firstIndex { $0.firm == picked } ?? -1
            )
            if let rival = rows.first(where: { $0.firm == picked }), let c = compare {
                comparison(rival, c)
            }
        }
    }

    @ViewBuilder
    private func comparison(_ rival: RivalRowDto, _ c: RivalCompare) -> some View {
        let goods = c.goods.sorted { $0.yourLots + $0.theirLots > $1.yourLots + $1.theirLots }
        Eyebrow(text: "You against \(rival.name)")
        SystemsSplit {
            ChartPanel(spec: ChartSpec(title: "Revenue by quarter", series: [
                PlotSeries(name: "You", color: Theme.gold, points: Figures.points(c.you) { $0.revenueCents }),
                PlotSeries(name: rival.name, color: PlotColors.blue, points: Figures.points(c.them) { $0.revenueCents }),
            ], kind: .bar))
            ChartPanel(spec: ChartSpec(title: "Net income by quarter", series: [
                PlotSeries(name: "You", color: Theme.gold, points: Figures.points(c.you) { $0.netIncomeCents }),
                PlotSeries(name: rival.name, color: PlotColors.blue, points: Figures.points(c.them) { $0.netIncomeCents }),
            ], kind: .bar))
        } right: {
            ChartPanel(spec: ChartSpec(title: "Cash at each close", series: [
                PlotSeries(name: "You", color: Theme.gold, points: Figures.points(c.you) { $0.cashCents }),
                PlotSeries(name: rival.name, color: PlotColors.blue, points: Figures.points(c.them) { $0.cashCents }),
            ], scale: .level))
            ChartPanel(spec: ChartSpec(title: "Share price", series: [
                PlotSeries(name: "You", color: Theme.gold, points: Figures.points(c.you) { $0.sharePriceCents }),
                PlotSeries(name: rival.name, color: PlotColors.blue, points: Figures.points(c.them) { $0.sharePriceCents }),
            ], format: { Format.price($0) }, scale: .level))
        }
        Eyebrow(text: "Head to head, last week")
        if goods.isEmpty {
            EmptyNote(text: "Neither company sold goods last week.")
        } else {
            SystemsTable(
                headers: ["Good", "You", "Share", "Them", "Share"],
                rows: goods.map { [$0.name, Figures.count($0.yourLots), pctOf($0.yourShareBps), Figures.count($0.theirLots), pctOf($0.theirShareBps)] },
                yours: { goods[$0].yourLots > 0 && goods[$0].theirLots > 0 }
            )
        }
    }
}

// MARK: - 6. Economy history

/// The economy's weekly series and the corridor index, loaded together.
struct EconomyHistory {
    let series: [SeriesDto]
    let index: [PricePointDto]
}

struct EconomyHistorySection: View {
    var body: some View {
        EngineSection(key: "economy", load: { EconomyHistory(series: $0.economySeries(), index: $0.corridorIndexHistory()) }) { d in
            EconomyHistoryView(d: d)
        }
    }
}

struct EconomyHistoryView: View {
    let d: EconomyHistory

    var body: some View {
        let s = d.series
        VStack(alignment: .leading, spacing: 8) {
            Eyebrow(text: "Economy history")
            if !s.hasHistory { EmptyHistory() }
            SystemsSplit {
                ChartPanel(spec: ChartSpec(title: "Corridor share index", series: [PlotSeries(name: "Index", color: Theme.gold, points: Figures.points(d.index))],
                                           kind: .area, format: { String(format: "%.1f", Double($0) / 100) }, scale: .level))
                ChartPanel(spec: ChartSpec(title: "Bank rate and inflation", series: [
                    PlotSeries(name: "Bank rate", color: Theme.gold, points: s.of("policy_rate_bps")),
                    PlotSeries(name: "Inflation", color: Theme.negative, points: s.of("inflation_bps")),
                ], format: { pctOf($0) }))
                ChartPanel(spec: ChartSpec(title: "Price level", series: [PlotSeries(name: "Prices", color: PlotColors.violet, points: s.of("price_index_bps"))],
                                           format: { String(format: "%.1f", Double($0) / 100) }, scale: .level))
            } right: {
                ChartPanel(spec: ChartSpec(title: "Companies operating and gone", series: [
                    PlotSeries(name: "Operating", color: Theme.positive, points: s.of("firms_operating")),
                    PlotSeries(name: "Dissolved or merged", color: Theme.negative, points: s.of("firms_gone")),
                ], format: { Figures.count($0) }))
                if s.of("raids").contains(where: { $0.value > 0 }) {
                    ChartPanel(spec: ChartSpec(title: "Raids per week", series: [
                        PlotSeries(name: "Raids", color: Theme.warning, points: s.of("raids")),
                        PlotSeries(name: "Won", color: Theme.negative, points: s.of("raids_won")),
                    ], kind: .bar, format: { Figures.count($0) }))
                }
                ChartPanel(spec: ChartSpec(title: "Household spending per week", series: [PlotSeries(name: "Spending", color: PlotColors.blue, points: s.of("household_spend"))], kind: .bar))
            }
        }
    }
}
