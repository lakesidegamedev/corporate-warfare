import SwiftUI

/// What needs the player and what happened: decisions first, then the corridor's news. Empty
/// categories are not shown; a quiet week is one line.
struct InboxView: View {
    @EnvironmentObject private var store: AppStore
    @State private var showingPaper = false
    @State private var openLetter: LetterDto?

    private var subtitle: String {
        [store.selectedFirmDto?.name, store.overview?.weekLabel].compactMap { $0 }.joined(separator: " · ")
    }

    private var mandatory: Int { store.events.filter(\.mandatory).count }

    /// Live deals first, closed ones after.
    private var deals: [DealDto] {
        store.deals.sorted { dealRank($0.state) < dealRank($1.state) }
    }

    var body: some View {
        TabScreen(title: "Inbox", subtitle: subtitle, trailing: {
            Button { showingPaper = true } label: {
                Text("THE PAPER").font(Pixel.font(11)).foregroundStyle(Theme.gold)
                    .padding(.horizontal, 10).padding(.vertical, 6).background(Theme.panelRaised)
            }
            .buttonStyle(.plain)
        }) {
            if mandatory > 0 {
                CalloutRow(icon: "hand",
                           text: "\(mandatory) decision\(mandatory == 1 ? "" : "s") needed before time can advance.",
                           tint: Theme.warning)
            }
            LetterStack { openLetter = $0 }
            if !store.recentRaids.isEmpty {
                feedSection("Market raids") {
                    ForEach(Array(store.recentRaids.prefix(8).enumerated()), id: \.offset) { index, raid in
                        if index > 0 { Divider().overlay(Theme.stroke) }
                        RaidNewsRow(raid: raid).padding(.vertical, 6)
                    }
                }
            }
            if !store.rivalFeed.isEmpty {
                feedSection("Rival moves") {
                    ForEach(Array(store.rivalFeed.prefix(8).enumerated()), id: \.offset) { index, move in
                        if index > 0 { Divider().overlay(Theme.stroke) }
                        RivalMoveRow(move: move).padding(.vertical, 6)
                    }
                }
            }
            if !deals.isEmpty {
                feedSection("Takeover bids") {
                    ForEach(Array(deals.enumerated()), id: \.element.handle) { index, deal in
                        if index > 0 { Divider().overlay(Theme.stroke) }
                        DealRow(deal: deal).padding(.vertical, 6)
                    }
                }
            }
        }
        .sheet(item: $openLetter) { letter in
            NavigationStack {
                ScrollView {
                    // The live copy, so reading it or answering its card updates the page.
                    if let live = store.letters.first(where: { $0.id == letter.id }) ?? (letter.event == nil ? letter : nil) {
                        LetterPaperView(letter: live)
                            .padding(14)
                            .frame(maxWidth: .infinity)
                    } else {
                        // An answered decision has left the Inbox; close its page.
                        Color.clear.onAppear { openLetter = nil }
                    }
                }
                .background(Theme.ink)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { openLetter = nil } } }
            }
            .environmentObject(store)
            .presentationBackground(Theme.ink)
        }
        .sheet(isPresented: $showingPaper) {
            NavigationStack {
                PaperView()
                    .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { showingPaper = false } } }
            }
            .environmentObject(store)
            .presentationBackground(Theme.ink)
        }
    }

    private func feedSection<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 0) {
                Eyebrow(text: title).padding(.bottom, 4)
                content()
            }
        }
    }

    private func dealRank(_ state: String) -> Int {
        switch state {
        case "draft", "funded", "diligence", "offer_open", "review": return 0
        case "settled": return 1
        default: return 2
        }
    }
}

/// One decision with its choices as buttons.
struct DecisionCard: View {
    @EnvironmentObject private var store: AppStore
    let event: EventDto

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(alignment: .firstTextBaseline) {
                    Text(event.title).font(.headline).foregroundStyle(Theme.primaryText)
                    Spacer(minLength: 6)
                    if event.mandatory {
                        SignalPill(text: "Due week \(event.deadline)", tint: Theme.warning)
                    }
                }
                Text(event.subject)
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                ForEach(event.choices, id: \.id) { choice in
                    Button {
                        Task { await store.respond(event: event.id, choice: choice.id) }
                    } label: {
                        Text(choice.label)
                            .font(.callout.weight(.medium))
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 9)
                            .foregroundStyle(Theme.primaryText)
                            .background(Theme.panelRaised, in: Pixel.shape)
                            .overlay(Pixel.shape.stroke(Theme.stroke, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .disabled(store.isBusy)
                }
            }
        }
    }
}

/// A takeover bid on one line: bidder, target, stage and price.
private struct DealRow: View {
    let deal: DealDto

    private var stage: String { deal.state.replacingOccurrences(of: "_", with: " ").capitalized }
    private var tint: Color {
        switch deal.state {
        case "expired", "withdrawn", "rejected": return Theme.secondary
        case "settled": return Theme.positive
        default: return Theme.gold
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text("\(deal.buyer) → \(deal.target)")
                    .font(.callout)
                    .foregroundStyle(Theme.primaryText)
                    .lineLimit(2)
                Spacer(minLength: 6)
                SignalPill(text: stage, tint: tint)
            }
            Text("\(Format.money(deal.offerPriceCents))/share · escrow \(Format.moneyCompact(deal.escrowCents)) · \(deal.committedShares.formatted()) shares tendered")
                .font(.caption2.monospacedDigit())
                .foregroundStyle(Theme.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.85)
        }
        .accessibilityElement(children: .combine)
    }
}

/// One market raid in the news: who raided whom, where, and how it went.
struct RaidNewsRow: View {
    let raid: RaidRecordDto

    private var tone: Color {
        if raid.playerDefender && raid.success { return Theme.negative }
        if raid.playerAttacker && raid.success { return Theme.positive }
        return Theme.secondary
    }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            SectorBadge(sector: raid.industry, size: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(headline).font(.callout).foregroundStyle(Theme.primaryText)
                Text("Week \(raid.week) · \(WorldMap.shared.regionName(raid.region))")
                    .font(.caption).foregroundStyle(tone)
                if let who = chiefs {
                    Text(who).font(.caption2).foregroundStyle(Theme.secondary)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }

    /// The chief and temperament behind the attack, and the one who defended.
    private var chiefs: String? {
        func part(_ role: String, _ ceo: String, _ temperament: String) -> String? {
            ceo.isEmpty ? nil : "\(role) \(ceo) (\(temperamentLabel(temperament)))"
        }
        let parts = [part("Led by", raid.attackerCeo, raid.attackerTemperament),
                     part("against", raid.defenderCeo, raid.defenderTemperament)].compactMap { $0 }
        return parts.isEmpty ? nil : parts.joined(separator: " ")
    }

    private var headline: String {
        let market = Sector.name(raid.industry)
        if raid.success {
            return "\(raid.attacker) took \(raid.blocks) \(market) plant\(raid.blocks == 1 ? "" : "s") from \(raid.defender)"
        }
        return "\(raid.attacker) raided \(raid.defender) in \(market) and failed"
    }
}
