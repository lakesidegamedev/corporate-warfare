import SwiftUI

enum AppTab: Hashable {
    case map, office, markets, inbox, portfolio
}

struct RootView: View {
    @EnvironmentObject private var store: AppStore
    @State private var tab: AppTab

    init(initialTab: AppTab = .map) {
        _tab = State(initialValue: initialTab)
    }

    var body: some View {
        VStack(spacing: 0) {
            if let notice = store.recoveryNotice {
                RecoveryNoticeBanner(text: notice)
            }
            TabView(selection: $tab) {
                MapHomeView()
                    .tabItem { Label("Map", systemImage: "map.fill") }
                    .tag(AppTab.map)
                CEOOfficeView()
                    .tabItem { Label("CEO Office", systemImage: "building.2.fill") }
                    .tag(AppTab.office)
                MarketsView()
                    .tabItem { Label("Markets", systemImage: "chart.bar.xaxis") }
                    .tag(AppTab.markets)
                InboxView()
                    .tabItem { Label("Inbox", systemImage: "tray.full.fill") }
                    .badge(store.events.filter { $0.mandatory }.count)
                    .tag(AppTab.inbox)
                OverviewView()
                    .tabItem { Label("Portfolio", systemImage: "building.columns.fill") }
                    .tag(AppTab.portfolio)
            }
            .tint(Theme.gold)
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
        HStack(spacing: 10) {
            Button {
                Task { await store.advance(weeks: 1) }
            } label: {
                Label("Next week", systemImage: "forward.frame")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
            .controlSize(.large)
            .disabled(store.isBusy || store.runSpeed > 0)
            SpeedControl()
        }
    }
}

/// Pause, 1x, 2x, 4x. One tick is one week; 1x is a week a second.
struct SpeedControl: View {
    @EnvironmentObject private var store: AppStore
    var compact = false

    private let speeds: [(Double, String)] = [(0, "pause.fill"), (1, "1x"), (2, "2x"), (4, "4x")]

    var body: some View {
        HStack(spacing: 2) {
            ForEach(speeds, id: \.0) { speed, label in
                let selected = speed == 0 ? store.runSpeed == 0 : store.runSpeed == speed
                Button {
                    store.setSpeed(speed)
                } label: {
                    Group {
                        if speed == 0 {
                            Image(systemName: label)
                        } else {
                            Text(label).monospacedDigit()
                        }
                    }
                    .font((compact ? Font.caption : Font.callout).weight(.bold))
                    .frame(minWidth: compact ? 30 : 38, minHeight: compact ? 28 : 36)
                    .foregroundStyle(selected ? Theme.ink : Theme.primaryText)
                    .background(selected ? Theme.gold : Color.clear, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(speed == 0 ? "Pause" : "Speed \(label)")
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(2)
        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
    }
}
