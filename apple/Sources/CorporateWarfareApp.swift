import SwiftUI

@main
struct CorporateWarfareApp: App {
    @StateObject private var store = AppStore()

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
        }
    }
}
