import SwiftUI

/// Board goals, chapters and the Hall of Careers (1.9). The engine owns the state; these views
/// read `AppStore.hall` and send `ChooseMandate` through the usual submit path.

/// What a met or missed goal does, from content/mandates.json (the engine's numbers, in words).
private let rewardLine = "A management bonus of 2% of a year's operating profit (at least $50K) and +1.5 reputation."
private let riskLine = "Missing it costs 1.0 reputation. Two misses in a row can bring an activist campaign."
private let cashCoverRiskLine = "One week under the line loses it. Missing costs 1.0 reputation, and two misses in a row can bring an activist campaign."

private func numberWord(_ n: Int) -> String {
    switch n {
    case 1: return "one"
    case 2: return "two"
    case 3: return "three"
    default: return "\(n)"
    }
}

/// A thin stepped bar: twenty blocks, the filled ones in the tint.
struct PixelBar: View {
    let fraction: Double
    var tint: Color = Theme.gold

    var body: some View {
        let lit = Int(min(max(fraction, 0), 1) * 20 + 0.001)
        HStack(spacing: 2) {
            ForEach(0..<20, id: \.self) { index in
                Rectangle().fill(index < lit ? tint : Theme.stroke).frame(height: 8)
            }
        }
        .accessibilityHidden(true)
    }
}

private func bonusText(_ chapter: ChapterDto) -> String {
    let rep = Double(chapter.reputationBps) / 100
    let repText = String(format: "%@%.1f reputation", rep >= 0 ? "+" : "", rep)
    return chapter.bonusCents > 0 ? "Paid a bonus of \(Format.moneyCompact(chapter.bonusCents)) and \(repText)." : String(format: "Cost %.1f reputation.", abs(rep))
}

/// The board's offer: the chapter, the goals, and a Choose button on each. Not a pause.
struct BoardOfferCard: View {
    @EnvironmentObject private var store: AppStore
    let chapter: ChapterDto
    @State private var rejection: CommandRejection?

    var body: some View {
        let weeks = Int(chapter.offerWeeksLeft)
        Panel {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .top, spacing: 8) {
                    PixelIcon("flag").foregroundStyle(Theme.gold)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(chapter.name).font(Pixel.font(16, bold: true)).foregroundStyle(Theme.primaryText)
                        Text("The board offers \(numberWord(chapter.offered.count)) \(chapter.offered.count == 1 ? "goal" : "goals") for this year. Pick one, or leave it and nothing is lost.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                    Spacer(minLength: 4)
                    SignalPill(text: weeks == 1 ? "Decide within 1 week" : "Decide within \(weeks) weeks", tint: weeks <= 1 ? Theme.warning : Theme.gold)
                }
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 10) {
                        ForEach(chapter.offered, id: \.index) { choice($0) }
                    }
                    .frame(minWidth: 640)
                    VStack(spacing: 10) {
                        ForEach(chapter.offered, id: \.index) { choice($0) }
                    }
                }
                if let rejection {
                    Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                }
            }
        }
    }

    private func line(_ label: String, _ text: String, _ tint: Color) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(Pixel.font(10)).tracking(0.8).foregroundStyle(tint)
            Text(text).font(.caption).foregroundStyle(Theme.primaryText.opacity(0.9)).fixedSize(horizontal: false, vertical: true)
        }
    }

    private func choice(_ mandate: MandateDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(mandate.title).font(.subheadline.weight(.bold)).foregroundStyle(Theme.primaryText)
            line("MEASURED", mandate.detail, Theme.secondary)
            line("REWARD", rewardLine, Theme.positive)
            line("RISK", mandate.key == "cash_cover" ? cashCoverRiskLine : riskLine, Theme.warning)
            Spacer(minLength: 2)
            Button("Choose") {
                Task { rejection = await store.chooseMandate(firm: chapter.firm, pick: Int(mandate.index)) }
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .tint(Theme.gold)
            .disabled(store.isBusy)
            .frame(maxWidth: .infinity)
        }
        .padding(12)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.panelRaised, in: Pixel.shape)
        .accessibilityElement(children: .contain)
    }
}

