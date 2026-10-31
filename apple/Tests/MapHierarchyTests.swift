import XCTest
@testable import CorporateWarfare

/// The map's close-zoom reading rules: labels never overlap and hold steady, a lane holds vehicles two
/// lengths apart, and cities take their kind's model.
final class MapHierarchyTests: XCTestCase {
    func testLowerPriorityLabelIsDropped() {
        let planner = LabelPlanner()
        planner.begin()
        planner.reserve(CGRect(x: 100, y: 100, width: 40, height: 40))
        // The city's first spot crosses the building, so it takes the second.
        let city = planner.place("c:A", size: CGSize(width: 50, height: 12), slots: [CGPoint(x: 110, y: 110), CGPoint(x: 150, y: 20)])
        XCTAssertEqual(city, CGPoint(x: 150, y: 20))
        // The region name would sit on the city name, so it goes.
        XCTAssertNil(planner.place("r:X", size: CGSize(width: 50, height: 12), slots: [CGPoint(x: 150, y: 20)]))
        planner.end()
    }

    func testShowingLabelHoldsAndHiddenOneNeedsRoom() {
        let planner = LabelPlanner()
        let size = CGSize(width: 30, height: 10)
        let spot = [CGPoint(x: 50, y: 0)]
        planner.begin(); planner.reserve(CGRect(x: 0, y: 0, width: 40, height: 40))
        XCTAssertNotNil(planner.place("c:A", size: size, slots: spot)); planner.end()
        // The building grew half a point into the label: a showing label squeezes by.
        planner.begin(); planner.reserve(CGRect(x: 0, y: 0, width: 50.5, height: 40))
        XCTAssertNotNil(planner.place("c:A", size: size, slots: spot)); planner.end()
        // A dropped label needs clear air: touching is not enough to bring it back.
        planner.begin(); planner.reserve(CGRect(x: 0, y: 0, width: 50, height: 40))
        XCTAssertNil(planner.place("c:B", size: size, slots: spot)); planner.end()
        planner.begin(); planner.reserve(CGRect(x: 0, y: 0, width: 48, height: 40))
        XCTAssertNil(planner.place("c:B", size: size, slots: spot)); planner.end()
        planner.begin(); planner.reserve(CGRect(x: 0, y: 0, width: 40, height: 40))
        XCTAssertNotNil(planner.place("c:B", size: size, slots: spot)); planner.end()
    }

    func testPanningDoesNotChangeTheLabelSet() {
        let planner = LabelPlanner()
        var previous: Set<String>?
        for shift in 0..<40 {
            var shown = Set<String>()
            planner.begin()
            for i in 0..<30 {
                let at = CGPoint(x: 13 * Double(i % 6) + Double(shift), y: 11 * Double(i / 6))
                if planner.place("c:\(i)", size: CGSize(width: 30, height: 10), slots: [at]) != nil { shown.insert("c:\(i)") }
            }
            planner.end()
            if let previous { XCTAssertEqual(previous, shown, "label set changed while only panning (shift \(shift))") }
            previous = shown
        }
    }

    func testBigCityKeepsItsNameInACrowdedScene() {
        let planner = LabelPlanner()
        let size = CGSize(width: 50, height: 12)
        let block = CGRect(x: 100, y: 100, width: 40, height: 40)
        func crowd() {
            planner.reserve(block)
            for i in 0..<8 { planner.reserve(CGRect(x: 60 + CGFloat(i) * 12, y: 60, width: 10, height: 120)) }
            planner.reserve(CGRect(x: 60, y: 60, width: 100, height: 10)); planner.reserve(CGRect(x: 60, y: 170, width: 100, height: 10))
        }
        planner.begin(); crowd()
        let put = planner.placeCity("c:Columbus", size: size, slots: [CGPoint(x: 143, y: 114), CGPoint(x: 10, y: 114)],
                                    center: CGPoint(x: 120, y: 120), reach: 26)
        XCTAssertTrue(put.leader, "the name moves out and gets a leader line")
        XCTAssertFalse(CGRect(origin: put.corner, size: size).intersects(block), "the name stays off the block")
        // A region name afterwards cannot take its place.
        XCTAssertNil(planner.place("r:Ohio", size: size, slots: [put.corner]))
        planner.end()
        planner.begin(); crowd()
        XCTAssertTrue(planner.placeCity("c:Columbus", size: size, slots: [CGPoint(x: 143, y: 114)], center: CGPoint(x: 120, y: 120), reach: 26).leader)
        planner.end()
    }

    func testBigCityIsNeverDroppedWhenNothingIsClear() {
        let planner = LabelPlanner()
        planner.begin()
        planner.reserve(CGRect(x: -1000, y: -1000, width: 2000, height: 2000))
        let put = planner.placeCity("c:Chicago", size: CGSize(width: 60, height: 12), slots: [.zero], center: .zero, reach: 20)
        XCTAssertTrue(put.corner.x.isFinite && put.corner.y.isFinite)
        planner.end()
    }

    func testLaneHoldsVehiclesTwoLengthsApart() {
        XCTAssertEqual(FreightLook.maxPerLane(laneLength: 100, vehicleLength: 10), 5)
        XCTAssertEqual(FreightLook.maxPerLane(laneLength: 5, vehicleLength: 10), 1)
    }

    func testCityKinds() {
        XCTAssertEqual(CityKinds.of("Houston"), "city_port")
        XCTAssertEqual(CityKinds.of("Pittsburgh"), "city_steel")
        XCTAssertEqual(CityKinds.of("Detroit"), "city_auto")
        XCTAssertEqual(CityKinds.of("Springfield"), "city")
    }
}
