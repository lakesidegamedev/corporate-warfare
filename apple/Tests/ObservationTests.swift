import XCTest
@testable import CorporateWarfare

/// A screen redraws only for the fields it reads. Before the store used the Observation framework,
/// any change to any of its fields redrew every screen that watched the store.
@MainActor
final class ObservationTests: XCTestCase {
    private final class Count: @unchecked Sendable {
        var value = 0
    }

    private func makeStore() throws -> AppStore {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("CorporateWarfareObservation-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return AppStore(saveManager: SaveManager(directory: directory))
    }

    func testAChangeToAnotherFieldDoesNotNotifyAReader() throws {
        let store = try makeStore()
        let fired = Count()
        withObservationTracking { _ = store.statusLine } onChange: { fired.value += 1 }
        store.showSetup = true
        store.errorMessage = "unrelated"
        XCTAssertEqual(fired.value, 0, "fields the screen did not read do not redraw it")
        store.statusLine = "changed"
        XCTAssertEqual(fired.value, 1, "the field it read does")
    }

    func testTutorialChangesReachAScreenThatReadsThem() throws {
        let store = try makeStore()
        let fired = Count()
        withObservationTracking { _ = store.tutorial.requestedPage } onChange: { fired.value += 1 }
        store.tutorial.requestedPage = .map
        XCTAssertEqual(fired.value, 1, "tutorial progress is observed through the store")
    }
}
