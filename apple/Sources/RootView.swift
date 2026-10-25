import SwiftUI

struct RootView: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        TabView {
            OverviewView()
                .tabItem { Label("Portfolio", systemImage: "building.columns") }
            CompanyView()
                .tabItem { Label("Company", systemImage: "building.2") }
            RegionsView()
                .tabItem { Label("Regions", systemImage: "map") }
            MarketsView()
                .tabItem { Label("Markets", systemImage: "chart.bar.xaxis") }
            InboxView()
                .tabItem { Label("Inbox", systemImage: "tray") }
        }
    }
}

struct ContextChip: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "building.2")
                .font(.caption)
            Text(store.selectedFirmDto?.name ?? "No company selected")
                .font(.caption.weight(.medium))
                .lineLimit(1)
            Spacer()
            if let week = store.overview?.weekLabel {
                Text(week)
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(Theme.secondaryText(for: scheme))
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Theme.surface(for: scheme))
        .clipShape(Capsule())
    }
}

struct RunControls: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        HStack(spacing: 12) {
            Button {
                Task { await store.advance(weeks: 1) }
            } label: {
                Label("Step week", systemImage: "forward.frame")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)

            Button {
                store.toggleRun()
            } label: {
                Label(store.runSpeed > 0 ? "Pause" : "Play", systemImage: store.runSpeed > 0 ? "pause.fill" : "play.fill")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
        }
        .controlSize(.large)
        .disabled(store.isBusy)
    }
}
