import SwiftUI

// The career choices on the new-game screen (2.0): the three companies offered at the helm, the
// ambition with what bronze, silver and gold take, and the career length. The figures and the
// medal wording come from the engine, so no threshold is typed here.

/// What the player picked: nothing but the defaults starts the game exactly as before.
struct CareerChoice: Equatable {
    var helm = false
    var helmFirm: UInt64?
    var ambition = ""
    var years: UInt32 = 0

    /// True when the choices change nothing about how a career is made.
    var isDefault: Bool { !helm && ambition.isEmpty && years == 0 }

    func options() -> CareerOptionsDto {
        CareerOptionsDto(
            start: helm ? "helm" : "founder",
            helmFirm: helm ? helmFirm : nil,
            ambition: ambition,
            years: years
        )
    }
}

/// One selectable card in the style of the new-game option grid.
private struct PickCard<Content: View>: View {
    let on: Bool
    let action: () -> Void
    @ViewBuilder var content: Content

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 4) {
                content
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(on ? Theme.gold.opacity(0.14) : Theme.panel, in: Pixel.shape)
            .overlay(Pixel.shape.stroke(on ? Theme.gold : Theme.stroke, lineWidth: on ? 2 : 1))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The three companies on offer at the helm, and the line that says what the start costs.
struct HelmOfferCards: View {
    let offers: [HelmOfferDto]?
    let error: String?
    let pick: UInt64?
    let onPick: (UInt64) -> Void

    var body: some View {
        if let error {
            Text(error).font(.footnote).foregroundStyle(Theme.warning)
        } else if let offers {
            if offers.isEmpty {
                Text("This scenario has no company to take the helm of. Found your own instead.")
                    .font(.footnote).foregroundStyle(Theme.secondary)
            } else {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 280), spacing: 10, alignment: .top)], spacing: 10) {
                    ForEach(offers, id: \.firm) { o in
                        PickCard(on: o.firm == pick, action: { onPick(o.firm) }) {
                            Text("#\(o.rank)  \(o.name)")
                                .font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                            Text("\(Sector.name(o.industry)) in \(o.region)")
                                .font(.caption).foregroundStyle(Theme.secondary)
                            Text("Value \(Format.moneyCompact(o.valueCents)), sales \(o.revenueYearCents > 0 ? Format.moneyCompact(o.revenueYearCents) + " a year" : "none yet"), \(o.plants) plants, \(o.workers.formatted()) workers")
                                .font(.caption).foregroundStyle(Theme.primaryText)
                                .fixedSize(horizontal: false, vertical: true)
                            Text("Price of control \(Format.moneyCompact(o.priceCents)). Your fortune \(Format.moneyCompact(o.fortuneCents)).")
                                .font(.caption).foregroundStyle(Theme.gold)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
                if let chosen = offers.first(where: { $0.firm == pick }) {
                    Text("\(chosen.name) is in \(chosen.region). Taking the helm buys control from its outside shareholders at today's price: \(Format.moneyCompact(chosen.priceCents)) for \(chosen.sharesNeeded.formatted()) shares. The sitting chief executive leaves.")
                        .font(.footnote).foregroundStyle(Theme.primaryText)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(CareerText.fortuneNote(chosen))
                        .font(.footnote)
                        .foregroundStyle(chosen.fortuneCents != chosen.standardFortuneCents ? Theme.warning : Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        } else {
            Text("Finding the companies on offer...").font(.footnote).foregroundStyle(Theme.gold)
        }
    }
}

/// The ambition (or none) with what each medal takes, and the career length.
struct AmbitionPicker: View {
    let helm: Bool
    @Binding var choice: CareerChoice

    private let lengths = careerLengths()

    private var shownYears: UInt32 { choice.years == 0 ? lengths.defaultYears : choice.years }

    var body: some View {
        let guide = ambitionGuide(start: helm ? "helm" : "founder", years: shownYears)
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), spacing: 10, alignment: .top)], spacing: 10) {
            PickCard(on: choice.ambition.isEmpty, action: { choice.ambition = "" }) {
                Text("No ambition").font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text("A sandbox: no goal, no medals and no ending. Play as long as you like.")
                    .font(.caption).foregroundStyle(Theme.secondary)
            }
            ForEach(guide, id: \.key) { a in
                PickCard(on: choice.ambition == a.key, action: { choice.ambition = a.key }) {
                    Text(a.name).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text(a.sentence).font(.caption).foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    Text("Bronze: \(a.bronze).").font(.caption).foregroundStyle(medalTint("bronze"))
                        .fixedSize(horizontal: false, vertical: true)
                    Text("Silver: \(a.silver).").font(.caption).foregroundStyle(medalTint("silver"))
                        .fixedSize(horizontal: false, vertical: true)
                    Text("Gold: \(a.gold).").font(.caption).foregroundStyle(medalTint("gold"))
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        if !choice.ambition.isEmpty {
            Eyebrow(text: "How long")
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 10, alignment: .top)], spacing: 10) {
                ForEach(lengths.years, id: \.self) { y in
                    PickCard(on: shownYears == y, action: { choice.years = y }) {
                        Text("\(y) years").font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                        Text(y == lengths.defaultYears ? "The usual length" : (y < lengths.defaultYears ? "A shorter game" : "A long game"))
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
            }
            Text("The medals above are for \(shownYears) years. A career ends after that, or sooner if the board removes you, a rival takes your company or you are left with none.")
                .font(.caption).foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
