import SwiftUI

/// Every screen the app can open, shared by the phone tabs, the desktop sidebar and the tutorial's
/// Show me button.
enum GamePage: String, CaseIterable, Identifiable {
    case map, office, research, stats, markets, actions, inbox, portfolio

    var id: String { rawValue }

    var title: String {
        switch self {
        case .map: return "Map"
        case .office: return "CEO Office"
        case .research: return "Research"
        case .stats: return "Stats"
        case .markets: return "Markets"
        case .actions: return "Actions"
        case .inbox: return "Inbox"
        case .portfolio: return "Portfolio"
        }
    }

    var icon: String {
        switch self {
        case .map: return "map"
        case .office: return "office"
        case .research: return "research"
        case .stats: return "stats"
        case .markets: return "markets"
        case .actions: return "command"
        case .inbox: return "inbox"
        case .portfolio: return "portfolio"
        }
    }

    var group: String {
        switch self {
        case .map: return "Play"
        case .office, .research, .stats: return "Company"
        case .markets, .actions, .inbox: return "World"
        case .portfolio: return "You"
        }
    }
}

/// One tutorial step: what to do, why it matters, where it happens and how it is recognised.
struct TutorialStep: Identifiable {
    let id: String
    let title: String
    let what: String
    let why: String
    /// The screen Show me opens.
    let page: GamePage
    /// Done when the player opens this page, sends one of these commands, or plays a week.
    var visit: GamePage?
    var commands: Set<String> = []
    var playWeek = false
}

/// The guided tutorial: eight steps that tick off when the player actually does them. Screens
/// report visits, the store reports every accepted command and every settled week.
@MainActor
final class TutorialModel: ObservableObject {
    static let steps: [TutorialStep] = [
        TutorialStep(
            id: "office", title: "Meet your company",
            what: "Open the CEO Office.",
            why: "It is your company's dashboard: cash, profit, what you make, what it costs and where you sell.",
            page: .office, visit: .office
        ),
        TutorialStep(
            id: "week", title: "Play a week",
            what: "Press Next week, or set the speed to 1x and let time run.",
            why: "Everything happens a week at a time: plants produce, buyers buy, wages and loans get paid.",
            page: .office, playWeek: true
        ),
        TutorialStep(
            id: "price", title: "Set a price",
            what: "In the pricing table, pick a product and slide its price against the market.",
            why: "Buyers take the cheapest offer first. Undercut to sell out, or price higher for margin and risk unsold stock.",
            page: .office, commands: ["SetPricePosture", "SetPrice"]
        ),
        TutorialStep(
            id: "actions", title: "Find any action",
            what: "Open the Command Center and search for something, like IPO or loan.",
            why: "Every action in the game is listed there with a plain explanation and a preview of what it costs.",
            page: .actions, visit: .actions
        ),
        TutorialStep(
            id: "research", title: "Fund research",
            what: "On the Research screen, set a modest weekly research budget.",
            why: "Each technology level makes your plants need fewer inputs and workers and lifts quality. Start small: research that costs more than the company earns is a loss.",
            page: .research, commands: ["SetResearchBudget"]
        ),
        TutorialStep(
            id: "economy", title: "Read the economy",
            what: "Open Markets and look at the economy card.",
            why: "Booms lift demand and busts cut it. The central bank's rate sets what your loans cost.",
            page: .markets, visit: .markets
        ),
        TutorialStep(
            id: "build", title: "Grow",
            what: "Pick a region on the map with room to build and build plants there.",
            why: "Room to build means the region imports what it could buy from you. New plants take a few weeks to open.",
            page: .map, commands: ["BuildFacility", "ExpandFacility"]
        ),
        TutorialStep(
            id: "stats", title: "Check the results",
            what: "Open Stats to see revenue, profit and the balance sheet over time.",
            why: "Watch whether each move paid off. Cash running low is the first thing that sinks a company.",
            page: .stats, visit: .stats
        ),
    ]

    private enum Keys {
        static let done = "cw.tutorial.done"
        static let hidden = "cw.tutorial.hidden"
    }

    @Published private(set) var done: Set<String>
    @Published var hidden: Bool
    /// Set by Show me; the shell opens the page and clears it.
    @Published var requestedPage: GamePage?
    private var startTick: UInt32?