/// The goal in force: what it is, a progress bar, the number and a sentence, weeks left.
struct MandateStrip: View {
    @EnvironmentObject private var store: AppStore
    let chapter: ChapterDto
    let mandate: MandateDto

    var body: some View {
        let now = store.overview?.tick ?? chapter.startWeek
        let left = max(Int(chapter.endWeek) - Int(now), 0)
        let percent = min(max(Int(mandate.progressBps) / 100, 0), 100)
        let tint = mandate.lost ? Theme.negative : (mandate.progressBps >= 10_000 ? Theme.positive : Theme.gold)
        let chapterWord = chapter.name.components(separatedBy: ":").first ?? "the chapter"
        Panel {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    PixelIcon("target").foregroundStyle(Theme.gold)
                    VStack(alignment: .leading, spacing: 1) {
                        Eyebrow(text: "Board goal")
                        Text(mandate.title).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    }
                    Spacer()
                    Text(mandate.lost ? "LOST" : "\(percent)%")
                        .font(Pixel.font(18, bold: true)).foregroundStyle(tint)
                }
                PixelBar(fraction: Double(mandate.progressBps) / 10_000, tint: tint)
                Text(mandate.progressText).font(.caption).foregroundStyle(Theme.secondary)
                Text(left == 1 ? "1 week left in \(chapterWord)" : "\(left) weeks left in \(chapterWord)")
                    .font(.caption2).foregroundStyle(Theme.secondary)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Board goal: \(mandate.title), \(percent) percent, \(left) weeks left")
    }
}

/// Shown once when a chapter closes with a goal met or missed.
struct ChapterResultPanel: View {
    let chapter: ChapterDto
    var onDismiss: () -> Void = {}

    var body: some View {
        let met = chapter.outcome == "met"
        let goal = chapter.chosen.flatMap { chapter.offered.indices.contains(Int($0)) ? chapter.offered[Int($0)] : nil }
        Panel {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    PixelIcon(met ? "check_circle" : "close").foregroundStyle(met ? Theme.positive : Theme.negative)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(chapter.name).font(Pixel.font(15, bold: true)).foregroundStyle(Theme.primaryText)
                        Text(met ? "The board's goal was met." : "The board's goal was missed.")
                            .font(.callout.weight(.semibold)).foregroundStyle(met ? Theme.positive : Theme.negative)
                    }
                    Spacer()
                    Button("Close", action: onDismiss).buttonStyle(PixelButtonStyle()).tint(Theme.gold)
                }
                if let goal { Text(goal.title).font(.callout).foregroundStyle(Theme.primaryText) }
                Text(bonusText(chapter)).font(.caption).foregroundStyle(Theme.secondary)
            }
        }
    }
}

/// The offer, the goal in force, or a chapter result, whichever applies, for the top of the CEO Office.
struct ChapterBlock: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let result = store.chapterResult {
                ChapterResultPanel(chapter: result) { store.dismissChapterResult() }
            }
            if let offer = store.boardOffer {
                BoardOfferCard(chapter: offer)
            }
            if let active = store.activeMandate {
                MandateStrip(chapter: active.chapter, mandate: active.mandate)
            }
        }
    }
}

private func badgeIcon(_ id: String) -> String {
    switch id {
    case "year_one": return "calendar"
    case "first_mandate": return "check_circle"
    case "five_mandates": return "star"
    case "three_in_a_row": return "trend_up"
    case "first_raid": return "target"
    case "first_listing": return "markets"
    case "subsidiary_listed": return "branch"
    case "first_merger": return "swap"
    case "ten_companies": return "briefcase"
    case "three_states": return "globe"
    case "five_divisions": return "stack"
    case "market_leader": return "flag"
    case "thousand_workers": return "people"
    case "net_worth_double": return "coin"
    case "net_worth_tenfold": return "cash"
    case "debt_free": return "bank"
    case "hostile_survived": return "umbrella"
    case "bank_rescued": return "hand"
    case "all_events": return "history"
    case "thirty_years": return "sparkle"
    default: return "star"
    }
}

