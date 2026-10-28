import SwiftUI

enum AppTab: Hashable {
    case map, office, markets, inbox, portfolio
}

struct RootView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var tab: AppTab
    /// A screen that has no tab on iPhone (research, actions, stats), opened by the tutorial.
    @State private var presented: GamePage?

    init(initialTab: AppTab = .map) {
        _tab = State(initialValue: initialTab)
    }

    var body: some View {
        if sizeClass == .regular {
            DesktopShell()
        } else {
            phone
        }
    }

    private var phone: some View {
        VStack(spacing: 0) {
            if let notice = store.recoveryNotice {
                RecoveryNoticeBanner(text: notice)
            }
            PauseBanner()
            TutorialPanel(compact: true)
            TabView(selection: $tab) {
                MapHomeView()
                    .tabItem {
                        PixelIcons.tabImage("map")
                        Text("MAP")
                    }
                    .tag(AppTab.map)
                CEOOfficeView()
                    .safeAreaInset(edge: .top, spacing: 0) { TimeStrip() }
                    .tabItem {
                        PixelIcons.tabImage("office")
                        Text("CEO OFFICE")
                    }
                    .tag(AppTab.office)
                MarketsView()
                    .safeAreaInset(edge: .top, spacing: 0) { TimeStrip() }
                    .tabItem {
                        PixelIcons.tabImage("markets")
                        Text("MARKETS")
                    }
                    .tag(AppTab.markets)
                InboxView()
                    .safeAreaInset(edge: .top, spacing: 0) { TimeStrip() }
                    .tabItem {
                        PixelIcons.tabImage("inbox")
                        Text("INBOX")
                    }
                    .badge(store.events.filter { $0.mandatory }.count)
                    .tag(AppTab.inbox)
                OverviewView()
                    .safeAreaInset(edge: .top, spacing: 0) { TimeStrip() }
                    .tabItem {
                        PixelIcons.tabImage("portfolio")
                        Text("PORTFOLIO")
                    }
                    .tag(AppTab.portfolio)
            }
            .tint(Theme.gold)
        }
        .sheet(item: $store.openDivision) { division in
            DivisionDetailSheet(division: division).environmentObject(store)
        }
        .onAppear { store.tutorial.visited(tab.page) }
        .onChange(of: tab) { _, next in store.tutorial.visited(next.page) }
        .onChange(of: store.tutorial.requestedPage) { _, wanted in
            guard let wanted else { return }
            store.tutorial.requestedPage = nil
            show(wanted)
        }
        .sheet(item: $presented) { page in
            NavigationStack {
                Group {
                    switch page {
                    case .research: ResearchView()
                    case .stats: StatsView()
                    default: CommandCenterView(onClose: { presented = nil })
                    }
                }
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { presented = nil }
                    }
                }
            }
            .environmentObject(store)
            .presentationBackground(Theme.ink)
        }
    }

    private func show(_ page: GamePage) {
        switch page {
        case .map: tab = .map
        case .office: tab = .office
        case .markets: tab = .markets
        case .inbox: tab = .inbox
        case .portfolio: tab = .portfolio
        case .research, .stats, .actions: presented = page
        }
    }
}

extension AppTab {
    var page: GamePage {
        switch self {
        case .map: return .map
        case .office: return .office
        case .markets: return .markets
        case .inbox: return .inbox
        case .portfolio: return .portfolio
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
            PixelIcon("history")
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
                PixelIcon("close", size: 12)
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
            PixelIcon("office", size: 12)
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
        .clipShape(Pixel.small)
    }
}

struct RunControls: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        HStack(spacing: 10) {
            Button {
                Task { await store.advance(weeks: 1) }
            } label: {
                PixelLabel("Next week", icon: "next_week")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
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

    private let speeds: [(Double, String)] = [(0, "pause"), (1, "speed1"), (2, "speed2"), (4, "speed3")]

    var body: some View {
        HStack(spacing: 2) {
            ForEach(speeds, id: \.0) { speed, label in
                let selected = speed == 0 ? store.runSpeed == 0 : store.runSpeed == speed
                Button {
                    store.setSpeed(speed)
                } label: {
                    PixelIcon(label, size: compact ? 16 : 20)
                    .frame(minWidth: compact ? 30 : 38, minHeight: compact ? 28 : 36)
                    .foregroundStyle(selected ? Theme.ink : Theme.primaryText)
                    .background(selected ? Theme.gold : Color.clear, in: Pixel.shape)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(speed == 0 ? "Pause" : "Speed \(Int(speed)) times")
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(2)
        .background(Theme.panelRaised, in: Pixel.shape)
        .overlay(Pixel.shape.stroke(Theme.stroke, lineWidth: 1))
    }
}
