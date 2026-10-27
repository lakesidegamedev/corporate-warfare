import Foundation
import XCTest
@testable import CorporateWarfare

final class EngineBridgeTests: XCTestCase {
    func testGeneratedBindingsLoadTheRustLibrary() {
        let info = engineInfo()
        XCTAssertEqual(info.interfaceVersion, 1)
        XCTAssertEqual(info.version, "1.6.1")
        XCTAssertEqual(info.datasetHash.count, 64)
    }

    func testRealSessionCommandAdvanceSaveAndLoad() throws {
        let session = try SessionHandle(seed: 42, guided: true, fiveYear: true)
        let overview = session.overview()
        XCTAssertGreaterThan(overview.investorCashCents, 0)
        let firm = try XCTUnwrap(overview.primaryFirm)

        let reference = products().first?.referencePriceCents ?? 10_000
        let receipt = session.setPrice(firm: firm, product: 0, priceCents: reference)
        XCTAssertTrue(receipt.accepted, receipt.errorDetail ?? "rejected")

        let advanced = session.advance(weeks: 2)
        XCTAssertEqual(advanced.weeksAdvanced, 2)
        XCTAssertEqual(session.overview().tick, 2)

        // The same sequence is pinned in the Rust test
        // `the_cross_platform_fixture_hash_is_pinned`. Equal hashes on Linux and iOS are the
        // cross-platform determinism evidence for this fixture. The value moved on 2026-10-25
        // when rival command ids began using the persisted sequence counter, and again when
        // ordinary facilities stopped consuming a nonexistent ore deposit, on 2026-10-27 when the
        // world gained its world price state, and again in 1.7 preview 2 when the product chains
        // and multi-output plants changed the dataset. This constant needs
        // a fresh simulator run on the Mac runner before R05 is claimed again.
        XCTAssertEqual(session.canonicalHash(), "1ec1c70637ab5310fabb745148702b7f25763e40c5296474350e6c4ac3247fb3")

        let data = try session.exportSave()
        XCTAssertGreaterThan(data.count, 1_000)
        let restored = try importSave(bytes: data)
        XCTAssertEqual(session.canonicalHash(), restored.canonicalHash())
        XCTAssertEqual(restored.overview().tick, 2)
    }

    func testRejectedCommandKeepsTheRevision() throws {
        let session = try SessionHandle(seed: 43, guided: false, fiveYear: false)
        let before = session.revision()
        let receipt = session.setPrice(firm: 0xDEAD_BEEF, product: 0, priceCents: 1_000)
        XCTAssertFalse(receipt.accepted)
        XCTAssertNotNil(receipt.errorKey)
        XCTAssertEqual(session.revision(), before)
    }

    func testCorruptSaveIsRejected() throws {
        let session = try SessionHandle(seed: 44, guided: true, fiveYear: false)
        let data = try session.exportSave()
        var bytes = [UInt8](data)
        if bytes.count > 10 {
            bytes[bytes.count - 4] ^= 0x55
        }
        XCTAssertThrowsError(try importSave(bytes: Data(bytes)))
    }
}

/// Filesystem and lifecycle persistence evidence for P32. These run against real files in a
/// temporary folder, never the app container, and they exercise the same `SaveManager` the app
/// uses. They require the Mac runner: this host cannot build Swift.
final class SaveLifecycleTests: XCTestCase {
    private var root: URL!

    override func setUpWithError() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("cw-saves-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        root = url
    }

    override func tearDownWithError() throws {
        if let root {
            try? FileManager.default.removeItem(at: root)
        }
        root = nil
    }

    private func manager() -> SaveManager {
        SaveManager(directory: root)
    }

    /// Atomic replace: the new payload is current and the file it replaced is still readable as
    /// the slot's last-good copy.
    func testAtomicReplaceKeepsThePreviousFileReadable() throws {
        let saves = manager()
        let session = try SessionHandle(seed: 61, guided: true, fiveYear: true)
        let first = try session.exportSave()
        let firstHash = session.canonicalHash()
        try saves.write(first, slot: "autosave-0", week: 0, canonicalHash: firstHash, kind: .autosave)

        _ = session.advance(weeks: 1)
        let second = try session.exportSave()
        let secondHash = session.canonicalHash()
        XCTAssertNotEqual(first, second)
        try saves.write(second, slot: "autosave-0", week: 1, canonicalHash: secondHash, kind: .autosave)

        XCTAssertEqual(try saves.loadBytes(slot: "autosave-0"), second)
        XCTAssertEqual(try saves.lastGoodBytes(slot: "autosave-0"), first)
        XCTAssertEqual(try importSave(bytes: try saves.lastGoodBytes(slot: "autosave-0")).canonicalHash(), firstHash)
        XCTAssertEqual(try importSave(bytes: try saves.loadBytes(slot: "autosave-0")).canonicalHash(), secondHash)
        XCTAssertEqual(saves.metadata(forSlot: "autosave-0")?.week, 1)
        XCTAssertEqual(saves.lastGoodMetadata(forSlot: "autosave-0")?.week, 0)
        XCTAssertEqual(saves.list().count, 1)
    }

