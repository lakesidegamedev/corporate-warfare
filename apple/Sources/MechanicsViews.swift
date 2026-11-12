import SwiftUI

// The 2.0 mechanics on the existing screens' structure (2.0): raid campaigns with weeks left and
// a chance, plants on offer and the player's listings, and where a new plant would pay. Every
// figure is the engine's; these views add words and the buttons that send commands.

/// Plain words for plants on offer. Pure, so the tests can check them without a screen.
enum PlantLotText {
    /// "Break-up auction" or "Listed by <seller>".
    static func route(_ lot: PlantLotDto) -> String {
        lot.route == "break_up" ? "Break-up auction of \(lot.sellerName)" : "Listed by \(lot.sellerName)"
    }

    /// One line for an Actions form picker.
    static func pickLabel(_ lot: PlantLotDto) -> String {
        var line = "\(lot.plantName) in \(lot.regionName), \(lot.blocks) plant\(lot.blocks == 1 ? "" : "s"), book \(Format.moneyCompact(lot.bookCents))"
        if lot.yours {
            line += lot.bids.isEmpty ? ", no bids yet" : ", \(lot.bids.count) bid\(lot.bids.count == 1 ? "" : "s")"
        } else {
            line += ", \(lot.weeksLeft) week\(lot.weeksLeft == 1 ? "" : "s") left"
        }
        return line
    }

    static func weeks(_ n: UInt32) -> String {
        n == 0 ? "closes this week" : "\(n) week\(n == 1 ? "" : "s") left"
    }
}

/// Plain words for raid campaigns.
enum RaidText {
    static func title(_ c: RaidCampaignDto) -> String {
        switch c.role {
        case "against_player": return "\(c.attackerName) is raiding you"
        case "by_player": return "You are raiding \(c.defenderName)"
        default: return "\(c.attackerName) is raiding \(c.defenderName)"
        }
    }

    static func decided(_ c: RaidCampaignDto) -> String {
        c.weeksLeft == 0 ? "Decided this week" : "Decided in \(c.weeksLeft) week\(c.weeksLeft == 1 ? "" : "s"), in week \(c.resolvesWeek)"
    }

    static func stake(_ c: RaidCampaignDto) -> String {
        "\(c.blocksAtStake) of \(c.divisionBlocks) plant\(c.divisionBlocks == 1 ? "" : "s")"
    }
}

