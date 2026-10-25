import SwiftUI

struct RegionsView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @State private var compare: Set<String> = []
    @State private var showingBuild = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    ContextChip()
                    SectionCard(title: "Corridor") {
                        Text("Eleven BEA economic areas of the Great Lakes corridor. Region codes and names come from the verified 2004 roster; county membership and calibrated demand are labelled engineering estimates.")
                            .font(.footnote)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                    }

                    ForEach(store.regions, id: \.code) { region in
                        SectionCard(title: region.name) {
                            HStack(spacing: 12) {
                                MetricTile(title: "Wage (mid)", value: Format.money(region.wageMidCents), note: "per week")
                                MetricTile(title: "Employed", value: "\(region.employed.formatted())", note: "of \(region.population.formatted())")
                            }
                            HStack(spacing: 12) {
                                MetricTile(title: "Demand index", value: Format.percentBps(region.demandIndexBps))
                                MetricTile(title: "Land index", value: Format.percentBps(region.landIndexBps))
                            }
                            if let selected = store.selectedRegionCode {
                                Text("Distance from \(selected): \(region.distanceKm) km · freight \(Format.money(Int64(region.distanceKm) * 5)) per lot")
                                    .font(.caption)
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                            }
                            HStack {
                                Button(compare.contains(region.code) ? "Selected" : "Compare") {
                                    if compare.contains(region.code) {
                                        compare.remove(region.code)
                                    } else if compare.count < 3 {
                                        compare.insert(region.code)
                                    }
                                }
                                Button("Select") {
                                    Task { await store.selectRegion(region.code) }
                                }
                                Button("Build here") {
                                    showingBuild = true
                                }
                            }
                            .buttonStyle(.bordered)
                        }
                    }
                }
                .padding(16)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Regions")
            .sheet(isPresented: $showingBuild) { BuildSheet().environmentObject(store) }
        }
    }
}

struct MarketsView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @State private var showingPrice = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    ContextChip()
                    SectionCard(title: "Destination market") {
                        Text(store.selectedRegionCode ?? store.regions.first?.name ?? "Corridor")
                            .font(.headline)
                        Text("Prices are cents per whole lot. Service products expire weekly and cannot be stockpiled.")
                            .font(.footnote)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                        Button("Set a price") { showingPrice = true }
                    }

                    ForEach(store.marketRows) { row in
                        SectionCard(title: row.product.name) {
                            HStack(spacing: 12) {
                                MetricTile(
                                    title: "Reference",
                                    value: Format.money(row.product.referencePriceCents)
                                )
                                MetricTile(
                                    title: "Last sale",
                                    value: Format.money(row.market?.lastPriceCents ?? row.product.referencePriceCents)
                                )
                            }
                            HStack(spacing: 12) {
                                MetricTile(
                                    title: "Regional stock",
                                    value: Format.lots(row.market?.regionalStockLots ?? 0)
                                )
                                MetricTile(
                                    title: "Sold last week",
                                    value: Format.lots(row.market?.lastWeekSalesLots ?? 0)
                                )
                            }
                            Text("Gateway buys at \(Format.money(row.market?.gatewayBuyCents ?? 0)), sells at \(Format.money(row.market?.gatewaySellCents ?? 0)). Finite volume each week.")
                                .font(.caption)
                                .foregroundStyle(Theme.secondaryText(for: scheme))
                        }
                    }
                }
                .padding(16)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Markets")
            .sheet(isPresented: $showingPrice) { PriceSheet().environmentObject(store) }
        }
    }
}
