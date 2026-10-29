import SwiftUI

/// The weekly brief: what changed, what is due and what to do, in the engine's own numbers. A
/// suggestion's Review button opens a confirmation with its command filled in and previewed;
/// nothing is sent until the player confirms there.
struct BriefPanel: View {
    @EnvironmentObject private var store: AppStore
    @AppStorage("briefCollapsed") private var collapsed = false
    let currency: String
    let firmName: String
    @State private var review: ConfirmRequest?

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    PixelIcon("flag", size: 14).foregroundStyle(Theme.gold)
                    Eyebrow(text: "This week's brief")
                    Spacer()
                    Button(collapsed ? "Show" : "Hide") {
                        collapsed.toggle()
                        store.briefOpen = !collapsed
                        if !collapsed { Task { await store.refreshBrief() } }
                    }
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                }
                if !collapsed {
                    if let brief = store.brief {
                        content(brief)
                    } else {
                        Text("Reading the week...").font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
            }
        }
        .onAppear {
            store.briefOpen = !collapsed
            if !collapsed { Task { await store.refreshBrief() } }
        }
        .onDisappear { store.briefOpen = false }
        .sheet(item: $review) { request in
            ConfirmCommandSheet(request: request).environmentObject(store)
        }
    }

    private func heading(_ text: String) -> some View {
        Text(text.uppercased()).font(.caption2.weight(.bold)).foregroundStyle(Theme.gold)
    }

    @ViewBuilder
    private func content(_ brief: BriefDto) -> some View {
        heading("What changed")
        if brief.changes.isEmpty {
            Text("A quiet week.").font(.caption).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(brief.changes.enumerated()), id: \.offset) { _, change in
            Text(change.text).font(.footnote).foregroundStyle(Theme.primaryText)
                .fixedSize(horizontal: false, vertical: true)
        }
        heading("What is due")
        if brief.due.isEmpty {
            Text("Nothing is waiting on you.").font(.caption).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(brief.due.enumerated()), id: \.offset) { _, due in
            HStack(alignment: .firstTextBaseline) {
                Text(due.text).font(.footnote).foregroundStyle(Theme.primaryText)
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 8)
                SignalPill(
                    text: due.weeksLeft == 0 ? "This week" : due.weeksLeft == 1 ? "1 week" : "\(due.weeksLeft) weeks",
                    tint: due.weeksLeft <= 1 ? Theme.warning : Theme.gold
                )
            }
        }
        heading("What to do")
        if brief.moves.isEmpty {
            Text("No move stands out. Hold your course.").font(.caption).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(brief.moves.enumerated()), id: \.offset) { _, move in
            VStack(alignment: .leading, spacing: 4) {
                Text(move.headline).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text(move.detail).font(.caption).foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                HStack {
                    Text(figures(move)).font(.caption).foregroundStyle(Theme.positive)
                    Spacer()
                    Button("Review") {
                        guard let json = move.commands.first else { return }
                        review = ConfirmRequest(
                            title: move.headline,
                            actorName: firmName,
                            detail: move.detail,
                            submitTitle: move.actionLabel == "Review" ? "Do it" : move.actionLabel,
                            json: json
                        )
                    }
                    .buttonStyle(PixelButtonStyle())
                    .tint(Theme.gold)
                }
            }
            .padding(10)
            .background(Theme.panelRaised, in: Pixel.shape)
        }
    }

    private func figures(_ move: BriefMoveDto) -> String {
        var parts: [String] = []
        if move.weeklyGainCents > 0 {
            parts.append("About \(Format.moneyCompact(move.weeklyGainCents, currency: currency)) a week")
        }
        if move.costCents > 0 { parts.append("costs \(Format.moneyCompact(move.costCents, currency: currency))") }
        if move.hasPayback && move.costCents > 0 && move.paybackWeeks > 0 {
            parts.append("pays back in about \(move.paybackWeeks) weeks")
        }
        return parts.joined(separator: ", ")
    }
}