/// Where the 2.0 screens are reached from the CEO office.
struct MechanicsLinks: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(spacing: 8) {
            link("Raids: campaigns against you, by you and between rivals", icon: "flag") { RaidsView() }
            link("Plants on offer and your listings", icon: "markets") { PlantLotsView() }
            link("Where a new plant would pay", icon: "plus") { OpeningsView() }
        }
    }

    private func link<D: View>(_ title: String, icon: String, @ViewBuilder destination: @escaping () -> D) -> some View {
        NavigationLink {
            destination().environment(store)
        } label: {
            HStack {
                PixelLabel(title, icon: icon)
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

// MARK: - Raids

struct RaidsView: View {
    @Environment(AppStore.self) private var store
    @State private var campaigns: [RaidCampaignDto] = []
    @State private var loaded = false
    @State private var rejection: CommandRejection?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Raids").font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                Text("A rival that wants part of a business must announce it four weeks ahead. The raid is decided against the defence the target has when the weeks run out.")
                    .font(.footnote).foregroundStyle(Theme.secondary)
                if let rejection {
                    Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                }
                group("Against you", role: "against_player", empty: "No rival has announced a raid on you.")
                group("By you", role: "by_player", empty: "You have no raid running.")
                group("Between rivals", role: "between_rivals", empty: "No raid is running between rivals.")
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .task(id: store.historyRefreshKey) { await reload() }
    }

    private func reload() async {
        if let list = await store.read({ $0.raidCampaigns() }) { campaigns = list }
        loaded = true
    }

    @ViewBuilder private func group(_ title: String, role: String, empty: String) -> some View {
        let rows = campaigns.filter { $0.role == role }
        Eyebrow(text: title)
        if rows.isEmpty {
            EmptyNote(text: loaded ? empty : "Looking...")
        }
        ForEach(Array(rows.enumerated()), id: \.offset) { _, c in
            RaidCampaignCard(campaign: c) { rejection = await abandon(c) }
        }
    }

    private func abandon(_ c: RaidCampaignDto) async -> CommandRejection? {
        let result = await store.submit(CommandJSON.command("AbandonRaid", [
            "firm": c.attacker, "target": c.defender, "region": c.region, "industry": c.industry
        ]))
        await reload()
        return result
    }
}

struct RaidCampaignCard: View {
    let campaign: RaidCampaignDto
    var onAbandon: () async -> Void

    var body: some View {
        let c = campaign
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(RaidText.title(c)).font(.headline).foregroundStyle(Theme.primaryText)
                Text("\(c.industryName) in \(c.regionName)").font(.caption).foregroundStyle(Theme.secondary)
                DetailRow(title: "Decision", value: RaidText.decided(c))
                DetailRow(title: "Chance today", value: Format.percentBps(c.chanceBps), tone: c.role == "against_player" ? -1 : 0)
                DetailRow(title: "At stake", value: RaidText.stake(c))
                DetailRow(title: "Attack budget", value: Format.moneyCompact(c.budgetCents))
                DetailRow(title: "Attack against defence", value: "\(Format.moneyCompact(c.attackCents)) against \(Format.moneyCompact(c.defenceCents))")
                DetailRow(title: "Price if it lands", value: Format.moneyCompact(c.priceCents))
                if c.role == "against_player" {
                    Text("Defend: raise your marketing budget. The raid is decided against the defence you have in week \(c.resolvesWeek), not today's.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    NavigationLink {
                        ResearchView()
                    } label: {
                        PixelLabel("Defend: set the marketing budget", icon: "target")
                            .font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 4)
                    }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
                } else if c.role == "by_player" {
                    Button {
                        Task { await onAbandon() }
                    } label: {
                        PixelLabel("Abandon the raid", icon: "blocked")
                            .font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 4)
                    }
                    .buttonStyle(PixelButtonStyle())
                    .tint(Theme.negative)
                    Text("The budget is already committed: it buys advertising whether you abandon or not.")
                        .font(.caption2).foregroundStyle(Theme.secondary)
                }
            }
        }
    }
}

// MARK: - Plants on offer

struct PlantLotsView: View {
    @Environment(AppStore.self) private var store
    @State private var lots: [PlantLotDto] = []
    @State private var loaded = false
    @State private var bidding: PlantLotDto?
    @State private var rejection: CommandRejection?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Plants on offer").font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                Text("A failed company's plants are auctioned for four weeks, and rivals list plants they want to sell. Bids are sealed until the deadline. A break-up auction goes to the best bid at or above the scrap price; a listed plant sells only if its owner accepts.")
                    .font(.footnote).foregroundStyle(Theme.secondary)
                if let rejection {
                    Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                }
                Eyebrow(text: "On offer")
                let onOffer = lots.filter { !$0.yours }
                if onOffer.isEmpty {
                    EmptyNote(text: loaded ? "No plant is on offer." : "Looking...")
                }
                ForEach(Array(onOffer.enumerated()), id: \.offset) { _, lot in
                    lotCard(lot)
                }
                Eyebrow(text: "Your listings")
                let mine = lots.filter { $0.yours }
                if mine.isEmpty {
                    EmptyNote(text: "You have no plant listed for sale.")
                }
                ForEach(Array(mine.enumerated()), id: \.offset) { _, lot in
                    listingCard(lot)
                }
                Eyebrow(text: "List one of your plants")
                Text("Offer a plant to rivals for four weeks. It keeps running. When bidding closes you see every bid and have four more weeks to accept the best one or keep the plant.")
                    .font(.caption).foregroundStyle(Theme.secondary)
                let listed = Set(mine.map(\.facility))
                let plants = store.facilities.filter { !listed.contains($0.handle) }
                if plants.isEmpty {
                    EmptyNote(text: "No plant to list.")
                }
                ForEach(Array(plants.enumerated()), id: \.offset) { _, plant in
                    HStack {
                        Text(plantName(plant)).font(.callout).foregroundStyle(Theme.primaryText)
                        Spacer(minLength: 6)
                        Button("List") { Task { await list(plant) } }
                            .buttonStyle(PixelButtonStyle())
                            .disabled(store.isBusy)
                    }
                    .padding(10)
                    .background(Theme.panel, in: Pixel.shape)
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .task(id: store.historyRefreshKey) { await reload() }
        .sheet(item: $bidding) { lot in
            PlantBidSheet(lot: lot) { rejection = $0; Task { await reload() } }
                .environment(store)
                .presentationDetents([.medium, .large])
                .presentationBackground(Theme.panel)
        }
    }

