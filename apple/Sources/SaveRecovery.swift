import Foundation

/// Save metadata sidecar, diagnostics bundle and recovery values.
///
/// The sidecar stays a flat JSON object of strings so a sidecar written by an earlier build is
/// still readable after an update. Nothing here reaches the network: recovery and diagnostics are
/// local files that the player reviews before choosing to share.

/// One save's metadata sidecar (`<slot>.json`). Values are stored as strings for compatibility
/// with the first released build, which wrote `week` and `savedAt` only.
struct SaveMetadata: Equatable {
    enum Kind: String {
        case autosave
        case manual
        case other
    }

    var slot: String
    var week: UInt32
    var savedAt: Date
    var byteCount: Int
    var canonicalHash: String
    var engineVersion: String
    var datasetHash: String
    var kind: Kind

    init(
        slot: String,
        week: UInt32,
        savedAt: Date,
        byteCount: Int,
        canonicalHash: String,
        engineVersion: String,
        datasetHash: String,
        kind: Kind
    ) {
        self.slot = slot
        self.week = week
        self.savedAt = savedAt
        self.byteCount = byteCount
        self.canonicalHash = canonicalHash
        self.engineVersion = engineVersion
        self.datasetHash = datasetHash
        self.kind = kind
    }

    init?(dictionary: [String: String], slot: String) {
        guard !dictionary.isEmpty else { return nil }
        self.slot = slot
        self.week = UInt32(dictionary["week"] ?? "") ?? 0
        self.savedAt = SaveMetadata.date(from: dictionary["savedAt"]) ?? Date.distantPast
        self.byteCount = Int(dictionary["bytes"] ?? "") ?? 0
        self.canonicalHash = dictionary["hash"] ?? ""
        self.engineVersion = dictionary["engine"] ?? ""
        self.datasetHash = dictionary["dataset"] ?? ""
        self.kind = Kind(rawValue: dictionary["kind"] ?? "") ?? .other
    }

    var dictionary: [String: String] {
        [
            "slot": slot,
            "week": String(week),
            "savedAt": SaveMetadata.string(from: savedAt),
            "bytes": String(byteCount),
            "hash": canonicalHash,
            "engine": engineVersion,
            "dataset": datasetHash,
            "kind": kind.rawValue,
        ]
    }

    static func string(from date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        return formatter.string(from: date)
    }

    static func date(from text: String?) -> Date? {
        guard let text, !text.isEmpty else { return nil }
        let formatter = ISO8601DateFormatter()
        if let date = formatter.date(from: text) {
            return date
        }
        return DateFormatter.isoLocal.date(from: text)
    }

    static func stamp(_ date: Date = Date()) -> String {
        DateFormatter.fileStamp.string(from: date)
    }
}

extension DateFormatter {
    /// Sidecar and file naming stamp. Fixed locale and zone so a device language change cannot
    /// rename a slot.
    static var fileStamp: DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyyMMdd-HHmmss"
        return formatter
    }

    static var isoLocal: DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm:ss"
        return formatter
    }
}

/// A last-good save that a fresh engine import already accepted. The UI may offer these bytes;
/// nothing is applied to the live session until the player accepts.
struct SaveRecoveryCandidate: Equatable {
    let slot: String
    let week: UInt32
    let savedAt: Date
    let url: URL
    let data: Data
    let canonicalHash: String
}

/// Why a slot could not be opened. Carried to the recovery screen so it can name the slot, week
/// and timestamp instead of a generic failure.
struct SaveRecoveryIssue: Identifiable, Equatable {
    let id = UUID()
    let slot: String
    let week: UInt32?
    let savedAt: Date?
    let errorKey: String
    let hasLastGood: Bool
    /// Copy of the damaged file kept for support. The original is never moved, renamed or deleted.
    let preservedCopy: URL?
}

/// Stable keys for failures raised by the Swift save layer. These are host keys, not engine
/// domain keys, and are recorded in the diagnostics bundle so a report can name the failure.
enum SaveRecoveryError: LocalizedError, Equatable {
    case writeFailed(slot: String)
    case noLastGood(slot: String)
    case invalidLastGood(slot: String, key: String)
    case unreadableLastGood(slot: String)

