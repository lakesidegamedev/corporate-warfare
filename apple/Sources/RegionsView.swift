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
                    SectionCard(title: "Regions, \(String(ResultBuckets.startYear))") {
                        Text("The economic regions of this scenario. County membership and calibrated demand are labelled engineering estimates.")
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