/// An 8-bit badge: the pixel icon in a stepped frame. Locked badges are dim and grey.
struct AchievementBadge: View {
    let item: AchievementDto
    var size: CGFloat = 52

    var body: some View {
        PixelIcon(badgeIcon(item.id), size: size * 0.6, colored: item.earned)
            .foregroundStyle(Theme.secondary)
            .frame(width: size, height: size)
            .pixelFrame(item.earned ? Theme.panelRaised : Theme.panel, outline: item.earned ? Theme.gold : Theme.stroke)
            .opacity(item.earned ? 1 : 0.35)
            .padding(.bottom, 3)
    }
}

private func outcomeLabel(_ outcome: String) -> String {
    switch outcome {
    case "met": return "Met"
    case "missed": return "Missed"
    case "lapsed": return "No goal chosen"
    case "cancelled": return "Cancelled"
    default: return "In progress"
    }
}

private func outcomeTint(_ outcome: String) -> Color {
    switch outcome {
    case "met": return Theme.positive
    case "missed": return Theme.negative
    case "open": return Theme.gold
    default: return Theme.secondary
    }
}

/// One finished or left career, kept in app settings for the main menu's list.
struct CareerSummary: Codable, Equatable, Identifiable {
    var title: String
    var scenario: String
    var weeks: Int
    var chapters: Int
    var met: Int
    var missed: Int
    var achievements: Int
    var id: String { "\(scenario)|\(title)" }

    init(title: String, scenario: String, hall: HallDto) {
        self.title = title.isEmpty ? "Untitled career" : title
        self.scenario = scenario
        weeks = Int(hall.weeks)
        chapters = hall.chapters.filter { $0.status == "closed" }.count
        met = Int(hall.mandatesMet)
        missed = Int(hall.mandatesMissed)
        achievements = Int(hall.achievementsEarned)
    }

    static let key = "careerSummaries"
    static let kept = 12

    static func load() -> [CareerSummary] {
        guard let data = UserDefaults.standard.data(forKey: key),
              let list = try? JSONDecoder().decode([CareerSummary].self, from: data) else { return [] }
        return list
    }

    /// Keep a career's summary; leaving a career again replaces its earlier line.
    static func record(_ summary: CareerSummary) {
        let list = ([summary] + load().filter { $0.id != summary.id }).prefix(kept)
        if let data = try? JSONEncoder().encode(Array(list)) { UserDefaults.standard.set(data, forKey: key) }
    }
}

struct CareerLine: View {
    let career: CareerSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(career.title).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText).lineLimit(1)
            Text("\(career.scenario) · \(career.weeks / 52) years").font(.caption2).foregroundStyle(Theme.secondary)
            Text("\(career.chapters) \(career.chapters == 1 ? "chapter" : "chapters") · \(career.met) met, \(career.missed) missed · \(career.achievements) of 20 achievements")
                .font(.caption).foregroundStyle(Theme.primaryText.opacity(0.9))
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panelRaised, in: Pixel.shape)
        .accessibilityElement(children: .combine)
    }
}

