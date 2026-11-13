import SwiftUI

// The iPad and Mac tables: plants, goods, the league table, regions and the statements, each on
// the shared DataGrid with the same built-in views as the desktop. Data comes from the same engine
// calls the other Apple screens use; a row opens the detail those screens already have.

private func pctText(_ bps: Int64) -> String { String(format: "%.1f%%", Double(bps) / 100) }
private func wholePct(_ bps: Int32) -> String { "\((Int(bps) + 50) / 100)%" }
private func signed(_ n: Int64) -> String { (n > 0 ? "+" : n < 0 ? "-" : "") + Figures.count(abs(n)) }

/// What a row opens: the views the app already has for a good, a region or a company.
enum GridDetail: Identifiable {
    case good(UInt32)
    case region(UInt32)
    case company(UInt64)

    var id: String {
        switch self {
        case .good(let p): return "good-\(p)"
        case .region(let r): return "region-\(r)"
        case .company(let f): return "company-\(f)"
        }
    }
}

struct GridDetailSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let detail: GridDetail

    var body: some View {
        switch detail {
        case .company(let firm):
            CompanyPageSheet(firm: firm)
        case .good(let product):
            let good = store.commodities.first { $0.product == product }
            sheet(good?.name ?? "Market") { GoodDepthSection(product: product, referenceCents: good?.referenceCents ?? 0) }
        case .region(let region):
            sheet(store.boards.first { $0.index == region }?.name ?? "Region") { RegionDossierSection(region: region) }
        }
    }

    private func sheet<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        NavigationStack {
            ScrollView {
                content()
                    .padding(16)
                    .frame(maxWidth: 900)
                    .frame(maxWidth: .infinity)
            }
            .background(Theme.panel)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
    }
}

// MARK: - Plants

struct PlantGridRow {
    let f: FacilityDto
    let company: String
    let strategy: String
    let industry: String
}

enum PlantGrid {
    static func status(_ s: String) -> String {
        if s.hasPrefix("operating") { return "Running" }
        if s.hasPrefix("building") { return "Building" }
        if s.hasPrefix("mothballed") { return "Mothballed" }
        if s.hasPrefix("retooling") { return "Retooling" }
        return s.prefix(1).uppercased() + s.dropFirst()
    }

    static func use(_ f: FacilityDto) -> Int64 { f.capacityLots > 0 ? f.lastOutputLots * 10_000 / f.capacityLots : 0 }

    static let views: [GridViewState] = [
        GridViewState("Operations", ["name", "company", "region", "status", "plants", "capacity", "output", "use", "bottleneck"], sort: "use", descending: false),
        GridViewState("Staff", ["name", "region", "status", "low", "mid", "high", "target"]),
        GridViewState("Idle and closed", ["name", "company", "region", "status", "capacity", "reopen"], sort: "reopen",
                      filters: [GridFilter(op: .not, column: "status", text: "Running")]),
        GridViewState("By region", ["name", "region", "company", "status", "plants", "capacity", "output", "use"], sort: "region", descending: false),
    ]

    static let columns: [GridColumn<PlantGridRow>] = [
        GridColumn("name", "Plant", width: nil, numeric: false, topic: "About", text: { $0.strategy }),
        GridColumn("company", "Company", width: 130, numeric: false, topic: "About", text: { $0.company }, tone: { _ in Theme.secondary }),
        GridColumn("region", "Region", width: 120, numeric: false, topic: "About", text: { $0.f.region }, tone: { _ in Theme.secondary }),
        GridColumn("status", "Status", width: 90, numeric: false, topic: "About", text: { PlantGrid.status($0.f.status) },
                   tone: { $0.f.status.hasPrefix("operating") ? Theme.positive : Theme.warning }),
        GridColumn("plants", "Plants", width: 60, topic: "Operations", text: { "\($0.f.blocks)" }, value: { Int64($0.f.blocks) }),
        GridColumn("capacity", "Capacity", width: 80, topic: "Operations", text: { Figures.count($0.f.capacityLots) }, value: { $0.f.capacityLots }),
        GridColumn("output", "Made last wk", width: 100, topic: "Operations", text: { Figures.count($0.f.lastOutputLots) }, value: { $0.f.lastOutputLots }),
        GridColumn("use", "Used", width: 70, topic: "Operations", text: { "\(PlantGrid.use($0.f) / 100)%" }, value: { PlantGrid.use($0.f) }, scale: 100),
        GridColumn("bottleneck", "Held back by", width: 130, numeric: false, topic: "Operations",
                   text: { $0.f.bottleneck.isEmpty || $0.f.bottleneck == "none" ? GridCells.dash : $0.f.bottleneck }, tone: { _ in Theme.secondary }),
        GridColumn("target", "Target", width: 64, topic: "Operations", text: { wholePct($0.f.targetBps) }, value: { Int64($0.f.targetBps) }, scale: 100),
        GridColumn("low", "Low skill", width: 76, topic: "Staff", text: { Figures.count($0.f.workersLow) }, value: { $0.f.workersLow }),
        GridColumn("mid", "Mid skill", width: 76, topic: "Staff", text: { Figures.count($0.f.workersMid) }, value: { $0.f.workersMid }),
        GridColumn("high", "High skill", width: 76, topic: "Staff", text: { Figures.count($0.f.workersHigh) }, value: { $0.f.workersHigh }),
        GridColumn("reopen", "To reopen", width: 84, topic: "Money",
                   text: { $0.f.reopenCostCents > 0 ? Format.moneyCompact($0.f.reopenCostCents) : GridCells.dash }, value: { $0.f.reopenCostCents }, scale: 100),
    ]
}

