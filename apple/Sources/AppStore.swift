import Combine
import Foundation
import SwiftUI

@MainActor
final class AppStore: ObservableObject {
    @Published var sessionReady = false
    @Published var overview: OverviewDto?
    @Published var firms: [FirmDto] = []
    @Published var regions: [RegionDto] = []
    /// Every region's board (conditions, room to build, sector cells), every plant, the player's
    /// divisions for the map, the strategy catalogue and the selected firm's focus and logistics.
    @Published var boards: [RegionBoardDto] = []
    @Published var mapPlants: [MapPlantDto] = []
    @Published var mapDivisions: [MapDivisionDto] = []
    @Published var divisions: [DivisionDto] = []
    /// The division whose detail sheet is open, from the map, a region board, the CEO Office or the company page.
    @Published var openDivision: DivisionDto?
    /// Goods the selected company holds back from sale, with any open buy orders.
    @Published var held: [HeldStockDto] = []
    /// Freight lanes this week, lane contracts, and the corridor's freight at a glance.
    @Published var lanes: [LaneDto] = []
    @Published var laneContracts: [LaneContractDto] = []
    /// Groups, transfer prices, reputation, bonds, ventures and activists.
    @Published var structure: StructureViewDto?
    @Published var freight: FreightSummaryDto?
    @Published var raidTargets: [RaidOptionDto] = []
    @Published var recentRaids: [RaidRecordDto] = []
    @Published var rivals: [RivalSummaryDto] = []
    @Published var rivalFeed: [RivalMoveFeedDto] = []
    /// The board's quarterly goal for the primary company and its recent results.
    @Published var objective: ObjectiveDto?
    @Published var objectiveHistory: [ObjectiveResultDto] = []
    /// This career's chapters, board goals and achievements. Read when the Hall page is open, right
    /// after a choice, and when the week changes with time paused or at 1x; never at 2x or 4x.
    @Published var hall: HallDto?
    /// The weekly brief for the selected company. Read only while the CEO Office shows it, and at
    /// 2x or 4x no more than once a week.
    @Published var brief: BriefDto?
    var briefOpen = false
    /// The selected company's executives and the people it could hire. Read only while the CEO
    /// Office shows them.
    @Published var executives: ExecutivesDto?
    var executivesOpen = false
    /// Races, polls, laws and tenders. Read only while the Politics section is on screen.
    @Published var politics: PoliticsDto?
    var politicsOpen = false
    private var briefWeek: UInt32?
    private var briefRevision: UInt64?
    private var briefFirm: UInt64?
    var hallPageOpen = false
    private var hallStale = false
    /// The last chapter whose result the player has been shown.
    private var resultSeen: UInt32 = 0
    private var resultSeenReady = false
    @Published var strategyCatalogue: [StrategyDto] = []
    /// Products with the reference prices of the career's own scenario.
    @Published var productCatalogue: [ProductDto] = []
    /// The scenario the catalogues above were loaded for.
    private var catalogueScenario = ""
    @Published var firmOps: FirmOpsDto?
    @Published var facilities: [FacilityDto] = []
    @Published var finance: FinanceDto?
    @Published var productLines: [ProductLineDto] = []
    @Published var commodities: [CommodityDto] = []
    @Published var stocks: [StockDto] = []
    @Published var ownership: OwnershipDto?
    /// The selected company's four-week results, loaded when the stats screen opens.
    @Published var history: [PeriodDto] = []
    @Published var contracts: [ContractDto] = []
    @Published var loans: [LoanDto] = []
    @Published var insurance: [InsuranceDto] = []
    @Published var deals: [DealDto] = []
    @Published var events: [EventDto] = []
    /// The Inbox's letters: the last eight weeks, decisions pinned on top. Read with the rest of the week.
    @Published var letters: [LetterDto] = []
    /// The Inbox badge: unread letters plus decisions already read but still due.
    var inboxBadge: Int { letters.filter { !$0.read || $0.mandatory }.count }
    /// The economy, central bank and government, for the Markets economy card and the status bar.
    @Published var economy: EconomyDto?
    /// A back issue of the paper to open on the next visit, set by the History card.
    @Published var paperWeek: UInt32?
    /// The selected company's technology position.
    @Published var research: ResearchDto?
    /// What the CFO says is costing the selected company money. Empty when it is not losing money.
    @Published var cfoAdvice: [CfoAdviceDto] = []
    @Published var groupTree: [GroupNodeDto] = []
    @Published var boardVotes: [BoardVoteDto] = []
    @Published var institutions: [InstitutionDto] = []
    @Published var labour: LabourDto?
    @Published var policyState: PolicyStateDto?
    @Published var recovery: RecoveryDto?
    @Published var fundingBuyers: [BuyerDto] = []
    @Published var recipeOptions: [RecipeDto] = []
    @Published var lastErrorDetail: String?
    @Published var selectedFirm: UInt64?
    @Published var selectedRegionCode: String?
    @Published var errorMessage: String?
    @Published var statusLine: String = ""
    @Published var isBusy = false
    @Published var runSpeed: Double = 0
    @Published var saves: [StoredSave] = []
    @Published var marketRows: [MarketRow] = []
    @Published var score: ScoreDto?
    @Published var showOnboarding = false
    /// Set from Settings (Set up again): shows the first-run setup over the app.
    @Published var showSetup = false
    /// Set when a slot failed to import. Drives the recovery screen.
    @Published var recoveryIssue: SaveRecoveryIssue?
    /// Set after a last-good save was accepted, so the next screen can say so once.
    @Published var recoveryNotice: String?
    /// Last engine or save error key, recorded for the diagnostics bundle only.
    @Published var lastErrorKey: String?

    /// Tutorial progress. Its changes republish through the store so every screen redraws.
    let tutorial = TutorialModel()
    private var tutorialSink: AnyCancellable?
    private let engine = Engine()
    private let saveManager: SaveManager
    private var runTask: Task<Void, Never>?
    /// Increments whenever the run loop starts or stops, so a cancelled loop cannot clear the
    /// handle of a newer one.
    private var runGeneration = 0

    init(saveManager: SaveManager = .shared) {
        self.saveManager = saveManager
        tutorialSink = tutorial.objectWillChange.sink { [weak self] _ in
            self?.objectWillChange.send()
        }
    }

    struct MarketRow: Identifiable, Equatable {
        let product: ProductDto
        let market: MarketDto?
        var id: UInt32 { product.index }
    }

    var selectedFirmDto: FirmDto? {
        guard let selectedFirm else { return nil }
        return firms.first { $0.handle == selectedFirm }
    }

    var controlledFirms: [FirmDto] {
        firms.filter { $0.controlled }
    }

    func refreshSaves() {
        saves = saveManager.list()
    }

