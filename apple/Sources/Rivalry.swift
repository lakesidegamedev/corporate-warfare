import SwiftUI

/// A small capsule naming how a rival's chief executive plays.
struct TemperamentTag: View {
    let key: String

    var body: some View {
        Text(temperamentLabel(key))
            .font(.caption2.weight(.semibold))
            .foregroundStyle(Theme.gold)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Theme.panelRaised, in: Pixel.small)
            .accessibilityLabel("Temperament \(temperamentLabel(key))")
    }
}

/// Grudge and ally capsules for a rival's standing with you.
struct StandingTags: View {
    let tag: RivalTagDto

    var body: some View {
        HStack(spacing: 4) {
            if tag.grudgeBps > 0 {
                Label { Text(tag.grudgeBps >= 6_000 ? "Bitter grudge" : "Grudge") } icon: { PixelIcon("target", size: 10) }
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Theme.negative)
                    .accessibilityLabel("Holds a grudge against you")
            }
            if tag.ally {
                Label { Text("Ally") } icon: { PixelIcon("people", size: 10) }
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Theme.positive)
                    .accessibilityLabel("Allied with you")
            }
        }
    }
}

/// A rival's chief executive: portrait, name, temperament, and grudge or ally tags.
struct RivalTagLine: View {
    let tag: RivalTagDto
    var portrait: CGFloat = 28

