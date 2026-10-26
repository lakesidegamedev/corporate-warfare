import SwiftUI

/// The commodity board for one region, built for scanning: one line per good with its price and
/// a signal for what the market is doing (imported shortage, tight, glut, yours), the supply and
/// demand relation in plain terms underneath, and the detail one tap away.
struct MarketsView: View {
    @EnvironmentObject private var store: AppStore
    @State private var filter: MarketFilter = .all
    @State private var expanded: UInt32?
    @State private var pricing: PriceTarget?

    private var regionCode: String? { store.marketRegionCode }
    private var state: RegionBoardDto? { store.boards.first { $0.code == regionCode } }

    /// Product id to the sector whose strategies make it. Goods no strategy makes are listed last.
    private var sectorOfProduct: [String: String] {
        var map: [String: String] = [:]
        for strategy in store.strategyCatalogue where map[strategy.output] == nil {
            map[strategy.output] = strategy.industry
        }
        return map
    }

    /// Goods the selected company's plants make.
    private var yours: Set<String> {
        let outputs = Dictionary(store.strategyCatalogue.map { ($0.id, $0.output) }, uniquingKeysWith: { first, _ in first })
        return Set(store.facilities.compactMap { outputs[$0.recipe] })
    }

    private func signal(_ row: AppStore.MarketRow) -> MarketSignal {
        let id = row.market?.productId ?? ""
        let sold = row.market?.lastWeekSalesLots ?? 0
        let stock = row.market?.regionalStockLots ?? 0
        if let room = room(for: row), room.importLots > 0 { return .shortage }
        if (sold == 0 && stock > 0) || (sold > 0 && stock > sold * 4) { return .glut }
        if sold > 0 && stock < sold { return .tight }
        return yours.contains(id) ? .steady : .none
    }

    private var groups: [BoardGroup] {
        let sectors = sectorOfProduct
        var bySector: [String: [AppStore.MarketRow]] = [:]
        // Financial balances such as bank deposits carry no price and are not traded goods.
        for row in store.marketRows where row.product.referencePriceCents > 0 {
            let id = row.market?.productId ?? ""
            let keep: Bool
            switch filter {
            case .all: keep = true
            case .opportunities: keep = signal(row) == .shortage || signal(row) == .tight
            case .gluts: keep = signal(row) == .glut
            case .yours: keep = yours.contains(id)
            }
            guard keep else { continue }
            let sector = row.market.flatMap { sectors[$0.productId] } ?? "other"
            bySector[sector, default: []].append(row)
        }
        return (Sector.order + ["other"]).compactMap { sector in
            guard let rows = bySector[sector], !rows.isEmpty else { return nil }
            return BoardGroup(sector: sector, rows: rows.sorted { $0.product.name < $1.product.name })
        }
    }

    var body: some View {
        TabScreen(title: "Markets", subtitle: state.map { "\($0.stateName) · prices per lot" }) {
            regionMenu
        } content: {
            filterBar
            if groups.isEmpty {
                CalloutRow(symbol: "line.3.horizontal.decrease.circle", text: "No goods match this filter in \(state?.name ?? "this region").", tint: Theme.secondary)
            }
            ForEach(groups) { group in
                Panel(padding: 12) {
                    VStack(alignment: .leading, spacing: 0) {
                        HStack(spacing: 8) {
                            if group.sector != "other" {
                                SectorBadge(sector: group.sector, size: 20)
                            }
                            Eyebrow(text: group.sector == "other" ? "Other goods" : Sector.name(group.sector))
                        }
                        .padding(.bottom, 6)
                        ForEach(Array(group.rows.enumerated()), id: \.element.id) { index, row in
                            if index > 0 { Divider().overlay(Theme.stroke) }
                            BoardRow(
                                row: row,
                                room: room(for: row),
                                signal: signal(row),
                                yours: yours.contains(row.market?.productId ?? ""),
                                expanded: expanded == row.id,
                                onTap: { withAnimation(.easeOut(duration: 0.15)) { expanded = expanded == row.id ? nil : row.id } },
                                onSetPrice: { pricing = PriceTarget(product: row.product.index) }
                            )
                        }
                    }
                }
            }
        }
        .sheet(item: $pricing) { target in
            PriceSheet(initialProduct: target.product).environmentObject(store)
        }
    }

    private var regionMenu: some View {
        let states = Array(Set(store.boards.map(\.stateCode))).sorted()
        return Menu {
            ForEach(states, id: \.self) { code in
                Section(store.boards.first { $0.stateCode == code }?.stateName ?? code) {
                    ForEach(store.boards.filter { $0.stateCode == code }, id: \.code) { item in
                        Button(item.name) { Task { await store.selectRegion(item.code) } }
                    }
                }
            }
        } label: {
            HStack(spacing: 6) {
                Text(state?.name ?? "Region")
                    .font(.callout.weight(.semibold))
                    .lineLimit(1)
                Image(systemName: "chevron.up.chevron.down").font(.caption2.weight(.bold))
            }
            .foregroundStyle(Theme.primaryText)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Theme.panelRaised, in: Capsule())
            .overlay(Capsule().stroke(Theme.stroke, lineWidth: 1))
        }
        .accessibilityLabel("Region: \(state?.name ?? "none")")
    }

    private var filterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(MarketFilter.allCases) { option in
                    Button {
                        filter = option
                    } label: {
                        Text(option.title)
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .foregroundStyle(filter == option ? Theme.ink : Theme.primaryText)
                            .background(filter == option ? Theme.gold : Theme.panel, in: Capsule())
                            .overlay(Capsule().stroke(Theme.stroke, lineWidth: filter == option ? 0 : 1))
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(filter == option ? .isSelected : [])
                }
            }
        }
    }

    private func room(for row: AppStore.MarketRow) -> RoomDto? {
        guard let id = row.market?.productId else { return nil }
        return state?.room.first { $0.product == id }
    }
}

