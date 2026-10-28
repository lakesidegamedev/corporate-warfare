import SwiftUI
import UIKit
import XCTest
@testable import CorporateWarfare

/// At the top speed the app must stay responsive: the phone shell is hosted in a real window,
/// time runs at 4x, and the test switches between every tab and opens and closes a sheet while
/// timing how long the main thread is held up. Before the run loop moved off the main actor and
/// coalesced its refreshes, each week republished every property one by one and navigation stalled.
@MainActor
final class MaxSpeedTests: XCTestCase {
    func testNavigationAndSheetsRespondAtMaxSpeed() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("CorporateWarfareMaxSpeed-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let store = AppStore(saveManager: SaveManager(directory: directory))
        await store.newCareer(guided: true, fiveYear: false, seed: 7)
        XCTAssertTrue(store.sessionReady)
        if let primary = store.overview?.primaryFirm { await store.selectFirm(primary) }

        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 393, height: 852)
        let host = UIHostingController(rootView: AnyView(RootView().environmentObject(store)))
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer { window.isHidden = true }

        let startWeek = store.overview?.tick ?? 0
        store.setSpeed(4)
        XCTAssertEqual(store.runSpeed, 4)

        var worstStall = 0.0
        /// Sleeps in short slices and records the longest the main actor was late to wake.
        func breathe(_ seconds: Double) async {
            let end = Date.timeIntervalSinceReferenceDate + seconds
            while Date.timeIntervalSinceReferenceDate < end {
                let before = Date.timeIntervalSinceReferenceDate
                try? await Task.sleep(nanoseconds: 20_000_000)
                worstStall = max(worstStall, Date.timeIntervalSinceReferenceDate - before - 0.02)
                // A decision or warning stops time; answer it the way a player would and run on.
                if store.overview?.pause != nil {
                    await store.continueCareer()
                    if store.runSpeed == 0 { store.setSpeed(4) }
                }
            }
        }

        await breathe(1)
        for tab in [AppTab.office, .markets, .inbox, .portfolio, .map, .office] {
            host.rootView = AnyView(RootView(initialTab: tab).id(tab).environmentObject(store))
            await breathe(0.6)
        }

        // A sheet opens and closes while time runs.
        if let division = store.divisions.first {
            store.openDivision = division
            var presented = false
            for _ in 0..<150 {
                if host.presentedViewController != nil { presented = true; break }
                await breathe(0.02)
            }
            XCTAssertTrue(presented, "the division sheet opens at 4x")
            store.openDivision = nil
            var dismissed = false
            for _ in 0..<150 {
                if host.presentedViewController == nil { dismissed = true; break }
                await breathe(0.02)
            }
            XCTAssertTrue(dismissed, "the division sheet closes at 4x")
        }

        let endWeek = store.overview?.tick ?? 0
        store.setSpeed(0)
        XCTAssertEqual(store.runSpeed, 0, "pause takes at once")
        XCTAssertGreaterThan(endWeek, startWeek + 2, "time really ran at 4x")
        XCTAssertLessThan(worstStall, 1.0, "the main thread was never held for a second at 4x")
        print("max-speed: weeks \(startWeek) to \(endWeek), worst main-thread stall \(Int(worstStall * 1000)) ms")
    }
}
