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

    func submit(json: String) -> ReceiptDto? {
        session?.submitJson(json: json)
    }

    func exportSave() throws -> Data {
        guard let session else { throw SaveError.noSession }
        let bytes = try session.exportSave()
        return Data(bytes)
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
}

/// Atomic temp-write + replace with a last-good backup, per the save contract.
struct SaveManager {
    static let shared = SaveManager()

    private var directory: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        let url = base.appendingPathComponent("CorporateWarfare", isDirectory: true)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    func write(_ data: Data, slot: String, week: UInt32) throws -> URL {
        let target = directory.appendingPathComponent("\(slot).cwsave")
        let temporary = directory.appendingPathComponent("\(slot).tmp")
        try data.write(to: temporary, options: .atomic)
        if FileManager.default.fileExists(atPath: target.path) {
            let backup = directory.appendingPathComponent("\(slot).lastgood")
            try? FileManager.default.removeItem(at: backup)
            try? FileManager.default.copyItem(at: target, to: backup)
        }
        _ = try FileManager.default.replaceItemAt(target, withItemAt: temporary)
        let metadata = ["week": String(week), "savedAt": ISO8601DateFormatter().string(from: Date())]
        if let json = try? JSONSerialization.data(withJSONObject: metadata) {
            try? json.write(to: directory.appendingPathComponent("\(slot).json"), options: .atomic)
        }
        return target
    }

    func list() -> [StoredSave] {
        let urls = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        var saves: [StoredSave] = []
        for url in urls where url.pathExtension == "cwsave" {
            let slot = url.deletingPathExtension().lastPathComponent
            let metaURL = directory.appendingPathComponent("\(slot).json")
            var week: UInt32 = 0
            var date = Date.distantPast
            if let data = try? Data(contentsOf: metaURL),
               let object = try? JSONSerialization.jsonObject(with: data) as? [String: String] {
                week = UInt32(object["week"] ?? "0") ?? 0
                if let text = object["savedAt"] {
                    date = ISO8601DateFormatter().date(from: text) ?? Date.distantPast
                }
            }
            saves.append(StoredSave(id: slot, url: url, week: week, savedAt: date, slot: slot))
        }
        return saves.sorted { $0.savedAt > $1.savedAt }
    }

    func latest() -> StoredSave? {
        list().first
    }

    func load(_ save: StoredSave) throws -> Data {
        try Data(contentsOf: save.url)
    }

    func delete(_ save: StoredSave) {
        try? FileManager.default.removeItem(at: save.url)
        try? FileManager.default.removeItem(at: directory.appendingPathComponent("\(save.slot).json"))
    }
}
