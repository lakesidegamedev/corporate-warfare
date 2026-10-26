import SwiftUI

/// The commodity board for one state: every good grouped by the sector that makes it, with the
/// last clearing price against reference, local stock, weekly sales and any import room.
struct MarketsView: View {
    @EnvironmentObject private var store: AppStore
    @State private var showingPrice = false

    private var stateCode: String? { store.selectedRegionCode ?? store.states.first?.code }
    private var state: StateDto? { store.states.first { $0.code == stateCode } }

    /// Product id to the sector whose strategies make it. Goods no strategy makes are listed last.
    private var sectorOfProduct: [String: String] {
        var map: [String: String] = [:]
        for strategy in store.strategyCatalogue where map[strategy.output] == nil {
            map[strategy.output] = strategy.industry
        }
        return map
    }

    private var groups: [BoardGroup] {
        let sectors = sectorOfProduct
        var bySector: [String: [AppStore.MarketRow]] = [:]
        for row in store.marketRows {
            let sector = row.market.flatMap { sectors[$0.product] } ?? "other"
            bySector[sector, default: []].append(row)
        }
        return (Sector.order + ["other"]).compactMap { sector in
            guard let rows = bySector[sector], !rows.isEmpty else { return nil }
            return BoardGroup(sector: sector, rows: rows.sorted { $0.product.name < $1.product.name })
        }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    statePicker
                    if let state {
                        Text("\(state.name) clears every good weekly. Prices are per lot. Services expire each week and cannot be stockpiled.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                    ForEach(groups) { group in
                        Panel {
                            VStack(alignment: .leading, spacing: 8) {
                                HStack(spacing: 8) {
                                    if group.sector != "other" {
                                        SectorBadge(sector: group.sector, size: 22)
                                    }
                                    Eyebrow(text: group.sector == "other" ? "Other goods" : Sector.name(group.sector))
                                }
                                header
                                ForEach(group.rows) { row in
                                    BoardRow(row: row, room: room(for: row))
                                }
                            }
                        }
                    }
                }
                .padding(16)
            }
            .background(Theme.ink)
            .navigationTitle("Markets")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button("Set a price") { showingPrice = true }
                        .disabled(store.overview?.primaryFirm == nil)
                }
            }
            .sheet(isPresented: $showingPrice) { PriceSheet().environmentObject(store) }
        }
    }

    private var statePicker: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(store.states, id: \.code) { item in
                    let selected = item.code == stateCode
                    Button {
                        Task { await store.selectRegion(item.code) }
                    } label: {
                        Text(item.name)
                            .font(.callout.weight(selected ? .semibold : .regular))
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .foregroundStyle(selected ? Theme.ink : Theme.primaryText)
                            .background(selected ? Theme.gold : Theme.panelRaised, in: Capsule())
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(selected ? .isSelected : [])
                }
            }
        }
    }

    private var header: some View {
        HStack {
            Text("Good").frame(maxWidth: .infinity, alignment: .leading)
            Text("Price").frame(width: 92, alignment: .trailing)
            Text("vs ref").frame(width: 56, alignment: .trailing)
            Text("Sold/wk").frame(width: 64, alignment: .trailing)
        }
        .font(.caption2.weight(.semibold))
        .foregroundStyle(Theme.secondary)
    }

    private func room(for row: AppStore.MarketRow) -> RoomDto? {
        guard let id = row.market?.product else { return nil }
        return state?.room.first { $0.product == id }
    }
}

private struct BoardGroup: Identifiable {
    let sector: String
    let rows: [AppStore.MarketRow]
    var id: String { sector }
}

private struct BoardRow: View {
    let row: AppStore.MarketRow
    let room: RoomDto?

    private var price: Int64 { row.market?.lastPriceCents ?? row.product.referencePriceCents }
    private var deltaBps: Int64 {
        let reference = row.product.referencePriceCents
        guard reference > 0 else { return 0 }
        return (price - reference) * 10_000 / reference
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(row.product.name)
                    .font(.callout)
                    .foregroundStyle(Theme.primaryText)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text(Format.money(price))
                    .font(.callout.monospacedDigit())
                    .foregroundStyle(Theme.primaryText)
                    .frame(width: 92, alignment: .trailing)
                Text(String(format: "%+.0f%%", Double(deltaBps) / 100))
                    .font(.caption.monospacedDigit().weight(.semibold))
                    .foregroundStyle(deltaBps > 200 ? Theme.positive : deltaBps < -200 ? Theme.negative : Theme.secondary)
                    .frame(width: 56, alignment: .trailing)
                Text(Format.lots(row.market?.lastWeekSalesLots ?? 0))
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(Theme.secondary)
                    .frame(width: 64, alignment: .trailing)
            }
            HStack(spacing: 10) {
                Text("Stock \(Format.lots(row.market?.regionalStockLots ?? 0))")
                if let market = row.market {
                    Text("Import \(Format.money(market.gatewaySellCents)) · export \(Format.money(market.gatewayBuyCents))")
                }
                if let room {
                    Text("\(room.importLots.formatted()) lots/wk imported")
                        .foregroundStyle(Theme.positive)
                }
            }
            .font(.caption2.monospacedDigit())
            .foregroundStyle(Theme.secondary)
            .lineLimit(1)
            .minimumScaleFactor(0.8)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
    }
}
