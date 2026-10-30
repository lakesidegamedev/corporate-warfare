import SwiftUI

/// The Executives section of the CEO Office: the three posts (chief financial officer, chief
/// operating officer, logistics chief), the person in each with their portrait, level, what they
/// change in plain words with the number, pay and contract left. Hire opens the candidate list with
/// the effect each would have at this company and the cost; Dismiss quotes the severance first.
struct ExecutivesPanel: View {
    @EnvironmentObject private var store: AppStore
    let currency: String
    @State private var hiring: ExecutivePostDto?
    @State private var dismissing: ExecutivePostDto?
    @State private var rejection: CommandRejection?

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    PixelIcon("people", size: 14).foregroundStyle(Theme.gold)
                    Eyebrow(text: "Executives")
                    Spacer()
                    if let pay = store.executives?.weeklyPayCents, pay > 0 {
                        Text("\(Format.moneyCompact(pay, currency: currency)) a week")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
                if let data = store.executives {
                    ForEach(data.posts, id: \.post) { post in postCard(post) }
                } else {
                    Text("Reading the books...").font(.caption).foregroundStyle(Theme.secondary)
                }
                if let rejection {
                    Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                }
            }
        }
        .onAppear {
            store.executivesOpen = true
            Task { await store.refreshExecutives() }
        }
        .onDisappear { store.executivesOpen = false }
        .sheet(item: $hiring) { post in
            HireSheet(post: post, currency: currency).environmentObject(store)
        }
        .confirmationDialog(
            dismissing.map { "Dismiss \($0.holder?.name ?? "")?" } ?? "",
            isPresented: Binding(get: { dismissing != nil }, set: { if !$0 { dismissing = nil } }),
            titleVisibility: .visible
        ) {
            if let post = dismissing, let holder = post.holder, let firm = store.executives?.firm {
                Button("Dismiss for \(Format.moneyCompact(holder.severanceCents, currency: currency)) severance", role: .destructive) {
                    Task { rejection = await store.dismissExecutive(firm: firm, post: Int(post.postCode)) }
                }
            }
            Button("Keep", role: .cancel) {}
        } message: {
            if let post = dismissing, let holder = post.holder {
                Text("Paid now from company cash. The \(post.title.lowercased()) post opens at once and you lose: \(holder.effect).")
            }
        }
    }

    private func postCard(_ post: ExecutivePostDto) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(post.title.uppercased()).font(.caption2.weight(.bold)).foregroundStyle(Theme.gold)
            if let e = post.holder {
                HStack(alignment: .top, spacing: 10) {
                    PixelPortrait(person: e.portraitId, size: 56)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(e.name).font(.subheadline.weight(.bold)).foregroundStyle(Theme.primaryText)
                        Text("Level \(e.level) of 5 · \(temperamentLabel(e.temperament))")
                            .font(.caption).foregroundStyle(Theme.gold)
                        Text("\(e.effect).").font(.caption).foregroundStyle(Theme.primaryText)
                        Text("Pay \(Format.moneyCompact(e.payCents, currency: currency)) a week · contract \(Self.contractLeft(Int(e.weeksLeft)))")
                            .font(.caption2).foregroundStyle(Theme.secondary)
                    }
                }
                Button("Dismiss") { dismissing = post }
                    .buttonStyle(PixelButtonStyle())
                    .disabled(store.isBusy)
            } else {
                Text("Empty. \(post.blurb)").font(.caption).foregroundStyle(Theme.secondary)
                Button("Hire") { hiring = post }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .disabled(store.isBusy)
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panelRaised, in: Pixel.shape)
    }

    static func contractLeft(_ weeks: Int) -> String {
        if weeks <= 0 { return "ends this week" }
        if weeks < 13 { return "\(weeks) weeks left" }
        return "\(weeks / 52).\(weeks % 52 * 10 / 52) years left"
    }
}

extension ExecutivePostDto: Identifiable {
    public var id: UInt32 { postCode }
}

/// The candidates for a post, best first, each with the effect they would have here and the cost.
struct HireSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let post: ExecutivePostDto
    let currency: String
    @State private var rejection: CommandRejection?

    private var people: [CandidateDto] {
        store.executives?.candidates.first { $0.postCode == post.postCode }?.people ?? []
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if people.isEmpty {
                        Text("Nobody is looking for work right now. New names turn up every quarter.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                    ForEach(people, id: \.id) { c in card(c) }
                    if let rejection {
                        Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                    }
                }
                .padding(12)
            }
            .background(Theme.ink)
            .navigationTitle("Hire a \(post.title.lowercased())")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
    }

    private func card(_ c: CandidateDto) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .top, spacing: 10) {
                PixelPortrait(person: c.portraitId, size: 48)
                VStack(alignment: .leading, spacing: 2) {
                    Text(c.name).font(.subheadline.weight(.bold)).foregroundStyle(Theme.primaryText)
                    Text("Level \(c.level) of 5 · \(temperamentLabel(c.temperament)) · \(c.age)")
                        .font(.caption).foregroundStyle(Theme.gold)
                    Text("\(c.effect).").font(.caption).foregroundStyle(Theme.primaryText)
                    if !c.hireNote.isEmpty { Text(c.hireNote).font(.caption2).foregroundStyle(Theme.secondary) }
                }
            }
            let worth = c.paybackWeeks > 0
                ? "At your figures it is worth about \(Format.moneyCompact(c.yearlyValueCents, currency: currency)) a year and the fee is back in \(BriefPanel.paybackText(Int64(c.paybackWeeks)))."
                : "At your figures it does not cover the pay today."
            Text("Pay \(Format.moneyCompact(c.askCents, currency: currency)) a week for two years, \(Format.moneyCompact(c.signingFeeCents, currency: currency)) to sign. \(worth)")
                .font(.caption2).foregroundStyle(Theme.secondary)
            if !c.affordable {
                Text("Not enough cash: the fee and 26 weeks of pay must be in hand.")
                    .font(.caption2).foregroundStyle(Theme.warning)
            }
            Button("Hire \(c.name.split(separator: " ").first.map(String.init) ?? c.name)") {
                guard let firm = store.executives?.firm else { return }
                Task {
                    rejection = await store.hireExecutive(firm: firm, post: Int(post.postCode), person: Int(c.id))
                    if rejection == nil { dismiss() }
                }
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .tint(Theme.gold)
            .disabled(store.isBusy || !c.affordable)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panelRaised, in: Pixel.shape)
    }
}

/// A rival's executives in its dossier.
struct RivalExecutivesList: View {
    let posts: [ExecutivePostDto]
    let currency: String

    var body: some View {
        let held = posts.filter { $0.holder != nil }
        if !held.isEmpty {
            Eyebrow(text: "Executives")
            ForEach(held, id: \.post) { post in
                if let e = post.holder {
                    HStack(spacing: 8) {
                        PixelPortrait(person: e.portraitId, size: 32)
                        VStack(alignment: .leading) {
                            Text("\(e.name), \(post.title.lowercased())")
                                .font(.caption.weight(.semibold)).foregroundStyle(Theme.primaryText)
                            Text("Level \(e.level) · pay \(Format.moneyCompact(e.payCents, currency: currency)) a week · to move them, offer at least \(Format.moneyCompact(e.poachPriceCents, currency: currency))")
                                .font(.caption2).foregroundStyle(Theme.secondary)
                        }
                    }
                }
            }
        }
    }
}
