import SwiftUI

struct OverviewView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @State private var showingBridge = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    ContextChip()

                    if let overview = store.overview {
                        if let pause = overview.pause {
                            DecisionBanner(text: "Paused: \(pause)")
                        }

                        HStack(spacing: 12) {
                            MetricTile(
                                title: "Investor cash",
                                value: Format.moneyCompact(overview.investorCashCents),
                                note: "personal, unpledged"
                            )
                            MetricTile(
                                title: "Net worth",
                                value: Format.moneyCompact(overview.investorNetWorthCents),
                                note: "holdings at fair value"
                            )
                        }
                        HStack(spacing: 12) {
                            MetricTile(
                                title: "Company cash",
                                value: Format.moneyCompact(overview.firmCashCents),
                                note: overview.primaryName
                            )
                            MetricTile(
                                title: "Last week EBIT",
                                value: Format.signedMoney(overview.lastWeekEbitCents),
                                tone: overview.lastWeekEbitCents >= 0 ? 1 : -1
                            )
                        }

                        RunControls()

                        SectionCard(title: "Four-week cash forecast") {
                            Text(Format.money(overview.forecastFourWeekCents))
                                .font(.title2.monospacedDigit())
                            Text("Cash plus expected receipts minus four weeks of mandatory payroll and scheduled principal.")
                                .font(.footnote)
                                .foregroundStyle(Theme.secondaryText(for: scheme))
                            Button("Open cash bridge") { showingBridge = true }
                        }

                        if !overview.alerts.isEmpty {
                            SectionCard(title: "Needs attention") {
                                ForEach(overview.alerts, id: \.self) { alert in
                                    Label(alert, systemImage: "exclamationmark.triangle")
                                        .font(.callout)
                                        .foregroundStyle(Theme.warning)
                                }
                            }
                        }

                        SectionCard(title: "Your group") {
                            if store.controlledFirms.isEmpty {
                                EmptyNote(text: "You do not control a company. Open Company to review the portfolio or start again from the management contract when eligible.")
                            } else {
                                ForEach(store.controlledFirms) { firm in
                                    Button {
                                        Task { await store.selectFirm(firm.handle) }
                                    } label: {
                                        HStack {
                                            VStack(alignment: .leading) {
                                                Text(firm.name).font(.body.weight(.medium))
                                                Text("\(firm.industry) · \(firm.region) · grade \(firm.grade)")
                                                    .font(.caption)
                                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                                            }
                                            Spacer()
                                            Text(Format.moneyCompact(firm.cashCents))
                                                .font(.callout.monospacedDigit())
                                        }
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }

                        if let status = store.statusLine.isEmpty ? nil : store.statusLine {
                            Text(status)
                                .font(.caption)
                                .foregroundStyle(Theme.secondaryText(for: scheme))
                        }
                    } else {
                        ProgressView("Opening the corridor…")
                    }
                }
                .padding(16)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Portfolio")
            .sheet(isPresented: $showingBridge) {
                CashBridgeView()
                    .environmentObject(store)
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