struct PlantsGridScreen: View {
    @Environment(AppStore.self) private var store
    @State private var rows: [PlantGridRow] = []

    private var scopes: [GridScope<PlantGridRow>] {
        let companies: [GridScope<PlantGridRow>] = Array(Set(rows.map { $0.company })).sorted().map { c in
            GridScope<PlantGridRow>(id: "company:\(c)", label: c, kind: "Company", test: { $0.company == c })
        }
        let regions: [GridScope<PlantGridRow>] = Array(Set(rows.map { $0.f.region })).sorted().map { r in
            GridScope<PlantGridRow>(id: "region:\(r)", label: r, kind: "Region", test: { $0.f.region == r })
        }
        return companies + regions
    }

    var body: some View {
        DataGrid("Plants", screen: "plants", views: PlantGrid.views, rows: rows, columns: PlantGrid.columns,
                 key: { AnyHashable($0.f.handle) },
                 words: { "\($0.strategy) \($0.company) \($0.f.region) \(PlantGrid.status($0.f.status))" },
                 scopes: scopes,
                 onOpen: { r in
                     store.openDivision = store.divisions.first {
                         ($0.regionName == r.f.region || $0.region == r.f.region) && (r.industry.isEmpty || $0.industry == r.industry)
                     }
                 })
            .padding(8)
            .task(id: "\(store.historyRefreshKey)-\(store.controlledFirms.count)") { await load() }
    }

    private func load() async {
        let firms = store.controlledFirms.map { ($0.handle, $0.name) }
        var names: [String: (String, String)] = [:]
        for s in store.strategyCatalogue where names[s.id] == nil { names[s.id] = (s.name, s.industry) }
        let lookup = names
        rows = await store.read { h -> [PlantGridRow] in
            firms.flatMap { firm in
                h.facilities(firm: firm.0).map { f in
                    PlantGridRow(f: f, company: firm.1, strategy: lookup[f.recipe]?.0 ?? f.recipe, industry: lookup[f.recipe]?.1 ?? "")
                }
            }
        } ?? []
    }
}

// MARK: - Goods

struct GoodsGridData {
    var flows: [UInt32: GoodSeriesDto] = [:]
    var closes: [UInt32: [Int64]] = [:]
    var shares: [UInt32: Int64] = [:]
}

enum GoodsGrid {
    static let views: [GridViewState] = [
        GridViewState("Overview", ["name", "sector", "price", "price.percent", "price.trend", "made", "sold", "sold.percent", "through", "stock", "yours"], sort: "sold"),
        GridViewState("Prices", ["name", "sector", "price", "vsnormal", "week", "price.change", "price.percent", "price.trend"], sort: "price.percent"),
        GridViewState("Supply", ["name", "sector", "made", "made.percent", "sold", "sold.percent", "through", "stock", "stock.trend"], sort: "made"),
        GridViewState("My business", ["name", "sector", "price", "price.percent", "yours", "yours.change", "yours.trend", "sold", "stock"], sort: "yours", owner: .mine),
        GridViewState("Trade", ["name", "sector", "price", "price.percent", "imported", "imported.change", "exported", "exported.change", "made"], sort: "imported"),
    ]

