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
                    SectionCard(title: "Scenario: Great Lakes, 2000") {
                        Text("Eleven BEA economic areas around the Great Lakes and Ohio Valley. Region codes and names come from the verified 2004 roster; county membership and calibrated demand are labelled engineering estimates.")
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
