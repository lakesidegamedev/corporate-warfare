import SwiftUI

/// A state's economy board: who operates there, what it imports and how its conditions treat
/// each sector. The Build action starts the build flow in this state.
struct StateBoardView: View {
    @EnvironmentObject private var store: AppStore
    let state: StateDto
    let onBuild: () -> Void
    let onClose: () -> Void

    private var plants: [MapPlantDto] {
        store.mapPlants.filter { $0.region == state.code }
            .sorted { ($0.player ? 0 : 1, $0.industry, $0.firmName) < ($1.player ? 0 : 1, $1.industry, $1.firmName) }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                header
                Button(action: onBuild) {
                    Label("Build a plant in \(state.name)", systemImage: "plus.circle.fill")
                        .font(.headline)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 6)
                }
                .buttonStyle(.borderedProminent)
                .tint(Theme.gold)
                .foregroundStyle(Theme.ink)
                .disabled(store.overview?.primaryFirm == nil)

                Panel {
                    VStack(alignment: .leading, spacing: 10) {
                        Eyebrow(text: "Room to build")
                        if state.room.isEmpty {
                            Text("Local plants meet this state's demand. Nothing is imported right now.")
                                .font(.callout).foregroundStyle(Theme.secondary)
                        } else {
                            ForEach(state.room.prefix(6), id: \.product) { room in
                                HStack {
                                    Text(room.productName).font(.callout).foregroundStyle(Theme.primaryText)
                                    Spacer()
                                    VStack(alignment: .trailing, spacing: 1) {
                                        Text("\(room.importLots.formatted()) lots/wk")
                                            .font(.callout.monospacedDigit()).foregroundStyle(Theme.positive)
                                        Text("at \(Format.money(room.importPriceCents))")
                                            .font(.caption2.monospacedDigit()).foregroundStyle(Theme.secondary)
                                    }
                                }
                                .accessibilityElement(children: .combine)
                            }
                            Text("Imports are demand no plant in the state is meeting.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                    }
                }

                Panel {
                    VStack(alignment: .leading, spacing: 10) {
                        Eyebrow(text: "Conditions")
                        ConditionBar(label: "Power grid reliability", bps: state.powerGridBps)
                        ConditionBar(label: "Roads", bps: state.roadBps)
                        ConditionBar(label: "Workforce skill", bps: state.skillBps)
                        ConditionBar(label: "Broadband reach", bps: state.broadbandBps)
                        HStack {
                            conditionFigure("Wage index", state.wageIndexBps)
                            conditionFigure("Cost of living", state.costOfLivingBps)
                            conditionFigure("Crime", state.crimeBps)
                            conditionFigure("Union", state.organisationBps)
                        }
                    }
                }

                Panel {
                    VStack(alignment: .leading, spacing: 10) {
                        Eyebrow(text: "Plants in \(state.name)")
                        if plants.isEmpty {
                            Text("No company operates here yet.").font(.callout).foregroundStyle(Theme.secondary)
                        }
                        ForEach(plants, id: \.handle) { plant in
                            HStack(spacing: 10) {
                                SectorBadge(sector: plant.industry, size: 26)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(plant.firmName)
                                        .font(.callout.weight(plant.player ? .semibold : .regular))
                                        .foregroundStyle(plant.player ? Theme.gold : Theme.primaryText)
                                    Text("\(plant.strategy) · \(plant.blocks) block\(plant.blocks == 1 ? "" : "s")")
                                        .font(.caption).foregroundStyle(Theme.secondary)
                                }
                                Spacer()
                                Text(plant.status == "operating" ? "\(plant.utilisationBps / 100)%" : plant.status.capitalized)
                                    .font(.caption.monospacedDigit())
                                    .foregroundStyle(plant.status == "operating" ? Theme.primaryText : Theme.warning)
                            }
                            .accessibilityElement(children: .combine)
                        }
                    }
                }
            }
            .padding(16)
        }
        .background(Theme.panel)
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 4) {
                Text(state.name).font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                Text("Hub \(state.hubCity) · \(state.populationRoot) people (2000 Census)")
                    .font(.caption).foregroundStyle(Theme.secondary)
            }
            Spacer()
            Button(action: onClose) {
                Image(systemName: "xmark.circle.fill").font(.title2).foregroundStyle(Theme.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Close \(state.name)")
        }
    }

    private func conditionFigure(_ label: String, _ bps: Int32) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption2).foregroundStyle(Theme.secondary)
            Text(String(format: "%.0f", Double(bps) / 100)).font(.callout.monospacedDigit()).foregroundStyle(Theme.primaryText)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private extension StateDto {
    var populationRoot: String { population2000.formatted() }
}
