import SwiftUI

/// The character cast's portraits, shared with every platform (Resources/cast-portraits.json): a
/// 32 by 32 bust with three moods (neutral, pleased, displeased) and its 16 by 16 reduction for every
/// person, named or generated, and a 64 by 64 office portrait for the named chiefs.
enum CastPortraits {
    struct Bust: Decodable {
        let palette: [String: String]
        let expressions: [String: [String]]
        let small: [String: [String]]
    }

    struct Office: Decodable {
        let palette: [String: String]
        let rows: [String]
    }

    private struct File: Decodable {
        let portraits: [String: Bust]
        let office: [String: Office]?
    }

    private static let file: File? = Bundle.main.url(forResource: "cast-portraits", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(File.self, from: $0) }

    static let all: [String: Bust] = file?.portraits ?? [:]
    static let office: [String: Office] = file?.office ?? [:]

    /// The mood to draw: the atlas has neutral, pleased and displeased, and anything else reads neutral.
    static func mood(_ key: String) -> String {
        key == "pleased" || key == "displeased" ? key : "neutral"
    }
}

/// One cast member's portrait as exact pixels. `mood` is neutral, pleased or displeased. Below 32
/// points it is the 16 by 16 bust for rows and chips; with `office` set and a 64 by 64 office
/// portrait on file (chiefs on company pages and dossiers) that is drawn instead.
struct PixelPortrait: View {
    let person: String
    var size: CGFloat = 40
    var mood: String = "neutral"
    var office = false
    /// Draws the 16 by 16 bust at any size, for a row of small busts in the CEO Office.
    var forceSmall = false

    var body: some View {
        Canvas { context, area in
            var context = context
            let rect = CGRect(origin: .zero, size: area)
            if office, let art = CastPortraits.office[person] {
                PixelIcons.draw(
                    rows: art.rows, palette: art.palette, gridSize: art.rows.count,
                    in: &context, rect: rect, tint: nil, key: "portrait|\(person)|office"
                )
                return
            }
            guard let art = CastPortraits.all[person] else { return }
            let m = CastPortraits.mood(mood)
            let small = forceSmall || size < 32
            guard let rows = small ? art.small[m] : art.expressions[m] else { return }
            PixelIcons.draw(
                rows: rows, palette: art.palette, gridSize: rows.count,
                in: &context, rect: rect, tint: nil, key: "portrait|\(person)|\(m)|\(small ? 16 : 32)"
            )
        }
        .frame(width: size, height: size)
        .background(Theme.panelRaised, in: Pixel.small)
        .clipShape(Pixel.small)
        .accessibilityHidden(true)
    }
}

func temperamentLabel(_ key: String) -> String {
    key.prefix(1).uppercased() + key.dropFirst()
}

func temperamentNote(_ key: String) -> String {
    switch key {
    case "integrator": return "Ties supply chains together and buys suppliers."
    case "raider": return "Takes rivals' plants whenever they are weak."
    case "financier": return "Leans on credit, deals and the balance sheet."
    case "speculator": return "Bets on new technology and big swings."
    case "builder": return "Grows organically, plant by plant."
    case "cautious": return "Keeps cash and avoids risk."
    default: return ""
    }
}

/// A portrait, a name and one line, for posts and chiefs.
struct PersonLine: View {
    let person: PersonDto
    let role: String

