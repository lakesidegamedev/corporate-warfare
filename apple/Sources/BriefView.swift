import SwiftUI

/// The weekly brief: what changed, what is due and what to do, in the engine's own numbers. A
/// suggestion's Review button opens the game's own sheet for that move (build flow, raid panel,
/// lane reservation, pricing posture, mandate offer, division detail) filled in, or a plain
/// confirmation with the fix ready; nothing is sent until the player confirms there.

/// A move the player pressed Review on.
struct BriefReviewItem: Identifiable {
    let id = UUID()
    let move: BriefMoveDto
}

struct BriefPanel: View {
    @Environment(AppStore.self) private var store
    @AppStorage("briefCollapsed") private var collapsed = false
    let currency: String
    let firmName: String
    @State private var review: BriefReviewItem?

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
        .sheet(item: $review) { item in
            reviewSheet(item.move).environment(store)
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
            Text(brief.hold.isEmpty ? "No move stands out. Hold your course." : "Hold your course. \(brief.hold)")
                .font(.caption).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(brief.moves.enumerated()), id: \.offset) { _, move in
            VStack(alignment: .leading, spacing: 4) {
                Text(move.headline).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text(move.detail).font(.caption).foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                HStack {
                    Text(figures(move)).font(.caption).foregroundStyle(Theme.positive)
                    Spacer()
                    Button("Review") { review = BriefReviewItem(move: move) }
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
            parts.append("pays back in about \(Self.paybackText(move.paybackWeeks))")
        }
        return parts.joined(separator: ", ")
    }

    /// Weeks under a year, years to one decimal after.
    static func paybackText(_ weeks: Int64) -> String {
        weeks < 52 ? "\(weeks) weeks" : "\(weeks * 10 / 52 / 10).\(weeks * 10 / 52 % 10) years"
    }

    /// The posture the move would set, read from its first command.
    private func presetBps(_ move: BriefMoveDto) -> Int32? {
        guard let json = move.commands.first,
              let range = json.range(of: "\"bps\":-?[0-9]+", options: .regularExpression) else { return nil }
        return Int32(json[range].dropFirst(6))
    }

    private func confirmation(_ move: BriefMoveDto) -> some View {
        Group {
            if let json = move.commands.first {
                ConfirmCommandSheet(request: ConfirmRequest(
                    title: move.headline,
                    actorName: firmName,
                    detail: move.detail,
                    submitTitle: move.actionLabel == "Review" ? "Do it" : move.actionLabel,
                    json: json
                ))
            } else {
                briefNote(move)
            }
        }
    }

    /// A move with no command behind it, such as a company losing money with no single cause:
    /// the plain reading of the income statement.
    private func briefNote(_ move: BriefMoveDto) -> some View {
        NavigationStack {
            ScrollView {
                Text(move.detail).font(.callout).foregroundStyle(Theme.primaryText)
                    .fixedSize(horizontal: false, vertical: true).padding(16)
            }
            .background(Theme.panel)
            .navigationTitle(move.headline)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { review = nil } } }
        }
    }

    private func card<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        NavigationStack {
            ScrollView { content().padding(16) }
                .background(Theme.panel)
                .navigationTitle(title)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { review = nil } } }
        }
    }

    /// Each move opens the sheet the rest of the game already uses for it.
    @ViewBuilder
    private func reviewSheet(_ move: BriefMoveDto) -> some View {
        let t = move.target
        switch t.dialog {
        case "build":
            BuildSheet(
                region: store.regions.first { $0.code == t.region }?.index,
                recipe: t.recipe,
                blocks: t.blocks
            )
        case "pricing":
            if let line = store.productLines.first(where: { $0.product == t.product }) {
                PostureSheet(line: line, presetBps: presetBps(move))
            } else {
                confirmation(move)
            }
        case "lane":
            if let lane = store.lanes.first(where: { $0.a == t.region && $0.b == t.regionTo && $0.mode == t.mode }) {
                card(move.headline) {
                    VStack(alignment: .leading, spacing: 10) {
                        Text(move.detail).font(.caption).foregroundStyle(Theme.secondary)
                        LaneDetail(lane: lane, initialLots: Double(t.lots), initialWeeks: Double(t.weeks))
                    }
                }
            } else {
                confirmation(move)
            }
        case "mandate":
            card("The board's offer") {
                if let offer = store.boardOffer {
                    BoardOfferCard(chapter: offer)
                } else {
                    Text("The offer has lapsed or been taken.").font(.callout).foregroundStyle(Theme.secondary)
                }
            }
        case "raid":
            RaidPanel(target: t.target, region: t.region, industry: t.industry)
        case "division":
            if let division = store.divisions.first(where: { $0.region == t.region && $0.industry == t.industry }) {
                DivisionDetailSheet(division: division)
            } else {
                confirmation(move)
            }
        default:
            confirmation(move)
        }
    }
}
