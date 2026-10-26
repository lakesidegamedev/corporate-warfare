import Foundation

/// Serial engine owner. Every Rust call that can mutate the world happens here, never on the
/// main actor, and the UI only receives immutable DTOs.
actor Engine {
    private var session: SessionHandle?

    var isReady: Bool { session != nil }

    func start(seed: UInt64, guided: Bool, fiveYear: Bool) throws -> OverviewDto {
        let handle = try SessionHandle(seed: seed, guided: guided, fiveYear: fiveYear)
        session = handle
        return handle.overview()
    }

    func loadSave(_ data: Data) throws -> OverviewDto {
        // The engine builds the new world before the old handle is released, so a failed import
        // leaves the live session exactly as it was.
        let handle = try importSave(bytes: data)
        session = handle
        return handle.overview()
    }

    func overview() -> OverviewDto? {
        session?.overview()
    }

    func advance(weeks: UInt32) -> AdvanceDto? {
        session?.advance(weeks: weeks)
    }

    func firms() -> [FirmDto] {
        session?.firms() ?? []
    }

    func regionBoards(firm: UInt64?) -> [RegionBoardDto] {
        session?.regionBoards(firm: firm) ?? []
    }

    func divisions(firm: UInt64) -> [DivisionDto] {
        session?.divisions(firm: firm) ?? []
    }

    func mapDivisions() -> [MapDivisionDto] {
        session?.mapDivisions() ?? []
    }

    func buildQuote(firm: UInt64, region: UInt32, recipe: UInt32, blocks: UInt32) -> BuildQuoteDto? {
        session?.buildQuote(firm: firm, region: region, recipe: recipe, blocks: blocks)
    }

    func raidTargets(firm: UInt64) -> [RaidOptionDto] {
        session?.raidTargets(firm: firm) ?? []
    }

    func raidQuote(firm: UInt64, target: UInt64, region: UInt32, industry: UInt32, budgetCents: Int64) -> RaidQuoteDto? {
        session?.raidQuote(firm: firm, target: target, region: region, industry: industry, budgetCents: budgetCents)
    }

    func marketSnapshotJSON() -> String {
        session?.marketSnapshotJson() ?? ""
    }

    func recentRaids() -> [RaidRecordDto] {
        session?.recentRaids() ?? []
    }

    func objective() -> ObjectiveDto? {
        session?.objective()
    }

    func objectiveHistory() -> [ObjectiveResultDto] {
        session?.objectiveHistory() ?? []
    }

    func mapPlants() -> [MapPlantDto] {
        session?.mapPlants() ?? []
    }

    func firmOps(firm: UInt64) -> FirmOpsDto? {
        session?.firmOps(firm: firm)
    }

    func facilities(firm: UInt64) -> [FacilityDto] {
        session?.facilities(firm: firm) ?? []
    }

    func regions(selected: String?) -> [RegionDto] {
        session?.regions(selectedCode: selected) ?? []
    }

    func finance(firm: UInt64) -> FinanceDto? {
        session?.finance(firm: firm)
    }

    func ownership(firm: UInt64) -> OwnershipDto? {
        session?.ownership(firm: firm)
    }

    func contracts(firm: UInt64) -> [ContractDto] {
        session?.contracts(firm: firm) ?? []
    }

    func loans(firm: UInt64) -> [LoanDto] {
        session?.loans(firm: firm) ?? []
    }

    func insurance(firm: UInt64) -> [InsuranceDto] {
        session?.insurance(firm: firm) ?? []
    }

    func market(region: UInt32, product: UInt32) -> MarketDto? {
        session?.market(region: region, product: product)
    }

    func deals() -> [DealDto] {
        session?.deals() ?? []
    }

    func events() -> [EventDto] {
        session?.events() ?? []
    }

    func groupTree() -> [GroupNodeDto] {
        session?.groupTree() ?? []
    }

    func boardVotes(firm: UInt64) -> [BoardVoteDto] {
        session?.boardVotes(firm: firm) ?? []
    }

    func labour(firm: UInt64) -> LabourDto? {
        session?.labour(firm: firm)
    }

    func policyState(firm: UInt64) -> PolicyStateDto? {
        session?.policyState(firm: firm)
    }

    func institutions() -> [InstitutionDto] {
        session?.institutions() ?? []
    }

    func recovery() -> RecoveryDto? {
        session?.recovery()
    }

    func fundingBuyers() -> [BuyerDto] {
        session?.fundingBuyers() ?? []
    }

    func insuranceQuote(insurer: UInt64, facility: UInt64, limit: Int64) -> QuoteDto? {
        session?.insuranceQuote(insurer: insurer, facility: facility, limitCents: limit)
    }

    func previewIssue(firm: UInt64, qty: Int64, priceCents: Int64) -> PreviewDto? {
        session?.previewIssue(firm: firm, qty: qty, priceCents: priceCents)
    }

    func previewBuild(firm: UInt64, region: UInt32, recipe: UInt32, blocks: UInt32) -> PreviewDto? {
        session?.previewBuild(firm: firm, region: region, recipe: recipe, blocks: blocks)
    }

    func previewJson(_ json: String) -> PreviewDto? {
        session?.previewJson(json: json)
    }

    func setPrice(firm: UInt64, product: UInt32, price: Int64) -> ReceiptDto? {
        session?.setPrice(firm: firm, product: product, priceCents: price)
    }

    func setWage(firm: UInt64, tier: UInt32, wage: Int64) -> ReceiptDto? {
        session?.setWageOffer(firm: firm, tier: tier, wageCents: wage)
    }

    func setTarget(firm: UInt64, bps: Int32) -> ReceiptDto? {
        session?.setProductionTarget(firm: firm, targetBps: bps)
    }

    func build(firm: UInt64, region: UInt32, recipe: UInt32) -> ReceiptDto? {
        session?.buildFacility(firm: firm, region: region, recipe: recipe, blocks: 1)
    }

    func dividend(firm: UInt64, amount: Int64) -> ReceiptDto? {
        session?.payDividend(firm: firm, amountCents: amount)
    }

    func submit(json: String, actor: UInt64 = 0) -> ReceiptDto? {
        session?.submitJsonAs(actor: actor, json: json)
    }

    func exportSave() throws -> Data {
        guard let session else { throw SaveError.noSession }
        let bytes = try session.exportSave()
        return Data(bytes)
    }

    func score() -> ScoreDto? {
        session?.score()
    }

    func dismissPause() {
        session?.dismissPause()
    }

    func canonicalHash() -> String {
        session?.canonicalHash() ?? ""
    }
}

