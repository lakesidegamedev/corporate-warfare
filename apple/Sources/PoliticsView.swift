import SwiftUI

/// Politics: every state's governor race and the national leader race with the two candidates, their
/// portraits and stated positions, the weekly poll, the laws in force and coming, a Give button with
/// the cap and the disclosure warning, and the public tenders with a bid form that quotes cost and
/// profit. Lives under Government in the Economy card.
struct PoliticsPanel: View {
    @EnvironmentObject private var store: AppStore
    let currency: String
    @State private var giving: (ElectionRaceDto, PoliticalCandidateDto)?
    @State private var bidding: PublicTenderDto?
    @State private var showAll = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let data = store.politics, data.ready {
                Eyebrow(text: "Politics")
                ForEach(Array((showAll ? data.races : Array(data.races.prefix(3))).enumerated()), id: \.offset) { _, race in
                    raceCard(race)
                }
                if data.races.count > 3 {
                    Button(showAll ? "Show fewer races" : "Show all \(data.races.count) races") { showAll.toggle() }
                        .font(.caption)
                }
                let open = data.tenders.filter { $0.status == "open" }
                let rest = Array(data.tenders.filter { $0.status != "open" }.prefix(4))
                if !open.isEmpty || !rest.isEmpty {
                    Eyebrow(text: "Public tenders")
                    ForEach(open, id: \.id) { tenderRow($0, canBid: true) }
                    ForEach(rest, id: \.id) { tenderRow($0, canBid: false) }
                }
            }
        }
        .onAppear {
            store.politicsOpen = true
            Task { await store.refreshPolitics() }
        }
        .onDisappear { store.politicsOpen = false }
        .sheet(isPresented: Binding(get: { giving != nil }, set: { if !$0 { giving = nil } })) {
            if let giving {
                GiveSheet(race: giving.0, candidate: giving.1, currency: currency, note: store.politics?.disclosureNote ?? "")
                    .environmentObject(store)
            }
        }
        .sheet(item: $bidding) { tender in
            BidSheet(tender: tender, currency: currency).environmentObject(store)
        }
    }

    private func partyName(_ key: String) -> String { key == "business" ? "Commerce Party" : "Workers' Party" }

    private func raceCard(_ race: ElectionRaceDto) -> some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(race.office == 255 ? "Leader of the country" : "Governor of \(race.place)")
                    .font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text("In office: \(race.governor) (\(partyName(race.governorParty))) · vote \(ResultBuckets.weekLabel(race.electionWeek)), in \(race.weeksLeft) weeks")
                    .font(.caption).foregroundStyle(Theme.secondary)
                HStack {
                    Text("\(race.pollBps / 100)%").font(.caption)
                    GeometryReader { proxy in
                        ZStack(alignment: .leading) {
                            Pixel.small.fill(Theme.negative.opacity(0.7))
                            Pixel.small.fill(Theme.positive)
                                .frame(width: proxy.size.width * CGFloat(min(max(Double(race.pollBps) / 10_000, 0), 1)))
                        }
                    }
                    .frame(height: 8)
                    Text("\((10_000 - race.pollBps) / 100)%").font(.caption)
                }
                ForEach(race.candidates, id: \.id) { c in
                    HStack(alignment: .top, spacing: 8) {
                        PixelPortrait(person: c.id, size: 40)
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(c.name) · \(partyName(c.party))").font(.footnote.weight(.semibold)).foregroundStyle(Theme.primaryText)
                            ForEach(c.positions, id: \.self) { Text("· \($0)").font(.caption).foregroundStyle(Theme.secondary) }
                            if c.fundsCents > 0 {
                                Text("Gifts \(Format.moneyCompact(c.fundsCents, currency: currency)), worth +\(c.bonusBps / 100).\(c.bonusBps % 100 / 10) points in the poll")
                                    .font(.caption2).foregroundStyle(Theme.secondary)
                            }
                        }
                        Spacer()
                        if race.open { Button("Give") { giving = (race, c) }.font(.caption) }
                    }
                }
                if race.office != 255 {
                    if !race.lawsInForce.isEmpty {
                        Text("Laws in force").font(.caption2).foregroundStyle(Theme.secondary)
                        ForEach(race.lawsInForce, id: \.self) { Text("· \($0)").font(.caption) }
                    }
                    if !race.lawsPending.isEmpty {
                        Text("Coming \(ResultBuckets.weekLabel(race.pendingWeek))").font(.caption2).foregroundStyle(Theme.warning)
                        ForEach(race.lawsPending, id: \.self) { Text("· \($0)").font(.caption) }
                    }
                }
            }
        }
    }

    private func tenderRow(_ t: PublicTenderDto, canBid: Bool) -> some View {
        Panel(padding: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(t.kindLabel.prefix(1).uppercased() + t.kindLabel.dropFirst()) in \(t.place)")
                        .font(.footnote.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text(tenderLine(t)).font(.caption).foregroundStyle(Theme.secondary)
                }
                Spacer()
                if canBid { Button("Bid") { bidding = t }.font(.caption) }
            }
        }
    }

    private func tenderLine(_ t: PublicTenderDto) -> String {
        switch t.status {
        case "open":
            var line = "Up to \(Format.moneyCompact(t.budgetCents, currency: currency)) · \(t.minWeeks) to \(t.maxWeeks) weeks · closes \(ResultBuckets.weekLabel(t.closesWeek)) · \(t.bids) bids"
            if t.yourPriceCents > 0 { line += " · your bid \(Format.moneyCompact(t.yourPriceCents, currency: currency)) in \(t.yourWeeks) weeks" }
            return line
        case "building":
            return "\(t.winner) builds it for \(Format.moneyCompact(t.priceCents, currency: currency)) in \(t.promisedWeeks) weeks · \(t.progressBps / 100)% built, \(Format.moneyCompact(t.paidCents, currency: currency)) paid"
        case "done": return "Finished by \(t.winner)"
        default: return "Lapsed"
        }
    }
}