    private func plantName(_ plant: FacilityDto) -> String {
        let name = store.strategyCatalogue.first { $0.id == plant.recipe }?.name ?? plant.recipe
        return "\(name) · \(WorldMap.shared.regionName(plant.region)) · \(plant.blocks) plant\(plant.blocks == 1 ? "" : "s")"
    }

    private func reload() async {
        if let list = await store.read({ $0.plantLots() }) { lots = list }
        loaded = true
    }

    private func list(_ plant: FacilityDto) async {
        rejection = await store.submit(CommandJSON.command("ListPlant", ["facility": plant.handle]))
        await reload()
    }

    private func answer(_ variant: String, _ lot: PlantLotDto) async {
        rejection = await store.submit(CommandJSON.command(variant, ["lot": lot.lot]))
        await reload()
    }

    private func lotCard(_ lot: PlantLotDto) -> some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(lot.plantName).font(.headline).foregroundStyle(Theme.primaryText)
                Text("\(PlantLotText.route(lot)) · \(lot.regionName) · \(lot.blocks) plant\(lot.blocks == 1 ? "" : "s")")
                    .font(.caption).foregroundStyle(Theme.secondary)
                DetailRow(title: "Bidding", value: PlantLotText.weeks(lot.weeksLeft))
                DetailRow(title: "Book value", value: Format.moneyCompact(lot.bookCents))
                if lot.route == "break_up" {
                    DetailRow(title: "Scrap price (lowest bid that wins)", value: Format.moneyCompact(lot.reserveCents))
                }
                DetailRow(title: "Cost to build the same new", value: Format.moneyCompact(lot.replacementCents))
                DetailRow(title: "Worth to you a week", value: Format.signedMoney(lot.weeklyCents), tone: lot.weeklyCents >= 0 ? 1 : -1)
                DetailRow(title: "Earnings so far", value: Format.signedMoney(lot.earningsCents), tone: lot.earningsCents >= 0 ? 1 : -1)
                if lot.playerBidCents > 0 {
                    DetailRow(title: "Your sealed bid", value: Format.moneyCompact(lot.playerBidCents), tone: 1)
                }
                Button {
                    bidding = lot
                } label: {
                    PixelLabel(lot.playerBidCents > 0 ? "Change or withdraw your bid" : "Bid for this plant", icon: "plus")
                        .font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 4)
                }
                .buttonStyle(PixelButtonStyle(prominent: lot.playerBidCents == 0))
                .tint(Theme.gold)
                .foregroundStyle(lot.playerBidCents == 0 ? Theme.ink : Theme.primaryText)
                .disabled(store.isBusy || lot.weeksLeft == 0)
            }
        }
    }

    private func listingCard(_ lot: PlantLotDto) -> some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(lot.plantName).font(.headline).foregroundStyle(Theme.primaryText)
                Text("\(lot.regionName) · \(lot.blocks) plant\(lot.blocks == 1 ? "" : "s") · book \(Format.moneyCompact(lot.bookCents))")
                    .font(.caption).foregroundStyle(Theme.secondary)
                if lot.bids.isEmpty {
                    Text(lot.weeksLeft > 0 ? "Bids are sealed. Bidding closes in \(lot.weeksLeft) week\(lot.weeksLeft == 1 ? "" : "s")." : "No rival bid. The offer lapses in week \(lot.answerByWeek).")
                        .font(.footnote).foregroundStyle(Theme.secondary)
                } else {
                    Text("Bids put to you. Answer by week \(lot.answerByWeek) or the offer lapses.")
                        .font(.footnote).foregroundStyle(Theme.gold)
                    ForEach(Array(lot.bids.sorted { $0.priceCents > $1.priceCents }.enumerated()), id: \.offset) { _, bid in
                        DetailRow(title: bid.bidderName, value: Format.moneyCompact(bid.priceCents), tone: 1)
                    }
                    Button {
                        Task { await answer("AcceptPlantBid", lot) }
                    } label: {
                        PixelLabel("Accept the best bid", icon: "check")
                            .font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 4)
                    }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
                    .disabled(store.isBusy)
                }
                Button {
                    Task { await answer("WithdrawListing", lot) }
                } label: {
                    PixelLabel("Take it off the market", icon: "blocked")
                        .font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 4)
                }
                .buttonStyle(PixelButtonStyle())
                .disabled(store.isBusy)
            }
        }
    }
}