enum SaveError: LocalizedError {
    case noSession
    case writeFailed

    var errorDescription: String? {
        switch self {
        case .noSession: return "No career is open."
        case .writeFailed: return "The save could not be written."
        }
    }
}

struct StoredSave: Identifiable, Hashable {
    let id: String
    let url: URL
    let week: UInt32
    let savedAt: Date
    let slot: String
    let hasLastGood: Bool
}

/// Atomic temp-write + flush + replace with a rolling last-good copy per slot, three autosave
/// slots rotated by week and a bounded manual slot list, per `docs/rules/INTERFACE.md`.
///
/// Write path: the payload is written to `<slot>.tmp`, flushed to disk, the current file is
/// promoted to `<slot>.lastgood` (staged and swapped, so a failure keeps the older copy), then the
/// temporary file replaces the current one. A crash anywhere before the final replace leaves the
/// previous save readable.
struct SaveManager {
    static let shared = SaveManager()

    /// Autosave every four weeks across three slots, so the last three checkpoints survive.
    static let autosaveSlotCount = 3
    /// Manual slots retained; the oldest is pruned after a new manual save.
    static let manualSlotLimit = 10

    private let root: URL?

    /// One save is written at a time. The stop-save runs off the main actor, so two save paths
    /// could otherwise share the same `<slot>.tmp` file.
    private static let writeLock = NSLock()

    /// `directory` is injectable so tests use real files in a temporary folder instead of the
    /// app container.
    init(directory: URL? = nil) {
        root = directory
    }

    var directory: URL {
        if let root { return root }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return base.appendingPathComponent("CorporateWarfare", isDirectory: true)
    }

    func url(forSlot slot: String) -> URL {
        directory.appendingPathComponent("\(slot).cwsave")
    }

    func lastGoodURL(forSlot slot: String) -> URL {
        directory.appendingPathComponent("\(slot).lastgood")
    }

    func metadataURL(forSlot slot: String) -> URL {
        directory.appendingPathComponent("\(slot).json")
    }

    func lastGoodMetadataURL(forSlot slot: String) -> URL {
        directory.appendingPathComponent("\(slot).lastgood.json")
    }

    func temporaryURL(forSlot slot: String) -> URL {
        directory.appendingPathComponent("\(slot).tmp")
    }

    func markerURL(forSlot slot: String) -> URL {
        directory.appendingPathComponent("\(slot).unreadable")
    }

    // MARK: Slot names

    static func autosaveSlot(week: UInt32) -> String {
        "autosave-\(Int(week / 4) % autosaveSlotCount)"
    }

