import SwiftUI

// Careers (2.0): the Career section, the full career page and the end of career screen. The
// engine owns the state; these views only show what career_standing and career_summary say.

/// Plain words for career figures. Pure, so the tests can check them without a screen.
enum CareerText {
    static func ambition(_ key: String?) -> String {
        guard let key, !key.isEmpty else { return "No ambition" }
        switch key {
        case "richest": return "Richest"
        case "biggest": return "Biggest"
        case "dominant": return "Dominant"
        case "survivor": return "Survivor"
        default: return key.prefix(1).uppercased() + key.dropFirst()
        }
    }

    static func medal(_ key: String) -> String {
        switch key {
        case "bronze": return "Bronze"
        case "silver": return "Silver"
        case "gold": return "Gold"
        default: return "None yet"
        }
    }

    /// Net worth over the starting fortune: 10000 basis points is no growth, 23000 is 2.3 times.
    static func growth(_ bps: Int64) -> String {
        String(format: "%.1f times", Double(bps) / 10_000.0)
    }

    static func ending(_ s: CareerSummaryDto) -> String {
        switch s.ending {
        case "won": return "You won"
        case "completed": return "Career complete"
        case "no_company": return "No company left"
        case "removed": return "Removed by the board"
        case "taken_over": return "Taken over"
        default: return s.lost ? "Career lost" : "Career complete"
        }
    }

    static func start(_ s: CareerSummaryDto) -> String {
        if s.start == "helm" {
            if let company = s.helmCompany {
                return "Took the helm of \(company)." + (s.helmFortuneScaled ? " The starting fortune was scaled up to pay for control." : "")
            }
            return "Took the helm of an established company."
        }
        return "Started as a founder."
    }

    /// "3 years 12 weeks", "9 weeks" or "none".
    static func timeLeft(weeks: UInt32) -> String {
        if weeks == 0 { return "none" }
        let years = weeks / 52
        let rest = weeks % 52
        var parts: [String] = []
        if years > 0 { parts.append("\(years) year\(years == 1 ? "" : "s")") }
        if rest > 0 { parts.append("\(rest) week\(rest == 1 ? "" : "s")") }
        return parts.joined(separator: " ")
    }

    /// The offered company's fortune line, saying plainly when it was scaled up.
    static func fortuneNote(_ o: HelmOfferDto) -> String {
        if o.fortuneCents == o.standardFortuneCents {
            return "Your fortune after buying control: \(Format.moneyCompact(o.fortuneCents)), the scenario's own."
        }
        return "This start gives you \(Format.moneyCompact(o.fortuneCents)) instead of the scenario's usual \(Format.moneyCompact(o.standardFortuneCents)), because the usual fortune cannot pay for control of this company. It is scaled up to cover the price."
    }
}

func medalTint(_ key: String) -> Color {
    switch key {
    case "gold": return Theme.gold
    case "silver": return Color(red: 0.765, green: 0.800, blue: 0.839)
    case "bronze": return Color(red: 0.722, green: 0.451, blue: 0.235)
    default: return Theme.secondary
    }
}

/// A medal drawn in hard-edged blocks: two ribbon ends over a square disc with a square inside.
struct MedalArt: View {
    let medal: String

    var body: some View {
        let disc = medalTint(medal)
        let u: CGFloat = 6
        ZStack(alignment: .topLeading) {
            Rectangle().fill(Color(red: 0.722, green: 0.251, blue: 0.243)).frame(width: 3 * u, height: 5 * u).offset(x: 2 * u, y: 0)
            Rectangle().fill(Color(red: 0.243, green: 0.435, blue: 0.722)).frame(width: 3 * u, height: 5 * u).offset(x: 7 * u, y: 0)
            Rectangle().fill(Theme.ink).frame(width: 12 * u, height: 12 * u).offset(x: 0, y: 4 * u)
            Rectangle().fill(disc).frame(width: 10 * u, height: 10 * u).offset(x: u, y: 5 * u)
            Rectangle().fill(disc.opacity(0.55)).frame(width: 6 * u, height: 6 * u).offset(x: 3 * u, y: 7 * u)
            Rectangle().fill(disc).frame(width: 4 * u, height: 4 * u).offset(x: 4 * u, y: 8 * u)
        }
        .frame(width: 12 * u, height: 16 * u, alignment: .topLeading)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(medal == "none" || medal.isEmpty ? "No medal" : "\(CareerText.medal(medal)) medal")
    }
}

// MARK: - Standing

/// Where the career stands: ambition, medal, rank, what the next medal needs and the board's warning.
struct CareerStandingPanel: View {
    let standing: CareerStandingDto

