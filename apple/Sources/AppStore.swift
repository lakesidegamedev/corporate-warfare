import Foundation
import SwiftUI

@MainActor
final class AppStore: ObservableObject {
    @Published var sessionReady = false
    @Published var overview: OverviewDto?
    @Published var firms: [FirmDto] = []
    @Published var regions: [RegionDto] = []
    @Published var facilities: [FacilityDto] = []
    @Published var finance: FinanceDto?
    @Published var ownership: OwnershipDto?
    @Published var contracts: [ContractDto] = []
    @Published var loans: [LoanDto] = []
    @Published var insurance: [InsuranceDto] = []
    @Published var deals: [DealDto] = []
    @Published var events: [EventDto] = []
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
    /// Set when a slot failed to import. Drives the recovery screen.
    @Published var recoveryIssue: SaveRecoveryIssue?
    /// Set after a last-good save was accepted, so the next screen can say so once.
    @Published var recoveryNotice: String?
    /// Last engine or save error key, recorded for the diagnostics bundle only.
    @Published var lastErrorKey: String?

    private let engine = Engine()
    private let saveManager: SaveManager
    private var runTask: Task<Void, Never>?
    /// Increments whenever the run loop starts or stops, so a cancelled loop cannot clear the
    /// handle of a newer one.
    private var runGeneration = 0

    init(saveManager: SaveManager = .shared) {
        self.saveManager = saveManager
    }

    struct MarketRow: Identifiable {
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

    func resume(_ save: StoredSave, silent: Bool = false) async {
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

    func advance(weeks: UInt32) async {
        guard sessionReady else { return }
        await perform {
            if let result = await self.engine.advance(weeks: weeks) {
                if let pause = result.pause {
                    self.statusLine = "Paused: \(pause)"
                } else {
                    self.statusLine = "Advanced \(result.weeksAdvanced) week(s)."
                }
            }
            await self.reload()
            // Autosave on the four-week boundary only; background and manual saves cover the rest.
            if let overview = self.overview, overview.tick % 4 == 0 {
                await self.save(slot: SaveManager.autosaveSlot(week: overview.tick), kind: .autosave)
            }
        }
    }

    func toggleRun() {
        if runTask != nil {
            stopRunLoop()
            return
        }
        runGeneration += 1
        let generation = runGeneration
        runSpeed = 1
        runTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                await self.advance(weeks: 1)
                if self.overview?.pause != nil { break }
                try? await Task.sleep(nanoseconds: 1_000_000_000)
            }
            await MainActor.run {
                guard self.runGeneration == generation else { return }
                self.runTask = nil
                self.runSpeed = 0
            }
        }
    }

    /// Stop the run loop without touching the session. Cancellation is checked between weeks only,
    /// never inside settlement.
    func stopRunLoop() {
        runGeneration += 1
        runTask?.cancel()
        runTask = nil
        runSpeed = 0
    }

    func reload() async {
        overview = await engine.overview()
        firms = await engine.firms()
        regions = await engine.regions(selected: selectedRegionCode)
        events = await engine.events()
        deals = await engine.deals()
        groupTree = await engine.groupTree()
        institutions = await engine.institutions()
        recovery = await engine.recovery()
        fundingBuyers = await engine.fundingBuyers()
        if recipeOptions.isEmpty {
            recipeOptions = recipes()
        }
        if let selectedFirm {
            facilities = await engine.facilities(firm: selectedFirm)
            finance = await engine.finance(firm: selectedFirm)
            ownership = await engine.ownership(firm: selectedFirm)
            contracts = await engine.contracts(firm: selectedFirm)
            loans = await engine.loans(firm: selectedFirm)
            insurance = await engine.insurance(firm: selectedFirm)
            boardVotes = await engine.boardVotes(firm: selectedFirm)
            labour = await engine.labour(firm: selectedFirm)
            policyState = await engine.policyState(firm: selectedFirm)
        } else {
            facilities = []
            finance = nil
            ownership = nil
            contracts = []
            loans = []
            insurance = []
            boardVotes = []
            labour = nil
            policyState = nil
        }
        saves = saveManager.list()
        score = await engine.score()
        await loadMarkets()
    }

    func loadMarkets() async {
        let regionIndex = regions.first { $0.code == selectedRegionCode }?.index ?? regions.first?.index ?? 0
        var rows: [MarketRow] = []
        for product in products() {
            let market = await engine.market(region: regionIndex, product: product.index)
            rows.append(MarketRow(product: product, market: market))
        }
        marketRows = rows
    }

    /// Leave the week-260 score sheet and continue the same career in sandbox.
    func continueCareer() async {
        await perform {
            await self.engine.dismissPause()
            await self.reload()
        }
    }

    func selectFirm(_ handle: UInt64) async {
        selectedFirm = handle
        await reload()
    }

    func selectRegion(_ code: String?) async {
        selectedRegionCode = code
        regions = await engine.regions(selected: code)
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
        let week = overview?.tick ?? 0
        let slot = SaveManager.autosaveSlot(week: week)

        let save = Task.detached(priority: .userInitiated) { () -> StopSaveOutcome in
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

    @discardableResult
    private func save(slot: String, kind: SaveMetadata.Kind) async -> Bool {
        guard sessionReady else { return false }
        do {
            let data = try await engine.exportSave()
            let hash = await engine.canonicalHash()
            let week = overview?.tick ?? 0
            _ = try saveManager.write(
                data,
                slot: slot,
                week: week,
                canonicalHash: hash,
                kind: kind
            )
            return true
        } catch {
            lastErrorKey = ErrorKey.of(error) ?? "error.save_write_failed"
            statusLine = "Save failed. The previous save is unchanged."
            errorMessage = SaveCopy.message(forKey: lastErrorKey)
            return false
        }
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

    func payDividend(amount: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.dividend(firm: selectedFirm, amount: amount)
            self.handle(receipt)
            await self.reload()
        }
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
            await reload()
            return nil
        }
        let key = receipt.errorKey ?? "error.internal_fault"
        lastErrorDetail = receipt.errorDetail
        statusLine = key
        return CommandRejection(key: key, detail: receipt.errorDetail ?? "")
    }

    /// Investor-only commands always act as the investor, never through a subsidiary board.
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

    func previewBuild(firm: UInt64, recipe: UInt32, blocks: UInt32) async -> PreviewDto? {
        await engine.previewBuild(firm: firm, recipe: recipe, blocks: blocks)
    }

    func setPrimaryGroup(_ firm: UInt64) async -> CommandRejection? {
        await submitAsInvestor(CommandJSON.command("SetPrimaryGroup", ["firm": firm]))
    }

    func foundFirm(name: String, region: UInt32, capitalCents: Int64, recipe: UInt32) async -> CommandRejection? {
        await submitAsInvestor(CommandJSON.command("FoundFirm", [
            "name": name,
            "region": region,
            "capital_cents": capitalCents,
            "recipe": recipe
        ]))
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