    /// Stop time, save, and return to the main menu. The menu's Continue resumes this save.
    func exitToMenu() async {
        stopRunLoop()
        if sessionReady, let career = await engine.hallOfCareers(), career.weeks > 0 {
            let id = await engine.scenarioId()
            let scenario = scenarios().first { $0.id == id }?.name ?? id
            CareerSummary.record(CareerSummary(title: overview?.primaryName ?? "", scenario: scenario, hall: career))
        }
        resultSeenReady = false
        hall = nil
        if let overview, sessionReady {
            await save(slot: SaveManager.autosaveSlot(week: overview.tick), kind: .autosave)
        }
        _ = await pendingWrite?.value
        saves = saveManager.list()
        sessionReady = false
        statusLine = ""
    }

    /// A result that was already waiting when the career opened is not news.
    private func noteResultsSeen(_ hall: HallDto?) {
        guard !resultSeenReady, let hall else { return }
        resultSeen = hall.chapters.last { $0.outcome == "met" || $0.outcome == "missed" }?.number ?? 0
        resultSeenReady = true
    }

    /// Read the brief now, for the panel when it opens or is unfolded.
    func refreshBrief() async {
        guard let firm = selectedFirm else { return }
        let fresh = await engine.weeklyBrief(firm: firm)
        briefWeek = overview?.tick
        briefRevision = overview?.revision
        briefFirm = firm
        if fresh != brief { brief = fresh }
    }

    /// Read the executives now, for the section when it opens or a command changed them.
    func refreshExecutives() async {
        guard let firm = selectedFirm else { return }
        let fresh = await engine.executives(firm: firm)
        if fresh != executives { executives = fresh }
    }

    func refreshPolitics() async {
        let fresh = await engine.politics(firm: selectedFirm)
        if fresh != politics { politics = fresh }
    }

    func giveToCandidate(firm: UInt64, office: Int, candidate: Int, cents: Int64) async -> CommandRejection? {
        let result = await submit(CommandJSON.command("GiveToCandidate", ["firm": firm, "office": office, "candidate": candidate, "cents": cents]))
        await refreshPolitics()
        return result
    }

    func bidOnTender(firm: UInt64, tender: Int, priceCents: Int64, weeks: Int) async -> CommandRejection? {
        let result = await submit(CommandJSON.command("BidOnTender", ["firm": firm, "tender": tender, "price_cents": priceCents, "weeks": weeks]))
        await refreshPolitics()
        return result
    }

    func hireExecutive(firm: UInt64, post: Int, person: Int) async -> CommandRejection? {
        let result = await submit(CommandJSON.command("HireExecutive", ["firm": firm, "post": post, "person": person]))
        await refreshExecutives()
        return result
    }

    func dismissExecutive(firm: UInt64, post: Int) async -> CommandRejection? {
        let result = await submit(CommandJSON.command("DismissExecutive", ["firm": firm, "post": post]))
        await refreshExecutives()
        return result
    }

    /// Read the chapter data now, for the Hall page when it opens or the week moves.
    func refreshHall() async {
        let fresh = await engine.hallOfCareers()
        noteResultsSeen(fresh)
        if fresh != hall { hall = fresh }
        score = await engine.score()
    }

    /// The board's offer for the current chapter, if one can still be taken.
    var boardOffer: ChapterDto? {
        hall?.chapters.last { $0.status == "current" && $0.chosen == nil && $0.offerWeeksLeft > 0 && !$0.offered.isEmpty }
    }

    /// The goal in force for the current chapter.
    var activeMandate: (chapter: ChapterDto, mandate: MandateDto)? {
        guard let chapter = hall?.chapters.last(where: { $0.status == "current" }),
              let picked = chapter.chosen, chapter.outcome == "open",
              chapter.offered.indices.contains(Int(picked)) else { return nil }
        return (chapter, chapter.offered[Int(picked)])
    }

    /// A chapter that closed met or missed and has not been shown yet.
    var chapterResult: ChapterDto? {
        hall?.chapters.last { ($0.outcome == "met" || $0.outcome == "missed") && $0.number > resultSeen }
    }

    func dismissChapterResult() {
        resultSeen = hall?.chapters.map(\.number).max() ?? resultSeen
        objectWillChange.send()
    }

    /// Take the board's goal in position `pick` (0 to 2) for `firm`.
    func chooseMandate(firm: UInt64, pick: Int) async -> CommandRejection? {
        hallStale = true
        return await submit(CommandJSON.command("ChooseMandate", ["firm": firm, "pick": pick]))
    }