    static func columns(_ d: GoodsGridData) -> [GridColumn<CommodityDto>] {
        [
            GridColumn("name", "Good", width: nil, numeric: false, text: { $0.name + ($0.yours ? " *" : "") }, tone: { $0.yours ? Theme.gold : Theme.primaryText }),
            GridColumn("sector", "Sector", width: 120, numeric: false, topic: "About", text: { $0.sector == "other" ? GridCells.dash : Sector.name($0.sector) }, tone: { _ in Theme.secondary }),
            GridColumn("price", "Price", width: 84, topic: "Price", text: { Format.money($0.priceCents) }, value: { $0.priceCents },
                       series: { d.closes[$0.product] ?? $0.historyCents }, format: { Format.money($0) }, scale: 100),
            GridColumn("vsnormal", "Vs normal", width: 76, topic: "Price",
                       text: { $0.referenceCents > 0 ? Change.text(($0.priceCents - $0.referenceCents) * 10_000 / $0.referenceCents) : "" },
                       value: { $0.referenceCents > 0 ? ($0.priceCents - $0.referenceCents) * 10_000 / $0.referenceCents : nil },
                       tone: { GridCells.tint($0.priceCents - $0.referenceCents) }, scale: 100),
            GridColumn("week", "Week", width: 62, topic: "Price", text: { Change.text($0.changeWeekBps) }, value: { $0.changeWeekBps }, tone: { Change.tint($0.changeWeekBps) }, scale: 100),
            GridColumn("made", "Made", width: 72, topic: "Supply", text: { Figures.count($0.madeLots) }, value: { $0.madeLots }, series: { d.flows[$0.product]?.made ?? [] }),
            GridColumn("sold", "Sold", width: 72, topic: "Supply", text: { Figures.count($0.soldLots) }, value: { $0.soldLots }, series: { d.flows[$0.product]?.sold ?? [] }),
            GridColumn("through", "Sell-through", width: 84, topic: "Supply",
                       text: { $0.madeLots > 0 ? "\($0.soldLots * 100 / $0.madeLots)%" : "" }, value: { $0.madeLots > 0 ? $0.soldLots * 100 / $0.madeLots : nil }),
            GridColumn("stock", "Stock", width: 70, topic: "Supply", text: { Figures.count($0.stockLots) }, value: { $0.stockLots }, series: { d.flows[$0.product]?.stock ?? [] }),
            GridColumn("imported", "Imported", width: 72, topic: "Trade", text: { $0.importedLots > 0 ? Figures.count($0.importedLots) : "" }, value: { $0.importedLots },
                       tone: { _ in Theme.positive }, series: { d.flows[$0.product]?.imported ?? [] }),
            GridColumn("exported", "Exported", width: 72, topic: "Trade", text: { $0.exportedLots > 0 ? Figures.count($0.exportedLots) : "" }, value: { $0.exportedLots },
                       tone: { _ in Theme.secondary }, series: { d.flows[$0.product]?.exported ?? [] }),
            GridColumn("yours", "Your share", width: 80, topic: "You",
                       text: { (d.shares[$0.product] ?? 0) > 0 ? pctText(d.shares[$0.product] ?? 0) : "" }, value: { d.shares[$0.product] ?? 0 },
                       tone: { _ in Theme.gold },
                       series: { g in
                           guard let f = d.flows[g.product] else { return [] }
                           return zip(f.yourSold, f.sold).map { y, t in t > 0 ? y * 10_000 / t : 0 }
                       },
                       format: pctText, scale: 100),
        ]
    }
}

struct GoodsGridScreen: View {
    @Environment(AppStore.self) private var store
    @State private var data = GoodsGridData()
    @State private var detail: GridDetail?

    var body: some View {
        let goods = store.commodities
        let shares = data.shares
        let sectors = Array(Set(goods.map(\.sector).filter { $0 != "other" })).sorted { (Sector.engineIndex($0) ?? 99) < (Sector.engineIndex($1) ?? 99) }
        DataGrid("Goods", screen: "goods", views: GoodsGrid.views, rows: goods, columns: GoodsGrid.columns(data),
                 key: { AnyHashable($0.product) },
                 words: { "\($0.name) \(Sector.name($0.sector))" },
                 mine: { $0.yours || (shares[$0.product] ?? 0) > 0 },
                 scopes: sectors.map { s in GridScope<CommodityDto>(id: "sector:\(s)", label: Sector.name(s), kind: "Sector", test: { $0.sector == s }) },
                 onOpen: { detail = .good($0.product) })
            .padding(8)
            .sheet(item: $detail) { GridDetailSheet(detail: $0).environment(store) }
            .task(id: "\(store.historyRefreshKey)-\(goods.count)") {
                let products = goods.map(\.product)
                data = await store.read { h in
                    GoodsGridData(
                        flows: Dictionary(h.goodsSeries().map { ($0.product, $0) }, uniquingKeysWith: { a, _ in a }),
                        closes: Dictionary(h.goodsPriceSeries().map { (UInt32(truncatingIfNeeded: $0.id), $0.values) }, uniquingKeysWith: { a, _ in a }),
                        shares: Dictionary(products.map { ($0, h.goodDepth(product: $0)?.yourShareBps ?? 0) }, uniquingKeysWith: { a, _ in a })
                    )
                } ?? GoodsGridData()
            }
    }
}

// MARK: - League table

struct LeagueGridData {
    var rows: [RivalRowDto] = []
    var periods: [UInt64: [PeriodDto]] = [:]
    var shares: [UInt64: [Int64]] = [:]
    var debt: [UInt64: [Int64]] = [:]
    var contest: [UInt64: [HeadToHeadDto]] = [:]
}

