import SwiftUI

/// A region's board: its state and specializations, who operates in each sector, what it
/// imports, its conditions and the plants there. Build opens the build flow in this region;
/// a rival division you share a cell with can be raided from here.
struct RegionBoardView: View {
    @EnvironmentObject private var store: AppStore
    let board: RegionBoardDto
    let onBuild: () -> Void
    let onClose: () -> Void
    @State private var raiding: RaidOptionDto?

    private var plants: [MapPlantDto] {
        store.mapPlants.filter { $0.region == board.code }
            .sorted { ($0.player ? 0 : 1, $0.industry, $0.firmName) < ($1.player ? 0 : 1, $1.industry, $1.firmName) }
    }

    private var cells: [SectorCellDto] {
        board.sectors.sorted {
            ($0.ownBlocks > 0 ? 0 : 1, -Int($0.operators), Sector.order.firstIndex(of: $0.industry) ?? 99)
                < ($1.ownBlocks > 0 ? 0 : 1, -Int($1.operators), Sector.order.firstIndex(of: $1.industry) ?? 99)
        }
    }

    private var raids: [RaidOptionDto] {
        store.raidTargets.filter { $0.region == board.code }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                header
                Button(action: onBuild) {
                    PixelLabel("Build in \(board.name)", icon: "plus")
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
                        Eyebrow(text: "Sectors here")
                        if cells.isEmpty {
                            Text("No sector operates or imports here yet.").font(.callout).foregroundStyle(Theme.secondary)
                        }
                        ForEach(cells, id: \.industry) { cell in
                            SectorCellRow(cell: cell, specialized: specialization(of: cell.industry))
                        }
                        Text("Share is capacity value against all capacity here plus unmet imports. Expanding past half a market costs more.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }

                if !raids.isEmpty {
                    Panel {
                        VStack(alignment: .leading, spacing: 10) {
                            Eyebrow(text: "Market raids")
                            ForEach(raids, id: \.target) { option in
                                raidRow(option)
                            }
                            Text("A campaign takes a share of a rival's plants in a market you both serve. The rival always keeps one plant.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                    }
                }

                Panel {
                    VStack(alignment: .leading, spacing: 10) {
                        Eyebrow(text: "Room to build")
                        if board.room.isEmpty {
                            Text("Local plants meet this region's demand. Nothing is imported right now.")
                                .font(.callout).foregroundStyle(Theme.secondary)
                        } else {
                            ForEach(board.room.prefix(6), id: \.product) { room in
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
                        }
                    }
                }

                Panel {
                    VStack(alignment: .leading, spacing: 10) {
                        Eyebrow(text: "\(board.stateName) conditions")
                        ConditionBar(label: "Power grid reliability", bps: board.powerGridBps)
                        ConditionBar(label: "Roads", bps: board.roadBps)
                        ConditionBar(label: "Workforce skill", bps: board.skillBps)
                        ConditionBar(label: "Broadband reach", bps: board.broadbandBps)
                        HStack {
                            figure("Wage index", board.wageIndexBps)
                            figure("Land", board.landIndexBps)
                            figure("Cost of living", board.costOfLivingBps)
                            figure("Union", board.organisationBps)
                        }
                    }
                }

                Panel {
                    VStack(alignment: .leading, spacing: 10) {
                        Eyebrow(text: "Plants in \(board.name)")
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
                                    Text("\(plant.strategy) · \(plant.blocks) plant\(plant.blocks == 1 ? "" : "s")")
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
        .sheet(item: $raiding) { option in
            RaidSheet(option: option, regionName: board.name).environmentObject(store)
        }
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 4) {
                Eyebrow(text: board.stateName)
                Text(board.name).font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                Text("Hub \(board.hubCity) · \(board.population2000.formatted()) people (2000)")
                    .font(.caption).foregroundStyle(Theme.secondary)
                if !board.specialization.isEmpty {
                    HStack(spacing: 6) {
                        ForEach(Array(board.specialization.enumerated()), id: \.offset) { index, sector in
                            HStack(spacing: 4) {
                                SectorBadge(sector: sector, size: 16)
                                Text(Sector.name(sector) + (index == 0 ? " specialist" : ""))
                                    .font(.caption2.weight(.semibold))
                                    .foregroundStyle(Theme.primaryText)
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(Theme.panelRaised, in: Capsule())
                        }
                    }
                    .padding(.top, 2)
                    .accessibilityElement(children: .combine)
                    .accessibilityLabel("\(board.stateName) specializes in \(board.specialization.map(Sector.name).joined(separator: " and "))")
                }
            }
            Spacer()
            Button(action: onClose) {
                PixelIcon("close", size: 24).foregroundStyle(Theme.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Close \(board.name)")
        }
    }

    private func specialization(of industry: String) -> Int? {
        board.specialization.firstIndex(of: industry)
    }

    private func raidRow(_ option: RaidOptionDto) -> some View {
        let week = store.overview?.tick ?? 0
        let ready = option.readyWeek <= week
        return HStack(spacing: 10) {
            SectorBadge(sector: option.industry, size: 26)
            VStack(alignment: .leading, spacing: 1) {
                Text(option.targetName).font(.callout).foregroundStyle(Theme.primaryText)
                Text("\(option.rivalBlocks) plants · \(option.quote.chanceBps / 100)% at a 2:1 campaign")
                    .font(.caption.monospacedDigit()).foregroundStyle(Theme.secondary)
            }
            Spacer()
            Button(ready ? "Raid" : "Week \(option.readyWeek)") { raiding = option }
                .buttonStyle(.bordered)
                .tint(Theme.negative)
                .disabled(!ready || store.isBusy)
        }
    }

    private func figure(_ label: String, _ bps: Int32) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption2).foregroundStyle(Theme.secondary)
            Text(String(format: "%.0f", Double(bps) / 100)).font(.callout.monospacedDigit()).foregroundStyle(Theme.primaryText)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// One sector type in a region: operators, the leader and the player's position.
struct SectorCellRow: View {
    let cell: SectorCellDto
    let specialized: Int?

    var body: some View {
        HStack(spacing: 10) {
            SectorBadge(sector: cell.industry, size: 26)
            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 4) {
                    Text(Sector.name(cell.industry)).font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    if specialized != nil {
                        PixelIcon("star", size: 12).foregroundStyle(Theme.gold)
                            .accessibilityLabel("State specialty")
                    }
                }
                Text(subtitle).font(.caption).foregroundStyle(Theme.secondary).lineLimit(1)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 1) {
                if cell.ownBlocks > 0 {
                    Text("\(cell.ownShareBps / 100)%").font(.callout.monospacedDigit().weight(.semibold)).foregroundStyle(Theme.gold)
                    Text("\(cell.ownBlocks) yours").font(.caption2).foregroundStyle(Theme.secondary)
                } else if cell.headroomCents > 0 {
                    Text("Imports").font(.caption.weight(.semibold)).foregroundStyle(Theme.positive)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }

    private var subtitle: String {
        if cell.operators == 0 { return "No operator; demand is imported" }
        let lead = cell.leader.isEmpty ? "" : " · \(cell.leader) \(cell.leaderShareBps / 100)%"
        return "\(cell.operators) operator\(cell.operators == 1 ? "" : "s")" + lead
    }
}

/// Choose a campaign size against a rival division and launch the raid.
struct RaidSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let option: RaidOptionDto
    let regionName: String
    @State private var multiple: Double = 2
    @State private var quote: RaidQuoteDto?
    @State private var rejection: CommandRejection?
    @State private var result: RaidRecordDto?

    private var budget: Int64 { Int64((Double(option.quote.defenceCents) * multiple).rounded()) }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Panel {
                        VStack(alignment: .leading, spacing: 8) {
                            Eyebrow(text: "Target")
                            Text(option.targetName).font(.headline).foregroundStyle(Theme.primaryText)
                            Text("\(Sector.name(option.industry)) in \(regionName) · \(option.rivalBlocks) plants")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                    }
                    Panel {
                        VStack(alignment: .leading, spacing: 10) {
                            Eyebrow(text: "Campaign")
                            ValueSlider(
                                value: $multiple, range: 0.5...4, step: 0.1, ticks: 35,
                                label: { String(format: "%.1f to 1", $0) },
                                detail: { "Budget \(Format.money(Int64((Double(option.quote.defenceCents) * $0).rounded()))) against their defence of \(Format.money(option.quote.defenceCents))" },
                                onCommit: { _ in Task { await refresh() } }
                            )
                            row("Budget", Format.money(budget))
                            if let quote {
                                row("Chance", "\(quote.chanceBps / 100)%")
                                row("Plants at stake", "\(quote.blocksAtStake) plant\(quote.blocksAtStake == 1 ? "" : "s")")
                                row("Price if it lands", Format.money(quote.priceCents))
                            }
                            Text("The budget buys advertising whatever happens. If the raid lands you take the plants into your division at your strategy and pay the rival half their book value.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                    }
                    if let result {
                        PixelLabel(
                            result.success ? "Took \(result.blocks) plant\(result.blocks == 1 ? "" : "s") from \(result.defender)" : "The campaign did not move \(result.defender)'s customers",
                            icon: result.success ? "check" : "blocked"
                        )
                        .font(.callout.weight(.semibold))
                        .foregroundStyle(result.success ? Theme.positive : Theme.warning)
                    }
                    if let rejection {
                        Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                    }
                    Button {
                        Task { await launch() }
                    } label: {
                        PixelLabel("Launch campaign", icon: "flag")
                            .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(Theme.negative)
                    .disabled(store.isBusy || result != nil)
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle("Market raid")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button(result == nil ? "Cancel" : "Done") { dismiss() } }
            }
            .task { await refresh() }
        }
    }

    private func row(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label).font(.callout).foregroundStyle(Theme.secondary)
            Spacer()
            Text(value).font(.callout.monospacedDigit()).foregroundStyle(Theme.primaryText)
        }
    }

    private func refresh() async {
        quote = await store.raidQuote(option, budgetCents: budget)
    }

    private func launch() async {
        rejection = await store.raid(option, budgetCents: budget)
        if rejection == nil {
            result = store.recentRaids.first
        }
    }
}
