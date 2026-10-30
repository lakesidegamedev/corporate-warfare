import SwiftUI
import UIKit
import XCTest
@testable import CorporateWarfare

/// The new GPU map in the simulator: a career with the setting on, real frames through Metal, a
/// tap that picks a region, and snapshots of the whole map screen on a phone and an iPad. The
/// simulator's Metal may be software, so nothing here gates on speed; the numbers are printed.
@MainActor
final class GpuMapTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("CorporateWarfareGpuMap-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDown() {
        UserDefaults.standard.removeObject(forKey: GraphicsKeys.newMap)
        UserDefaults.standard.removeObject(forKey: GraphicsKeys.showFps)
    }

    private func career() async throws -> AppStore {
        let store = AppStore(saveManager: SaveManager(directory: directory))
        await store.newCareer(guided: true, fiveYear: false, seed: 7)
        XCTAssertTrue(store.sessionReady)
        if let primary = store.overview?.primaryFirm { await store.selectFirm(primary) }
        for _ in 0..<10 {
            await store.advance(weeks: 1)
            await store.continueCareer()
        }
        return store
    }

    private func wait(_ seconds: Double, until done: () -> Bool) async {
        let end = Date.timeIntervalSinceReferenceDate + seconds
        while !done() && Date.timeIntervalSinceReferenceDate < end {
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
    }

    private func distinctShades(_ pixels: [UInt8]) -> (shades: Int, commonShare: Double) {
        var counts: [UInt32: Int] = [:]
        var i = 0
        while i + 3 < pixels.count {
            counts[UInt32(pixels[i] >> 3) << 16 | UInt32(pixels[i + 1] >> 3) << 8 | UInt32(pixels[i + 2] >> 3), default: 0] += 1
            i += 4
        }
        let total = max(pixels.count / 4, 1)
        return (counts.count, Double(counts.values.max() ?? 0) / Double(total))
    }

    /// The engine draws the career's map through Metal: the frame is not the clear colour, and a
    /// tap on a region's anchor picks that region.
    func testGpuMapDrawsFramesAndPicksRegions() async throws {
        let store = try await career()
        let map = WorldMap.forRegions(store.boards.map(\.code))
        let scenario = try XCTUnwrap(map.scenarioId)
        let size = CGSize(width: 393, height: 852)
        let camera = GpuCamera(MapCamera.scenario(in: size, map: map), size: size, tilt: GraphicsSettings.pitch(forTilt: 1))
        let json = GpuMapUpdate.json(plants: store.mapPlants, divisions: [], layer: .presence, shading: [:],
                                     night: false, clouds: 2, week: 28)
        let anchors = map.regions.compactMap { r in map.anchor(of: r.code).map { (code: r.code, lon: $0.lon * 180 / .pi, lat: $0.lat * 180 / .pi) } }
        let started = Date.timeIntervalSinceReferenceDate
        let frame = try XCTUnwrap(
            GpuMapProbe.frame(scenario: scenario, camera: camera, width: 786, height: 1704, updateJSON: json, anchors: anchors),
            "the engine opens the GPU map and draws a frame")
        let seconds = Date.timeIntervalSinceReferenceDate - started
        let stats = distinctShades(frame.rgba)
        print("gpu-map: readback frame on \(frame.adapter), open and first frame \(Int(seconds * 1000)) ms, \(stats.shades) shades, most common \(Int(stats.commonShare * 100)) percent, \(frame.picks.count) of \(anchors.count) anchors picked")
        XCTAssertGreaterThan(stats.shades, 12, "the frame has terrain, not one colour")
        XCTAssertLessThan(stats.commonShare, 0.6, "the frame is not the clear colour")
        XCTAssertFalse(frame.picks.isEmpty, "a tap on a region picks one")
        for (code, picked) in frame.picks { XCTAssertTrue(map.regions.contains { $0.code == picked }, "\(code) picks a real region (\(picked))") }
        XCTAssertGreaterThan(frame.picks.filter { $0.key == $0.value }.count, anchors.count / 2, "most anchors pick their own region")
    }

    /// The layer path: a Metal view in a window, drawn on the render thread and presented with no
    /// readback, then the whole map screen with the setting on, on a phone and an iPad.
    func testMapScreenWithTheNewMapOn() async throws {
        let store = try await career()
        let map = WorldMap.forRegions(store.boards.map(\.code))
        let scenario = try XCTUnwrap(map.scenarioId)

        // The Metal view alone, in its own window.
        let session = GpuMapSession(scenario: scenario)
        defer { session.close() }
        let size = CGSize(width: 393, height: 852)
        let lone = makeWindow(size)
        session.view.frame = CGRect(origin: .zero, size: size)
        lone.rootViewController = UIViewController()
        lone.rootViewController?.view.addSubview(session.view)
        lone.makeKeyAndVisible()
        let opened = Date.timeIntervalSinceReferenceDate
        await wait(150) { session.isReady || session.failure != nil }
        XCTAssertNil(session.failure, "the GPU map opens on the simulator: \(session.failure ?? "")")
        XCTAssertTrue(session.isReady)
        print("gpu-map: layer path ready after \(Int((Date.timeIntervalSinceReferenceDate - opened) * 1000)) ms on \(session.adapter)")
        session.update(GpuMapUpdate.json(plants: store.mapPlants, divisions: [], layer: .presence, shading: [:],
                                         night: false, clouds: 2, week: 28))
        let camera = GpuCamera(MapCamera.scenario(in: size, map: map), size: size, tilt: GraphicsSettings.pitch(forTilt: 1))
        session.request(camera: camera, size: size)
        await wait(60) { session.framesPresented >= 3 }
        XCTAssertGreaterThanOrEqual(session.framesPresented, 3, "frames are presented to the layer")
        XCTAssertNil(session.failure)
        XCTAssertNotNil(session.shown, "the frame on screen has its camera")
        // A tap on the first region's anchor, projected by the camera of the frame shown.
        if let region = map.regions.first, let a = map.anchor(of: region.code), let shown = session.shown {
            let xy = session.project(camera: shown.camera, size: shown.size, lonLat: [a.lon * 180 / .pi, a.lat * 180 / .pi])
            XCTAssertFalse(xy[0].isNaN)
            let picked = session.pick(camera: shown.camera, size: shown.size, at: CGPoint(x: CGFloat(xy[0]), y: CGFloat(xy[1])))
            XCTAssertNotNil(picked, "a tap on a region picks one")
        }
        print("gpu-map: \(session.line); \(session.framesPresented) frames presented")
        // Moving the camera draws a new frame.
        let before = session.framesPresented
        var moved = MapCamera.scenario(in: size, map: map)
        moved.lon += 0.5
        session.request(camera: GpuCamera(moved, size: size, tilt: GraphicsSettings.pitch(forTilt: 1)), size: size)
        await wait(30) { session.framesPresented > before }
        XCTAssertGreaterThan(session.framesPresented, before, "a new camera draws a new frame")
        lone.isHidden = true
        session.close()

        // The map screen with the setting on.
        UserDefaults.standard.set(true, forKey: GraphicsKeys.newMap)
        UserDefaults.standard.set(true, forKey: GraphicsKeys.showFps)
        var previous: GpuMapSession? = session
        for (name, size, compact) in [("phone", CGSize(width: 393, height: 852), true), ("pad", CGSize(width: 1180, height: 820), false)] {
            let window = makeWindow(size)
            let root = MapHomeView().environmentObject(store)
                .environment(\.horizontalSizeClass, compact ? .compact : .regular)
                .preferredColorScheme(.dark)
                .frame(width: size.width, height: size.height)
            let host = UIHostingController(rootView: root)
            host.overrideUserInterfaceStyle = .dark
            window.rootViewController = host
            window.makeKeyAndVisible()
            host.view.frame = window.bounds
            host.view.layoutIfNeeded()
            await wait(150) { GpuMapSession.lastOpened.map { $0 !== previous && ($0.isReady || $0.failure != nil) } ?? false }
            let live = GpuMapSession.lastOpened
            previous = live
            XCTAssertNotNil(live, "\(name): the map screen opens the GPU map when the setting is on")
            XCTAssertNil(live?.failure, "\(name): \(live?.failure ?? "")")
            await wait(60) { (live?.framesPresented ?? 0) >= 3 }
            XCTAssertGreaterThanOrEqual(live?.framesPresented ?? 0, 3, "\(name): the map screen presents frames")
            await wait(2) { false }
            print("gpu-map: \(name) screen: \(live?.line ?? "none"), \(live?.framesPresented ?? 0) frames")
            let format = UIGraphicsImageRendererFormat.default()
            format.opaque = true
            let image = UIGraphicsImageRenderer(size: size, format: format).image { _ in
                host.view.drawHierarchy(in: CGRect(origin: .zero, size: size), afterScreenUpdates: true)
            }
            if let data = image.pngData() {
                try data.write(to: FileManager.default.temporaryDirectory.appendingPathComponent("cw-snapshot-gpu-map-\(name).png"))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "gpu-map-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
            window.isHidden = true
            window.rootViewController = nil
            await wait(1) { false }
        }
        UserDefaults.standard.set(false, forKey: GraphicsKeys.newMap)
    }

    private func makeWindow(_ size: CGSize) -> UIWindow {
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(origin: .zero, size: size)
        return window
    }
}