    init() {
        let defaults = UserDefaults.standard
        done = Set(defaults.stringArray(forKey: Keys.done) ?? [])
        hidden = defaults.bool(forKey: Keys.hidden)
    }

    var current: TutorialStep? { Self.steps.first { !done.contains($0.id) } }
    var finished: Bool { current == nil }

    private func persist() {
        let defaults = UserDefaults.standard
        defaults.set(Array(done), forKey: Keys.done)
        defaults.set(hidden, forKey: Keys.hidden)
    }

    private func complete(_ step: TutorialStep) {
        guard !done.contains(step.id) else { return }
        done.insert(step.id)
        persist()
    }

    func visited(_ page: GamePage) {
        for step in Self.steps where step.visit == page { complete(step) }
    }

    func sent(_ command: String) {
        for step in Self.steps where step.commands.contains(command) { complete(step) }
    }

    func week(_ tick: UInt32) {
        guard let start = startTick else {
            startTick = tick
            return
        }
        if tick > start {
            for step in Self.steps where step.playWeek { complete(step) }
        }
    }

    /// A new career counts its weeks from its own first tick.
    func beginCareer() {
        startTick = nil
    }

    func hide() {
        hidden = true
        persist()
    }

    func restart() {
        done = []
        hidden = false
        startTick = nil
        persist()
    }
}

/// The current step: docked in a corner on iPad and Mac, a slim bar on iPhone.
struct TutorialPanel: View {
    @EnvironmentObject private var store: AppStore
    var compact: Bool

    /// A weekly research budget sized to the company: about 1 in 2,000 of its cash, in steps of
    /// 500, never under 500. Shown with its cost, the weeks to the next level and what that buys.
    private var researchHint: String? {
        guard let research = store.research, let firm = store.selectedFirmDto else { return nil }
        if research.level >= research.maxLevel { return "Your company is at the top technology level." }
        let budget = max(50_000, (firm.cashCents / 2_000 + 25_000) / 50_000 * 50_000)
        let remaining = max(research.nextThresholdCents - research.progressCents, 0)
        let weeks = (remaining + budget - 1) / budget
        return "Try \(Format.money(budget)) a week (about \(weeks) weeks to level \(research.level + 1)). "
            + "That level takes inputs and labour per unit from \(research.intensityBps / 100)% to \(research.nextIntensityBps / 100)% of baseline "
            + "and quality from \(research.qualityBps / 100) to \(research.nextQualityBps / 100)."
    }

    var body: some View {
        let tutorial = store.tutorial
        if let step = tutorial.current, !tutorial.hidden {
            let number = (TutorialModel.steps.firstIndex { $0.id == step.id } ?? 0) + 1
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Text("\(number)")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(Theme.ink)
                        .frame(width: 22, height: 22)
                        .background(Theme.gold, in: Circle())
                    Text(step.title)
                        .font(.subheadline.weight(.bold))
                        .foregroundStyle(Theme.primaryText)
                    Spacer(minLength: 4)
                    Text("\(number) of \(TutorialModel.steps.count)")
                        .font(.caption2)
                        .foregroundStyle(Theme.secondary)
                }
                Text(step.what)
                    .font(.footnote)
                    .foregroundStyle(Theme.primaryText)
                    .fixedSize(horizontal: false, vertical: true)
                if !compact {
                    Text(step.why)
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    if step.id == "research", let hint = researchHint {
                        Text(hint)
                            .font(.caption)
                            .foregroundStyle(Theme.gold)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                HStack(spacing: 6) {
                    ForEach(TutorialModel.steps) { item in
                        Circle()
                            .fill(tutorial.done.contains(item.id) ? Theme.positive : (item.id == step.id ? Theme.gold : Theme.stroke))
                            .frame(width: 7, height: 7)
                    }
                    Spacer()
                    Button("Hide") { tutorial.hide() }
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .buttonStyle(.plain)
                    Button {
                        tutorial.requestedPage = step.page
                    } label: {
                        Text("Show me")
                            .font(.footnote.weight(.bold))
                            .foregroundStyle(Theme.ink)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(Theme.gold, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, compact ? 8 : 12)
            .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: compact ? 0 : 14, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: compact ? 0 : 14, style: .continuous)
                    .stroke(Theme.gold.opacity(0.6), lineWidth: 1)
            )
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Tutorial step \(number): \(step.title)")
        }
    }
}
