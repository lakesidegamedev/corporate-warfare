import SwiftUI
import UIKit
import XCTest
@testable import CorporateWarfare

/// The first time the map draws its structures layer must not hold up the main thread: the voxel
/// models are read in the background from launch, the renderer is warmed behind the menu, and the
/// layer then draws inside the budget even on the CI simulator.
@MainActor
final class FirstMapDrawTests: XCTestCase {
    func testFirstStructuresDrawIsInsideTheBudget() async throws {
        try await runFirstDraw()
    }

    /// The same run with the new GPU map switched on (it does not change the classic layers drawn
    /// here); the numbers are printed next to the default ones.
    func testFirstStructuresDrawWithTheNewMapOn() async throws {
        UserDefaults.standard.set(true, forKey: GraphicsKeys.newMap)
        defer { UserDefaults.standard.removeObject(forKey: GraphicsKeys.newMap) }
        try await runFirstDraw()
    }

    private func runFirstDraw() async throws {
        // What the app does at launch.
        let loadStart = Date.timeIntervalSinceReferenceDate
        VoxelModels.preload()
        var waited = 0
        while VoxelModels.models.isEmpty && waited < 500 {
            try? await Task.sleep(nanoseconds: 10_000_000)
            waited += 1
        }
        let loadSeconds = Date.timeIntervalSinceReferenceDate - loadStart
        XCTAssertTrue(VoxelModels.has("manufacturing/1"), "the models arrive in the background")
        let warmStart = Date.timeIntervalSinceReferenceDate
        RenderWarmup.run()
        let warmSeconds = Date.timeIntervalSinceReferenceDate - warmStart
        print("first-draw: models ready after \(Int(loadSeconds * 1000)) ms, renderer warm-up \(Int(warmSeconds * 1000)) ms")

        let size = CGSize(width: 393, height: 852)
        let map = WorldMap.shared
        let region = map.regions.first?.code ?? ""
        let divisions = [DivisionMarker(region: region, sector: "manufacturing", plants: 2)]
        let look = MapLook(detail: 2, tilt: GraphicsSettings.pitch(forTilt: 1), extrude: true, lighting: 2, dayNight: false,
                           cityLights: true, clouds: 0, traffic: 1, buildings: 2, frameRate: 60, animations: false,
                           showFps: false, ultra: false, earth: true)
        MainStalls.reset()
        MainStalls.enabled = true
        defer { MainStalls.enabled = false }
        // Framed on the scenario, as the map opens, then close in where the models stand.
        var cameras = [("scenario", MapCamera.scenario(in: size, map: map))]
        var close = MapCamera.scenario(in: size, map: map).zoomed(by: 2)
        if let anchor = map.anchor(of: region) {
            close.lon = anchor.lon * 180 / .pi
            close.lat = anchor.lat * 180 / .pi
        }
        cameras.append(("close", close))
        for (name, camera) in cameras {
            let scene = MapScene(map: map, camera: camera, look: look, tint: MapLayer.presence.tintRGB, shading: [:], heights: [:],
                                 selected: nil, divisions: divisions, sun: nil, displayScale: UIScreen.main.scale)
            let geometry = MapGeometry(map: map, camera: camera, size: size, tilt: look.tilt, detail: look.detail)
            let started = Date.timeIntervalSinceReferenceDate
            let view = Canvas { context, _ in
                MainStalls.time("first.\(name)") { scene.drawStatic(in: &context, geometry: geometry, phase: .structures) }
            }
            .drawingGroup()
            .frame(width: size.width, height: size.height)
            let renderer = ImageRenderer(content: view)
            renderer.scale = UIScreen.main.scale
            XCTAssertNotNil(renderer.cgImage)
            let seconds = Date.timeIntervalSinceReferenceDate - started
            print("first-draw: structures \(name) \(Int(seconds * 1000)) ms; \(MainStalls.summary())")
            XCTAssertLessThan(seconds, 1.5, "the first structures draw (\(name)) takes under 1.5 s")
        }
    }
}
