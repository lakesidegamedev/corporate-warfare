import XCTest
@testable import CorporateWarfare

final class EngineBridgeTests: XCTestCase {
    func testGeneratedBindingsLoadTheRustLibrary() {
        let info = engineInfo()
        XCTAssertEqual(info.interfaceVersion, 1)
        XCTAssertEqual(info.version, "0.1.0")
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
        // when rival command ids began using the persisted sequence counter; this constant
        // needs a fresh simulator run on the Mac runner before R05 is claimed again.
        XCTAssertEqual(session.canonicalHash(), "5437029ff5c6d98f3dc264c19ee98b7face66bd7949c80d1064715c3127199f2")

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