extension PlantLotDto: Identifiable {
    public var id: UInt32 { lot }
}

/// A sealed bid for a plant on offer. A bid of zero withdraws the player's own.
struct PlantBidSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let lot: PlantLotDto
    var onDone: (CommandRejection?) -> Void
    @State private var dollars: String = ""
    @State private var rejection: CommandRejection?

    private var cents: Int64? {
        guard let value = Double(dollars.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: ",", with: "")), value >= 0 else { return nil }
        return Int64((value * 100).rounded())
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text(lot.plantName).font(.headline).foregroundStyle(Theme.primaryText)
                    Text("\(PlantLotText.route(lot)) · \(PlantLotText.weeks(lot.weeksLeft))").font(.caption).foregroundStyle(Theme.secondary)
                    DetailRow(title: "Book value", value: Format.moneyCompact(lot.bookCents))
                    if lot.route == "break_up" {
                        DetailRow(title: "Scrap price", value: Format.moneyCompact(lot.reserveCents))
                    }
                    DetailRow(title: "Cost to build the same new", value: Format.moneyCompact(lot.replacementCents))
                    DetailRow(title: "Worth to you a week", value: Format.signedMoney(lot.weeklyCents))
                    if lot.playerBidCents > 0 {
                        DetailRow(title: "Your sealed bid now", value: Format.moneyCompact(lot.playerBidCents), tone: 1)
                    }
                    Text("Your company must hold the cash when you bid and again when bidding closes. Enter 0 to withdraw a bid.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    HStack {
                        Text("Your bid $").foregroundStyle(Theme.secondary)
                        TextField("0", text: $dollars)
                            .keyboardType(.numberPad)
                            .textFieldStyle(.roundedBorder)
                            .frame(maxWidth: 220)
                    }
                    if let rejection {
                        Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                    }
                    Button {
                        Task { await send() }
                    } label: {
                        Text(cents == 0 ? "Withdraw my bid" : "Place sealed bid")
                            .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
                    .disabled(cents == nil || store.isBusy)
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle("Bid for a plant")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .onAppear {
                let start = lot.playerBidCents > 0 ? lot.playerBidCents : max(lot.reserveCents, 0)
                dollars = start > 0 ? String(start / 100) : ""
            }
        }
    }

    private func send() async {
        guard let cents else { return }
        let firm = lot.playerBidFirm != 0 ? lot.playerBidFirm : (store.selectedFirm ?? 0)
        guard firm != 0 else {
            rejection = CommandRejection(key: "error.invalid_entity", detail: "")
            return
        }
        let result = await store.submit(CommandJSON.command("BidForPlant", ["firm": firm, "lot": lot.lot, "price_cents": cents]))
        rejection = result
        if result == nil {
            onDone(nil)
            dismiss()
        }
    }
}

// MARK: - Openings

struct OpeningsView: View {
    @Environment(AppStore.self) private var store
    @State private var openings: [CapacityOpeningDto] = []
    @State private var loaded = false
    @State private var building: CapacityOpeningDto?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Where a new plant would pay").font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                Text("The engine's reading of room to build, best first: goods the corridor imports that a home plant would replace, and households that found no seller. Rivals read the same signal.")
                    .font(.footnote).foregroundStyle(Theme.secondary)
                if openings.isEmpty {
                    EmptyNote(text: loaded ? "No opening shows room to build right now." : "Looking...")
                }
                ForEach(Array(openings.enumerated()), id: \.offset) { _, o in
                    openingCard(o)
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .task(id: store.historyRefreshKey) {
            if let list = await store.read({ $0.capacityOpenings(limit: 20) }) { openings = list }
            loaded = true
        }
        .sheet(item: $building) { o in
            OpeningBuildSheet(opening: o)
                .environment(store)
                .presentationDetents([.medium, .large])
                .presentationBackground(Theme.panel)
        }
    }

