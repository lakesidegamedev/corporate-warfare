import ImageIO
import MetalKit
import SwiftUI
import UIKit
import XCTest
@testable import CorporateWarfare

/// Renders the main screens of a live career into PNG snapshots for review. Each screen is hosted
/// in a real window so navigation stacks, scroll views and the Canvas map draw as on a device.
/// Files go to the simulator's temporary directory as `cw-snapshot-*.png`; the Apple workflow
/// collects them as an artifact.
@MainActor
final class SnapshotTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("CorporateWarfareSnapshots-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    func testRenderMainScreens() async throws {
        let store = AppStore(saveManager: SaveManager(directory: directory))
        await store.newCareer(guided: true, fiveYear: false, seed: 7)
        XCTAssertTrue(store.sessionReady)
        // Ten weeks so plants run, gateways record imports and the map has room to show.
        for _ in 0..<10 {
            await store.advance(weeks: 1)
            await store.continueCareer()
        }
        if let primary = store.overview?.primaryFirm {
            await store.selectFirm(primary)
        }
        XCTAssertFalse(store.mapPlants.isEmpty, "the map has plants")
        XCTAssertEqual(store.boards.count, 31)
        XCTAssertEqual(WorldMap.shared.regions.count, 31, "the bundled map decodes its regions")
        XCTAssertEqual(WorldMap.forScenario("texas-1981").regions.count, 22, "the bundled map decodes the Texas regions")
        XCTAssertEqual(WorldMap.forRegions(["TX-HOU"]).regions.count, 22, "Texas region codes pick the Texas map")
        XCTAssertEqual(WorldMap.forScenario("silicon-valley-1995").regions.count, 16, "the bundled map decodes the California regions")
        XCTAssertEqual(WorldMap.forRegions(["CA-SJC"]).regions.count, 16, "California region codes pick the California map")
        XCTAssertEqual(WorldMap.forScenario("ruhr-rhine-1991").regions.count, 18, "the bundled map decodes the Ruhr and Rhine regions")
        XCTAssertEqual(WorldMap.forRegions(["DE-RUH"]).regions.count, 18, "Ruhr region codes pick the Ruhr map")
        XCTAssertEqual(WorldMap.forRegions(store.boards.map(\.code)).regions.count, 31, "a Great Lakes career keeps its map")
        XCTAssertFalse(store.mapDivisions.isEmpty, "the player's division is on the map")
        XCTAssertGreaterThanOrEqual(store.stocks.count, 20, "the stock board lists the corridor's companies")
        XCTAssertFalse(store.commodities.isEmpty, "the commodity board has goods")
        XCTAssertEqual(store.stocks.first?.historyCents.count, 10, "ten weekly closes after ten weeks")
        XCTAssertEqual(store.divisions.count, 1)

        // The cloud textures load in the background; wait for them so the map shows its clouds.
        Pixel.configureSystemBars()
        for _ in 0..<100 {
            if CloudRenderer.shared?.ready(["clouds", "clouds-great-lakes-2000"]) ?? true { break }
            try await Task.sleep(nanoseconds: 100_000_000)
        }
        XCTAssertNotNil(CloudRenderer.shared, "Metal is available for the clouds")
        XCTAssertTrue(CloudRenderer.shared?.ready(["clouds", "clouds-great-lakes-2000"]) ?? false, "the cloud textures load")
        renderClouds()
        let phone = CGSize(width: 393, height: 852)
        let pad = CGSize(width: 1180, height: 820)
        let home = store.boards.first { $0.code == store.divisions.first?.region }
        render("map-phone", phone, compact: true, MapHomeView().environmentObject(store))
        render("map-pad", pad, compact: false, MapHomeView().environmentObject(store))
        renderCloseMaps(store, pad)
        if let home {
            render("region-phone", phone, compact: true,
                   RegionBoardView(board: home, onBuild: {}, onClose: {}).environmentObject(store))
            render("build-phone", phone, compact: true,
                   BuildFlowView(regionCode: home.code).environmentObject(store))
        }
        render("ceo-phone", phone, compact: true, CEOOfficeView().environmentObject(store))
        render("ceo-pad", pad, compact: false, CEOOfficeView().environmentObject(store))
        render("markets-phone", phone, compact: true, MarketsView().environmentObject(store))
        render("markets-commodities-phone", phone, compact: true, MarketsView(initialSection: .commodities).environmentObject(store))
        render("markets-regions-phone", phone, compact: true, MarketsView(initialSection: .regions).environmentObject(store))
        if let stock = store.stocks.first(where: { $0.controlledByYou }) ?? store.stocks.first {
            render("stock-sheet-phone", phone, compact: true, StockSheet(stock: stock).environmentObject(store))
        }
        render("markets-pad", pad, compact: false, MarketsView().environmentObject(store))
        render("inbox-phone", phone, compact: true, InboxView().environmentObject(store))
        render("root-phone", phone, compact: true, RootView().environmentObject(store))
        // Each tab inside the real tab bar, so content against the bar is reviewed as shipped.
        render("tab-office-phone", phone, compact: true, RootView(initialTab: .office).environmentObject(store))
        render("tab-markets-phone", phone, compact: true, RootView(initialTab: .markets).environmentObject(store))
        render("tab-inbox-phone", phone, compact: true, RootView(initialTab: .inbox).environmentObject(store))
        render("tab-portfolio-phone", phone, compact: true, RootView(initialTab: .portfolio).environmentObject(store))
        // The menu's globe with its clouds, and the settings with the Clouds option and credits.
        render("menu-phone", phone, compact: true, StartView().environmentObject(store))
        render("menu-pad", pad, compact: false, StartView().environmentObject(store))
        render("settings-phone", phone, compact: true,
               ScrollView { GraphicsSettingsPanel().padding(16) }.background(Theme.ink))
        if let home { renderSplitView(store, region: home.code) }
    }

    /// The app in iPad Split View and Slide Over on a 13-inch iPad (1032 by 1376 points): a third,
    /// a half and two thirds of the screen in portrait and landscape, with the size class iPadOS
    /// gives each. Compact slices use the phone layout, regular ones the sidebar shell.
    private func renderSplitView(_ store: AppStore, region: String) {
        let slices: [(name: String, size: CGSize, compact: Bool)] = [
            ("portrait-third", CGSize(width: 320, height: 1376), true),
            ("portrait-half", CGSize(width: 507, height: 1376), true),
            ("portrait-two-thirds", CGSize(width: 694, height: 1376), true),
            ("landscape-third", CGSize(width: 375, height: 1032), true),
            ("landscape-half", CGSize(width: 683, height: 1032), false),
            ("landscape-two-thirds", CGSize(width: 993, height: 1032), false),
        ]
        let firm = store.selectedFirm ?? store.overview?.primaryFirm
        for slice in slices {
            let name = "split-\(slice.name)"
            if slice.compact {
                render("\(name)-map", slice.size, compact: true, RootView(initialTab: .map).environmentObject(store))
                render("\(name)-markets", slice.size, compact: true, RootView(initialTab: .markets).environmentObject(store))
            } else {
                render("\(name)-map", slice.size, compact: false, DesktopShell(initialPage: .map).environmentObject(store))
                render("\(name)-markets", slice.size, compact: false, DesktopShell(initialPage: .markets).environmentObject(store))
            }
            if let firm {
                render("\(name)-company", slice.size, compact: slice.compact, CompanyPageSheet(firm: firm).environmentObject(store))
            }
            render("\(name)-build", slice.size, compact: slice.compact, BuildFlowView(regionCode: region).environmentObject(store))
        }
    }

    /// The still map past the zoom where models stand, north up and turned, so the voxel
    /// buildings, the rivals' muted ones, the city blocks, the vehicles, the countryside on the
    /// satellite Earth and the bearing are reviewed; once more in midwinter and once with the
    /// Earth off. Labels must stay on top.
    private func renderCloseMaps(_ store: AppStore, _ size: CGSize) {
        let map = WorldMap.forRegions(store.boards.map(\.code))
        let grouped = Dictionary(grouping: store.mapDivisions) { $0.region + "/" + $0.industry }
        let divisions = grouped.values.compactMap { items -> DivisionMarker? in
            guard let first = items.first else { return nil }
            return DivisionMarker(region: first.region, sector: first.industry, plants: Int(items.reduce(UInt32(0)) { $0 + $1.blocks }))
        }
        let home = divisions.first.flatMap { map.anchor(of: $0.region) }
        var look = MapLook(detail: 2, tilt: GraphicsSettings.pitch(forTilt: 1), extrude: true, lighting: 2, dayNight: false,
                           cityLights: true, clouds: 0, traffic: 1, buildings: 2, frameRate: 60, animations: false,
                           showFps: false, ultra: false, earth: true)
        var framed = MapCamera.scenario(in: size, map: map).zoomed(by: 2)
        if let home {
            framed.lon = home.lon * 180 / .pi
            framed.lat = home.lat * 180 / .pi
        }
        Countryside.prepare(map)
        let now = Earth.week(of: store.overview?.weekLabel)
        // The bearings show the buildings' sides turning; the pan pair shows the roads staying on
        // the ground as the camera moves; the Freight mode shows its arcs and nothing else moving.
        let panned = framed.panned(by: CGSize(width: 140, height: 90), in: size, pitch: 0.4)
        let shots: [(String, MapCamera, Int, Bool, Bool)] = [
            ("map-close-pad", framed, now, true, false), ("map-turned-pad", framed.rotated(by: 35), now, true, false),
            ("map-bearing-90-pad", framed.rotated(by: 90), now, true, false),
            ("map-bearing-180-pad", framed.rotated(by: 180), now, true, false),
            ("map-bearing-270-pad", framed.rotated(by: 270), now, true, false),
            ("map-pan-a-pad", framed.zoomed(by: 1.6), now, true, false),
            ("map-pan-b-pad", panned.zoomed(by: 1.6), now, true, false),
            ("map-vehicles-pad", framed.zoomed(by: 1.6), now, true, false), ("map-winter-pad", framed, 2, true, false),
            ("map-close-plain-pad", framed, now, false, false), ("map-freight-pad", framed, now, true, true),
        ]
        for (name, camera, week, earth, freight) in shots {
            look.earth = earth
            let scene = MapScene(map: map, camera: camera, look: look, tint: MapLayer.presence.tintRGB, shading: [:], heights: [:],
                                 selected: nil, divisions: divisions, lanes: store.lanes, freightLayer: freight, sun: nil,
                                 plants: store.mapPlants,
                                 economyPhase: store.economy?.phase, week: week, displayScale: UIScreen.main.scale)
            let geometry = MapGeometry(map: map, camera: camera, size: size, tilt: look.tilt, detail: look.detail)
            XCTAssertTrue(geometry.closeUp, "\(name) is close enough for models")
            let ground = EarthPainter().snapshot(EarthView(camera: camera, size: size, tilt: look.tilt, week: week,
                                                           bounds: map.playBounds, scenario: map.scenarioId),
                                                 scale: UIScreen.main.scale)
            if earth { XCTAssertNotNil(ground, "\(name) has the satellite Earth") }
            render(name, size, compact: false, ZStack {
                Canvas { context, _ in scene.drawStatic(in: &context, geometry: geometry, phase: .sky) }
                if earth, let ground {
                    Image(decorative: ground, scale: UIScreen.main.scale).resizable().frame(width: size.width, height: size.height)
                }
                Canvas { context, _ in
                    scene.drawStatic(in: &context, geometry: geometry, phase: .ground)
                    scene.drawOverlay(in: &context, geometry: geometry, seconds: 20, part: .traffic)
                    scene.drawStatic(in: &context, geometry: geometry, phase: .structures)
                    scene.drawOverlay(in: &context, geometry: geometry, seconds: 20, part: .effects)
                }
            }.background(MapScene.spaceColor.color()))
        }
    }

    /// The clouds alone, drawn off screen over a dark sea, on the globe and over the scenario.
    private func renderClouds() {
        let size = CGSize(width: 600, height: 400)
        let map = WorldMap.shared
        let painter = CloudPainter()
        let cameras = [("clouds-globe", MapCamera(lon: -88, lat: 38, zoom: 1.2)),
                       ("clouds-scenario", MapCamera.scenario(in: size, map: map))]
        for (name, camera) in cameras {
            let p = MapProjection(camera: camera, size: size, tilt: 28 * .pi / 180)
            let sky = CloudSky(camera: camera, size: size, tilt: 28 * .pi / 180, sun: MapSun.fixed(p), seconds: 20,
                               amount: 3, bounds: map.playBounds, altitude: 46 * p.t, zoom: camera.zoom,
                               shadows: true, scenario: map.scenarioId)
            guard let clouds = painter.snapshot(sky) else {
                XCTFail("\(name) did not render")
                continue
            }
            let image = UIGraphicsImageRenderer(size: size).image { context in
                UIColor(red: 0.086, green: 0.208, blue: 0.333, alpha: 1).setFill()
                context.fill(CGRect(origin: .zero, size: size))
                UIImage(cgImage: clouds).draw(in: CGRect(origin: .zero, size: size))
            }
            XCTAssertGreaterThan(distinctShades(image), 8, "\(name) shows clouds")
            if let data = image.pngData() {
                try? data.write(to: FileManager.default.temporaryDirectory.appendingPathComponent("cw-snapshot-\(name).png"))
            }
        }
    }

    private func render<V: View>(_ name: String, _ size: CGSize, compact: Bool, _ view: V) {
        let image = capture(size, scale: UIScreen.main.scale, opaque: false, compact: compact, view)
        guard let data = image.pngData() else {
            return XCTFail("snapshot \(name) did not encode")
        }
        XCTAssertGreaterThan(distinctShades(image), 8, "snapshot \(name) rendered a blank image")
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("cw-snapshot-\(name).png")
        try? data.write(to: url)
        let attachment = XCTAttachment(image: image)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// App Store screenshots at Apple's required display sizes. The iPhone is 440 x 956 pt at 3x
    /// (1320 x 2868) and the iPad 1032 x 1376 pt at 2x (2064 x 2752). App Store Connect rejects a
    /// PNG that carries an alpha channel, so each image is flattened to RGB before it is written.
    func testRenderStoreScreenshots() async throws {
        let store = AppStore(saveManager: SaveManager(directory: directory))
        await store.newCareer(guided: true, fiveYear: false, seed: 7)
        XCTAssertTrue(store.sessionReady)
        // Ten weeks so plants run, gateways record imports and the map has room to show.
        for _ in 0..<10 {
            await store.advance(weeks: 1)
            await store.continueCareer()
        }
        if let primary = store.overview?.primaryFirm {
            await store.selectFirm(primary)
        }
        XCTAssertFalse(store.mapPlants.isEmpty, "the map has plants")

        let phone = CGSize(width: 440, height: 956)
        let pad = CGSize(width: 1032, height: 1376)
        let controlled = store.stocks.first { $0.controlledByYou } ?? store.stocks.first

        storeShot("iphone", "01-map", phone, 3, compact: true, RootView(initialTab: .map).environmentObject(store))
        storeShot("iphone", "02-office", phone, 3, compact: true, RootView(initialTab: .office).environmentObject(store))
        storeShot("iphone", "03-markets", phone, 3, compact: true, RootView(initialTab: .markets).environmentObject(store))
        if let stock = controlled {
            storeShot("iphone", "04-company", phone, 3, compact: true, StockSheet(stock: stock).environmentObject(store))
        }
        storeShot("iphone", "05-inbox", phone, 3, compact: true, RootView(initialTab: .inbox).environmentObject(store))
        storeShot("iphone", "06-portfolio", phone, 3, compact: true, RootView(initialTab: .portfolio).environmentObject(store))

        storeShot("ipad", "01-map", pad, 2, compact: false, RootView(initialTab: .map).environmentObject(store))
        storeShot("ipad", "02-office", pad, 2, compact: false, RootView(initialTab: .office).environmentObject(store))
        storeShot("ipad", "03-markets", pad, 2, compact: false, RootView(initialTab: .markets).environmentObject(store))
    }

    private func storeShot<V: View>(_ device: String, _ name: String, _ size: CGSize, _ scale: CGFloat, compact: Bool, _ view: V) {
        let image = capture(size, scale: scale, opaque: true, compact: compact, view)
        let pixels = CGSize(width: size.width * scale, height: size.height * scale)
        guard let data = storePNG(image, pixels: pixels) else {
            return XCTFail("store screenshot \(device)-\(name) did not encode")
        }
        // PNG header: width and height follow the 16-byte signature and chunk header, then bit
        // depth and colour type. Colour type 2 is RGB; 6 would mean the alpha channel survived.
        let width = data[16..<20].reduce(0) { $0 << 8 | Int($1) }
        let height = data[20..<24].reduce(0) { $0 << 8 | Int($1) }
        XCTAssertEqual(width, Int(pixels.width), "\(device)-\(name) width")
        XCTAssertEqual(height, Int(pixels.height), "\(device)-\(name) height")
        XCTAssertEqual(data[25], UInt8(2), "\(device)-\(name) must be RGB without an alpha channel")
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("cw-store-\(device)-\(name).png")
        try? data.write(to: url)
    }

    /// Redraw onto an opaque RGB canvas; App Store Connect rejects screenshots with transparency.
    private func storePNG(_ image: UIImage, pixels: CGSize) -> Data? {
        guard let cg = image.cgImage,
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let context = CGContext(
                  data: nil, width: Int(pixels.width), height: Int(pixels.height),
                  bitsPerComponent: 8, bytesPerRow: 0, space: space,
                  bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
              ) else { return nil }
        let bounds = CGRect(origin: .zero, size: pixels)
        context.setFillColor(UIColor.black.cgColor)
        context.fill(bounds)
        context.interpolationQuality = .high
        context.draw(cg, in: bounds)
        guard let flat = context.makeImage() else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data as CFMutableData, "public.png" as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(destination, flat, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return data as Data
    }

    private func capture<V: View>(_ size: CGSize, scale: CGFloat, opaque: Bool, compact: Bool, _ view: V) -> UIImage {
        let root = view
            .environment(\.horizontalSizeClass, compact ? .compact : .regular)
            .preferredColorScheme(.dark)
            .frame(width: size.width, height: size.height)
        let host = UIHostingController(rootView: root)
        host.overrideUserInterfaceStyle = .dark
        // A window outside a scene is never drawn on iOS 13 and later; attach it to the test
        // host's scene or every snapshot comes back blank.
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(origin: .zero, size: size)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.frame = window.bounds
        host.view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.6))
        // A window snapshot cannot read a Metal layer, so each cloud layer's own last frame is
        // shown in an image view inside it for the capture, at the layer's place in the stack.
        var stand: [UIView] = []
        for view in metalViews(in: window) {
            guard let painter = view.delegate as? CloudPainter, let sky = painter.sky else { continue }
            XCTAssertGreaterThan(painter.drawn, 0, "the cloud layer drew in the window (\(painter.missed) draws found no drawable)")
            guard let frame = painter.snapshot(sky) else { continue }
            let still = UIImageView(image: UIImage(cgImage: frame))
            still.frame = view.bounds
            view.addSubview(still)
            stand.append(still)
        }
        let format = UIGraphicsImageRendererFormat.default()
        format.scale = scale
        format.opaque = opaque
        let image = UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
        stand.forEach { $0.removeFromSuperview() }
        window.isHidden = true
        return image
    }

    private func metalViews(in view: UIView) -> [MTKView] {
        (view as? MTKView).map { [$0] } ?? view.subviews.flatMap { metalViews(in: $0) }
    }

    /// Distinct grey levels across a coarse grid of samples. A blank or unrendered window has one.
    private func distinctShades(_ image: UIImage) -> Int {
        guard let cg = image.cgImage else { return 0 }
        let side = 48
        var pixels = [UInt8](repeating: 0, count: side * side)
        guard let context = CGContext(
            data: &pixels, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side,
            space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue
        ) else { return 0 }
        context.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
        return Set(pixels).count
    }
}
