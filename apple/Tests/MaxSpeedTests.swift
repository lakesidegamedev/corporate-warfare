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

        MainStalls.reset()
        MainStalls.enabled = true
        defer { MainStalls.enabled = false }
        var worstStall = 0.0
        var phase = "start"
        var worstByPhase: [String: Double] = [:]
        var stalls: [(phase: String, start: Double, end: Double)] = []
        /// Sleeps in short slices and records the longest the main actor was late to wake.
        func breathe(_ seconds: Double, measured: Bool = true) async {
            let end = Date.timeIntervalSinceReferenceDate + seconds
            while Date.timeIntervalSinceReferenceDate < end {
                let before = Date.timeIntervalSinceReferenceDate
                try? await Task.sleep(nanoseconds: 20_000_000)
                let after = Date.timeIntervalSinceReferenceDate
                let late = after - before - 0.02
                if late > 0.1 { stalls.append((measured ? phase : phase + " (unmeasured)", before, after)) }
                if measured {
                    worstStall = max(worstStall, late)
                    worstByPhase[phase] = max(worstByPhase[phase] ?? 0, late)
                }
                // A decision or warning stops time; answer it the way a player would and run on.
                if store.overview?.pause != nil {
                    await store.continueCareer()
                    if store.runSpeed == 0 { store.setSpeed(4) }
                }
            }
        }

        await breathe(1, measured: false)
        await breathe(1)
        for tab in [AppTab.office, .markets, .inbox, .portfolio, .map, .office] {
            phase = "\(tab)"
            host.rootView = AnyView(RootView(initialTab: tab).id(tab).environmentObject(store))
            // The new tab's first render is the navigation itself, not the run loop holding the
            // main thread, so it settles unmeasured; the time after it is measured.
            await breathe(0.8, measured: false)
            await breathe(0.6)
        }

        // A sheet opens and closes while time runs.
        phase = "sheet"
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
        for stall in stalls.sorted(by: { $0.end - $0.start > $1.end - $1.start }).prefix(8) {
            let named = MainStalls.inside(from: stall.start, to: stall.end).prefix(4)
                .map { "\($0.label) \(Int($0.seconds * 1000)) ms" }.joined(separator: ", ")
            print("max-speed stall: \(Int((stall.end - stall.start - 0.02) * 1000)) ms in \(stall.phase); named work: \(named.isEmpty ? "none" : named)")
        }
        print("max-speed by phase: " + worstByPhase.keys.sorted().map { "\($0) \(Int((worstByPhase[$0] ?? 0) * 1000)) ms" }.joined(separator: ", "))
        print("max-speed named work: \(MainStalls.summary())")
        XCTAssertLessThan(worstStall, 0.5, "the main thread was never held for half a second at 4x")
        print("max-speed: weeks \(startWeek) to \(endWeek), worst main-thread stall \(Int(worstStall * 1000)) ms")
    }
}