enum LeagueGrid {
    static let views: [GridViewState] = [
        GridViewState("Overview", ["name", "sector", "revenue", "profit", "margin", "price", "price.trend", "cap"], sort: "cap"),
        GridViewState("Finance", ["name", "revenue", "revenue.percent", "profit", "profit.change", "margin", "cash", "debt", "debt.change"], sort: "revenue"),
        GridViewState("Shares", ["name", "price", "price.percent", "price.trend", "cap", "fair", "float", "held"], sort: "cap"),
        GridViewState("Exchange", ["name", "price", "week", "quarter", "price.trend", "fair", "float", "cap", "held"], sort: "cap"),
        GridViewState("Against me", ["name", "sector", "overlap", "theirlots", "raids", "cap", "profit"], sort: "theirlots",
                      filters: [GridFilter(op: .above, column: "overlap", number: 0)]),
    ]

    static func columns(_ d: LeagueGridData, stocks: [UInt64: StockDto], raids: [String: Int]) -> [GridColumn<RivalRowDto>] {
        let ranked = d.rows.sorted { $0.marketCapCents > $1.marketCapCents }
        var rank: [UInt64: Int] = [:]
        for (i, r) in ranked.enumerated() { rank[r.firm] = i + 1 }
        let compact: (Int64) -> String = { Format.moneyCompact($0) }
        func results(_ firm: UInt64) -> [PeriodDto] { (d.periods[firm] ?? []).filter { $0.weeks >= 4 } }
        return [
            GridColumn("name", "Company", width: nil, numeric: false, text: { r in (rank[r.firm].map { n in "\(n). " } ?? "") + r.name },
                       tone: { $0.yours ? Theme.gold : Theme.primaryText }),
            GridColumn("sector", "Sector", width: 118, numeric: false, topic: "About", text: { Sector.name($0.industry) }, tone: { _ in Theme.secondary }),
            GridColumn("revenue", "Revenue", width: 84, topic: "Results", text: { compact($0.revenueCents) }, value: { $0.revenueCents },
                       series: { r in results(r.firm).map(\.revenueCents) }, step: 4, format: compact, scale: 100),
            GridColumn("profit", "Profit", width: 80, topic: "Results", text: { compact($0.profitCents) }, value: { $0.profitCents },
                       tone: { GridCells.tint($0.profitCents) }, series: { r in results(r.firm).map(\.netIncomeCents) }, step: 4, format: compact, scale: 100),
            GridColumn("margin", "Margin", width: 64, topic: "Results", text: { $0.netMarginBps.map(pctText) ?? "" }, value: { $0.netMarginBps },
                       tone: { GridCells.tint($0.netMarginBps ?? 0) }, scale: 100),
            GridColumn("cash", "Cash", width: 78, topic: "Balance sheet", text: { compact($0.cashCents) }, value: { $0.cashCents },
                       series: { r in (d.periods[r.firm] ?? []).map(\.cashCents) }, step: 4, format: compact, scale: 100),
            GridColumn("debt", "Debt", width: 78, topic: "Balance sheet", text: { compact($0.debtCents) }, value: { $0.debtCents },
                       tone: { _ in Theme.secondary }, series: { d.debt[$0.firm] ?? [] }, format: compact, scale: 100),
            GridColumn("price", "Share", width: 72, topic: "Shares", text: { Format.price($0.sharePriceCents) }, value: { $0.sharePriceCents },
                       series: { d.shares[$0.firm] ?? stocks[$0.firm]?.historyCents ?? [] }, format: { Format.price($0) }, scale: 100),
            GridColumn("week", "Week", width: 62, topic: "Shares", text: { r in stocks[r.firm].map { Change.text($0.changeWeekBps) } ?? "" },
                       value: { stocks[$0.firm]?.changeWeekBps }, tone: { Change.tint(stocks[$0.firm]?.changeWeekBps ?? 0) }, scale: 100),
            GridColumn("quarter", "Quarter", width: 66, topic: "Shares", text: { r in stocks[r.firm].map { Change.text($0.changeQuarterBps) } ?? "" },
                       value: { stocks[$0.firm]?.changeQuarterBps }, tone: { Change.tint(stocks[$0.firm]?.changeQuarterBps ?? 0) }, scale: 100),
            GridColumn("cap", "Value", width: 90, topic: "Shares", text: { compact($0.marketCapCents) }, value: { $0.marketCapCents }, scale: 100),
            GridColumn("fair", "Fair price", width: 76, topic: "Shares", text: { r in stocks[r.firm].map { Format.price($0.fairCents) } ?? GridCells.dash },
                       value: { stocks[$0.firm]?.fairCents }, scale: 100),
            GridColumn("float", "Float", width: 62, topic: "Shares", text: { r in stocks[r.firm].map { pctText($0.floatBps) } ?? GridCells.dash },
                       value: { stocks[$0.firm]?.floatBps }, scale: 100),
            GridColumn("divisions", "Divs", width: 48, topic: "About", text: { "\($0.divisions)" }, value: { Int64($0.divisions) }),
            GridColumn("held", "You hold", width: 72, topic: "You", text: { r in (stocks[r.firm]?.yourShares).flatMap { $0 > 0 ? Figures.count($0) : nil } ?? "" },
                       value: { stocks[$0.firm]?.yourShares ?? 0 }, tone: { _ in Theme.gold }),
            GridColumn("overlap", "Shared goods", width: 92, topic: "Against you", text: { r in (d.contest[r.firm]?.count).flatMap { $0 > 0 ? "\($0)" : nil } ?? "" },
                       value: { Int64(d.contest[$0.firm]?.count ?? 0) }),
            GridColumn("theirlots", "Their lots", width: 80, topic: "Against you",
                       text: { r in let n = (d.contest[r.firm] ?? []).reduce(Int64(0)) { $0 + $1.theirLots }; return n > 0 ? Figures.count(n) : "" },
                       value: { r in (d.contest[r.firm] ?? []).reduce(Int64(0)) { $0 + $1.theirLots } }),
            GridColumn("raids", "Raids on you", width: 92, topic: "Against you", text: { r in raids[r.name].map { "\($0)" } ?? "" },
                       value: { Int64(raids[$0.name] ?? 0) }, tone: { _ in Theme.negative }),
        ]
    }
}