    /// A corrupt current file is reported by the engine, and the slot's last-good copy is offered
    /// only after a fresh import accepts it. The damaged file itself is never touched.
    func testCorruptCurrentSaveFallsBackToLastGood() throws {
        let saves = manager()
        let slot = "autosave-1"
        let session = try SessionHandle(seed: 62, guided: true, fiveYear: true)
        let first = try session.exportSave()
        let firstHash = session.canonicalHash()
        try saves.write(first, slot: slot, week: 0, canonicalHash: firstHash, kind: .autosave)

        _ = session.advance(weeks: 4)
        let second = try session.exportSave()
        try saves.write(second, slot: slot, week: 4, canonicalHash: session.canonicalHash(), kind: .autosave)

        var bytes = [UInt8](try saves.loadBytes(slot: slot))
        bytes[bytes.count - 5] ^= 0x7f
        try Data(bytes).write(to: saves.url(forSlot: slot), options: .atomic)
        XCTAssertThrowsError(try importSave(bytes: Data(bytes)))

        let candidate = try saves.recover(slot: slot)
        XCTAssertEqual(candidate.slot, slot)
        XCTAssertEqual(candidate.week, 0)
        XCTAssertEqual(candidate.data, first)
        XCTAssertNotEqual(candidate.savedAt, Date.distantPast)
        XCTAssertEqual(try importSave(bytes: candidate.data).canonicalHash(), firstHash)

        // Recovery reads only the last-good copy: the damaged file is still on disk, unchanged.
        XCTAssertTrue(FileManager.default.fileExists(atPath: saves.url(forSlot: slot).path))
        XCTAssertEqual(try saves.loadBytes(slot: slot), Data(bytes))
    }

    /// Bytes leave the engine through the save manager and come back with the same canonical hash.
    func testExportedBytesImportAndMatchTheCanonicalHash() throws {
        let saves = manager()
        let session = try SessionHandle(seed: 63, guided: false, fiveYear: false)
        _ = session.advance(weeks: 2)
        let tick = session.overview().tick
        XCTAssertGreaterThan(tick, 0)
        let exported = try session.exportSave()
        let hash = session.canonicalHash()
        let slot = SaveManager.manualSlot()
        try saves.write(exported, slot: slot, week: tick, canonicalHash: hash, kind: .manual)

        let restored = try importSave(bytes: try saves.loadBytes(slot: slot))
        XCTAssertEqual(restored.canonicalHash(), hash)
        XCTAssertEqual(restored.overview().tick, tick)
        XCTAssertEqual(saves.metadata(forSlot: slot)?.canonicalHash, hash)
    }

    /// A save written by a future schema is rejected with the engine's typed unsupported-version
    /// fault, not a generic failure and not a partial world.
    func testFutureSchemaBytesAreRejectedWithATypedError() throws {
        let session = try SessionHandle(seed: 64, guided: true, fiveYear: false)
        var bytes = [UInt8](try session.exportSave())
        let current = UInt32(bytes[4])
            | UInt32(bytes[5]) << 8
            | UInt32(bytes[6]) << 16
            | UInt32(bytes[7]) << 24
        let future = current &+ 1
        withUnsafeBytes(of: future.littleEndian) { raw in
            for (offset, byte) in raw.enumerated() {
                bytes[4 + offset] = byte
            }
        }

        XCTAssertThrowsError(try importSave(bytes: Data(bytes))) { error in
            guard let ffi = error as? FfiError else {
                return XCTFail("expected a typed FfiError, got \(type(of: error))")
            }
            guard case .Save(let key, let detail) = ffi else {
                return XCTFail("expected a save fault, got \(ffi)")
            }
            XCTAssertEqual(key, "error.unsupported_version")
            XCTAssertFalse(detail.isEmpty)
        }
    }