    static func manualSlot(at date: Date = Date()) -> String {
        "manual-\(SaveMetadata.stamp(date))"
    }

    // MARK: Write

    @discardableResult
    func write(
        _ data: Data,
        slot: String,
        week: UInt32,
        canonicalHash: String = "",
        kind: SaveMetadata.Kind = .other
    ) throws -> URL {
        Self.writeLock.lock()
        defer { Self.writeLock.unlock() }
        try prepareDirectory()
        let target = url(forSlot: slot)
        let temporary = temporaryURL(forSlot: slot)
        try? FileManager.default.removeItem(at: temporary)

        do {
            try flush(data, to: temporary)
            if FileManager.default.fileExists(atPath: target.path) {
                // A file that already failed an import must never become the last good copy.
                if !FileManager.default.fileExists(atPath: markerURL(forSlot: slot).path) {
                    promoteLastGood(target: target, slot: slot)
                }
                _ = try FileManager.default.replaceItemAt(target, withItemAt: temporary)
            } else {
                try FileManager.default.moveItem(at: temporary, to: target)
            }
        } catch {
            try? FileManager.default.removeItem(at: temporary)
            throw SaveRecoveryError.writeFailed(slot: slot)
        }

        let info = engineInfo()
        let metadata = SaveMetadata(
            slot: slot,
            week: week,
            savedAt: Date(),
            byteCount: data.count,
            canonicalHash: canonicalHash,
            engineVersion: info.version,
            datasetHash: info.datasetHash,
            kind: kind
        )
        writeMetadata(metadata, to: metadataURL(forSlot: slot))
        try? FileManager.default.removeItem(at: markerURL(forSlot: slot))
        return target
    }

    /// Write the payload to a fresh file and flush it before the rename can publish it.
    private func flush(_ data: Data, to url: URL) throws {
        guard FileManager.default.createFile(atPath: url.path, contents: nil) else {
            throw SaveRecoveryError.writeFailed(slot: url.deletingPathExtension().lastPathComponent)
        }
        let handle = try FileHandle(forWritingTo: url)
        do {
            try handle.write(contentsOf: data)
            try handle.synchronize()
            try handle.close()
        } catch {
            try? handle.close()
            throw error
        }
    }

    /// Stage the file that is about to be replaced, then swap it into the last-good pair.
    private func promoteLastGood(target: URL, slot: String) {
        let backup = lastGoodURL(forSlot: slot)
        let staged = directory.appendingPathComponent("\(slot).lastgood.tmp")
        try? FileManager.default.removeItem(at: staged)
        do {
            try FileManager.default.copyItem(at: target, to: staged)
            if FileManager.default.fileExists(atPath: backup.path) {
                _ = try FileManager.default.replaceItemAt(backup, withItemAt: staged)
            } else {
                try FileManager.default.moveItem(at: staged, to: backup)
            }
            if let sidecar = try? Data(contentsOf: metadataURL(forSlot: slot)) {
                try? sidecar.write(to: lastGoodMetadataURL(forSlot: slot), options: .atomic)
            }
        } catch {
            // Keep the older last-good rather than a half-written replacement.
            try? FileManager.default.removeItem(at: staged)
        }
    }

    private func writeMetadata(_ metadata: SaveMetadata, to url: URL) {
        guard let data = try? JSONSerialization.data(withJSONObject: metadata.dictionary) else { return }
        try? data.write(to: url, options: .atomic)
    }

    private func prepareDirectory() throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    // MARK: Read

    func list() -> [StoredSave] {
        let urls = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        var saves: [StoredSave] = []
        for url in urls where url.pathExtension == "cwsave" {
            let slot = url.deletingPathExtension().lastPathComponent
            let sidecar = metadata(forSlot: slot)
            saves.append(
                StoredSave(
                    id: slot,
                    url: url,
                    week: sidecar?.week ?? 0,
                    savedAt: sidecar?.savedAt ?? Date.distantPast,
                    slot: slot,
                    hasLastGood: hasLastGood(slot: slot)
                )
            )
        }
        return saves.sorted {
            if $0.savedAt != $1.savedAt { return $0.savedAt > $1.savedAt }
            return $0.slot > $1.slot
        }
    }

    func latest() -> StoredSave? {
        list().first
    }

    func autosaveSlots() -> [StoredSave] {
        list().filter { $0.slot.hasPrefix("autosave-") }
    }

    func manualSlots() -> [StoredSave] {
        list().filter { $0.slot.hasPrefix("manual-") }
    }