struct LeagueGridScreen: View {
    @Environment(AppStore.self) private var store
    @State private var data = LeagueGridData()
    @State private var detail: GridDetail?

    /// Raids on your businesses in the raid log, by attacker.
    private var raidsOnYou: [String: Int] {
        var out: [String: Int] = [:]
        for r in store.recentRaids where r.playerDefender { out[r.attacker, default: 0] += 1 }
        return out
    }

    var body: some View {
        let stocks = Dictionary(store.stocks.map { ($0.firm, $0) }, uniquingKeysWith: { a, _ in a })
        let raids = raidsOnYou
        let sectors = Array(Set(data.rows.map(\.industry))).sorted { (Sector.engineIndex($0) ?? 99) < (Sector.engineIndex($1) ?? 99) }
        return DataGrid("League table", screen: "league", views: LeagueGrid.views, rows: data.rows,
                        columns: LeagueGrid.columns(data, stocks: stocks, raids: raids),
                        key: { AnyHashable($0.firm) },
                        words: { "\($0.name) \(Sector.name($0.industry))" },
                        mine: { $0.yours },
                        scopes: sectors.map { s in GridScope<RivalRowDto>(id: "sector:\(s)", label: Sector.name(s), kind: "Sector", test: { $0.industry == s }) },
                        onOpen: { detail = .company($0.firm) })
            .padding(8)
            .sheet(item: $detail) { GridDetailSheet(detail: $0).environment(store) }
            .task(id: "\(store.historyRefreshKey)-\(store.selectedFirm ?? 0)") {
                let me = store.selectedFirm ?? store.overview?.primaryFirm
                data = await store.read { h -> LeagueGridData in
                    let rows = h.rivalsBoard()
                    return LeagueGridData(
                        rows: rows,
                        periods: Dictionary(rows.map { ($0.firm, h.history(firm: $0.firm)) }, uniquingKeysWith: { a, _ in a }),
                        shares: Dictionary(h.sharePriceSeries().map { ($0.id, $0.values) }, uniquingKeysWith: { a, _ in a }),
                        debt: Dictionary(h.debtSeries(period: "week").map { ($0.id, $0.values) }, uniquingKeysWith: { a, _ in a }),
                        contest: me.map { you in
                            Dictionary(rows.filter { !$0.yours }.map { ($0.firm, h.headToHead(you: you, rival: $0.firm)) }, uniquingKeysWith: { a, _ in a })
                        } ?? [:]
                    )
                } ?? LeagueGridData()
            }
    }
}

// MARK: - Regions

struct RegionGridRow {
    let board: RegionBoardDto
    let dossier: RegionDossierDto?
    let myBlocks: Int
    let myDivisions: Int
    let unmetLots: Int64
    let rival: String?
    let rivalShareBps: Int32
}

enum RegionGrid {
    static func series(_ d: RegionDossierDto?, _ key: String) -> [Int64] { d?.series.first { $0.key == key }?.values ?? [] }

    static let views: [GridViewState] = [
        GridViewState("Overview", ["name", "wage", "skill", "roads", "power", "demand", "unmet", "migration", "plants", "rival"], sort: "plants"),
        GridViewState("Labour", ["name", "pop", "pop.percent", "idle", "wage", "wage.percent", "wage.trend", "wageidx", "skill"], sort: "wageidx", descending: false),
        GridViewState("Infrastructure", ["name", "roads", "power", "broadband", "skill", "land"], sort: "roads"),
        GridViewState("Households", ["name", "demand", "spend", "spend.percent", "spend.trend", "unmet", "migration"], sort: "unmet"),
        GridViewState("My plants", ["name", "plants", "divs", "wage", "wage.percent", "roads", "power", "unmet", "rival"], sort: "plants", owner: .mine),
    ]