    /// Termination between writing the temporary file and replacing the current file: the old
    /// save is still complete and a stray temporary file is not listed as a slot.
    func testInterruptedWriteLeavesTheOldSaveIntact() throws {
        let saves = manager()
        let slot = "autosave-2"
        let session = try SessionHandle(seed: 65, guided: true, fiveYear: true)
        let exported = try session.exportSave()
        let hash = session.canonicalHash()
        try saves.write(exported, slot: slot, week: 0, canonicalHash: hash, kind: .autosave)

        let interrupted = saves.temporaryURL(forSlot: slot)
        try Data("interrupted".utf8).write(to: interrupted)
        XCTAssertTrue(FileManager.default.fileExists(atPath: interrupted.path))

        XCTAssertEqual(try saves.loadBytes(slot: slot), exported)
        XCTAssertEqual(try importSave(bytes: try saves.loadBytes(slot: slot)).canonicalHash(), hash)
        XCTAssertEqual(saves.list().count, 1)
        XCTAssertFalse(saves.hasLastGood(slot: slot))
    }

    /// Three autosave slots rotate by week and manual saves are bounded.
    func testAutosaveSlotsRotateByWeekAndManualSlotsAreBounded() throws {
        XCTAssertEqual(SaveManager.autosaveSlot(week: 0), "autosave-0")
        XCTAssertEqual(SaveManager.autosaveSlot(week: 3), "autosave-0")
        XCTAssertEqual(SaveManager.autosaveSlot(week: 4), "autosave-1")
        XCTAssertEqual(SaveManager.autosaveSlot(week: 8), "autosave-2")
        XCTAssertEqual(SaveManager.autosaveSlot(week: 12), "autosave-0")

        let saves = manager()
        for index in 0..<12 {
            let slot = "manual-\(String(format: "%02d", index))"
            try saves.write(Data([UInt8(index)]), slot: slot, week: UInt32(index), kind: .manual)
        }
        XCTAssertEqual(saves.manualSlots().count, 12)

        saves.pruneManualSlots()
        let kept = saves.manualSlots()
        XCTAssertEqual(kept.count, SaveManager.manualSlotLimit)
        XCTAssertFalse(kept.contains { $0.slot == "manual-00" })
        XCTAssertFalse(kept.contains { $0.slot == "manual-01" })
        XCTAssertTrue(kept.contains { $0.slot == "manual-11" })
    }

    /// Recovering a slot that never had a second write fails with a named host error instead of
    /// pretending there is a copy.
    func testRecoverWithoutLastGoodFailsWithATypedError() throws {
        let saves = manager()
        try saves.write(Data([0x01, 0x02, 0x03]), slot: "autosave-0", week: 0, kind: .autosave)
        XCTAssertThrowsError(try saves.recover(slot: "autosave-0")) { error in
            guard let recovery = error as? SaveRecoveryError else {
                return XCTFail("expected SaveRecoveryError, got \(type(of: error))")
            }
            if case .noLastGood(let slot) = recovery {
                XCTAssertEqual(slot, "autosave-0")
            } else {
                XCTFail("expected noLastGood, got \(recovery)")
            }
        }
    }

    /// A slot that failed an import must not promote its damaged file over a good last-good copy
    /// on the next autosave.
    func testADamagedSlotNeverPromotesItsBrokenFileOverLastGood() throws {
        let saves = manager()
        let slot = "autosave-0"
        let session = try SessionHandle(seed: 66, guided: true, fiveYear: true)
        let good0 = try session.exportSave()
        try saves.write(good0, slot: slot, week: 0, canonicalHash: session.canonicalHash(), kind: .autosave)

        _ = session.advance(weeks: 1)
        try saves.write(try session.exportSave(), slot: slot, week: 1, canonicalHash: session.canonicalHash(), kind: .autosave)
        XCTAssertEqual(try saves.lastGoodBytes(slot: slot), good0)

        var bytes = [UInt8](try saves.loadBytes(slot: slot))
        bytes[bytes.count - 5] ^= 0xff
        try Data(bytes).write(to: saves.url(forSlot: slot), options: .atomic)
        XCTAssertThrowsError(try importSave(bytes: Data(bytes)))
        saves.markUnreadable(slot: slot)

        _ = session.advance(weeks: 1)
        let good2 = try session.exportSave()
        try saves.write(good2, slot: slot, week: 2, canonicalHash: session.canonicalHash(), kind: .autosave)

        XCTAssertEqual(try saves.lastGoodBytes(slot: slot), good0)
        XCTAssertEqual(try saves.loadBytes(slot: slot), good2)
        XCTAssertFalse(saves.isMarkedUnreadable(slot: slot))

        // With a good file current again, the next write rolls the last-good copy forward.
        _ = session.advance(weeks: 1)
        try saves.write(try session.exportSave(), slot: slot, week: 3, canonicalHash: session.canonicalHash(), kind: .autosave)
        XCTAssertEqual(try saves.lastGoodBytes(slot: slot), good2)
    }
}