    var errorKey: String {
        switch self {
        case .writeFailed: return "error.save_write_failed"
        case .noLastGood: return "error.save_no_last_good"
        case .invalidLastGood: return "error.save_invalid_last_good"
        case .unreadableLastGood: return "error.save_unreadable_last_good"
        }
    }

    var errorDescription: String? {
        switch self {
        case .writeFailed(let slot):
            return "\(slot) could not be written. The previous save is unchanged."
        case .noLastGood(let slot):
            return "There is no last good copy of \(slot)."
        case .invalidLastGood(let slot, _):
            return "The last good copy of \(slot) did not pass the engine's own check."
        case .unreadableLastGood(let slot):
            return "The last good copy of \(slot) could not be read from this device."
        }
    }
}

/// Extracts the engine's stable message key from a bridged error without leaking Rust text into
/// product copy.
enum ErrorKey {
    static func of(_ error: Error) -> String? {
        if let ffi = error as? FfiError {
            switch ffi {
            case .Save(let key, _): return key
            case .Domain(let key, _): return key
            case .Internal: return nil
            }
        }
        if let recovery = error as? SaveRecoveryError {
            return recovery.errorKey
        }
        return nil
    }
}

/// Product copy for save failures. The engine never supplies player-facing text.
enum SaveCopy {
    static func message(forKey key: String?) -> String {
        switch key {
        case "error.unsupported_version":
            return "This save was made by a newer version of Corporate Warfare. Update the game to open it."
        case "error.invalid_save":
            return "This save is damaged or incomplete."
        case "error.save_no_last_good":
            return "There is no last good copy of this save."
        case "error.save_invalid_last_good":
            return "The last good copy is also damaged."
        case "error.save_unreadable_last_good":
            return "The last good copy could not be read from this device."
        case "error.save_write_failed":
            return "The save could not be written. The previous save is unchanged."
        default:
            return "The save could not be opened."
        }
    }
}

/// Local diagnostics bundle. It carries build identity, the last engine error key and save
/// metadata only: no player name, no paths, no world contents.
struct DiagnosticBundle: Encodable {
    struct SaveEntry: Encodable {
        let slot: String
        let week: UInt32
        let savedAt: String
        let byteCount: Int
        let canonicalHash: String
        let hasLastGood: Bool
    }

    let generatedAt: String
    let appVersion: String
    let buildNumber: String
    let engineVersion: String
    let interfaceVersion: UInt32
    let rulesVersion: UInt32
    let datasetHash: String
    let lastErrorKey: String?
    let recoverySlot: String?
    let recoveryWeek: UInt32?
    let saves: [SaveEntry]
    let notes: [String]

    static func entry(_ save: StoredSave) -> SaveEntry {
        SaveEntry(
            slot: save.slot,
            week: save.week,
            savedAt: SaveMetadata.string(from: save.savedAt),
            byteCount: ((try? Data(contentsOf: save.url))?.count) ?? 0,
            canonicalHash: SaveManager.shared.metadata(forSlot: save.slot)?.canonicalHash ?? "",
            hasLastGood: save.hasLastGood
        )
    }

    /// Short review text shown before any share action.
    var reviewSummary: String {
        var lines = [
            "Build \(appVersion) (\(buildNumber))",
            "Engine \(engineVersion) · interface \(interfaceVersion) · rules \(rulesVersion)",
            "Dataset \(datasetHash.prefix(16))…",
            "Last error \(lastErrorKey ?? "none")",
            "Saves \(saves.count)",
        ]
        for save in saves {
            lines.append("• \(save.slot) week \(save.week), \(save.savedAt), \(save.byteCount) bytes")
        }
        return lines.joined(separator: "\n")
    }
}

/// A diagnostics file waiting for the player to review and optionally share.
struct DiagnosticExport: Identifiable, Equatable {
    let id = UUID()
    let url: URL
    let summary: String
}