    static let columns: [GridColumn<RegionGridRow>] = [
        GridColumn("name", "Region", width: nil, numeric: false, text: { $0.board.name }, tone: { $0.myBlocks > 0 ? Theme.gold : Theme.primaryText }),
        GridColumn("state", "State", width: 110, numeric: false, topic: "About", text: { $0.board.stateName }, tone: { _ in Theme.secondary }),
        GridColumn("pop", "Workers", width: 80, topic: "Labour", text: { $0.dossier.map { Figures.count($0.population) } ?? GridCells.dash },
                   value: { $0.dossier?.population }, series: { RegionGrid.series($0.dossier, "population") }),
        GridColumn("idle", "Without a job", width: 92, topic: "Labour", text: { r in r.dossier.map { Figures.count($0.population - $0.employed) } ?? GridCells.dash },
                   value: { r in r.dossier.map { $0.population - $0.employed } }),
        GridColumn("wage", "Wage, week", width: 84, topic: "Labour", text: { r in r.dossier.map { Format.money($0.wageCents) } ?? GridCells.dash },
                   value: { $0.dossier?.wageCents }, series: { RegionGrid.series($0.dossier, "wage") }, format: { Format.money($0) }, scale: 100),
        GridColumn("wageidx", "Wage level", width: 76, topic: "Labour", text: { wholePct($0.board.wageIndexBps) }, value: { Int64($0.board.wageIndexBps) }, scale: 100),
        GridColumn("skill", "Skills", width: 64, topic: "Labour", text: { wholePct($0.board.skillBps) }, value: { Int64($0.board.skillBps) }, scale: 100),
        GridColumn("roads", "Roads", width: 64, topic: "Infrastructure", text: { wholePct($0.board.roadBps) }, value: { Int64($0.board.roadBps) }, scale: 100),
        GridColumn("power", "Power", width: 64, topic: "Infrastructure", text: { wholePct($0.board.powerGridBps) }, value: { Int64($0.board.powerGridBps) }, scale: 100),
        GridColumn("broadband", "Broadband", width: 80, topic: "Infrastructure", text: { wholePct($0.board.broadbandBps) }, value: { Int64($0.board.broadbandBps) }, scale: 100),
        GridColumn("land", "Land", width: 62, topic: "Cost", text: { wholePct($0.board.landIndexBps) }, value: { Int64($0.board.landIndexBps) },
                   series: { RegionGrid.series($0.dossier, "land_index") }, format: { wholePct(Int32(truncatingIfNeeded: $0)) }, scale: 100),
        GridColumn("demand", "Demand", width: 70, topic: "Households", text: { wholePct($0.board.demandIndexBps) }, value: { Int64($0.board.demandIndexBps) }, scale: 100),
        GridColumn("spend", "Spend, week", width: 90, topic: "Households", text: { r in r.dossier.map { Format.moneyCompact($0.spendCents) } ?? GridCells.dash },
                   value: { $0.dossier?.spendCents }, series: { RegionGrid.series($0.dossier, "spend") }, format: { Format.moneyCompact($0) }, scale: 100),
        GridColumn("unmet", "Going without", width: 96, topic: "Households", text: { $0.unmetLots > 0 ? Figures.count($0.unmetLots) : "" },
                   value: { $0.unmetLots }, tone: { _ in Theme.warning }),
        GridColumn("migration", "Migration, yr", width: 96, topic: "Households", text: { signed($0.board.netMigrationYear) },
                   value: { $0.board.netMigrationYear }, tone: { GridCells.tint($0.board.netMigrationYear) }),
        GridColumn("plants", "Your plants", width: 80, topic: "You", text: { $0.myBlocks > 0 ? "\($0.myBlocks)" : "" }, value: { Int64($0.myBlocks) }, tone: { _ in Theme.gold }),
        GridColumn("divs", "Your divisions", width: 96, topic: "You", text: { $0.myDivisions > 0 ? "\($0.myDivisions)" : "" }, value: { Int64($0.myDivisions) }, tone: { _ in Theme.gold }),
        GridColumn("rival", "Leading rival", width: 150, numeric: false, topic: "Rivals",
                   text: { r in r.rival.map { "\($0) \(wholePct(r.rivalShareBps))" } ?? GridCells.dash }, tone: { _ in Theme.secondary }),
    ]
}

struct RegionsGridScreen: View {
    @Environment(AppStore.self) private var store
    @State private var dossiers: [UInt32: RegionDossierDto] = [:]
    @State private var unmet: [UInt32: Int64] = [:]
    @State private var detail: GridDetail?