    func metadata(forSlot slot: String) -> SaveMetadata? {
        SaveMetadata(dictionary: readSidecar(metadataURL(forSlot: slot)), slot: slot)
    }

    func lastGoodMetadata(forSlot slot: String) -> SaveMetadata? {
        SaveMetadata(dictionary: readSidecar(lastGoodMetadataURL(forSlot: slot)), slot: slot)
    }

    private func readSidecar(_ url: URL) -> [String: String] {
        guard let data = try? Data(contentsOf: url),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: String]
        else { return [:] }
        return object
    }

    func load(_ save: StoredSave) throws -> Data {
        try Data(contentsOf: save.url)
    }

    func loadBytes(slot: String) throws -> Data {
        try Data(contentsOf: url(forSlot: slot))
    }

    func lastGoodBytes(slot: String) throws -> Data {
        try Data(contentsOf: lastGoodURL(forSlot: slot))
    }

    func hasLastGood(slot: String) -> Bool {
        FileManager.default.fileExists(atPath: lastGoodURL(forSlot: slot).path)
    }

    func isMarkedUnreadable(slot: String) -> Bool {
        FileManager.default.fileExists(atPath: markerURL(forSlot: slot).path)
    }

    /// Remember that this slot's current file failed an import, so the next write does not
    /// promote it over a good last-good copy. Survives termination.
    func markUnreadable(slot: String) {
        try? prepareDirectory()
        _ = FileManager.default.createFile(
            atPath: markerURL(forSlot: slot).path,
            contents: Data(slot.utf8)
        )
    }

    // MARK: Recover

    /// Load `<slot>.lastgood` and offer it only after a fresh engine import accepts it. The failing
    /// current file is never read, renamed or deleted here.
    func recover(slot: String) throws -> SaveRecoveryCandidate {
        let backup = lastGoodURL(forSlot: slot)
        guard FileManager.default.fileExists(atPath: backup.path) else {
            throw SaveRecoveryError.noLastGood(slot: slot)
        }
        guard let data = try? Data(contentsOf: backup) else {
            throw SaveRecoveryError.unreadableLastGood(slot: slot)
        }
        let probe: SessionHandle
        do {
            probe = try importSave(bytes: data)
        } catch {
            throw SaveRecoveryError.invalidLastGood(
                slot: slot,
                key: ErrorKey.of(error) ?? "error.invalid_save"
            )
        }
        let overview = probe.overview()
        let metadata = lastGoodMetadata(forSlot: slot)
        return SaveRecoveryCandidate(
            slot: slot,
            week: metadata?.week ?? overview.tick,
            savedAt: metadata?.savedAt ?? Date.distantPast,
            url: backup,
            data: data,
            canonicalHash: probe.canonicalHash()
        )
    }

    /// Copy a file that failed to import into `Recovery/` for support. The original slot is left
    /// untouched, so a failed import never destroys the only copy.
    @discardableResult
    func preserveFailingCopy(slot: String, at date: Date = Date()) -> URL? {
        guard let data = try? Data(contentsOf: url(forSlot: slot)) else { return nil }
        let folder = directory.appendingPathComponent("Recovery", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        } catch {
            return nil
        }
        let copy = folder.appendingPathComponent("\(slot)-\(SaveMetadata.stamp(date)).cwsave")
        do {
            try data.write(to: copy, options: .atomic)
            return copy
        } catch {
            return nil
        }
    }

    // MARK: Diagnostics

    func writeDiagnosticBundle(_ data: Data, at date: Date = Date()) throws -> URL {
        try prepareDirectory()
        let folder = directory.appendingPathComponent("Diagnostics", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let url = folder.appendingPathComponent("diagnostics-\(SaveMetadata.stamp(date)).json")
        try data.write(to: url, options: .atomic)
        return url
    }

    // MARK: Delete and prune

    /// Explicit player deletion. Removes the slot, its metadata and its last-good copy. Quarantined
    /// copies under `Recovery/` are kept.
    func delete(_ save: StoredSave) {
        try? FileManager.default.removeItem(at: save.url)
        try? FileManager.default.removeItem(at: metadataURL(forSlot: save.slot))
        try? FileManager.default.removeItem(at: lastGoodURL(forSlot: save.slot))
        try? FileManager.default.removeItem(at: lastGoodMetadataURL(forSlot: save.slot))
        try? FileManager.default.removeItem(at: markerURL(forSlot: save.slot))
    }

    func pruneManualSlots(keeping limit: Int = SaveManager.manualSlotLimit) {
        let manual = manualSlots()
        guard manual.count > limit else { return }
        for save in manual.dropFirst(limit) {
            delete(save)
        }
    }
}
