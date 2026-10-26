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
        XCTAssertEqual(store.states.count, 8)
        XCTAssertGreaterThan(CorridorMap.shared.playStates.count, 7, "the bundled map decodes")

        let phone = CGSize(width: 393, height: 852)
        let pad = CGSize(width: 1180, height: 820)
        let ohio = store.states.first { $0.code == "OH" }
        render("map-phone", phone, compact: true, MapHomeView().environmentObject(store))
        render("map-pad", pad, compact: false, MapHomeView().environmentObject(store))
        if let ohio {
            render("state-phone", phone, compact: true,
                   StateBoardView(state: ohio, onBuild: {}, onClose: {}).environmentObject(store))
            render("build-phone", phone, compact: true,
                   BuildFlowView(stateCode: ohio.code).environmentObject(store))
        }
        render("ceo-phone", phone, compact: true, CEOOfficeView().environmentObject(store))
        render("ceo-pad", pad, compact: false, CEOOfficeView().environmentObject(store))
        render("markets-phone", phone, compact: true, MarketsView().environmentObject(store))
        render("markets-pad", pad, compact: false, MarketsView().environmentObject(store))
        render("inbox-phone", phone, compact: true, InboxView().environmentObject(store))
        render("root-phone", phone, compact: true, RootView().environmentObject(store))
    }

    private func render<V: View>(_ name: String, _ size: CGSize, compact: Bool, _ view: V) {
        let root = view
            .environment(\.horizontalSizeClass, compact ? .compact : .regular)
            .preferredColorScheme(.dark)
            .frame(width: size.width, height: size.height)
        let host = UIHostingController(rootView: root)
        host.overrideUserInterfaceStyle = .dark
        let window = UIWindow(frame: CGRect(origin: .zero, size: size))
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
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("cw-snapshot-\(name).png")
        try? data.write(to: url)
        let attachment = XCTAttachment(image: image)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        window.isHidden = true
    }
}
