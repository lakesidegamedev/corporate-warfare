import SwiftUI

@main
struct CorporateWarfareApp: App {
    @StateObject private var store = AppStore()
    @AppStorage(OnboardingKeys.completed) private var onboardingCompleted = false
    @Environment(\.scenePhase) private var scenePhase

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
            .tint(Theme.navy)
            .task {
                await store.restoreLatestIfAvailable()
            }
            .onChange(of: store.sessionReady) { _, ready in
                if ready && !onboardingCompleted {
                    store.showOnboarding = true
                }
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
