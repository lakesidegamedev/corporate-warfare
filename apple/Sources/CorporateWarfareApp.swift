import SwiftUI

@main
struct CorporateWarfareApp: App {
    @StateObject private var store: AppStore
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage(SetupKeys.state) private var setupState = SetupKeys.done

    private static var isUITesting: Bool {
        ProcessInfo.processInfo.arguments.contains("--ui-testing")
    }

    init() {
        // Before GraphicsSettings.migrate() writes a preset, so a player from 1.8 is told apart from a new install.
        SetupGate.prepare(uiTesting: Self.isUITesting)
        // Before any view reads a graphics option, so a fresh install starts on High.
        GraphicsSettings.migrate()
        // Solid pixel-styled system bars: no glass, no blur.
        Pixel.configureSystemBars()
        // Start the shared clock at launch, so the sun begins over the Americas at noon.
        _ = MapClock.now
        // Read in the background now, so the first map draw never waits for the models.
        VoxelModels.preload()
        // The world map decodes off the main thread; the menu shows its globe once it is ready.
        WorldMap.prewarm()
        // The office's art and portraits too, so the first visit to the office is not a stall.
        UiWarmup.start()
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
                if setupState == SetupKeys.fresh {
                    SetupWizardView(firstRun: true, onFinish: { setupState = SetupKeys.done })
                        .environmentObject(store)
                } else if setupState == SetupKeys.upgrade {
                    WhatsNewView(
                        onRunSetup: { setupState = SetupKeys.done; store.showSetup = true },
                        onContinue: { setupState = SetupKeys.done }
                    )
                } else if store.sessionReady {
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
                // Behind the menu, so the map's first frame does not pay for the renderer's setup.
                try? await Task.sleep(nanoseconds: 400_000_000)
                RenderWarmup.run()
            }
            .onChange(of: store.sessionReady) { _, ready in
                // Back at the menu: free the map scene kept for the next visit to the map tab.
                if !ready { GpuMapSession.discardParked() }
            }
            .onChange(of: scenePhase) { _, phase in
                // Background asks for one bounded stop-and-save. Foreground never replays an
                // accepted command; it only re-reads the live session.
                Task { await store.handleScenePhase(phase) }
            }
            .fullScreenCover(isPresented: $store.showSetup) {
                SetupWizardView(firstRun: false, onFinish: { store.showSetup = false })
                    .environmentObject(store)
                    .preferredColorScheme(.dark)
            }
            .sheet(isPresented: $store.showOnboarding) {
                OnboardingView()
                    .environmentObject(store)
            }
        }
    }
}
