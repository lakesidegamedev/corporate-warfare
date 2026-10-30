import SwiftUI
import XCTest
@testable import CorporateWarfare

/// Buildings are fixed objects on the ground: a lot in metres from the hub, a size in metres.
@MainActor
final class MapBuildingLotsTests: XCTestCase {
    func testBuildingsStandAtTheirWorldPositionAtEveryZoomTiltAndTurn() throws {
        let size = CGSize(width: 393, height: 852)
        let map = WorldMap.shared
        let region = try XCTUnwrap(map.regions.first?.code)
        let hub = try XCTUnwrap(map.anchor(of: region))
        let divisions = [DivisionMarker(region: region, sector: "manufacturing", plants: 4)]
        // Manufacturing is the fifth sector, so its lot is 120 km west and 35 km south of the hub, worked out by hand.
        let east = -120_000.0, north = -35_000.0
        let lon = hub.lon + east / (6_371_000 * cos(hub.lat))
        let lat = hub.lat + north / 6_371_000
        var metres: [Double] = []
        for zoom in [2.0, 3.0, 5.0, 9.0] {
            for tilt in [0.0, 0.7] {
                for bearing in [0.0, 37.0, 200.0] {
                    var camera = MapCamera.scenario(in: size, map: map).zoomed(by: zoom)
                    camera.bearing = bearing
                    let g = MapGeometry(map: map, camera: camera, size: size, tilt: tilt, detail: 2)
                    let b = try XCTUnwrap(g.buildings(divisions, [:]).first)
                    let expected = try XCTUnwrap(g.projection.ground(lon: lon, lat: lat))
                    XCTAssertEqual(b.gx, Double(expected.x), accuracy: 0.5, "zoom \(zoom) tilt \(tilt) bearing \(bearing)")
                    XCTAssertEqual(b.gy, Double(expected.y), accuracy: 0.5, "zoom \(zoom) tilt \(tilt) bearing \(bearing)")
                    let hubGround = try XCTUnwrap(g.projection.ground(lon: hub.lon, lat: hub.lat))
                    metres.append(hypot(b.gx - Double(hubGround.x), b.gy - Double(hubGround.y)) / g.pointsPerMetre)
                    // A voxel is a fixed width in metres once past the far-zoom minimum.
                    let unit = g.unit(WorldScale.playerVoxel, displayScale: 3)
                    if unit > WorldScale.minUnitPixels / 3 * 1.001 {
                        XCTAssertEqual(unit / g.pointsPerMetre, WorldScale.playerVoxel, accuracy: 1)
                    }
                }
            }
        }
        // The same distance from the hub, in metres, at every camera.
        let first = try XCTUnwrap(metres.first)
        for m in metres { XCTAssertEqual(m, first, accuracy: first * 0.03) }
    }

    func testRivalSlotsAreStableAndNeverShared() {
        let keys = ["OH-CEN/chemical_industries/2", "OH-CEN/retail/3", "OH-CEN/healthcare/2", "OH-CEN/financial/4"]
        let slots = WorldScale.rivalSlots(keys)
        XCTAssertEqual(Set(slots.values).count, 4)
        XCTAssertEqual(slots, WorldScale.rivalSlots(keys.reversed()))
    }
}
