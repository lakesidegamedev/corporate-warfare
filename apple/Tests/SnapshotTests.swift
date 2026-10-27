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
        XCTAssertEqual(WorldMap.forRegions(store.boards.map(\.code)).regions.count, 31, "a Great Lakes career keeps its map")
        XCTAssertFalse(store.mapDivisions.isEmpty, "the player's division is on the map")
        XCTAssertGreaterThanOrEqual(store.stocks.count, 20, "the stock board lists the corridor's companies")
        XCTAssertFalse(store.commodities.isEmpty, "the commodity board has goods")
        XCTAssertEqual(store.stocks.first?.historyCents.count, 10, "ten weekly closes after ten weeks")
        XCTAssertEqual(store.divisions.count, 1)

        let phone = CGSize(width: 393, height: 852)
        let pad = CGSize(width: 1180, height: 820)
        let home = store.boards.first { $0.code == store.divisions.first?.region }
        render("map-phone", phone, compact: true, MapHomeView().environmentObject(store))
        render("map-pad", pad, compact: false, MapHomeView().environmentObject(store))
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
    }

    private func render<V: View>(_ name: String, _ size: CGSize, compact: Bool, _ view: V) {
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
        let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
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
        window.isHidden = true
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
