import SwiftUI

/// The character cast's 16 by 16 pixel portraits, shared with every platform (Resources/cast-portraits.json).
enum CastPortraits {
    private struct File: Decodable {
        let size: Int
        let portraits: [String: PixelIcons.Icon]
    }

    private static let file: File? = Bundle.main.url(forResource: "cast-portraits", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(File.self, from: $0) }

    static let size: Int = file?.size ?? 16
    static let all: [String: PixelIcons.Icon] = file?.portraits ?? [:]
}

/// One cast member's portrait as exact pixels.
struct PixelPortrait: View {
    let person: String
    var size: CGFloat = 40

    var body: some View {
        Canvas { context, area in
            guard let art = CastPortraits.all[person] else { return }
            var context = context
            PixelIcons.draw(
                rows: art.rows, palette: art.palette, gridSize: CastPortraits.size,
                in: &context, rect: CGRect(origin: .zero, size: area), tint: nil
            )
        }
        .frame(width: size, height: size)
        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 4))
        .clipShape(RoundedRectangle(cornerRadius: 4))
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
            PixelPortrait(person: person.id, size: 36)
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

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let ceo = dossier.ceo {
                HStack(alignment: .top, spacing: 10) {
                    PixelPortrait(person: ceo.id, size: 64)
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
                        DossierBody(dossier: dossier)
                    }
                }
            }
        }
        .task(id: firm) { dossier = await store.dossier(firm) }
    }
}

/// A rival line that opens its dossier.
struct RivalLeaderRow: View {
    @EnvironmentObject private var store: AppStore
    let name: String
    let firm: UInt64
    let detail: String
    @State private var open = false
    @State private var dossier: DossierDto?

    var body: some View {
        Button {
            open = true
        } label: {
            HStack {
                Text(name).font(.callout).foregroundStyle(Theme.primaryText).lineLimit(1)
                Spacer()
                Text(detail).font(.caption).foregroundStyle(Theme.secondary)
                PixelIcon("chevron_down", size: 10).foregroundStyle(Theme.secondary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .sheet(isPresented: $open) {
            NavigationStack {
                ScrollView {
                    if let dossier { DossierBody(dossier: dossier).padding(14) }
                }
                .background(Theme.ink)
                .navigationTitle(name)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { open = false } } }
            }
            .task { dossier = await store.dossier(firm) }
        }
    }
}