    var body: some View {
        let firm = store.selectedFirm ?? store.overview?.primaryFirm ?? 0
        let rows = store.boards.map { b -> RegionGridRow in
            let mine = store.divisions.filter { $0.region == b.code }
            let lead = b.sectors.filter { $0.leaderFirm != firm && $0.leaderShareBps > 0 }.max { $0.leaderShareBps < $1.leaderShareBps }
            return RegionGridRow(board: b, dossier: dossiers[b.index], myBlocks: mine.reduce(0) { $0 + Int($1.blocks) }, myDivisions: mine.count,
                                 unmetLots: unmet[b.index] ?? 0, rival: lead?.leader, rivalShareBps: lead?.leaderShareBps ?? 0)
        }
        let states = Array(Set(rows.map(\.board.stateName))).sorted()
        return DataGrid("Regions", screen: "regions", views: RegionGrid.views, rows: rows, columns: RegionGrid.columns,
                        key: { AnyHashable($0.board.index) },
                        words: { "\($0.board.name) \($0.board.stateName) \($0.board.hubCity)" },
                        mine: { $0.myBlocks > 0 },
                        scopes: states.map { s in GridScope<RegionGridRow>(id: "state:\(s)", label: s, kind: "State", test: { $0.board.stateName == s }) },
                        onOpen: { detail = .region($0.board.index) })
            .padding(8)
            .sheet(item: $detail) { GridDetailSheet(detail: $0).environment(store) }
            .task(id: "\(store.historyRefreshKey)-\(firm)-\(store.boards.count)") {
                let indices = store.boards.map(\.index)
                let loaded = await store.read { h -> ([UInt32: RegionDossierDto], [UInt32: Int64]) in
                    var ds: [UInt32: RegionDossierDto] = [:]
                    for i in indices { if let d = h.regionDossier(region: i, firm: firm) { ds[i] = d } }
                    // What households go without: the engine's capacity openings, counted once per good.
                    var most: [UInt32: [String: Int64]] = [:]
                    for o in h.capacityOpenings(limit: 2000) {
                        let before = most[o.region]?[o.productId] ?? 0
                        most[o.region, default: [:]][o.productId] = max(before, o.unmetLots)
                    }
                    return (ds, most.mapValues { $0.values.reduce(0, +) })
                }
                if let loaded {
                    dossiers = loaded.0
                    unmet = loaded.1
                }
            }
    }
}

// MARK: - Finance statements

struct GridStatementLine {
    enum Kind { case money, pct, times, count, price }
    let group: String
    let title: String
    let total: Bool
    let kind: Kind
    let value: (StatementDto) -> Int64?

    init(_ group: String, _ title: String, total: Bool = false, kind: Kind = .money, _ value: @escaping (StatementDto) -> Int64?) {
        self.group = group
        self.title = title
        self.total = total
        self.kind = kind
        self.value = value
    }

    func show(_ v: Int64?) -> String {
        guard let v else { return GridCells.dash }
        switch kind {
        case .money: return Format.moneyCompact(v)
        case .pct: return String(format: "%.1f%%", Double(v) / 100)
        case .times: return String(format: "%.2fx", Double(v) / 10_000)
        case .price: return Format.price(v)
        case .count: return Figures.count(v)
        }
    }
}

enum FinanceGrid {
    static let lines: [GridStatementLine] = [
        GridStatementLine("Income statement", "Revenue") { $0.revenueCents },
        GridStatementLine("Income statement", "Cost of goods") { -$0.costOfGoodsCents },
        GridStatementLine("Income statement", "Gross profit", total: true) { $0.grossProfitCents },
        GridStatementLine("Income statement", "Wages") { -$0.wagesCents },
        GridStatementLine("Income statement", "Operating costs") { -$0.operatingExpenseCents },
        GridStatementLine("Income statement", "Other income") { $0.otherCents },
        GridStatementLine("Income statement", "Depreciation") { -$0.depreciationCents },
        GridStatementLine("Income statement", "Operating profit", total: true) { $0.ebitCents },
        GridStatementLine("Income statement", "Interest paid") { -$0.interestExpenseCents },
        GridStatementLine("Income statement", "Interest earned") { $0.interestIncomeCents },
        GridStatementLine("Income statement", "Tax") { -$0.taxCents },
        GridStatementLine("Income statement", "Net income", total: true) { $0.netIncomeCents },
        GridStatementLine("Balance sheet", "Cash") { $0.cashCents },
        GridStatementLine("Balance sheet", "Inventory") { $0.inventoryCents },
        GridStatementLine("Balance sheet", "Current assets", total: true) { $0.currentAssetsCents },
        GridStatementLine("Balance sheet", "Plant and construction") { $0.fixedAssetsCents },
        GridStatementLine("Balance sheet", "Total assets", total: true) { $0.assetsCents },
        GridStatementLine("Balance sheet", "Current liabilities") { $0.currentLiabilitiesCents },
        GridStatementLine("Balance sheet", "Debt") { $0.debtCents },
        GridStatementLine("Balance sheet", "Total liabilities", total: true) { $0.liabilitiesCents },
        GridStatementLine("Balance sheet", "Equity", total: true) { $0.equityCents },
        GridStatementLine("Balance sheet", "Shares issued", kind: .count) { $0.sharesIssued },
        GridStatementLine("Balance sheet", "Share price", kind: .price) { $0.sharePriceCents },
        GridStatementLine("Cash flow", "Opening cash") { $0.cashOpeningCents },
        GridStatementLine("Cash flow", "Operating") { $0.cashOperatingCents },
        GridStatementLine("Cash flow", "Investing") { $0.cashInvestingCents },
        GridStatementLine("Cash flow", "Financing") { $0.cashFinancingCents },
        GridStatementLine("Cash flow", "Tax paid") { $0.cashTaxCents },
        GridStatementLine("Cash flow", "Closing cash", total: true) { $0.cashClosingCents },
        GridStatementLine("Ratios", "Gross margin", kind: .pct) { $0.grossMarginBps },
        GridStatementLine("Ratios", "Net margin", kind: .pct) { $0.netMarginBps },
        GridStatementLine("Ratios", "Return on equity", kind: .pct) { $0.roeBps },
        GridStatementLine("Ratios", "Return on assets", kind: .pct) { $0.roaBps },
        GridStatementLine("Ratios", "Debt to equity", kind: .times) { $0.debtToEquityBps },
        GridStatementLine("Ratios", "Current ratio", kind: .times) { $0.currentRatioBps },
        GridStatementLine("Ratios", "Interest cover", kind: .times) { $0.interestCoverBps },
        GridStatementLine("Ratios", "Revenue growth", kind: .pct) { $0.revenueGrowthBps },
    ]

