import SwiftUI
import XCTest
@testable import CorporateWarfare

final class MapRotationTests: XCTestCase {
    /// A tap on a turned, tilted map lands in the region drawn under it, and a drag still moves the
    /// ground with the finger whatever the bearing.
    func testATapOnATurnedMapSelectsTheRegionUnderIt() throws {
        let map = WorldMap.shared
        let size = CGSize(width: 1200, height: 800)
        let tilt = 28 * Double.pi / 180
        for bearing in [0.0, 35, 120, 270] {
            let camera = MapCamera.scenario(in: size, map: map).zoomed(by: 1.5).rotated(by: bearing)
            let p = MapProjection(camera: camera, size: size, tilt: tilt)
            let anchor = try XCTUnwrap(map.anchor(of: "OH-CEN"))
            let tap = p.project(anchor).0
            XCTAssertEqual(map.regions.first { p.contains($0.rings, tap) }?.code, "OH-CEN",
                           "a tap on Columbus at bearing \(bearing)")
            // A drag to the right moves the centre right on screen, whatever the bearing.
            let moved = camera.panned(by: CGSize(width: 100, height: 0), in: size)
            let centre = GeoPoint(lonDegrees: camera.lon, latDegrees: camera.lat)
            let before = p.project(centre).0
            let after = MapProjection(camera: moved, size: size, tilt: tilt).project(centre).0
            XCTAssertGreaterThan(after.x - before.x, 50, "the ground follows the finger at bearing \(bearing)")
        }
    }

    /// A fly back to north takes the short way round.
    func testInterpolateTurnsTheShortWayRound() {
        let a = MapCamera(lon: -85, lat: 40, zoom: 8, bearing: 350)
        let b = MapCamera(lon: -85, lat: 40, zoom: 8, bearing: 0)
        let half = MapCamera.interpolate(a, b, 0.5)
        XCTAssertEqual(half.bearing, 355, accuracy: 1e-9)
        XCTAssertEqual(MapCamera(lon: 0, lat: 0, zoom: 1).rotated(by: -30).bearing, 330, accuracy: 1e-9)
    }

    /// A double-tap zooms about 3x on the tapped ground: the ground under the finger stays under
    /// it, and the zoom is applied once.
    func testZoomAroundAPointKeepsTheGroundUnderTheFinger() {
        let size = CGSize(width: 1200, height: 800)
        let camera = MapCamera(lon: -85, lat: 40, zoom: 100)
        let ground = GeoPoint(lonDegrees: -84.9, latDegrees: 40.08)
        let tap = MapProjection(camera: camera, size: size, tilt: 0).project(ground).0
        let zoomed = camera.zoomed(by: 3, around: tap, in: size)
        XCTAssertEqual(zoomed.zoom, 300, accuracy: 1e-9)
        let after = MapProjection(camera: zoomed, size: size, tilt: 0).project(ground).0
        XCTAssertEqual(after.x, tap.x, accuracy: 4)
        XCTAssertEqual(after.y, tap.y, accuracy: 4)
    }

    /// The zoom stops at the maximum without a hard stop before it, and the pan follows the factor
    /// that was actually applied.
    func testZoomAroundAPointReachesTheMaximum() {
        let size = CGSize(width: 400, height: 800)
        let near = MapCamera(lon: -85, lat: 40, zoom: 1000)
        let zoomed = near.zoomed(by: 3, around: CGPoint(x: 300, y: 200), in: size)
        XCTAssertEqual(zoomed.zoom, MapCamera.maxZoom, accuracy: 1e-9)
        XCTAssertGreaterThanOrEqual(MapCamera.maxZoom, MapCamera.modelZoom)
        let atMax = MapCamera(lon: -85, lat: 40, zoom: MapCamera.maxZoom)
        let same = atMax.zoomed(by: 3, around: CGPoint(x: 300, y: 200), in: size)
        XCTAssertEqual(same.lon, atMax.lon, accuracy: 1e-12)
        XCTAssertEqual(same.lat, atMax.lat, accuracy: 1e-12)
    }
}
