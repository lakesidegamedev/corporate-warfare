import XCTest
@testable import CorporateWarfare

/// The world map and the office art are read off the main thread, so launch and opening a career do
/// not freeze the screen. The timings are printed; the checks gate that the work really ran in the
/// background and gave the same maps the main thread would have built.
@MainActor
final class LaunchWarmupTests: XCTestCase {
    func testWorldMapPrewarmsInTheBackgroundAndIsTheSameMap() async throws {
        let start = Date.timeIntervalSinceReferenceDate
        WorldMap.prewarm()
        let returned = Date.timeIntervalSinceReferenceDate - start
        var waited = 0
        while WorldMap.sharedIfReady == nil && waited < 1000 {
            try? await Task.sleep(nanoseconds: 10_000_000)
            waited += 1
        }
        let ready = Date.timeIntervalSinceReferenceDate - start
        print("launch: prewarm call returned after \(Int(returned * 1000)) ms, default map ready after \(Int(ready * 1000)) ms")
        let map = try XCTUnwrap(WorldMap.sharedIfReady, "the default map is built in the background")
        XCTAssertTrue(map === WorldMap.shared, "the menu and the map share one default map")
        XCTAssertLessThan(returned, 0.05, "starting the warm-up does not wait for the world file")
    }

    func testCareerMapIsBuiltOffMainAndMatchesWhatTheMapViewAsks() async throws {
        let start = Date.timeIntervalSinceReferenceDate
        let id = await WorldMap.prepare(scenario: "texas-1981")
        print("launch: career map prepared in \(Int((Date.timeIntervalSinceReferenceDate - start) * 1000)) ms")
        XCTAssertEqual(id, "texas-1981")
        let asked = WorldMap.forRegions(["TX-01"])
        XCTAssertTrue(asked === WorldMap.forScenario("texas-1981"), "the view finds the prepared map, not a second one")
        let custom = await WorldMap.prepare(scenario: "not-a-scenario")
        XCTAssertEqual(custom, WorldMap.shared.scenarioId, "an unknown scenario falls back to the default map")
    }

    func testOfficeArtWarmsIntoTheBitmapCache() throws {
        let start = Date.timeIntervalSinceReferenceDate
        UiWarmup.run()
        print("launch: office art warmed in \(Int((Date.timeIntervalSinceReferenceDate - start) * 1000)) ms")
        let art = try XCTUnwrap(OfficeRoomArt.file)
        let again = Date.timeIntervalSinceReferenceDate
        XCTAssertNotNil(PixelIcons.bitmap(key: "office|room", rows: art.room.rows, palette: art.room.palette))
        XCTAssertLessThan(Date.timeIntervalSinceReferenceDate - again, 0.05, "the room bitmap is already cached")
    }
}