    static let periods = (0..<8).map { "p\($0)" }

    static let views: [GridViewState] = [
        GridViewState("Income statement", ["name"] + FinanceGrid.periods, filters: [GridFilter(op: .equals, column: "group", text: "Income")]),
        GridViewState("Balance sheet", ["name"] + FinanceGrid.periods, filters: [GridFilter(op: .equals, column: "group", text: "Balance")]),
        GridViewState("Cash flow", ["name"] + FinanceGrid.periods, filters: [GridFilter(op: .equals, column: "group", text: "Cash flow")]),
        GridViewState("Ratios", ["name"] + FinanceGrid.periods, filters: [GridFilter(op: .equals, column: "group", text: "Ratios")]),
        GridViewState("Trend", ["name", "p0", "p0.change", "p0.percent", "p0.trend"], filters: [GridFilter(op: .equals, column: "group", text: "Income")]),
    ]

    /// The newest period first; the newest carries the whole history, so its change, percent and
    /// trend follow the timeframe. [statements] are oldest first, as the engine gives them.
    static func columns(_ statements: [StatementDto], step: Int) -> [GridColumn<GridStatementLine>] {
        let newest = Array(statements.reversed().prefix(8))
        let name = GridColumn<GridStatementLine>("name", "Line", width: nil, numeric: false, topic: "About", text: { $0.title },
                                             tone: { $0.total ? Theme.gold : Theme.primaryText })
        let group = GridColumn<GridStatementLine>("group", "Statement", width: 120, numeric: false, topic: "About", text: { $0.group }, tone: { _ in Theme.secondary })
        let cols = newest.enumerated().map { (i, s) -> GridColumn<GridStatementLine> in
            let history: ((GridStatementLine) -> [Int64])? = i == 0 ? { l in l.kind == .money ? statements.compactMap { l.value($0) } : [] } : nil
            return GridColumn<GridStatementLine>(
                "p\(i)", s.label + (Int(s.weeks) < step ? " *" : ""), width: 90, topic: "Periods",
                text: { $0.show($0.value(s)) },
                value: { $0.value(s) },
                tone: { l in
                    guard l.total, l.kind == .money, let v = l.value(s) else { return Theme.primaryText }
                    return v >= 0 ? Theme.positive : Theme.negative
                },
                series: history,
                step: step,
                format: { Format.moneyCompact($0) },
                scale: 100
            )
        }
        return [name, group] + cols
    }
}

struct FinanceGridScreen: View {
    private enum Period: String, CaseIterable, Identifiable {
        case week, quarter, year
        var id: String { rawValue }
        var label: String { self == .week ? "Weeks" : self == .quarter ? "Quarters" : "Years" }
        var step: Int { self == .week ? 1 : self == .quarter ? 13 : 52 }
    }

    @Environment(AppStore.self) private var store
    @State private var period = Period.quarter
    @State private var statements: [StatementDto] = []

    var body: some View {
        let firm = store.selectedFirm
        DataGrid("Statements", screen: "finance", views: FinanceGrid.views, rows: FinanceGrid.lines,
                 columns: FinanceGrid.columns(statements, step: period.step),
                 key: { AnyHashable($0.group + $0.title) },
                 words: { "\($0.group) \($0.title)" },
                 accessory: AnyView(PixelChoices(options: Period.allCases.map { ($0, $0.label) }, selection: $period, label: "Periods").frame(width: 260)))
            .padding(8)
            .task(id: "\(store.historyRefreshKey)-\(firm ?? 0)-\(period.rawValue)") {
                guard let firm else { statements = []; return }
                let code = period.rawValue
                statements = await store.read { $0.statements(firm: firm, period: code) } ?? []
            }
    }
}