    var body: some View {
        HStack(spacing: 8) {
            if !tag.ceoId.isEmpty { PixelPortrait(person: tag.ceoId, size: portrait) }
            VStack(alignment: .leading, spacing: 2) {
                Text(tag.ceoName.isEmpty ? tag.firmName : tag.ceoName)
                    .font(.caption.weight(.semibold)).foregroundStyle(Theme.primaryText).lineLimit(1)
                HStack(spacing: 4) {
                    if !tag.temperament.isEmpty { TemperamentTag(key: tag.temperament) }
                    StandingTags(tag: tag)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// The chief and tags of one company, loaded from the engine. Draws nothing for a firm without a chief.
struct RivalIdentity: View {
    @EnvironmentObject private var store: AppStore
    let firm: UInt64
    var portrait: CGFloat = 28
    @State private var tag: RivalTagDto?

    var body: some View {
        Group {
            if let tag, !tag.ceoName.isEmpty || tag.grudgeBps > 0 || tag.ally {
                RivalTagLine(tag: tag, portrait: portrait)
            }
        }
        .task(id: firm) { tag = await store.rivalTag(firm) }
    }
}

/// A Raid button for one rival market. It opens the panel with the engine's odds, or says why
/// the raid is not open and how to open it.
struct RaidMarketButton: View {
    @EnvironmentObject private var store: AppStore
    let target: UInt64
    let region: String
    let industry: String
    var compact = false
    @State private var open = false

    var body: some View {
        Button { open = true } label: {
            PixelLabel("Raid", icon: "flag").font(compact ? .caption.weight(.semibold) : .callout.weight(.semibold))
        }
        .buttonStyle(PixelButtonStyle())
        .tint(Theme.negative)
        .disabled(store.isBusy)
        .accessibilityLabel("Raid \(Sector.name(industry)) in \(WorldMap.shared.regionName(region))")
        .sheet(isPresented: $open) {
            RaidPanel(target: target, region: region, industry: industry)
                .environmentObject(store)
                .presentationBackground(Theme.panel)
        }
    }
}

/// The raid campaign when the market is open, otherwise one line of why and a way to build in.
struct RaidPanel: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let target: UInt64
    let region: String
    let industry: String
    @State private var check: RaidCheckDto?
    @State private var building = false

    var body: some View {
        Group {
            if let option = check?.option {
                RaidSheet(option: option, regionName: WorldMap.shared.regionName(region))
            } else {
                NavigationStack {
                    VStack(alignment: .leading, spacing: 14) {
                        Panel {
                            VStack(alignment: .leading, spacing: 10) {
                                Eyebrow(text: "Market raid")
                                if let check {
                                    PixelLabel(check.reason, icon: "blocked")
                                        .font(.callout).foregroundStyle(Theme.warning)
                                    if check.needsBuild {
                                        Button { building = true } label: {
                                            PixelLabel("Build in \(WorldMap.shared.regionName(region))", icon: "plus")
                                                .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                                        }
                                        .buttonStyle(PixelButtonStyle(prominent: true))
                                        .tint(Theme.gold)
                                        .foregroundStyle(Theme.ink)
                                    }
                                } else {
                                    ProgressView()
                                }
                            }
                        }
                        Spacer()
                    }
                    .padding(16)
                    .background(Theme.panel)
                    .navigationTitle("Market raid")
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
                }
            }
        }
        .task { check = await store.raidCheck(target: target, region: region, industry: industry) }
        .sheet(isPresented: $building, onDismiss: { Task { check = await store.raidCheck(target: target, region: region, industry: industry) } }) {
            BuildFlowView(regionCode: region)
                .environmentObject(store)
                .presentationBackground(Theme.panel)
        }
    }
}

/// Every market a rival shares a plant footprint in, each with a Raid button.
struct RaidMarketsList: View {
    @EnvironmentObject private var store: AppStore
    let firm: UInt64

    private struct Market: Hashable { let region: String; let industry: String }

    private var markets: [Market] {
        let all = store.mapPlants.filter { $0.firm == firm && !$0.player }
            .map { Market(region: $0.region, industry: $0.industry) }
        var seen = Set<Market>()
        return all.filter { seen.insert($0).inserted }
            .sorted { ($0.region, $0.industry) < ($1.region, $1.industry) }
    }

    var body: some View {
        if !markets.isEmpty && firm != store.selectedFirm {
            VStack(alignment: .leading, spacing: 6) {
                Eyebrow(text: "Raid their markets")
                ForEach(markets.prefix(8), id: \.self) { market in
                    HStack(spacing: 8) {
                        SectorBadge(sector: market.industry, size: 20)
                        Text("\(Sector.name(market.industry)) · \(WorldMap.shared.regionName(market.region))")
                            .font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                        Spacer()
                        RaidMarketButton(target: firm, region: market.region, industry: market.industry, compact: true)
                    }
                }
            }
        }
    }
}

/// The CEO Office's compact rival board: who is out to get you, who is friendly, and who you can
/// raid right now with the engine's quoted chance.
struct RivalsCard: View {
    @EnvironmentObject private var store: AppStore
    @State private var raiding: RaidOptionDto?

    private var threats: [RivalSummaryDto] { store.rivals.filter { $0.tag.grudgeBps > 0 }.prefix(3).map { $0 } }
    private var allies: [RivalSummaryDto] { store.rivals.filter { $0.tag.ally }.prefix(3).map { $0 } }
    private var targets: [RivalSummaryDto] {
        store.rivals.filter { $0.bestRaid != nil }
            .sorted { ($0.bestRaid?.quote.chanceBps ?? 0) > ($1.bestRaid?.quote.chanceBps ?? 0) }
            .prefix(3).map { $0 }
    }

    var body: some View {
        if !store.rivals.isEmpty {
            Panel(padding: 12) {
                VStack(alignment: .leading, spacing: 10) {
                    Eyebrow(text: "Rivals")
                    group("Threats", icon: "target", tint: Theme.negative, rows: threats, empty: "Nobody holds a grudge against you.")
                    group("Allies", icon: "people", tint: Theme.positive, rows: allies, empty: "No allies yet. A joint venture or a friendly deal makes one.")
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Raid now").font(.caption.weight(.semibold)).foregroundStyle(Theme.secondary)
                        if targets.isEmpty {
                            Text("No rival shares a market with you yet. Build where a rival operates to raid them.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                        ForEach(targets, id: \.tag.firm) { row in
                            if let option = row.bestRaid {
                                HStack(spacing: 8) {
                                    RivalTagLine(tag: row.tag)
                                    Spacer()
                                    VStack(alignment: .trailing, spacing: 1) {
                                        Text("\(option.quote.chanceBps / 100)%").font(.callout.monospacedDigit().weight(.semibold))
                                            .foregroundStyle(Theme.primaryText)
                                        Text("\(Sector.name(option.industry))").font(.caption2).foregroundStyle(Theme.secondary)
                                    }
                                    Button("Raid") { raiding = option }
                                        .buttonStyle(PixelButtonStyle()).tint(Theme.negative).disabled(store.isBusy)
                                }
                            }
                        }
                    }
                }
            }
            .sheet(item: $raiding) { option in
                RaidSheet(option: option, regionName: WorldMap.shared.regionName(option.region)).environmentObject(store)
            }
        }
    }

    private func group(_ title: String, icon: String, tint: Color, rows: [RivalSummaryDto], empty: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.caption.weight(.semibold)).foregroundStyle(Theme.secondary)
            if rows.isEmpty {
                Text(empty).font(.caption).foregroundStyle(Theme.secondary)
            }
            ForEach(rows, id: \.tag.firm) { row in
                HStack(spacing: 8) {
                    PixelIcon(icon, size: 12).foregroundStyle(tint)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(row.tag.firmName).font(.callout).foregroundStyle(Theme.primaryText).lineLimit(1)
                        Text(row.tag.ceoName.isEmpty ? "\(row.sharedMarkets) shared markets"
                             : "\(row.tag.ceoName) · \(temperamentLabel(row.tag.temperament))")
                            .font(.caption2).foregroundStyle(Theme.secondary).lineLimit(1)
                    }
                }
            }
        }
    }
}

/// One rival move in the news, naming the chief and temperament behind it.
struct RivalMoveRow: View {
    let move: RivalMoveFeedDto

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(move.text).font(.callout).foregroundStyle(Theme.primaryText)
            Text(byline).font(.caption).foregroundStyle(Theme.secondary)
        }
        .accessibilityElement(children: .combine)
    }

    private var byline: String {
        let who = move.ceoName.isEmpty ? move.firmName : "\(move.ceoName) (\(temperamentLabel(move.temperament))) at \(move.firmName)"
        return "\(ResultBuckets.weekLabel(move.week)) · \(who)"
    }
}
