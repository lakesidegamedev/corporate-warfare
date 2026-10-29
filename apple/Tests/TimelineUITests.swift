import XCTest
@testable import CorporateWarfare

final class TimelineUITests: XCTestCase {
    /// The three new-game choices are the keys the engine plays, and the card's wording is stable.
    func testChoicesReachTheEngineAndWordingReads() throws {
        for key in ["historical", "off", "shuffled"] {
            let handle = try SessionHandle.start(options: NewGameDto(
                seed: 7, scenarioId: "texas-1981", guided: true, fiveYear: false, difficulty: "standard",
                economy: "historic", startingCapitalCents: nil, pricePath: "historical", timeline: key))
            XCTAssertEqual(handle.timeline().mode, key)
        }
        XCTAssertEqual(timelineAboutWeeks(1), "in about 1 week")
        XCTAssertEqual(timelineAboutWeeks(6), "in about 6 weeks")
        XCTAssertEqual(timelineDate(year: 1982, weekOfYear: 21), "week 21, 1982")
    }
}