    /// Tell the analysts what operating profit to expect this quarter, in cents.
    func giveGuidance(firm: UInt64, lowCents: Int64, highCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("GiveGuidance", ["firm": firm, "low_cents": lowCents, "high_cents": highCents]))
    }

    func restoreLatestIfAvailable() async {
        saves = saveManager.list()
        guard let latest = saves.first else { return }
        // A failed launch import leaves the app on the Start screen with a named recovery issue
        // instead of silently starting a new career over the top of the damaged slot.
        await resume(latest, silent: true)
    }

    func newCareer(guided: Bool, fiveYear: Bool, seed: UInt64) async {
        await perform {
            do {
                let overview = try await self.engine.start(seed: seed, guided: guided, fiveYear: fiveYear)
                self.apply(overview)
                await self.reload()
                await self.save(slot: SaveManager.autosaveSlot(week: overview.tick), kind: .autosave)
            } catch {
                self.lastErrorKey = ErrorKey.of(error)
                self.errorMessage = error.localizedDescription
            }
        }
    }

    /// A new career from the new-game wizard: scenario, difficulty, starting economy and capital.
    func startCareer(_ options: NewGameDto) async {
        await perform {
            do {
                let overview = try await self.engine.start(options: options)
                self.tutorial.beginCareer()
                self.apply(overview)
                await self.reload()
                await self.save(slot: SaveManager.autosaveSlot(week: overview.tick), kind: .autosave)
            } catch {
                self.lastErrorKey = ErrorKey.of(error)
                self.errorMessage = error.localizedDescription
            }
        }
    }

    /// A new career from an edited scenario. Its own difficulty, economy and capital apply.
    func startCustom(_ scenario: CustomScenarioDto, guided: Bool, seed: UInt64, fiveYear: Bool) async {
        await perform {
            do {
                let overview = try await self.engine.startCustom(scenario, guided: guided, seed: seed, fiveYear: fiveYear)
                self.tutorial.beginCareer()
                self.apply(overview)
                await self.reload()
                await self.save(slot: SaveManager.autosaveSlot(week: overview.tick), kind: .autosave)
            } catch {
                self.lastErrorKey = ErrorKey.of(error)
                self.errorMessage = error.localizedDescription
            }
        }
    }

    func resume(_ save: StoredSave, silent: Bool = false) async {
        _ = await pendingWrite?.value
        do {
            let data = try saveManager.load(save)
            let overview = try await engine.loadSave(data)
            apply(overview)
            recoveryIssue = nil
            await reload()
            statusLine = "Resumed \(save.slot) at week \(overview.tick)."
        } catch {
            // The issue names the slot from its own sidecar; a missing sidecar stays "unknown"
            // rather than claiming week zero.
            let metadata = saveManager.metadata(forSlot: save.slot)
            noteImportFailure(
                slot: save.slot,
                week: metadata?.week,
                savedAt: metadata?.savedAt,
                error: error,
                silent: silent
            )
        }
    }

    /// Load the last-good copy of the named slot. The candidate was already imported successfully
    /// by `SaveManager.recover`, and the live session is replaced only after that check passes.
    func recover(slot: String) async {
        do {
            let candidate = try saveManager.recover(slot: slot)
            let overview = try await engine.loadSave(candidate.data)
            apply(overview)
            recoveryIssue = nil
            errorMessage = nil
            await reload()
            let when = candidate.savedAt == .distantPast
                ? "an earlier session"
                : candidate.savedAt.formatted(date: .abbreviated, time: .shortened)
            let notice = "Recovered \(slot) from the last good save: week \(overview.tick), saved \(when)."
            recoveryNotice = notice
            statusLine = notice
        } catch {
            lastErrorKey = ErrorKey.of(error)
            errorMessage = SaveCopy.message(forKey: lastErrorKey)
            statusLine = error.localizedDescription
        }
    }

    func recoverLastGood() async {
        guard let issue = recoveryIssue else { return }
        await recover(slot: issue.slot)
    }

    func dismissRecoveryNotice() {
        recoveryNotice = nil
    }

    /// Settle weeks by hand (Next week). The run loop settles its weeks through `runWeek`.
    func advance(weeks: UInt32, background: Bool = false) async {
        guard sessionReady else { return }
        let body: () async -> Void = {
            let result = await self.engine.advance(weeks: weeks)
            await self.reload()
            self.staleWeeks = false
            await self.settled(result, tick: self.overview?.tick)
        }
        if background {
            await body()
        } else {
            await perform(body)
        }
    }

    /// One week of the run loop. It does not mark the app busy, so buttons stay put while time
    /// runs. With `refresh` off the week settles without republishing anything; the screens
    /// catch up on the next refreshed week. Returns true when time should stop.
    private func runWeek(refresh: Bool, speed: Double, generation: Int) async -> Bool {
        guard sessionReady else { return true }
        guard let result = await engine.advance(weeks: 1) else { return true }
        let paused = result.pause != nil
        let tick: UInt32?
        if refresh || paused {
            await reload()
            staleWeeks = false
            // Stop in the same step that shows the pause, so Continue never meets a loop that
            // is still winding down.
            if paused && runGeneration == generation {
                pausedSpeed = speed
                runTask = nil
                put(\.runSpeed, 0)
            }
            tick = overview?.tick
        } else {
            staleWeeks = true
            tick = await engine.overview()?.tick
        }
        await settled(result, tick: tick)
        return paused
    }

    /// The status line, market analytics every five weeks and the four-week autosave.
    private func settled(_ result: AdvanceDto?, tick: UInt32?) async {
        guard let result else { return }
        put(\.statusLine, result.pause != nil
            ? "Paused: \(overview?.pauseInfo?.title ?? "waiting for you")"
            : "Advanced \(result.weeksAdvanced) week(s).")
        guard let tick, result.weeksAdvanced > 0 else { return }
        let before = tick >= result.weeksAdvanced ? tick - result.weeksAdvanced : 0
        // Anonymous market-balance analytics every five weeks, in the background.
        if Telemetry.crossed(from: before, to: tick), Telemetry.enabled {
            Telemetry.send(snapshotJSON: await engine.marketSnapshotJSON())
        }
        // Autosave on the four-week boundary only; background and manual saves cover the rest.
        // The file write runs on the save queue, so the main thread never waits on the disk.
        if tick % 4 == 0 {
            await save(slot: SaveManager.autosaveSlot(week: tick), kind: .autosave, wait: false)
        }
    }

    /// Play or pause at 1x. Kept for the single play button.
    func toggleRun() {
        setSpeed(runTask == nil ? 1 : 0)
    }

    /// The fewest seconds between screen refreshes while time runs above 1x. The engine still
    /// settles every week; only publishing is coalesced, latest week wins.
    nonisolated static let runRefreshInterval = 0.4

    /// Time runs at `speed` weeks a second: 0 pauses, 1, 2 or 4 play. Changing speed while
    /// running takes effect from the next week without restarting the loop.
    ///
    /// The loop itself runs off the main actor and only hops onto it to publish a week, so at
    /// 4x the main thread stays free for navigation, sheets and the time controls.
    func setSpeed(_ speed: Double) {
        guard speed > 0 else {
            stopRunLoop()
            return
        }
        put(\.runSpeed, speed)
        if runTask != nil { return }
        runGeneration += 1
        let generation = runGeneration
        runTask = Task.detached(priority: .userInitiated) { [weak self] in
            var lastRefresh = -Double.infinity
            while !Task.isCancelled {
                guard let self else { return }
                let pace = max(await self.runSpeed, 1)
                let now = Date.timeIntervalSinceReferenceDate
                let refresh = pace <= 1 || now - lastRefresh >= AppStore.runRefreshInterval
                if refresh { lastRefresh = now }
                if await self.runWeek(refresh: refresh, speed: pace, generation: generation) { break }
                try? await Task.sleep(nanoseconds: UInt64(1_000_000_000 / pace))
            }
            await self?.runEnded(generation)
        }
    }

    /// Weeks settled without being published yet.
    private var staleWeeks = false
    /// The speed time was running at when a pause stopped it, for Continue to resume.
    private var pausedSpeed = 0.0


    private func runEnded(_ generation: Int) async {
        if runGeneration == generation {
            runTask = nil
            put(\.runSpeed, 0)
        }
        if staleWeeks && sessionReady {
            staleWeeks = false
            await reload()
        }
    }

    /// Stop the run loop without touching the session. Cancellation is checked between weeks only,
    /// never inside settlement.
    func stopRunLoop() {
        pausedSpeed = 0
        runGeneration += 1
        runTask?.cancel()
        runTask = nil
        runSpeed = 0
    }

    /// Show the investor's figures in another of the scenario's currencies and remember the choice.
    func chooseReportingCurrency(_ id: String) async {
        guard await engine.setReportingCurrency(id) else { return }
        UserDefaults.standard.set(id, forKey: Engine.reportingCurrencyKey)
        await reload()
    }

    /// Reads the whole session, then publishes it in one step. Every read happens first, off the
    /// main actor inside the engine, and only values that changed are republished, so a week
    /// costs SwiftUI one update instead of one per property, and none for screens it left alone.
    func reload() async {
        let overview = await engine.overview()
        let firms = await engine.firms()
        let regions = await engine.regions(selected: selectedRegionCode)
        let boards = await engine.regionBoards(firm: selectedFirm ?? overview?.primaryFirm)
        let mapPlants = await engine.mapPlants()
        let mapDivisions = await engine.mapDivisions()
        let recentRaids = await engine.recentRaids()
        let rivalFeed = await engine.rivalFeed(limit: 12)
        let objective = await engine.objective()
        let objectiveHistory = await engine.objectiveHistory()
        let hallDue = hallPageOpen || hallStale || runSpeed <= 1
        let hall = hallDue ? await engine.hallOfCareers() : self.hall
        if hallDue { hallStale = false }
        let briefFirmNow = selectedFirm
        let briefDue = briefOpen && briefFirmNow != nil
            && (overview?.tick != briefWeek || briefFirmNow != briefFirm
                || (overview?.revision != briefRevision && runSpeed <= 1))
        var brief = self.brief
        if briefDue, let firm = briefFirmNow {
            brief = await engine.weeklyBrief(firm: firm)
            briefWeek = overview?.tick
            briefRevision = overview?.revision
            briefFirm = firm
        }
        let scenario = await engine.scenarioId()
        var catalogues: (strategies: [StrategyDto], recipes: [RecipeDto], products: [ProductDto])?
        if strategyCatalogue.isEmpty || productCatalogue.isEmpty || scenario != catalogueScenario {
            catalogues = (await engine.strategies(), await engine.recipes(), await engine.products())
        }
        let events = await engine.events()
        let now = overview?.tick ?? 0
        let letters = await engine.letters(since: now > inboxWeeks ? now - inboxWeeks : 1)
        let economy = await engine.economy()
        let commodities = await engine.commodityBoard()
        let stocks = await engine.stockBoard()
        let deals = await engine.deals()
        let groupTree = await engine.groupTree()
        let institutions = await engine.institutions()
        let recovery = await engine.recovery()
        let fundingBuyers = await engine.fundingBuyers()
        let firm = await firmState()
        let score = await engine.score()
        let products = catalogues?.products ?? productCatalogue
        let marketCode = selectedRegionCode
            ?? selectedFirm.flatMap { handle in firms.first { $0.handle == handle } }
                .flatMap { firm in boards.first { $0.name == firm.region }?.code }
            ?? boards.first?.code
        let rows = await marketRows(code: marketCode, regions: regions, products: products)

        MainStalls.time("publish") {
            if let year = overview?.startYear { ResultBuckets.startYear = Int(year) }
            Format.configure(fx: economy?.fx)
            put(\.overview, overview)
            put(\.firms, firms)
            put(\.regions, regions)
            put(\.boards, boards)
            put(\.mapPlants, mapPlants)
            put(\.mapDivisions, mapDivisions)
            put(\.recentRaids, recentRaids)
            put(\.rivalFeed, rivalFeed)
            put(\.objective, objective)
            put(\.objectiveHistory, objectiveHistory)
            noteResultsSeen(hall)
            put(\.hall, hall)
            put(\.brief, brief)
            if let catalogues {
                put(\.strategyCatalogue, catalogues.strategies)
                put(\.recipeOptions, catalogues.recipes)
                put(\.productCatalogue, catalogues.products)
                catalogueScenario = scenario
            }
            put(\.events, events)
            put(\.letters, letters)
            put(\.economy, economy)
            put(\.commodities, commodities)
            put(\.stocks, stocks)
            put(\.deals, deals)
            put(\.groupTree, groupTree)
            put(\.institutions, institutions)
            put(\.recovery, recovery)
            put(\.fundingBuyers, fundingBuyers)
            put(\.facilities, firm.facilities)
            put(\.finance, firm.finance)
            put(\.productLines, firm.productLines)
            put(\.ownership, firm.ownership)
            put(\.history, firm.history)
            put(\.contracts, firm.contracts)
            put(\.loans, firm.loans)
            put(\.insurance, firm.insurance)
            put(\.boardVotes, firm.boardVotes)
            put(\.labour, firm.labour)
            put(\.policyState, firm.policyState)
            put(\.firmOps, firm.firmOps)
            put(\.divisions, firm.divisions)
            put(\.held, firm.held)
            put(\.lanes, firm.lanes)
            put(\.laneContracts, firm.laneContracts)
            put(\.freight, firm.freight)
            put(\.structure, firm.structure)
            put(\.raidTargets, firm.raidTargets)
            put(\.rivals, firm.rivals)
            put(\.research, firm.research)
            put(\.cfoAdvice, firm.cfoAdvice)
            put(\.score, score)
            put(\.marketRows, rows)
            if let tick = overview?.tick { tutorial.week(tick) }
        }
    }

    /// Assigns a published value only when it changed, so an unchanged screen does not redraw.
    private func put<Value: Equatable>(_ path: ReferenceWritableKeyPath<AppStore, Value>, _ value: Value) {
        if self[keyPath: path] != value { self[keyPath: path] = value }
    }

    /// Everything about the selected company, read in one pass; empty with no company selected.
    private struct FirmState {
        var facilities: [FacilityDto] = []
        var finance: FinanceDto?
        var productLines: [ProductLineDto] = []
        var ownership: OwnershipDto?
        var history: [PeriodDto] = []
        var contracts: [ContractDto] = []
        var loans: [LoanDto] = []
        var insurance: [InsuranceDto] = []
        var boardVotes: [BoardVoteDto] = []
        var labour: LabourDto?
        var policyState: PolicyStateDto?
        var firmOps: FirmOpsDto?
        var divisions: [DivisionDto] = []
        var held: [HeldStockDto] = []
        var lanes: [LaneDto] = []
        var laneContracts: [LaneContractDto] = []
        var freight: FreightSummaryDto?
        var structure: StructureViewDto?
        var raidTargets: [RaidOptionDto] = []
        var rivals: [RivalSummaryDto] = []
        var research: ResearchDto?
        var cfoAdvice: [CfoAdviceDto] = []
    }

    private func firmState() async -> FirmState {
        var state = FirmState()
        guard let selectedFirm else { return state }
        state.facilities = await engine.facilities(firm: selectedFirm)
        state.finance = await engine.finance(firm: selectedFirm)
        state.productLines = await engine.productLines(firm: selectedFirm)
        state.ownership = await engine.ownership(firm: selectedFirm)
        state.history = await engine.history(firm: selectedFirm)
        state.contracts = await engine.contracts(firm: selectedFirm)
        state.loans = await engine.loans(firm: selectedFirm)
        state.insurance = await engine.insurance(firm: selectedFirm)
        state.boardVotes = await engine.boardVotes(firm: selectedFirm)
        state.labour = await engine.labour(firm: selectedFirm)
        state.policyState = await engine.policyState(firm: selectedFirm)
        state.firmOps = await engine.firmOps(firm: selectedFirm)
        state.divisions = await engine.divisions(firm: selectedFirm)
        state.held = await engine.heldStock(firm: selectedFirm)
        state.lanes = await engine.lanes()
        state.laneContracts = await engine.laneContracts()
        state.freight = await engine.freightSummary()
        state.structure = await engine.structureView()
        state.raidTargets = await engine.raidTargets(firm: selectedFirm)
        state.rivals = await engine.rivals(firm: selectedFirm)
        state.research = await engine.research(firm: selectedFirm)
        state.cfoAdvice = await engine.cfoAdvice(firm: selectedFirm)
        return state
    }

    /// The region Markets shows: the chosen one, else the selected company's home region.
    var marketRegionCode: String? {
        selectedRegionCode
            ?? selectedFirmDto.flatMap { firm in boards.first { $0.name == firm.region }?.code }
            ?? boards.first?.code
    }

    func loadHistory() async {
        if let selectedFirm {
            history = await engine.history(firm: selectedFirm)
        } else {
            history = []
        }
    }

    func loadMarkets() async {
        let rows = await marketRows(code: marketRegionCode, regions: regions, products: productCatalogue)
        put(\.marketRows, rows)
    }

    private func marketRows(code: String?, regions: [RegionDto], products: [ProductDto]) async -> [MarketRow] {
        let regionIndex = regions.first { $0.code == code }?.index ?? regions.first?.index ?? 0
        var rows: [MarketRow] = []
        for product in products {
            let market = await engine.market(region: regionIndex, product: product.index)
            rows.append(MarketRow(product: product, market: market))
        }
        return rows
    }

    /// Leave the week-260 score sheet and continue the same career in sandbox.
    /// Continue past a pause. When the pause stopped running time, time runs on at the same speed.
    func continueCareer() async {
        let resume = pausedSpeed
        pausedSpeed = 0
        await perform {
            await self.engine.dismissPause()
            await self.reload()
        }
        if resume > 0 && overview?.pause == nil && sessionReady { setSpeed(resume) }
    }

    func selectFirm(_ handle: UInt64) async {
        selectedFirm = handle
        await reload()
    }

    func selectRegion(_ code: String?) async {
        selectedRegionCode = code
        regions = await engine.regions(selected: code)
        await loadMarkets()
    }

    // MARK: Lifecycle

    /// Scene-phase handling. Background asks for one bounded stop-and-save; foreground only
    /// re-reads the live session. No command is ever replayed on return: every accepted command
    /// already has a committed tick, and the run loop stays stopped until the player restarts it.
    func handleScenePhase(_ phase: ScenePhase) async {
        switch phase {
        case .background:
            stopRunLoop()
            await suspendAndSave()
        case .active:
            if sessionReady {
                await reload()
            }
        case .inactive:
            break
        @unknown default:
            break
        }
    }

    /// One bounded stop-and-save. The export and write run in an unstructured task; the caller
    /// stops waiting at the deadline while the atomic write path still guarantees that the
    /// previous save on disk stays complete and readable.
    func suspendAndSave(timeoutSeconds: Double = 3) async {
        guard sessionReady else { return }
        let engine = self.engine
        let saveManager = self.saveManager
        let week = overview?.tick ?? 0
        let slot = SaveManager.autosaveSlot(week: week)

        let previous = pendingWrite
        let save = Task.detached(priority: .userInitiated) { () -> StopSaveOutcome in
            // An autosave still on the save queue lands first, so it cannot overwrite this one.
            _ = await previous?.value
            do {
                let data = try await engine.exportSave()
                let hash = await engine.canonicalHash()
                try saveManager.write(
                    data,
                    slot: slot,
                    week: week,
                    canonicalHash: hash,
                    kind: .autosave
                )
                return .saved
            } catch {
                return .failed(ErrorKey.of(error) ?? "error.save_write_failed")
            }
        }

        switch await firstOutcome(of: save, timeoutSeconds: timeoutSeconds) {
        case .saved:
            statusLine = "Saved at week \(week) when the app left the foreground."
        case .failed(let key):
            lastErrorKey = key
            statusLine = "Stop save failed. The previous save is unchanged."
        case .timedOut:
            statusLine = "Stop save is still running. The previous save is unchanged."
        }
    }

    enum StopSaveOutcome: Sendable {
        case saved
        case failed(String)
        case timedOut
    }

    /// Resolves to whichever finishes first. A save that loses the race keeps running, so an
    /// atomic write already in progress can still complete after the app leaves the foreground.
    private func firstOutcome(
        of save: Task<StopSaveOutcome, Never>,
        timeoutSeconds: Double
    ) async -> StopSaveOutcome {
        await withCheckedContinuation { continuation in
            let gate = OutcomeGate(continuation)
            Task.detached(priority: .userInitiated) {
                gate.finish(await save.value)
            }
            Task.detached(priority: .utility) {
                try? await Task.sleep(nanoseconds: UInt64(timeoutSeconds * 1_000_000_000))
                gate.finish(.timedOut)
            }
        }
    }

    // MARK: Saves

    func manualSave() async {
        guard sessionReady else { return }
        let slot = SaveManager.manualSlot()
        let written = await save(slot: slot, kind: .manual)
        saveManager.pruneManualSlots()
        saves = saveManager.list()
        if written {
            statusLine = "Saved \(slot)."
        }
    }

    /// The last save file write handed to the save queue. Writes run in the order they were made.
    private var pendingWrite: Task<String?, Never>?

    /// Export on the engine, then write the file off the main actor behind any earlier write.
    /// With `wait` off the call returns once the bytes are exported and a failure is reported
    /// when the write finishes.
    @discardableResult
    private func save(slot: String, kind: SaveMetadata.Kind, wait: Bool = true) async -> Bool {
        guard sessionReady else { return false }
        let data: Data
        let hash: String
        do {
            data = try await engine.exportSave()
            hash = await engine.canonicalHash()
        } catch {
            saveFailed(ErrorKey.of(error) ?? "error.save_write_failed")
            return false
        }
        let week = overview?.tick ?? 0
        let manager = saveManager
        let previous = pendingWrite
        let write = Task.detached(priority: .userInitiated) { () -> String? in
            _ = await previous?.value
            do {
                try manager.write(data, slot: slot, week: week, canonicalHash: hash, kind: kind)
                return nil
            } catch {
                return ErrorKey.of(error) ?? "error.save_write_failed"
            }
        }
        pendingWrite = write
        guard wait else {
            Task { [weak self] in
                if let failure = await write.value { self?.saveFailed(failure) }
            }
            return true
        }
        if let failure = await write.value {
            saveFailed(failure)
            return false
        }
        return true
    }

    private func saveFailed(_ key: String) {
        lastErrorKey = key
        statusLine = "Save failed. The previous save is unchanged."
        errorMessage = SaveCopy.message(forKey: lastErrorKey)
    }

    /// An import failed. The failing file is copied aside and left in place; the recovery screen
    /// names the slot, week and timestamp and offers the last-good copy when one exists.
    private func noteImportFailure(
        slot: String,
        week: UInt32?,
        savedAt: Date?,
        error: Error,
        silent: Bool
    ) {
        let key = ErrorKey.of(error) ?? "error.invalid_save"
        lastErrorKey = key
        saveManager.markUnreadable(slot: slot)
        let preserved = saveManager.preserveFailingCopy(slot: slot)
        recoveryIssue = SaveRecoveryIssue(
            slot: slot,
            week: week,
            savedAt: savedAt,
            errorKey: key,
            hasLastGood: saveManager.hasLastGood(slot: slot),
            preservedCopy: preserved
        )
        if !silent {
            errorMessage = SaveCopy.message(forKey: key)
        }
        statusLine = "Could not open \(slot)."
    }

    // MARK: Diagnostics

    /// Build the local diagnostics file. Nothing leaves the device: the player reviews the summary
    /// and chooses whether to share the file. Returning the value lets the presenting screen own
    /// its own sheet instead of racing a single app-wide presentation.
    func makeDiagnostics() -> DiagnosticExport? {
        let info = engineInfo()
        let saves = saveManager.list()
        let bundle = DiagnosticBundle(
            generatedAt: SaveMetadata.string(from: Date()),
            appVersion: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "unknown",
            buildNumber: Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "unknown",
            engineVersion: info.version,
            interfaceVersion: info.interfaceVersion,
            rulesVersion: info.rulesVersion,
            datasetHash: info.datasetHash,
            lastErrorKey: lastErrorKey,
            recoverySlot: recoveryIssue?.slot,
            recoveryWeek: recoveryIssue?.week,
            saves: saves.map(DiagnosticBundle.entry),
            notes: [
                "Offline build: the app makes no network calls.",
                "This file is written locally and shared only when the player chooses.",
                "Damaged saves are copied to Recovery/ and never deleted automatically.",
            ]
        )
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(bundle)
            let url = try saveManager.writeDiagnosticBundle(data)
            return DiagnosticExport(url: url, summary: bundle.reviewSummary)
        } catch {
            errorMessage = "The diagnostics file could not be written."
            return nil
        }
    }

    // MARK: Actions

    func setPrice(product: UInt32, price: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.setPrice(firm: selectedFirm, product: product, price: price)
            self.handle(receipt)
            if receipt?.accepted == true { self.tutorial.sent("SetPrice") }
            await self.reload()
        }
    }

    func setWage(tier: UInt32, wage: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.setWage(firm: selectedFirm, tier: tier, wage: wage)
            self.handle(receipt)
            await self.reload()
        }
    }

    func setTarget(_ bps: Int32) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.setTarget(firm: selectedFirm, bps: bps)
            self.handle(receipt)
            await self.reload()
        }
    }

    func build(region: UInt32, recipe: UInt32) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.build(firm: selectedFirm, region: region, recipe: recipe)
            self.handle(receipt)
            if receipt?.accepted == true { self.tutorial.sent("BuildFacility") }
            await self.reload()
        }
    }

    func buildFacility(region: UInt32, recipe: UInt32, blocks: UInt32) async -> CommandRejection? {
        guard let selectedFirm else {
            return CommandRejection(key: "error.invalid_entity", detail: "")
        }
        return await submit(CommandJSON.command("BuildFacility", [
            "firm": selectedFirm,
            "region": region,
            "recipe": recipe,
            "blocks": blocks
        ]))
    }

    /// When the statements, charts and systems displays reload. Paused, after every command and
    /// week; while time runs, every four weeks, so redrawing their charts never holds up the main
    /// thread at top speed.
    var historyRefreshKey: String {
        guard let overview else { return "none" }
        return runSpeed > 0 ? "run-\(overview.tick / 4)" : "rev-\(overview.revision)"
    }

    /// Read from the engine off the main actor: statements, history and the systems displays.
    func read<T>(_ block: (SessionHandle) -> T) async -> T? {
        await engine.read(block)
    }

    /// A company's full page, with what the investor may see of it.
    func companyPage(_ firm: UInt64) async -> CompanyPageDto? {
        await engine.companyPage(firm: firm)
    }

    /// A company's chief executive and recent moves.
    func dossier(_ firm: UInt64) async -> DossierDto? {
        await engine.dossier(firm: firm)
    }

    /// A rival's chief, temperament and any grudge or tie with the selected company.
    func rivalTag(_ firm: UInt64) async -> RivalTagDto? {
        guard let you = selectedFirm ?? overview?.primaryFirm else { return nil }
        return await engine.rivalTag(firm: firm, you: you)
    }

    /// Whether the selected company can raid `target` in a market, and why not when it cannot.
    func raidCheck(target: UInt64, region: String, industry: String) async -> RaidCheckDto? {
        guard let firm = selectedFirm ?? overview?.primaryFirm else { return nil }
        return await engine.raidCheck(firm: firm, target: target, region: region, industry: industry)
    }

    /// One division's (or, with `facility` zero, the whole company's) freight for the week just
    /// closed, with each shipping policy priced.
    func shipping(facility: UInt64) async -> ShippingSummaryDto? {
        guard let firm = selectedFirm else { return nil }
        return await engine.shippingSummary(firm: firm, facility: facility)
    }

    /// What a lane reservation would cost and save, before any command is sent.
    func reserveQuote(lane: LaneDto, lots: Int32, weeks: UInt32) async -> ReserveQuoteDto? {
        await engine.reserveQuote(a: lane.a, b: lane.b, mode: lane.mode, lots: lots, weeks: weeks)
    }

    func buildQuote(region: UInt32, recipe: UInt32, blocks: UInt32) async -> BuildQuoteDto? {
        guard let firm = selectedFirm ?? overview?.primaryFirm else { return nil }
        return await engine.buildQuote(firm: firm, region: region, recipe: recipe, blocks: blocks)
    }

    func raidQuote(_ option: RaidOptionDto, budgetCents: Int64) async -> RaidQuoteDto? {
        guard let firm = selectedFirm ?? overview?.primaryFirm else { return nil }
        return await engine.raidQuote(
            firm: firm, target: option.target, region: option.regionIndex,
            industry: option.industryIndex, budgetCents: budgetCents
        )
    }

    /// Launch a market raid on a rival division; the outcome lands in `recentRaids`.
    func raid(_ option: RaidOptionDto, budgetCents: Int64) async -> CommandRejection? {
        guard let firm = selectedFirm ?? overview?.primaryFirm else {
            return CommandRejection(key: "error.invalid_entity", detail: "")
        }
        return await submit(CommandJSON.command("RaidMarket", [
            "firm": firm,
            "target": option.target,
            "region": option.regionIndex,
            "industry": option.industryIndex,
            "budget_cents": budgetCents
        ]))
    }

    func payDividend(amount: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.dividend(firm: selectedFirm, amount: amount)
            self.handle(receipt)
            await self.reload()
        }
    }

    /// Opening a letter marks it read; the engine keeps the flag and the list updates in place.
    func markLettersRead(_ ids: [String]) async {
        guard !ids.isEmpty else { return }
        letters = letters.map { letter in
            guard ids.contains(letter.id), !letter.read else { return letter }
            var read = letter
            read.read = true
            return read
        }
        await engine.markLettersRead(ids)
    }

    func respond(event: UInt64, choice: String) async {
        await perform {
            let json = CommandJSON.command("RespondEvent", ["event": event, "choice": choice])
            let receipt = await self.engine.submit(json: json)
            self.handle(receipt)
            await self.reload()
        }
    }

    func orderShares(firm: UInt64, buy: Bool, quantity: Int64, limit: Int64) async {
        await perform {
            let json = CommandJSON.command("PlaceShareOrder", [
                "investor": true,
                "firm": firm,
                "buy": buy,
                "qty": quantity,
                "limit_price_cents": limit
            ])
            let receipt = await self.engine.submit(json: json)
            self.handle(receipt)
            await self.reload()
        }
    }

    private func apply(_ overview: OverviewDto) {
        self.overview = overview
        self.selectedFirm = overview.primaryFirm
        self.sessionReady = true
    }

    // MARK: Catalogue commands

    /// The firm whose authority acts for the selected company.
    ///
    /// A directly held firm is operated by the investor (`0`). A subsidiary is operated by its
    /// controlling parent firm, which is the same authority check the simulator applies.
    var legalActor: UInt64 {
        guard let selectedFirm,
              let node = groupTree.first(where: { $0.handle == selectedFirm }) else {
            return 0
        }
        if node.directlyControlled { return 0 }
        return node.parent ?? 0
    }

    /// True when the selected firm is operated by a parent rather than the investor directly.
    var actingThroughParent: FirmDto? {
        guard legalActor != 0 else { return nil }
        return firms.first { $0.handle == legalActor }
    }

    /// Submit one intent and return its typed rejection, or nil when accepted.
    /// The engine's detail report for one division of the selected company.
    func divisionDetail(_ division: DivisionDto) async -> DivisionDetailDto? {
        guard let firm = selectedFirm ?? overview?.primaryFirm else { return nil }
        return await engine.divisionDetail(firm: firm, region: division.region, industry: division.industry)
    }

    func submit(_ json: String, actor: UInt64? = nil) async -> CommandRejection? {
        guard sessionReady else {
            return CommandRejection(key: "error.internal_fault", detail: "")
        }
        isBusy = true
        defer { isBusy = false }
        guard let receipt = await engine.submit(json: json, actor: actor ?? legalActor) else {
            return CommandRejection(key: "error.internal_fault", detail: "")
        }
        if receipt.accepted {
            errorMessage = nil
            lastErrorDetail = nil
            statusLine = "Accepted at revision \(receipt.revision)."
            tutorial.sent(CommandJSON.name(of: json))
            await reload()
            return nil
        }
        let key = receipt.errorKey ?? "error.internal_fault"
        lastErrorDetail = receipt.errorDetail
        statusLine = key
        return CommandRejection(key: key, detail: receipt.errorDetail ?? "")
    }

    /// Investor-only commands always act as the investor, never through a subsidiary board.
    /// Press a CFO button: send its commands in order, stopping at the first the engine refuses.
    func applyAdvice(_ advice: CfoAdviceDto) async -> CommandRejection? {
        for json in advice.commands {
            if let rejection = await submit(json) { return rejection }
        }
        statusLine = "Done: \(advice.actionLabel.lowercased())."
        return nil
    }

    func submitAsInvestor(_ json: String) async -> CommandRejection? {
        await submit(json, actor: 0)
    }

    /// Ask the session for a labelled preview. Never reserves or mutates.
    func preview(_ json: String) async -> PreviewDto? {
        await engine.previewJson(json)
    }

    func previewIssue(firm: UInt64, quantity: Int64, priceCents: Int64) async -> PreviewDto? {
        await engine.previewIssue(firm: firm, qty: quantity, priceCents: priceCents)
    }

    func previewBuild(firm: UInt64, region: UInt32, recipe: UInt32, blocks: UInt32) async -> PreviewDto? {
        await engine.previewBuild(firm: firm, region: region, recipe: recipe, blocks: blocks)
    }

    func setPrimaryGroup(_ firm: UInt64) async -> CommandRejection? {
        await submitAsInvestor(CommandJSON.command("SetPrimaryGroup", ["firm": firm]))
    }

    func foundFirm(name: String, region: UInt32, capitalCents: Int64, recipe: UInt32, secondary: Int? = nil) async -> CommandRejection? {
        var fields: [String: Any] = [
            "name": name,
            "region": region,
            "capital_cents": capitalCents,
            "recipe": recipe
        ]
        if let secondary { fields["secondary"] = secondary }
        return await submitAsInvestor(CommandJSON.command("FoundFirm", fields))
    }

    func acceptManagementContract() async -> CommandRejection? {
        await submitAsInvestor(CommandJSON.unit("AcceptManagementContract"))
    }

    func callMeeting(firm: UInt64) async -> CommandRejection? {
        await submit(CommandJSON.command("CallMeeting", ["firm": firm]))
    }

    func nominateCEO(firm: UInt64, investor: Bool) async -> CommandRejection? {
        await submit(CommandJSON.command("NominateCEO", ["firm": firm, "investor": investor]))
    }

    func castVote(firm: UInt64, voteId: UInt32, approve: Bool) async -> CommandRejection? {
        await submit(CommandJSON.command("CastVote", [
            "firm": firm,
            "vote_id": voteId,
            "approve": approve
        ]))
    }

    func proposeBoardAction(firm: UInt64, subject: String) async -> CommandRejection? {
        await submit(CommandJSON.command("ProposeBoardAction", ["firm": firm, "subject": subject]))
    }

    func respondTender(deal: UInt64, accept: Bool) async -> CommandRejection? {
        await submitAsInvestor(CommandJSON.command("RespondTender", ["deal": deal, "accept": accept]))
    }

    func startDiligence(buyer: UInt64, target: UInt64, kind: String, priceCents: Int64, maxShares: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("StartDiligence", [
            "buyer": buyer,
            "target": target,
            "kind": kind,
            "price_cents": priceCents,
            "max_shares": maxShares
        ]))
    }

    func fundTender(deal: UInt64) async -> CommandRejection? {
        await submit(CommandJSON.command("FundTender", ["deal": deal]))
    }

    func withdrawOffer(deal: UInt64) async -> CommandRejection? {
        await submit(CommandJSON.command("WithdrawOffer", ["deal": deal]))
    }

    func proposeMerger(deal: UInt64) async -> CommandRejection? {
        await submit(CommandJSON.command("ProposeMerger", ["deal": deal]))
    }

    func submitRemedy(deal: UInt64, divestShareBps: Int32) async -> CommandRejection? {
        await submit(CommandJSON.command("SubmitRemedy", [
            "deal": deal,
            "divest_share_bps": divestShareBps
        ]))
    }

    func issueShares(firm: UInt64, quantity: Int64, priceCents: Int64, buyerOutside: UInt32) async -> CommandRejection? {
        await submit(CommandJSON.command("IssueShares", [
            "firm": firm,
            "qty": quantity,
            "price_cents": priceCents,
            "buyer_outside": buyerOutside
        ]))
    }

    func repurchaseShares(firm: UInt64, quantity: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("RepurchaseShares", ["firm": firm, "qty": quantity]))
    }

    func retireTreasury(firm: UInt64, quantity: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("RetireTreasury", ["firm": firm, "qty": quantity]))
    }

    func launchIPO(firm: UInt64, newShares: Int64, priceCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("LaunchIPO", [
            "firm": firm,
            "new_shares": newShares,
            "price_cents": priceCents
        ]))
    }

    func requestLoan(firm: UInt64, amountCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("RequestLoan", ["firm": firm, "amount_cents": amountCents]))
    }

    func issueBond(firm: UInt64, principalCents: Int64, couponBps: Int32, weeks: UInt32) async -> CommandRejection? {
        await submit(CommandJSON.command("IssueBond", [
            "firm": firm,
            "principal_cents": principalCents,
            "coupon_bps": couponBps,
            "weeks": weeks
        ]))
    }

    func setDepositRate(bank: UInt64, rateBps: Int32) async -> CommandRejection? {
        await submit(CommandJSON.command("SetDepositRate", ["bank": bank, "rate_bps": rateBps]))
    }

    func depositFunds(bank: UInt64, firm: UInt64, amountCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("DepositFunds", [
            "bank": bank,
            "firm": firm,
            "amount_cents": amountCents
        ]))
    }

    func withdrawFunds(bank: UInt64, firm: UInt64, amountCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("WithdrawFunds", [
            "bank": bank,
            "firm": firm,
            "amount_cents": amountCents
        ]))
    }

    func approveLoan(bank: UInt64, borrower: UInt64, amountCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("ApproveLoan", [
            "bank": bank,
            "borrower": borrower,
            "amount_cents": amountCents
        ]))
    }

    func quotePolicy(insurer: UInt64, facility: UInt64, limitCents: Int64, deductibleCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("QuotePolicy", [
            "insurer": insurer,
            "facility": facility,
            "limit_cents": limitCents,
            "deductible_cents": deductibleCents
        ]))
    }

    func bindPolicy(insurer: UInt64, insured: UInt64, facility: UInt64, limitCents: Int64, deductibleCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("BindPolicy", [
            "insurer": insurer,
            "insured": insured,
            "facility": facility,
            "limit_cents": limitCents,
            "deductible_cents": deductibleCents
        ]))
    }

    func cancelPolicy(policy: UInt64) async -> CommandRejection? {
        await submit(CommandJSON.command("CancelPolicy", ["policy": policy]))
    }

    func setUnderwritingSpecialist(insurer: UInt64, specialist: Bool) async -> CommandRejection? {
        await submit(CommandJSON.command("SetUnderwritingLimit", [
            "insurer": insurer,
            "specialist": specialist
        ]))
    }

    func respondBargaining(firm: UInt64, choice: String, offerCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("RespondBargaining", [
            "firm": firm,
            "choice": choice,
            "offer_cents": offerCents
        ]))
    }

    func proposeSettlement(firm: UInt64, amountCents: Int64) async -> CommandRejection? {
        await submit(CommandJSON.command("ProposeSettlement", [
            "firm": firm,
            "amount_cents": amountCents
        ]))
    }

    func fileLobbyRequest(firm: UInt64, region: UInt32, kind: String) async -> CommandRejection? {
        await submit(CommandJSON.command("FileLobbyRequest", [
            "firm": firm,
            "region": region,
            "request": kind
        ]))
    }

    /// Loans, contracts, policies and votes of one company and every open deal, for the pickers
    /// in the Command Center forms.
    func pickers(firm: UInt64?) async -> ActionPickers {
        guard let firm else {
            return ActionPickers(deals: await engine.deals(), extra: await engine.structurePickMap())
        }
        return ActionPickers(
            loans: await engine.loans(firm: firm),
            contracts: await engine.contracts(firm: firm),
            policies: await engine.insurance(firm: firm),
            votes: await engine.boardVotes(firm: firm),
            deals: await engine.deals(),
            extra: await engine.structurePickMap()
        )
    }

    func insuranceQuote(insurer: UInt64, facility: UInt64, limitCents: Int64) async -> QuoteDto? {
        await engine.insuranceQuote(insurer: insurer, facility: facility, limit: limitCents)
    }

    private func handle(_ receipt: ReceiptDto?) {
        guard let receipt else { return }
        if receipt.accepted {
            statusLine = "Accepted at revision \(receipt.revision)."
            errorMessage = nil
            lastErrorDetail = nil
        } else {
            lastErrorKey = receipt.errorKey
            let key = receipt.errorKey ?? "error.internal_fault"
            errorMessage = Remedy.title(for: key)
            lastErrorDetail = receipt.errorDetail
            statusLine = receipt.errorDetail ?? key
        }
    }

    private func perform(_ block: @escaping () async -> Void) async {
        isBusy = true
        await block()
        isBusy = false
    }
}

/// Resumes a continuation exactly once from whichever racing task finishes first. The losing task
/// is not cancelled: a stop-save that outlives the deadline still finishes its atomic write.
private final class OutcomeGate: @unchecked Sendable {
    private let lock = NSLock()
    private var finished = false
    private let continuation: CheckedContinuation<AppStore.StopSaveOutcome, Never>

    init(_ continuation: CheckedContinuation<AppStore.StopSaveOutcome, Never>) {
        self.continuation = continuation
    }

    func finish(_ outcome: AppStore.StopSaveOutcome) {
        lock.lock()
        if finished {
            lock.unlock()
            return
        }
        finished = true
        lock.unlock()
        continuation.resume(returning: outcome)
    }
}