    private func strategy(_ o: CapacityOpeningDto) -> StrategyDto? {
        store.strategyCatalogue.first { $0.id == o.recipeId }
    }

    private func openingCard(_ o: CapacityOpeningDto) -> some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    SectorBadge(sector: o.industry, size: 24)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(strategy(o)?.name ?? o.recipeId).font(.headline).foregroundStyle(Theme.primaryText)
                        Text("\(Sector.name(o.industry)) in \(WorldMap.shared.regionName(o.regionCode))")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
                DetailRow(title: "The market is short by", value: "\(Format.lots(o.roomLots)) a week")
                DetailRow(title: "One plant makes", value: "\(Format.lots(o.blockLots)) a week")
                if !o.traded {
                    DetailRow(title: "Households going without", value: "\(Format.lots(o.unmetLots)) of \(Format.lots(o.wantedLots)) a week")
                }
                DetailRow(title: "Price a lot fetches", value: Format.price(o.priceCents))
                DetailRow(title: "Against its reference price", value: Format.percentBps(Int32(clamping: o.priceVsReferenceBps)))
                DetailRow(title: "It would add a week", value: Format.signedMoney(o.weeklyContributionCents), tone: o.weeklyContributionCents >= 0 ? 1 : -1)
                DetailRow(title: "Cost to build", value: Format.moneyCompact(o.buildCostCents))
                DetailRow(title: "Pays itself back in", value: o.paybackWeeks.map { "\($0) weeks" } ?? "never at today's prices")
                Button {
                    building = o
                } label: {
                    PixelLabel("Build here", icon: "plus")
                        .font(.callout.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 4)
                }
                .buttonStyle(PixelButtonStyle(prominent: true))
                .tint(Theme.gold)
                .foregroundStyle(Theme.ink)
                .disabled(store.isBusy || strategy(o) == nil)
            }
        }
    }
}

extension CapacityOpeningDto: Identifiable {
    public var id: String { recipeId + "@" + regionCode }
}

/// Confirm one plant at an opening against the engine's own quote.
struct OpeningBuildSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let opening: CapacityOpeningDto
    @State private var quote: BuildQuoteDto?
    @State private var rejection: CommandRejection?

    private var strategy: StrategyDto? { store.strategyCatalogue.first { $0.id == opening.recipeId } }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text(strategy?.name ?? opening.recipeId).font(.headline).foregroundStyle(Theme.primaryText)
                    Text("\(Sector.name(opening.industry)) in \(WorldMap.shared.regionName(opening.regionCode))")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    if let quote {
                        DetailRow(title: "Plant", value: Format.moneyCompact(quote.capexCents))
                        if quote.entryFeeCents > 0 {
                            DetailRow(title: "Entry fee", value: Format.moneyCompact(quote.entryFeeCents))
                        }
                        DetailRow(title: "Total", value: Format.moneyCompact(quote.totalCents))
                        if quote.opensDivision {
                            Text("This opens a new division in the region.").font(.caption).foregroundStyle(Theme.secondary)
                        }
                    } else {
                        ProgressView()
                    }
                    Text("The price is fixed when you order it and follows today's steel, building materials and construction prices.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    if let rejection {
                        Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                    }
                    Button {
                        Task { await build() }
                    } label: {
                        PixelLabel("Build one plant", icon: "plus")
                            .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
                    .disabled(strategy == nil || store.isBusy)
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle("Build here")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .task {
                guard let strategy else { return }
                quote = await store.buildQuote(region: opening.region, recipe: strategy.index, blocks: 1)
            }
        }
    }

    private func build() async {
        guard let strategy else { return }
        let result = await store.buildFacility(region: opening.region, recipe: strategy.index, blocks: 1)
        rejection = result
        if result == nil { dismiss() }
    }
}
