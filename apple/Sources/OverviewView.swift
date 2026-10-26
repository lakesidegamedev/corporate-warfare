import SwiftUI

/// The player's own position: personal wealth and the companies it rests on, where cash is
/// heading, the one recommended move, the clock, and game settings at the end.
struct OverviewView: View {
    @EnvironmentObject private var store: AppStore
    @State private var showingBridge = false

    var body: some View {
        TabScreen(title: "Portfolio", subtitle: store.overview?.weekLabel) {
            if let overview = store.overview {
                if let pause = overview.pause {
                    DecisionBanner(text: "Paused: \(pause)")
                    if pause == "five_year_score" {
                        fiveYearScore
                    }
                }
                wealth(overview)
                if !overview.nextActionKey.isEmpty {
                    CalloutRow(symbol: "lightbulb.fill", text: overview.nextActionReason)
                }
                ForEach(overview.alerts, id: \.self) { alert in
                    CalloutRow(symbol: "exclamationmark.triangle.fill", text: alert, tint: Theme.warning)
                }
                RunControls()
                Text("Next week settles one week. Auto-advance plays a week a second and stops for any decision.")
                    .font(.caption2)
                    .foregroundStyle(Theme.secondary)
                groupPanel
                GameSettingsPanel()
            } else {
                ProgressView("Opening the corridor…")
            }
        }
        .sheet(isPresented: $showingBridge) {
            CashBridgeView()
                .environmentObject(store)
        }
    }

    /// Net worth leads; cash on both sides of the investor/company line, the quarter's profit and
    /// where company cash is heading over four weeks.
    private func wealth(_ overview: OverviewDto) -> some View {
        let outlook = overview.forecastFourWeekCents - overview.firmCashCents
        let weeks = Int(overview.quarterWeeks)
        return VStack(alignment: .leading, spacing: 8) {
            StatGroup(
                headline: Stat(label: "Net worth", value: Format.moneyCompact(overview.investorNetWorthCents),
                               note: "your holdings at fair value plus cash"),
                stats: [
                    Stat(label: "Your cash", value: Format.moneyCompact(overview.investorCashCents)),
                    Stat(label: "Company cash", value: Format.moneyCompact(overview.firmCashCents)),
                    Stat(label: weeks >= 13 ? "Profit, quarter" : "Profit, \(weeks) wk",
                         value: Format.moneyCompact(overview.quarterNetIncomeCents),
                         tone: overview.quarterNetIncomeCents >= 0 ? Theme.positive : Theme.negative),
                ]
            )
            Button {
                showingBridge = true
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: outlook >= 0 ? "arrow.up.right" : "arrow.down.right")
                        .foregroundStyle(outlook >= 0 ? Theme.positive : Theme.negative)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Company cash in four weeks: \(Format.moneyCompact(overview.forecastFourWeekCents))")
                            .font(.callout.weight(.medium))
                            .foregroundStyle(Theme.primaryText)
                        Text("\(Format.signedMoney(outlook)) after payroll and loan principal due · see the cash bridge")
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(outlook >= 0 ? Theme.secondary : Theme.negative)
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(Theme.secondary)
                }
                .padding(12)
                .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder private var fiveYearScore: some View {
        if let score = store.score {
            SectionCard(title: "Five-year score") {
                Text("\(score.total) / 100")
                    .font(.largeTitle.monospacedDigit())
                DetailRow(title: "Growth (45%)", value: "\(score.returnComponent)")
                DetailRow(title: "Expansion (25%)", value: "\(score.expansionComponent)")
                DetailRow(title: "Leadership (10%)", value: "\(score.leadershipComponent)")
                DetailRow(title: "Resilience (10%)", value: "\(score.resilienceComponent)")
                DetailRow(title: "Conduct (10%)", value: "\(score.conductComponent)")
                Text("Growth compares personal net worth with the start: 20 when flat, 100 at double. Expansion counts divisions beyond your first and states beyond your home. Leadership counts markets where your group has the most capacity. Resilience is the share of weeks without past-due liabilities. Conduct deducts unresolved regulatory breaches and failed worker settlements. Holding your starting company scores about a third.")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondary)
                if score.assisted {
                    Text("Assisted recovery: this score is labelled.")
                        .font(.caption)
                        .foregroundStyle(Theme.warning)
                }
            }
        }
        Button {
            Task { await store.continueCareer() }
        } label: {
            Label("Continue this career in sandbox", systemImage: "arrow.right.circle")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
    }

    private var groupPanel: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Eyebrow(text: "Your companies")
                if store.groupTree.isEmpty {
                    Text("You do not control a company. Found one from the map.")
                        .font(.callout)
                        .foregroundStyle(Theme.secondary)
                }
                ForEach(store.groupTree, id: \.handle) { node in
                    Button {
                        Task { await store.selectFirm(node.handle) }
                    } label: {
                        HStack(spacing: 10) {
                            if node.depth > 0 {
                                Image(systemName: "arrow.turn.down.right")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondary)
                                    .accessibilityHidden(true)
                            }
                            VStack(alignment: .leading, spacing: 1) {
                                HStack(spacing: 6) {
                                    Text(node.name).font(.callout.weight(.medium)).foregroundStyle(Theme.primaryText).lineLimit(1)
                                    if store.selectedFirm == node.handle {
                                        Image(systemName: "checkmark.circle.fill").font(.caption).foregroundStyle(Theme.positive)
                                    }
                                }
                                Text("\(node.industry) · \(node.region) · control \(Format.percentBps(node.controlBps))")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondary)
                                    .lineLimit(1)
                            }
                            Spacer(minLength: 8)
                            Text(Format.moneyCompact(node.cashCents))
                                .font(.callout.monospacedDigit())
                                .foregroundStyle(Theme.primaryText)
                        }
                        .frame(minHeight: 44)
                        .padding(.leading, CGFloat(min(Int(node.depth), 4)) * 12)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("\(node.name), \(node.directlyControlled ? "direct holding" : "subsidiary"), control \(Format.percentBps(node.controlBps)), look-through \(Format.percentBps(node.lookThroughBps))")
                    .accessibilityHint("Makes this company the selected operating context.")
                }
            }
        }
    }
}

