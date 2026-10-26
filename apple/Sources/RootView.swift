import SwiftUI

struct RootView: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        VStack(spacing: 0) {
            if let notice = store.recoveryNotice {
                RecoveryNoticeBanner(text: notice)
            }
            TabView {
                MapHomeView()
                    .tabItem { Label("Map", systemImage: "map.fill") }
                CEOOfficeView()
                    .tabItem { Label("CEO Office", systemImage: "building.2.fill") }
                MarketsView()
                    .tabItem { Label("Markets", systemImage: "chart.bar.xaxis") }
                InboxView()
                    .tabItem { Label("Inbox", systemImage: "tray.full.fill") }
                    .badge(store.events.filter { $0.mandatory }.count)
                OverviewView()
                    .tabItem { Label("Portfolio", systemImage: "building.columns.fill") }
            }
        }
    }
}

/// One-time notice after a career was restored from a last-good copy. The player is told which
/// slot and week are now live, and can dismiss it.
struct RecoveryNoticeBanner: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "clock.arrow.circlepath")
                .font(.footnote)
                .foregroundStyle(Theme.warning)
            Text(text)
                .font(.footnote)
                .foregroundStyle(Theme.text(for: scheme))
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            Button {
                store.dismissRecoveryNotice()
            } label: {
                Image(systemName: "xmark")
                    .font(.caption2)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Dismiss recovery notice")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Theme.warning.opacity(0.12))
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
                // The chip truncates visually, so VoiceOver reads the full legal name.
                .accessibilityLabel(store.selectedFirmDto?.name ?? "No company selected")
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
