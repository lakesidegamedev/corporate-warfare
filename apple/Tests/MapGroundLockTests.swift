import SwiftUI
import UIKit
import XCTest
@testable import CorporateWarfare

/// Roads stay on the ground and models turn with the map.
@MainActor
final class MapGroundLockTests: XCTestCase {
    /// Where the satellite mesh puts a ground point on screen: the 1 degree cell's triangle around
    /// it, with the texture interpolated in perspective (as the shader now does) or flat on the
    /// screen (as it did, which slid the photo against the roads).
    private func meshPoint(_ lonDeg: Double, _ latDeg: Double, _ p: MapProjection, perspective: Bool) -> CGPoint? {
        let west = floor(lonDeg)
        let north = ceil(latDeg)
        func vertex(_ lon: Double, _ lat: Double) -> (x: Double, y: Double, w: Double)? {
            guard let g = p.ground(lon: lon * .pi / 180, lat: lat * .pi / 180) else { return nil }
            let at = p.lift(g)
            return (Double(at.x), Double(at.y), 1 / p.scale(atFlatY: g.y))
        }
        guard let k = vertex(west, north), let c1 = vertex(west + 1, north), let c2 = vertex(west, north - 1),
              let c3 = vertex(west + 1, north - 1) else { return nil }
        let u = lonDeg - west
        let v = north - latDeg
        // Triangles (k, c1, c2) and (c1, c3, c2), as the mesh is indexed.
        let parts: [(Double, (x: Double, y: Double, w: Double))] = u + v <= 1
            ? [(1 - u - v, k), (u, c1), (v, c2)]
            : [(1 - v, c1), (u + v - 1, c3), (1 - u, c2)]
        if perspective {
            let w = parts.reduce(0) { $0 + $1.0 * $1.1.w }
            let x = parts.reduce(0) { $0 + $1.0 * $1.1.x * $1.1.w }
            let y = parts.reduce(0) { $0 + $1.0 * $1.1.y * $1.1.w }
            return CGPoint(x: x / w, y: y / w)
        }
        return CGPoint(x: parts.reduce(0) { $0 + $1.0 * $1.1.x }, y: parts.reduce(0) { $0 + $1.0 * $1.1.y })
    }

    /// The same scene at two camera centres: a known interstate vertex lands on the same ground
    /// point of the satellite Earth in both, up close and tilted, where the flat interpolation was
    /// several points off and different in each.
    func testInterstatesStayOnTheGroundAsTheCameraPans() throws {
        let map = WorldMap.shared
        let size = CGSize(width: 393, height: 852)
        let road = try XCTUnwrap(map.interstates.max { $0.points.count < $1.points.count })
        let vertex = road.points[road.points.count / 2]
        let lonDeg = vertex.lon * 180 / .pi
        let latDeg = vertex.lat * 180 / .pi
        var first = MapCamera.scenario(in: size, map: map).zoomed(by: 3).rotated(by: 30)
        first.lon = lonDeg
        first.lat = latDeg
        let second = first.panned(by: CGSize(width: 90, height: 160), in: size, pitch: 0.5)
        var worstFlat = 0.0
        for tiltSetting in [1, 2] {
            let tilt = GraphicsSettings.pitch(forTilt: tiltSetting)
            for camera in [first, second] {
                let p = MapProjection(camera: camera, size: size, tilt: tilt)
                let road = p.project(vertex).0
                let ground = try XCTUnwrap(meshPoint(lonDeg, latDeg, p, perspective: true))
                XCTAssertEqual(Double(road.x), Double(ground.x), accuracy: 0.5, "tilt \(tiltSetting): the road sits on the ground across")
                XCTAssertEqual(Double(road.y), Double(ground.y), accuracy: 0.5, "tilt \(tiltSetting): the road sits on the ground down")
                if let flat = meshPoint(lonDeg, latDeg, p, perspective: false) {
                    worstFlat = max(worstFlat, Double(hypot(flat.x - road.x, flat.y - road.y)))
                }
            }
        }
        // The old flat interpolation is what drifted: keep the test honest about the cause.
        XCTAssertGreaterThan(worstFlat, 1, "the flat interpolation drifts from the projection")
    }

    /// A model's ground footprint turns with the map: the corner east of a building is east on the
    /// ground at every bearing, and a vehicle heading east keeps heading east as the map turns.
    func testModelsTurnWithTheMap() {
        let size = CGSize(width: 800, height: 600)
        let map = WorldMap.shared
        for bearing in [0.0, 90, 180, 270, 35] {
            let camera = MapCamera.scenario(in: size, map: map).zoomed(by: 2).rotated(by: bearing)
            let p = MapProjection(camera: camera, size: size, tilt: GraphicsSettings.pitch(forTilt: 1))
            let east = CGVector(dx: p.east.dx, dy: p.east.dy)
            XCTAssertEqual(VoxelModels.heading(east, projection: p), 0, accuracy: 1e-9, "east is east at bearing \(bearing)")
            let north = CGVector(dx: p.north.dx, dy: p.north.dy)
            XCTAssertEqual(VoxelModels.heading(north, projection: p), .pi / 2, accuracy: 1e-9, "north is north at bearing \(bearing)")
        }
        XCTAssertTrue(VoxelModels.has("manufacturing/1"), "the box models load")
        XCTAssertTrue(VoxelModels.has("truck/player"), "the vehicle models load")
    }

    /// The same building drawn at four bearings shows different sides: no two renders match.
    func testABuildingShowsItsSidesAsTheMapTurns() throws {
        let size = CGSize(width: 160, height: 160)
        let map = WorldMap.shared
        var images: [[UInt8]] = []
        for bearing in [0.0, 90, 180, 270] {
            let camera = MapCamera.scenario(in: size, map: map).rotated(by: bearing)
            let p = MapProjection(camera: camera, size: size, tilt: GraphicsSettings.pitch(forTilt: 2))
            let renderer = ImageRenderer(content: Canvas { context, _ in
                VoxelModels.draw("manufacturing/2", projection: p, ground: CGPoint(x: 80, y: 100), floor: 0, unit: 5,
                                 in: &context)
            }.frame(width: size.width, height: size.height).background(Color.black))
            renderer.scale = 1
            let image = try XCTUnwrap(renderer.cgImage)
            var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
            let context = try XCTUnwrap(CGContext(data: &pixels, width: image.width, height: image.height, bitsPerComponent: 8,
                                                  bytesPerRow: image.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            XCTAssertGreaterThan(pixels.enumerated().filter { $0.offset % 4 != 3 && $0.element > 20 }.count, 200,
                                 "the building is drawn at bearing \(bearing)")
            images.append(pixels)
            if let data = UIImage(cgImage: image).pngData() {
                try? data.write(to: FileManager.default.temporaryDirectory
                    .appendingPathComponent("cw-snapshot-model-bearing-\(Int(bearing)).png"))
            }
        }
        for i in 0..<images.count {
            for j in (i + 1)..<images.count {
                XCTAssertNotEqual(images[i], images[j], "bearings \(i * 90) and \(j * 90) show different sides")
            }
        }
    }
}
