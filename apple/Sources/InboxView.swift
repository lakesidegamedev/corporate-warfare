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
                                VStack(alignment: .leading, spacing: 6) {
                                    Text(event.title).font(.headline)
                                    Text(event.subject)
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                    if event.mandatory {
                                        Text("Required before time advances · deadline week \(event.deadline)")
                                            .font(.caption2)
                                            .foregroundStyle(Theme.warning)
                                    }
                                    HStack {
                                        ForEach(event.choices, id: \.id) { choice in
                                            Button(choice.label) {
                                                Task { await store.respond(event: event.id, choice: choice.id) }
                                            }
                                            .buttonStyle(.bordered)
                                            .font(.caption)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    SectionCard(title: "Deals") {
                        if store.deals.isEmpty {
                            EmptyNote(text: "No bids. A tender escrows the full maximum price plus two percent, then diligence runs two weeks before the four-week offer.")
                        } else {
                            ForEach(store.deals, id: \.handle) { deal in
                                VStack(alignment: .leading) {
                                    Text("\(deal.buyer) → \(deal.target)").font(.callout)
                                    Text("\(deal.state.replacingOccurrences(of: "_", with: " ")) · offer \(Format.money(deal.offerPriceCents))/share")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                    Text("Escrow \(Format.money(deal.escrowCents)) · committed \(deal.committedShares.formatted()) shares")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
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