/// Saving and analytics: game settings, not company business.
struct GameSettingsPanel: View {
    @EnvironmentObject private var store: AppStore
    @State private var analytics = Telemetry.enabled

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: "Game")
                HStack {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Save").font(.callout).foregroundStyle(Theme.primaryText)
                        Text("The game also saves itself every four weeks.").font(.caption2).foregroundStyle(Theme.secondary)
                    }
                    Spacer()
                    Button("Save now") { Task { await store.manualSave() } }
                        .font(.callout.weight(.semibold))
                        .tint(Theme.gold)
                }
                Divider().overlay(Theme.stroke)
                Toggle(isOn: $analytics) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Share anonymous market data").font(.callout).foregroundStyle(Theme.primaryText)
                        Text("A market summary every five weeks with a random id; no names, account or device data.")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary)
                    }
                }
                .tint(Theme.gold)
                .onChange(of: analytics) { _, value in Telemetry.enabled = value }
                if !store.statusLine.isEmpty {
                    Text(store.statusLine).font(.caption2).foregroundStyle(Theme.secondary)
                }
            }
        }
    }
}

struct DecisionBanner: View {
    let text: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "hand.raised.fill")
            Text(text).font(.callout.weight(.medium))
            Spacer()
        }
        .padding(12)
        .foregroundStyle(.white)
        .background(Theme.warning)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

struct CashBridgeView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if let finance = store.finance {
                    row("Opening cash", finance.bridgeOpeningCents)
                    row("Receipts", finance.bridgeReceiptsCents)
                    row("Operating payments", finance.bridgeOperatingCents)
                    row("Investment", finance.bridgeInvestmentCents)
                    row("Financing", finance.bridgeFinancingCents)
                    row("Tax", finance.bridgeTaxCents)
                    row("Closing cash", finance.bridgeClosingCents, bold: true)
                    Section("Why cash differs from profit") {
                        Text("Revenue is recognised when a sale settles. Investment in facilities moves cash into assets rather than expense, and depreciation is an expense with no cash movement.")
                            .font(.footnote)
                    }
                } else {
                    Text("Select a company to see its bridge.")
                }
            }
            .navigationTitle("Cash bridge")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }

    private func row(_ title: String, _ cents: Int64, bold: Bool = false) -> some View {
        HStack {
            Text(title).fontWeight(bold ? .semibold : .regular)
            Spacer()
            Text(Format.money(cents)).monospacedDigit().fontWeight(bold ? .semibold : .regular)
        }
    }
}