    var body: some View {
        HStack(spacing: 8) {
            PixelPortrait(person: person.id, size: 36, mood: person.mood)
            VStack(alignment: .leading, spacing: 1) {
                Text(person.name).font(.footnote.weight(.semibold)).foregroundStyle(Theme.primaryText).lineLimit(1)
                Text("\(role) · \(person.home)").font(.caption2).foregroundStyle(Theme.secondary).lineLimit(1)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// A rival's chief executive, their style, background and what the firm has done lately.
struct DossierBody: View {
    let dossier: DossierDto
    var currency: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let ceo = dossier.ceo {
                HStack(alignment: .top, spacing: 10) {
                    PixelPortrait(person: ceo.id, size: 72, mood: ceo.mood, office: ceo.officePortrait)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(ceo.name).font(.callout.weight(.bold)).foregroundStyle(Theme.primaryText)
                        Text("\(ceo.age) · \(ceo.home)").font(.caption).foregroundStyle(Theme.secondary)
                        Text("\(temperamentLabel(ceo.temperament)) · \(dossier.style)")
                            .font(.caption.weight(.semibold)).foregroundStyle(Theme.gold)
                        Text("Chief executive for \(dossier.ceoWeeks) weeks").font(.caption2).foregroundStyle(Theme.secondary)
                    }
                }
                Text(temperamentNote(ceo.temperament)).font(.caption).foregroundStyle(Theme.primaryText)
                Text(ceo.background).font(.caption).foregroundStyle(Theme.secondary)
            } else {
                Text("\(dossier.firmName) is run by its management. Style: \(dossier.style).")
                    .font(.caption).foregroundStyle(Theme.secondary)
            }
            ForEach(Array(dossier.grudges.enumerated()), id: \.offset) { _, line in
                HStack(alignment: .top, spacing: 6) {
                    PixelIcon("target", size: 12).foregroundStyle(Theme.negative)
                    Text(line).font(.caption).foregroundStyle(Theme.primaryText)
                }
            }
            ForEach(Array(dossier.allies.enumerated()), id: \.offset) { _, line in
                HStack(alignment: .top, spacing: 6) {
                    PixelIcon("people", size: 12).foregroundStyle(Theme.positive)
                    Text(line).font(.caption).foregroundStyle(Theme.primaryText)
                }
            }
            RivalExecutivesList(posts: dossier.executives, currency: currency)
            LikelyNextMove(text: dossier.likelyNext)
            DisclosedStakes(stakes: dossier.stakes)
            RaidMarketsList(firm: dossier.firm)
            if !dossier.moves.isEmpty {
                Eyebrow(text: "Recent moves")
                ForEach(Array(dossier.moves.prefix(5).enumerated()), id: \.offset) { _, move in
                    Text("\(ResultBuckets.weekLabel(move.week)): \(move.text)")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
            }
        }
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// The dossier of one company, loaded from the engine.
struct DossierSection: View {
    @EnvironmentObject private var store: AppStore
    let firm: UInt64
    @State private var dossier: DossierDto?

    var body: some View {
        Group {
            if let dossier, dossier.ceo != nil || !dossier.moves.isEmpty || !dossier.grudges.isEmpty || !dossier.allies.isEmpty {
                Panel(padding: 12) {
                    VStack(alignment: .leading, spacing: 8) {
                        Eyebrow(text: "Who runs it")
                        DossierBody(dossier: dossier, currency: store.firms.first { $0.handle == firm }?.currency ?? store.selectedFirmDto?.currency ?? "")
                    }
                }
            }
        }
        .task(id: firm) { dossier = await store.dossier(firm) }
    }
}

/// A rival line that opens its dossier, with a Raid button when the line names a market.
struct RivalLeaderRow: View {
    @EnvironmentObject private var store: AppStore
    let name: String
    let firm: UInt64
    let detail: String
    var region: String? = nil
    var industry: String? = nil
    @State private var open = false
    var body: some View {
        HStack(spacing: 8) {
            Button {
                open = true
            } label: {
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        FirmLogo(name: name, size: 22)
                        Text(name).font(.callout).foregroundStyle(Theme.primaryText).lineLimit(1)
                        Spacer()
                        Text(detail).font(.caption).foregroundStyle(Theme.secondary)
                        PixelIcon("chevron_down", size: 10).foregroundStyle(Theme.secondary)
                    }
                    RivalIdentity(firm: firm, portrait: 24)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if let region, let industry {
                RaidMarketButton(target: firm, region: region, industry: industry, compact: true)
            }
        }
        .sheet(isPresented: $open) { CompanyPageSheet(firm: firm) }
    }
}
