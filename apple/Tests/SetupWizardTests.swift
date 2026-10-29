import SwiftUI
import UIKit
import XCTest
@testable import CorporateWarfare

/// First-run setup: the preset advice, which players see which screen, and a picture of every step
/// (collected from `cw-snapshot-setup-*.png` by the Apple workflow).
@MainActor
final class SetupWizardTests: XCTestCase {
    func testPresetFollowsTheFrameTime() {
        XCTAssertEqual(SetupAdvice.recommend(frameMs: 5, screenPixels: 1_000_000), .high)
        XCTAssertEqual(SetupAdvice.recommend(frameMs: 40, screenPixels: 1_000_000), .medium)
        XCTAssertEqual(SetupAdvice.recommend(frameMs: 200, screenPixels: 1_000_000), .low)
        // The same frame time costs more on a bigger screen.
        XCTAssertEqual(SetupAdvice.recommend(frameMs: 30, screenPixels: 400_000), .high)
        XCTAssertEqual(SetupAdvice.recommend(frameMs: 30, screenPixels: 2_000_000), .medium)
        for ms in [0.1, 1, 10] {
            XCTAssertNotEqual(SetupAdvice.recommend(frameMs: ms, screenPixels: 1_000_000), .ultra)
        }
        XCTAssertEqual(SetupAdvice.defaultPreset, .high)
    }

    func testGateTellsNewInstallsFromUpgrades() {
        func defaults() -> UserDefaults {
            let suite = UserDefaults(suiteName: "cw-setup-\(UUID().uuidString)")!
            return suite
        }
        let fresh = defaults()
        SetupGate.prepare(fresh)
        XCTAssertEqual(fresh.string(forKey: SetupKeys.state), SetupKeys.fresh)
        // Still fresh after a second launch that did not finish setup.
        GraphicsSettings.migrate(fresh)
        SetupGate.prepare(fresh)
        XCTAssertEqual(fresh.string(forKey: SetupKeys.state), SetupKeys.fresh)

        let old = defaults()
        old.set("medium", forKey: GraphicsKeys.preset)
        SetupGate.prepare(old)
        XCTAssertEqual(old.string(forKey: SetupKeys.state), SetupKeys.upgrade)

        let testing = defaults()
        SetupGate.prepare(testing, uiTesting: true)
        XCTAssertEqual(testing.string(forKey: SetupKeys.state), SetupKeys.done)
    }

    func testEveryStepRenders() throws {
        let store = AppStore(saveManager: SaveManager(directory: FileManager.default.temporaryDirectory
            .appendingPathComponent("CorporateWarfareSetupTests-\(UUID().uuidString)", isDirectory: true)))
        let sizes: [(String, CGSize, Bool)] = [("phone", CGSize(width: 393, height: 852), true), ("pad", CGSize(width: 1180, height: 820), false)]
        for (label, size, compact) in sizes {
            for step in SetupStep.allCases {
                let view = SetupWizardView(firstRun: true, startStep: step, onFinish: {}).environmentObject(store)
                try render("setup-\(label)-\(step)", size, compact: compact, view)
            }
            try render("setup-\(label)-whatsnew", size, compact: compact, WhatsNewView(onRunSetup: {}, onContinue: {}))
        }
    }

    private func render<V: View>(_ name: String, _ size: CGSize, compact: Bool, _ view: V) throws {
        let root = view
            .environment(\.horizontalSizeClass, compact ? .compact : .regular)
            .preferredColorScheme(.dark)
            .frame(width: size.width, height: size.height)
        let host = UIHostingController(rootView: root)
        host.overrideUserInterfaceStyle = .dark
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(origin: .zero, size: size)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.frame = window.bounds
        host.view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(1.2))
        let format = UIGraphicsImageRendererFormat.default()
        format.opaque = true
        let image = UIGraphicsImageRenderer(size: size, format: format).image { _ in
            host.view.drawHierarchy(in: CGRect(origin: .zero, size: size), afterScreenUpdates: true)
        }
        window.isHidden = true
        let data = try XCTUnwrap(image.pngData(), "setup render \(name) did not encode")
        try data.write(to: FileManager.default.temporaryDirectory.appendingPathComponent("cw-snapshot-\(name).png"))
        let attachment = XCTAttachment(image: image)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
