import SwiftUI

struct InboxView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @State private var pendingEvent: EventDto?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    ContextChip()

                    SectionCard(title: "Decisions") {
                        if store.events.isEmpty {
                            EmptyNote(text: "No open decisions. Routine news is summarised with the quarterly report.")
                        } else {
                            ForEach(store.events, id: \.id) { event in
                                VStack(alignment: .leading, spacing: 8) {
                                    Text(event.title).font(.headline).foregroundStyle(Theme.primaryText)
                                    Text(event.subject)
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                    if event.mandatory {
                                        Text("Required before time advances · deadline week \(event.deadline)")
                                            .font(.caption2)
                                            .foregroundStyle(Theme.warning)
                                    }
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
                                                .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                                                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
                                        }
                                        .buttonStyle(.plain)
                                        .disabled(store.isBusy)
                                    }
                                }
                                .padding(.vertical, 4)
                            }
                        }
                    }

                    SectionCard(title: "Market news") {
                        if store.recentRaids.isEmpty {
                            EmptyNote(text: "No market raids yet. Rivals that share a market with you can raid it; so can you, from a region's board.")
                        } else {
                            ForEach(Array(store.recentRaids.prefix(8).enumerated()), id: \.offset) { _, raid in
                                RaidNewsRow(raid: raid)
                            }
                        }
                    }

                    SectionCard(title: "Deals") {
                        if store.deals.isEmpty {
                            EmptyNote(text: "No bids. A tender escrows the full maximum price plus two percent, then diligence runs two weeks before the four-week offer.")
                        } else {
                            ForEach(store.deals, id: \.handle) { deal in
                                VStack(alignment: .leading) {
                                    Text("\(deal.buyer) bids for \(deal.target)").font(.callout).foregroundStyle(Theme.primaryText)
                                    Text("\(deal.state.replacingOccurrences(of: "_", with: " ")) · offer \(Format.money(deal.offerPriceCents))/share")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondary)
                                    Text("Escrow \(Format.money(deal.escrowCents)) · committed \(deal.committedShares.formatted()) shares")
                                        .font(.caption2)
                                        .foregroundStyle(Theme.secondary)
                                }
                            }
                        }
                    }

                    SectionCard(title: "Recent obligations") {
                        Text("Loans, policies and contracts are listed on the Company tab under Debt and Supply.")
                            .font(.footnote)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                        Button("Save now") {
                            Task { await store.manualSave() }
                        }
                    }

                    if !store.statusLine.isEmpty {
                        Text(store.statusLine)
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                    }
                }
                .padding(16)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Inbox")
        }
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
                Text("Week \(raid.week) · \(CorridorMap.shared.regionName(raid.region))")
                    .font(.caption).foregroundStyle(tone)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private var headline: String {
        let market = Sector.name(raid.industry)
        if raid.success {
            return "\(raid.attacker) took \(raid.blocks) \(market) block\(raid.blocks == 1 ? "" : "s") from \(raid.defender)"
        }
        return "\(raid.attacker) raided \(raid.defender) in \(market) and failed"
    }
}
