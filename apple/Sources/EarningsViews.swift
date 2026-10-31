import SwiftUI

/// Earnings season on a company page and the stock sheet: the analyst's estimate, the last
/// results against it, the rating and guidance. Mirrors `Earnings.kt` on the other platforms.
enum EarningsText {
    static func rating(_ key: String) -> String {
        switch key {
        case "buy": return "Buy"
        case "sell": return "Sell"
        default: return "Hold"
        }
    }

    static func ratingTint(_ key: String) -> Color {
        switch key {
        case "buy": return Theme.positive
        case "sell": return Theme.negative
        default: return Theme.secondary
        }
    }

    /// "Reports this week" or "Reports in 5 weeks"; nil when no estimate is out.
    static func resultsWhen(_ e: EarningsDto) -> String? {
        guard e.covered, e.hasEstimate else { return nil }
        return e.weeksToResults <= 1 ? "Reports this week" : "Reports in \(e.weeksToResults) weeks"
    }

    static func credibility(_ bps: Int32) -> String {
        bps >= 7_500 ? "High" : bps >= 4_000 ? "Fair" : "Low"
    }
}

struct EarningsSection: View {
    let earnings: EarningsDto
    var onGiveGuidance: (() -> Void)?

    var body: some View {
        if earnings.covered {
            VStack(alignment: .leading, spacing: 4) {
                Eyebrow(text: "Earnings")
                HStack(spacing: 8) {
                    SignalPill(text: "Analysts: \(EarningsText.rating(earnings.rating))",
                               tint: EarningsText.ratingTint(earnings.rating))
                    if let when = EarningsText.resultsWhen(earnings) {
                        SignalPill(text: when, tint: Theme.gold)
                    }
                }
                if earnings.hasEstimate {
                    Text("Analysts expect \(Format.money(earnings.estimateCents)) in operating profit this quarter.")
                        .font(.caption).foregroundStyle(Theme.primaryText)
                } else {
                    Text("No estimate yet. The analyst writes one at the next quarter's close.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
                if earnings.hasGuidance {
                    Text("Guidance: \(Format.money(earnings.guideLowCents)) to \(Format.money(earnings.guideHighCents)). Meet the low end on results day and the market trusts you more.")
                        .font(.caption).foregroundStyle(Theme.gold)
                }
                if earnings.hasLast {
                    let beat = earnings.lastActualCents >= earnings.lastEstimateCents
                    Text("Last results (week \(earnings.lastWeek)): \(Format.money(earnings.lastActualCents)) against \(Format.money(earnings.lastEstimateCents)) expected, \(beat ? "a beat" : "a miss"). The share price moved \(Change.text(Int64(earnings.lastMoveBps))).")
                        .font(.caption)
                        .foregroundStyle(beat ? Theme.positive : Theme.negative)
                    if let met = earnings.lastGuidanceMet {
                        Text(met ? "Management met its guidance." : "Management missed its guidance.")
                            .font(.caption)
                            .foregroundStyle(met ? Theme.positive : Theme.negative)
                    }
                    if abs(earnings.streak) >= 2 {
                        Text("\(abs(earnings.streak)) quarters in a row \(earnings.streak > 0 ? "above" : "below") the estimate.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
                Text("Trust in management's numbers: \(EarningsText.credibility(earnings.credibilityBps))")
                    .font(.caption).foregroundStyle(Theme.secondary)
                if let onGiveGuidance, earnings.canGuide {
                    Button("Give guidance", action: onGiveGuidance).buttonStyle(PixelButtonStyle())
                }
            }
        }
    }
}

/// Tell the analysts what to expect: a low and a high end, prefilled around the estimate.
struct GuidanceSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let firm: UInt64
    let earnings: EarningsDto
    @State private var low = ""
    @State private var high = ""
    @State private var message: String?

    private var lowCents: Int64? { Double(low).map { Int64(($0 * 100).rounded()) } }
    private var highCents: Int64? { Double(high).map { Int64(($0 * 100).rounded()) } }
    private var valid: Bool {
        if let l = lowCents, let h = highCents { return l <= h }
        return false
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text("Analysts expect \(Format.money(earnings.estimateCents)) in operating profit this quarter. Tell them a range and they move their number toward the middle of it.")
                    Text("The risk: finish under the low end and the company loses reputation, the market trusts you less, and activist investors take notice. Finish above it and the market trusts you more, so a later miss costs less. A low end well under what the analysts expect is seen as sandbagging and earns no trust.")
                        .foregroundStyle(Theme.warning)
                }
                Section {
                    TextField("Low end of operating profit ($)", text: $low).keyboardType(.numbersAndPunctuation)
                    TextField("High end of operating profit ($)", text: $high).keyboardType(.numbersAndPunctuation)
                }
                if let message { Text(message).foregroundStyle(Theme.negative) }
            }
            .navigationTitle("Give guidance")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Give guidance") {
                        guard let l = lowCents, let h = highCents else { return }
                        Task {
                            if let rejection = await store.giveGuidance(firm: firm, lowCents: l, highCents: h) {
                                message = rejection.message
                            } else {
                                dismiss()
                            }
                        }
                    }
                    .disabled(!valid)
                }
            }
            .onAppear {
                let room = abs(earnings.estimateCents) / 20
                low = String(Int64((Double(earnings.estimateCents - room) / 100).rounded()))
                high = String(Int64((Double(earnings.estimateCents + room) / 100).rounded()))
            }
        }
    }
}
