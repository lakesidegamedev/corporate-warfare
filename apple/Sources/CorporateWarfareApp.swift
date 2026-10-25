import SwiftUI

@main
struct CorporateWarfareApp: App {
    @StateObject private var store = AppStore()
    @AppStorage(OnboardingKeys.completed) private var onboardingCompleted = false

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
            .sheet(isPresented: $store.showOnboarding) {
                OnboardingView()
                    .environmentObject(store)
            }
        }
    }
}
