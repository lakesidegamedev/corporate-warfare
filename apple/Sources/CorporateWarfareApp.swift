import SwiftUI

@main
struct CorporateWarfareApp: App {
    @StateObject private var store: AppStore
    @Environment(\.scenePhase) private var scenePhase

    private static var isUITesting: Bool {
        ProcessInfo.processInfo.arguments.contains("--ui-testing")
    }

    init() {
        // Before any view reads a graphics option, so a fresh install starts on High.
        GraphicsSettings.migrate()
        if Self.isUITesting {
            let directory = FileManager.default.temporaryDirectory
                .appendingPathComponent("CorporateWarfareUITests", isDirectory: true)
            try? FileManager.default.removeItem(at: directory)
            _store = StateObject(wrappedValue: AppStore(saveManager: SaveManager(directory: directory)))
        } else {
            _store = StateObject(wrappedValue: AppStore())
        }
    }

    var body: some Scene {
        WindowGroup {
            Group {
                if store.sessionReady {
                    RootView()
                        .environmentObject(store)
                } else {
                    StartView()
                        .environmentObject(store)
                }
            }
            .tint(Theme.gold)
            .preferredColorScheme(.dark)
            .task {
                // The app opens on the main menu; Continue there resumes the latest save.
                store.refreshSaves()
            }
            .onChange(of: scenePhase) { _, phase in
                // Background asks for one bounded stop-and-save. Foreground never replays an
                // accepted command; it only re-reads the live session.
                Task { await store.handleScenePhase(phase) }
            }
            .sheet(isPresented: $store.showOnboarding) {
                OnboardingView()
                    .environmentObject(store)
            }
        }
    }
}
