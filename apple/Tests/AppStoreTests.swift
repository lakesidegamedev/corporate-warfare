import XCTest
@testable import CorporateWarfare

@MainActor
final class AppStoreTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("CorporateWarfareStoreTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        if let directory {
            try? FileManager.default.removeItem(at: directory)
        }
    }

    func testNewCareerAdvanceStopSaveAndRestore() async throws {
        let saves = SaveManager(directory: directory)
        let store = AppStore(saveManager: saves)
        await store.newCareer(guided: true, fiveYear: true, seed: 314)
        XCTAssertTrue(store.sessionReady)
        XCTAssertFalse(store.firms.isEmpty)
        guard let firm = store.firms.first else {
            return XCTFail("a guided career has a firm")
        }
        await store.selectFirm(firm.handle)
        await store.advance(weeks: 1)
        XCTAssertEqual(store.overview?.tick, 1)
        await store.suspendAndSave()
        XCTAssertFalse(saves.list().isEmpty, "the stop-save wrote a real save")

        let restored = AppStore(saveManager: saves)
        await restored.restoreLatestIfAvailable()
        XCTAssertTrue(restored.sessionReady, "the store restores the saved session")
        XCTAssertEqual(restored.overview?.tick, 1)
    }

    func testARejectedPhoneCommandKeepsTheSessionUsable() async {
        let saves = SaveManager(directory: directory)
        let store = AppStore(saveManager: saves)
        await store.newCareer(guided: true, fiveYear: true, seed: 315)
        let rejection = await store.submit(
            CommandJSON.command(
                "SetProductionTarget",
                ["firm": UInt64(9_999_999), "target_bps": 9_000]
            )
        )
        XCTAssertNotNil(rejection, "a dead firm handle is rejected")
        XCTAssertTrue(store.sessionReady)
        XCTAssertEqual(store.overview?.tick, 0)
        await store.advance(weeks: 1)
        XCTAssertEqual(store.overview?.tick, 1, "the session remains usable after rejection")
    }

    func testChoosingTheBoardsGoalSendsChooseMandate() async throws {
        let store = AppStore(saveManager: SaveManager(directory: directory))
        await store.newCareer(guided: true, fiveYear: false, seed: 7)
        await store.advance(weeks: 2)
        await store.continueCareer()
        await store.refreshHall()
        let offer = try XCTUnwrap(store.boardOffer, "the board offers goals in the first weeks")
        XCTAssertEqual(CommandJSON.command("ChooseMandate", ["firm": UInt64(42), "pick": 1]).contains("\"ChooseMandate\""), true)
        let rejection = await store.chooseMandate(firm: offer.firm, pick: 1)
        XCTAssertNil(rejection)
        await store.refreshHall()
        let active = try XCTUnwrap(store.activeMandate)
        XCTAssertEqual(active.mandate.key, offer.offered[1].key)
        XCTAssertNil(store.boardOffer)
    }
}