    var body: some View {
        Panel {
            VStack(alignment: .leading, spacing: 8) {
                HStack(alignment: .firstTextBaseline) {
                    Eyebrow(text: "Career")
                    Spacer(minLength: 6)
                    if standing.ended {
                        SignalPill(text: "ENDED", tint: Theme.warning)
                    } else if standing.medal != "none" {
                        SignalPill(text: CareerText.medal(standing.medal).uppercased(), tint: medalTint(standing.medal))
                    }
                }
                Text(CareerText.ambition(standing.ambition))
                    .font(Pixel.font(16, bold: true))
                    .foregroundStyle(Theme.gold)
                DetailRow(title: "Medal now", value: CareerText.medal(standing.medal))
                DetailRow(title: "Company rank", value: "#\(standing.rank) of \(standing.league)")
                DetailRow(title: "Net worth vs start", value: CareerText.growth(standing.growthBps), tone: standing.growthBps >= 10_000 ? 1 : -1)
                DetailRow(title: "Time left", value: CareerText.timeLeft(weeks: standing.weeksLeft))
                if !standing.detail.isEmpty {
                    Text(standing.detail).font(.footnote).foregroundStyle(Theme.primaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if !standing.next.isEmpty {
                    Text("Next medal: \(standing.next)").font(.footnote).foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if standing.boardWarned {
                    CalloutRow(icon: "blocked", text: standing.boardWarning.isEmpty ? "The board has warned you." : standing.boardWarning, tint: Theme.warning)
                    if !standing.boardCanRemove {
                        Text("You hold enough of the votes that the board cannot remove you.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
            }
        }
    }
}

/// The Career section of the CEO office: the standing, and the way into the full page.
struct CareerSection: View {
    @Environment(AppStore.self) private var store
    @State private var standing: CareerStandingDto?

    var body: some View {
        Group {
            if let standing {
                VStack(spacing: 8) {
                    CareerStandingPanel(standing: standing)
                    NavigationLink {
                        CareerView().environment(store)
                    } label: {
                        HStack {
                            PixelLabel("Career: year by year and the moments that mattered", icon: "star")
                                .font(.callout.weight(.semibold))
                            Spacer()
                            PixelIcon("chevron_right", size: 12)
                        }
                        .padding(12)
                        .background(Theme.panel, in: Pixel.shape)
                    }
                    .tint(Theme.gold)
                }
            }
        }
        .task(id: store.historyRefreshKey) {
            if let loaded = await store.read({ $0.careerStanding() }) { standing = loaded } else { standing = nil }
        }
    }
}

/// The full career page: standing, the year by year record and the moments so far.
struct CareerView: View {
    @Environment(AppStore.self) private var store
    @State private var standing: CareerStandingDto?
    @State private var summary: CareerSummaryDto?
    @State private var loaded = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Career").font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                if let standing {
                    CareerStandingPanel(standing: standing)
                    if standing.ended {
                        Button {
                            store.careerEndRequested = true
                        } label: {
                            PixelLabel("See the end of career screen", icon: "star")
                                .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                        }
                        .buttonStyle(PixelButtonStyle(prominent: true))
                        .tint(Theme.gold)
                        .foregroundStyle(Theme.ink)
                    }
                } else if loaded {
                    CalloutRow(icon: "star", text: "This career has no ambition, so there is no goal, medal or ending. Start a new game and pick an ambition to play for one.", tint: Theme.secondary)
                }
                if let summary {
                    Text(CareerText.start(summary)).font(.footnote).foregroundStyle(Theme.secondary)
                    CareerYearsPanel(years: summary.yearsLog)
                    CareerMomentsPanel(events: summary.events)
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .task(id: store.historyRefreshKey) {
            if let s = await store.read({ $0.careerStanding() }) { standing = s } else { standing = nil }
            if let s = await store.read({ $0.careerSummary() }) { summary = s } else { summary = nil }
            loaded = true
        }
    }
}

struct CareerYearsPanel: View {
    let years: [CareerYearDto]

    var body: some View {
        Panel {
            VStack(alignment: .leading, spacing: 6) {
                Eyebrow(text: "Year by year")
                if years.isEmpty {
                    EmptyNote(text: "The first year closes after 52 weeks.")
                }
                ForEach(years, id: \.year) { y in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("Year \(y.year)").font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                            .frame(minWidth: 64, alignment: .leading)
                        VStack(alignment: .leading, spacing: 1) {
                            Text("#\(y.rank) of \(y.league) · value \(Format.moneyCompact(y.companyValueCents))")
                                .font(.caption).foregroundStyle(Theme.primaryText)
                            Text("Net worth \(Format.moneyCompact(y.netWorthCents)) · wealth place \(y.wealthRank)")
                                .font(.caption2).foregroundStyle(Theme.secondary)
                        }
                        Spacer(minLength: 6)
                        if y.medal != "none" && !y.medal.isEmpty {
                            SignalPill(text: CareerText.medal(y.medal).uppercased(), tint: medalTint(y.medal))
                        }
                    }
                    .accessibilityElement(children: .combine)
                }
            }
        }
    }
}

struct CareerMomentsPanel: View {
    let events: [CareerEventDto]
    var limit = 10

    var body: some View {
        Panel {
            VStack(alignment: .leading, spacing: 6) {
                Eyebrow(text: "The moments that mattered")
                if events.isEmpty {
                    EmptyNote(text: "Nothing has been recorded yet.")
                }
                ForEach(Array(events.sorted { $0.week < $1.week }.prefix(limit).enumerated()), id: \.offset) { _, e in
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text("Week \(e.week)").font(.caption).foregroundStyle(Theme.secondary)
                            .frame(minWidth: 64, alignment: .leading)
                        Text(e.text).font(.footnote).foregroundStyle(Theme.primaryText)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
    }
}

// MARK: - End of career

/// The end of a career, full screen: the medal and ambition, a one sentence verdict, the figures,
/// the year by year record, the moments that mattered and three ways on.
struct EndOfCareerView: View {
    let summary: CareerSummaryDto
    var onKeepPlaying: () -> Void
    var onNewCareer: () -> Void
    var onMainMenu: () -> Void

    private var medalLine: String {
        guard let ambition = summary.ambition else { return "No ambition: a sandbox career, no medal" }
        let medal = summary.medal == "none" || summary.medal.isEmpty ? "no medal" : "\(CareerText.medal(summary.medal)) medal"
        return "\(CareerText.ambition(ambition)): \(medal)"
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .center, spacing: 18) {
                    MedalArt(medal: summary.medal.isEmpty ? "none" : summary.medal)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(CareerText.ending(summary).uppercased())
                            .font(Pixel.font(22, bold: true))
                            .foregroundStyle(summary.lost ? Theme.negative : Theme.gold)
                            .accessibilityAddTraits(.isHeader)
                        Text(medalLine).font(Pixel.font(14))
                            .foregroundStyle(summary.medal == "none" || summary.medal.isEmpty ? Theme.primaryText : medalTint(summary.medal))
                        Text(CareerText.start(summary)).font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
                if !summary.sentence.isEmpty {
                    Text(summary.sentence).font(.body).foregroundStyle(Theme.primaryText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Panel {
                    VStack(alignment: .leading, spacing: 6) {
                        DetailRow(title: "Net worth at the start", value: Format.moneyCompact(summary.startWorthCents))
                        DetailRow(title: "Net worth at the end", value: Format.moneyCompact(summary.netWorthCents))
                        DetailRow(title: "Net worth vs start", value: CareerText.growth(summary.growthBps), tone: summary.growthBps >= 10_000 ? 1 : -1)
                        DetailRow(title: "Career length", value: "\(summary.years) years")
                    }
                }
                CareerYearsPanel(years: summary.yearsLog)
                CareerMomentsPanel(events: summary.events)
                VStack(spacing: 8) {
                    Button(action: onNewCareer) {
                        Text("Start a new career").font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
                    Button(action: onKeepPlaying) {
                        Text("Keep playing in this world").font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(PixelButtonStyle())
                    Button(action: onMainMenu) {
                        Text("Main menu").font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(PixelButtonStyle())
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
    }
}

/// Opens the end of career screen by itself the first time the engine says the career ended, and
/// again when Career asks for it. Choosing to keep playing keeps it closed for the session.
struct CareerEndHost: ViewModifier {
    @Environment(AppStore.self) private var store
    @State private var summary: CareerSummaryDto?
    @State private var dismissed = false

    func body(content: Content) -> some View {
        content
            .task(id: store.overview?.tick) { await check() }
            .onChange(of: store.careerEndRequested) { _, wanted in
                if wanted { Task { await open() } }
            }
            .fullScreenCover(isPresented: Binding(get: { summary != nil }, set: { if !$0 { summary = nil } })) {
                if let summary {
                    EndOfCareerView(
                        summary: summary,
                        onKeepPlaying: {
                            dismissed = true
                            self.summary = nil
                            store.careerEndRequested = false
                            Task { await store.continueCareer() }
                        },
                        onNewCareer: {
                            dismissed = true
                            self.summary = nil
                            store.careerEndRequested = false
                            store.wantsNewGame = true
                            Task { await store.exitToMenu() }
                        },
                        onMainMenu: {
                            dismissed = true
                            self.summary = nil
                            store.careerEndRequested = false
                            Task { await store.exitToMenu() }
                        }
                    )
                    .environment(store)
                    .preferredColorScheme(.dark)
                }
            }
    }

    /// Each week: has the engine ended the career? Time stops at that moment, once.
    private func check() async {
        guard !dismissed, summary == nil, store.sessionReady else { return }
        guard let ended = await store.read({ $0.careerStanding()?.ended == true }), ended else { return }
        store.setSpeed(0)
        await open()
    }

    private func open() async {
        guard let loaded = await store.read({ $0.careerSummary() }), let found = loaded, found.ended else {
            store.careerEndRequested = false
            return
        }
        summary = found
    }
}

extension View {
    func careerEndHost() -> some View { modifier(CareerEndHost()) }
}