private enum MarketFilter: String, CaseIterable, Identifiable {
    case all, opportunities, gluts, yours
    var id: String { rawValue }
    var title: String {
        switch self {
        case .all: return "All goods"
        case .opportunities: return "Short supply"
        case .gluts: return "Gluts"
        case .yours: return "You make"
        }
    }
}

enum MarketSignal: Equatable {
    case shortage, tight, glut, steady, none

    var label: String? {
        switch self {
        case .shortage: return "Imported"
        case .tight: return "Tight"
        case .glut: return "Glut"
        case .steady: return "Steady"
        case .none: return nil
        }
    }

    var tint: Color {
        switch self {
        case .shortage: return Theme.positive
        case .tight: return Theme.gold
        case .glut: return Theme.negative
        case .steady, .none: return Theme.secondary
        }
    }
}

private struct PriceTarget: Identifiable {
    let product: UInt32
    var id: UInt32 { product }
}

private struct BoardGroup: Identifiable {
    let sector: String
    let rows: [AppStore.MarketRow]
    var id: String { sector }
}

private struct BoardRow: View {
    let row: AppStore.MarketRow
    let room: RoomDto?
    let signal: MarketSignal
    let yours: Bool
    let expanded: Bool
    let onTap: () -> Void
    let onSetPrice: () -> Void

    private var price: Int64 { row.market?.lastPriceCents ?? row.product.referencePriceCents }
    private var sold: Int64 { row.market?.lastWeekSalesLots ?? 0 }
    private var stock: Int64 { row.market?.regionalStockLots ?? 0 }
    private var deltaBps: Int64 {
        let reference = row.product.referencePriceCents
        guard reference > 0 else { return 0 }
        return (price - reference) * 10_000 / reference
    }

    /// Supply against demand in words: what sells, how long stock lasts, what is imported.
    private var balance: String {
        var parts: [String] = []
        parts.append(sold > 0 ? "\(sold.formatted())/wk sold" : "nothing sold")
        if sold > 0 {
            let weeks = Double(stock) / Double(sold)
            parts.append(weeks < 0.1 ? "no stock" : String(format: "%.1f wk of stock", weeks))
        } else if stock > 0 {
            parts.append("\(stock.formatted()) in stock")
        }
        if let room, room.importLots > 0 {
            parts.append("\(room.importLots.formatted())/wk imported")
        }
        return parts.joined(separator: " · ")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Button(action: onTap) {
                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 6) {
                        Text(row.product.name)
                            .font(.callout.weight(.medium))
                            .foregroundStyle(Theme.primaryText)
                            .lineLimit(1)
                        if yours {
                            Image(systemName: "star.fill").font(.caption2).foregroundStyle(Theme.gold)
                                .accessibilityLabel("You make this")
                        }
                        if let label = signal.label, signal != .steady {
                            SignalPill(text: label, tint: signal.tint)
                        }
                        Spacer(minLength: 6)
                        Text(Format.money(price))
                            .font(.callout.weight(.semibold).monospacedDigit())
                            .foregroundStyle(Theme.primaryText)
                        Text(String(format: "%+.0f%%", Double(deltaBps) / 100))
                            .font(.caption2.monospacedDigit().weight(.semibold))
                            .foregroundStyle(deltaBps > 200 ? Theme.positive : deltaBps < -200 ? Theme.negative : Theme.secondary)
                            .frame(width: 40, alignment: .trailing)
                    }
                    Text(balance)
                        .font(.caption2.monospacedDigit())
                        .foregroundStyle(Theme.secondary)
                        .lineLimit(1)
                        .minimumScaleFactor(0.85)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if expanded {
                detail
            }
        }
        .padding(.vertical, 7)
        .accessibilityElement(children: .contain)
    }

    private var detail: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 14) {
                figure("Reference", Format.money(row.product.referencePriceCents))
                if let market = row.market, market.gatewaySellCents > 0 {
                    figure("Import at", Format.money(market.gatewaySellCents))
                    figure("Export at", Format.money(market.gatewayBuyCents))
                }
                figure("Stock", stock.formatted())
            }
            Text(advice)
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if yours {
                Button("Set your price", action: onSetPrice)
                    .font(.caption.weight(.semibold))
                    .buttonStyle(.borderedProminent)
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
            }
        }
        .padding(10)
        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private var advice: String {
        switch signal {
        case .shortage:
            return "The region buys from outside the corridor: local supply is short. A plant here would sell into that demand at up to the import price."
        case .tight:
            return "Selling faster than stock builds. Prices tend to rise; producers can raise output."
        case .glut:
            return "More than four weeks of stock against sales. Prices fall until it clears; producers cut output."
        case .steady, .none:
            return "Stock and sales are in balance."
        }
    }

    private func figure(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.caption2).foregroundStyle(Theme.secondary)
            Text(value).font(.caption.weight(.semibold).monospacedDigit()).foregroundStyle(Theme.primaryText)
        }
    }
}