extension PublicTenderDto: Identifiable {}

private struct GiveSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let race: ElectionRaceDto
    let candidate: PoliticalCandidateDto
    let currency: String
    let note: String
    @State private var text = "50000"
    @State private var rejection: CommandRejection?

    var body: some View {
        let room = max(race.capCents - race.youGaveCents, 0)
        let cents = (Int64(text) ?? 0) * 100
        NavigationStack {
            Form {
                Text("The law caps gifts at \(Format.moneyCompact(race.capCents, currency: currency)) per company for this race. You have given \(Format.moneyCompact(race.youGaveCents, currency: currency)), so \(Format.moneyCompact(room, currency: currency)) is left.")
                    .font(.caption)
                TextField("Amount", text: $text).keyboardType(.numberPad)
                Text(note).font(.caption).foregroundStyle(Theme.warning)
                if let rejection { Text(rejection.message).foregroundStyle(Theme.negative) }
            }
            .navigationTitle("Give to \(candidate.name)")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Give") {
                        guard let firm = store.selectedFirm else { return }
                        Task {
                            rejection = await store.giveToCandidate(firm: firm, office: Int(race.office), candidate: candidate.party == "business" ? 0 : 1, cents: cents)
                            if rejection == nil { dismiss() }
                        }
                    }
                    .disabled(store.selectedFirm == nil || cents < 100_000 || cents > room)
                }
            }
        }
    }
}

private struct BidSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let tender: PublicTenderDto
    let currency: String
    @State private var weeks = ""
    @State private var price = ""
    @State private var rejection: CommandRejection?

    var body: some View {
        let w = min(max(UInt32(weeks) ?? tender.minWeeks, tender.minWeeks), tender.maxWeeks)
        let span = Int64(max(tender.maxWeeks - tender.minWeeks, 1))
        // The cost to deliver falls in a straight line from the quickest to the slowest delivery.
        let cost = tender.costQuickCents + (tender.costSlowCents - tender.costQuickCents) * Int64(w - tender.minWeeks) / span
        let cents = (Int64(price) ?? 0) * 100
        NavigationStack {
            Form {
                Text("The state pays up to \(Format.moneyCompact(tender.budgetCents, currency: currency)). The best bid wins: lowest price, with each week of delivery counted as a hundredth of that budget. You build it, paying for contractors and materials as you go, and are paid in four stages as it is built. A week late costs a tenth of the price.")
                    .font(.caption)
                TextField("Delivery in weeks (\(tender.minWeeks) to \(tender.maxWeeks))", text: $weeks).keyboardType(.numberPad)
                TextField("Your price", text: $price).keyboardType(.numberPad)
                Text("It costs you about \(Format.moneyCompact(cost, currency: currency)) to deliver in \(w) weeks. At this price your profit is \(Format.moneyCompact(cents - cost, currency: currency)).")
                    .font(.caption).foregroundStyle(cents >= cost ? Theme.positive : Theme.negative)
                if let rejection { Text(rejection.message).foregroundStyle(Theme.negative) }
            }
            .navigationTitle("Bid on the \(tender.kindLabel)")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Submit bid") {
                        guard let firm = store.selectedFirm else { return }
                        Task {
                            rejection = await store.bidOnTender(firm: firm, tender: Int(tender.id), priceCents: cents, weeks: Int(w))
                            if rejection == nil { dismiss() }
                        }
                    }
                    .disabled(store.selectedFirm == nil || cents <= 0 || cents > tender.budgetCents)
                }
            }
        }
        .onAppear {
            if weeks.isEmpty { weeks = String((tender.minWeeks + tender.maxWeeks) / 2) }
            if price.isEmpty { price = String(min(tender.costQuickCents * 115 / 100, tender.budgetCents) / 100) }
        }
    }
}