/// The Hall of Careers for the open career: chapters in order, achievements as badges, the
/// five-year score, then earlier careers.
struct HallView: View {
    @EnvironmentObject private var store: AppStore
    @State private var earlier = CareerSummary.load()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Hall of Careers").font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                if let hall = store.hall {
                    Panel {
                        VStack(alignment: .leading, spacing: 6) {
                            Eyebrow(text: "This career")
                            Text("\(hall.mandatesMet) goals met, \(hall.mandatesMissed) missed · \(hall.achievementsEarned) of \(hall.achievements.count) achievements")
                                .font(.callout).foregroundStyle(Theme.primaryText)
                            if let score = store.score {
                                Text("Five-year score: \(score.total)").font(Pixel.font(16, bold: true)).foregroundStyle(Theme.gold)
                                Text("Return \(score.returnComponent) · Expansion \(score.expansionComponent) · Leadership \(score.leadershipComponent) · Resilience \(score.resilienceComponent)")
                                    .font(.caption2).foregroundStyle(Theme.secondary)
                            }
                        }
                    }
                    Eyebrow(text: "Chapters")
                    if hall.chapters.isEmpty {
                        CalloutRow(icon: "flag", text: "The first chapter opens in week 1.", tint: Theme.secondary)
                    }
                    ForEach(hall.chapters, id: \.number) { chapterRow($0) }
                    Eyebrow(text: "Achievements")
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), alignment: .topLeading)], alignment: .leading, spacing: 10) {
                        ForEach(hall.achievements.sorted { $0.earned && !$1.earned }, id: \.id) { achievement($0) }
                    }
                } else {
                    CalloutRow(icon: "flag", text: "The record opens once the career is running.", tint: Theme.secondary)
                }
                if !earlier.isEmpty {
                    Eyebrow(text: "Earlier careers")
                    ForEach(earlier) { CareerLine(career: $0) }
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .task {
            store.hallPageOpen = true
            await store.refreshHall()
        }
        .onDisappear { store.hallPageOpen = false }
        .onChange(of: store.overview?.tick) { _, _ in Task { await store.refreshHall() } }
    }

    private func chapterRow(_ chapter: ChapterDto) -> some View {
        let goal = chapter.chosen.flatMap { chapter.offered.indices.contains(Int($0)) ? chapter.offered[Int($0)] : nil }
        let year = ResultBuckets.startYear + (Int(chapter.startWeek) - 1) / 52
        return VStack(alignment: .leading, spacing: 3) {
            HStack {
                VStack(alignment: .leading, spacing: 1) {
                    Text(chapter.name).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text("\(year) · weeks \(chapter.startWeek) to \(chapter.endWeek)").font(.caption2).foregroundStyle(Theme.secondary)
                }
                Spacer()
                SignalPill(text: outcomeLabel(chapter.outcome), tint: outcomeTint(chapter.outcome))
            }
            Text(goal?.title ?? (chapter.offered.isEmpty ? "The board made no offer." : "No goal was chosen."))
                .font(.caption).foregroundStyle(Theme.primaryText.opacity(0.9))
            if chapter.outcome == "met" || chapter.outcome == "missed" {
                Text(bonusText(chapter)).font(.caption2).foregroundStyle(Theme.secondary)
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panelRaised, in: Pixel.shape)
        .accessibilityElement(children: .combine)
    }

    private func achievement(_ item: AchievementDto) -> some View {
        HStack(spacing: 10) {
            AchievementBadge(item: item)
            VStack(alignment: .leading, spacing: 1) {
                Text(item.title).font(.footnote.weight(.semibold))
                    .foregroundStyle(item.earned ? Theme.primaryText : Theme.secondary.opacity(0.7))
                Text(item.earned ? "Earned \(ResultBuckets.weekLabel(item.week))" : item.detail)
                    .font(.caption2).foregroundStyle(Theme.secondary.opacity(item.earned ? 1 : 0.6))
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(item.earned ? "\(item.title), earned" : "\(item.title), locked")
    }
}

/// The main menu's list of careers across games.
struct CareerListView: View {
    @Environment(\.dismiss) private var dismiss
    private let careers = CareerSummary.load()

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    if careers.isEmpty {
                        CalloutRow(icon: "flag", text: "No finished careers yet. Leave a career from its menu and it is kept here.", tint: Theme.secondary)
                    }
                    ForEach(careers) { CareerLine(career: $0) }
                }
                .padding(16)
            }
            .background(Theme.ink)
            .navigationTitle("Hall of Careers")
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}
