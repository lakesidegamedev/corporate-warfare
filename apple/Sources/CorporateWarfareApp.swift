import SwiftUI

@main
struct CorporateWarfareApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

private struct ContentView: View {
    private let info = engineInfo()

    var body: some View {
        VStack(spacing: 16) {
            Text("Corporate Warfare")
                .font(.largeTitle)
            Text("Engine \(info.version), interface \(info.interfaceVersion)")
            Text("Development build. Gameplay is not implemented.")
                .foregroundStyle(.secondary)
        }
        .padding()
    }
}
