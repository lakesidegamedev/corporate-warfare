import SwiftUI

/// The iPad and Mac layout: a sidebar for every screen, a status bar with the company, the
/// economy and the time controls always in view, and multi-pane screens.
struct DesktopShell: View {
    @EnvironmentObject private var store: AppStore
    @State private var page: GamePage
    @State private var query = ""
    @State private var actionsSeed = 0
    @State private var showingSettings = false

    init(initialPage: GamePage = .map) {
        _page = State(initialValue: initialPage)
    }

    var body: some View {
        // In Split View the window can be half an iPad or less; the sidebar and the tutorial
        // then narrow so the screen itself keeps most of the room.
        GeometryReader { proxy in
            shell(width: proxy.size.width)
        }
    }

    private func shell(width: CGFloat) -> some View {
        let sidebar: CGFloat = width >= 900 ? 214 : 176
        return ZStack(alignment: .bottomTrailing) {
            HStack(spacing: 0) {
                DesktopSidebar(page: page, onPage: open, onSettings: { showingSettings = true })
                    .frame(width: sidebar)
                Rectangle().fill(Theme.stroke).frame(width: 1)
                VStack(spacing: 0) {
                    if let notice = store.recoveryNotice {
                        RecoveryNoticeBanner(text: notice)
                    }
                    DesktopStatusBar { text in
                        query = text
                        actionsSeed += 1
                        open(.actions)
                    }
                    Rectangle().fill(Theme.stroke).frame(height: 1)
                    NewsTicker { open(.paper) }
                    Rectangle().fill(Theme.stroke).frame(height: 1)
                    PauseBanner()
                    content
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            TutorialPanel(compact: false)
                .frame(width: min(360, width - sidebar - 32))
                .padding(16)
        }
        .background(Theme.ink)
        .background(shortcuts)
        .onAppear { store.tutorial.visited(page) }
        .onChange(of: page) { _, next in store.tutorial.visited(next) }
        .onChange(of: store.tutorial.requestedPage) { _, wanted in
            guard let wanted else { return }
            open(wanted)
            store.tutorial.requestedPage = nil
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView().environmentObject(store)
        }
        .sheet(item: $store.openDivision) { division in
            DivisionDetailSheet(division: division).environmentObject(store)
        }
    }

    private func open(_ next: GamePage) {
        page = next
        store.tutorial.visited(next)
    }

    @ViewBuilder
    private var content: some View {
        switch page {
        case .map: MapHomeView(embedded: true)
        case .office: DesktopOffice()
        case .research: ResearchView()
        case .stats: NavigationStack { StatsView() }
        case .markets: DesktopMarkets()
        case .actions: CommandCenterView(initialQuery: query).id(actionsSeed)
        case .inbox: InboxView()
        case .portfolio: OverviewView()
        case .paper: PaperView()
        }
    }

    /// Keyboard shortcuts for a hardware keyboard: Command with 1 to 8 for the screens, K for
    /// the Command Center, N for the next week and P to play or pause.
    private var shortcuts: some View {
        ZStack {
            ForEach(Array(GamePage.allCases.enumerated()), id: \.offset) { index, item in
                Button(item.title) { open(item) }
                    .keyboardShortcut(KeyEquivalent(Character(String(index + 1))), modifiers: .command)
            }
            Button("Search actions") { open(.actions) }
                .keyboardShortcut("k", modifiers: .command)
            Button("Next week") {
                if !store.isBusy && store.runSpeed == 0 { Task { await store.advance(weeks: 1) } }
            }
            .keyboardShortcut("n", modifiers: .command)
            Button("Play or pause") { store.setSpeed(store.runSpeed > 0 ? 0 : 1) }
                .keyboardShortcut("p", modifiers: .command)
        }
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

// MARK: - Sidebar

private struct DesktopSidebar: View {
    @EnvironmentObject private var store: AppStore
    let page: GamePage
    let onPage: (GamePage) -> Void
    let onSettings: () -> Void

    private var groups: [String] {
        var seen: [String] = []
        for item in GamePage.allCases where !seen.contains(item.group) { seen.append(item.group) }
        return seen
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            PixelLogo()
                .frame(width: 112)
                .frame(maxWidth: .infinity)
                .padding(.bottom, 10)
            CompanySwitcher()
                .padding(.horizontal, 10)
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(groups, id: \.self) { group in
                        Text(group.uppercased())
                            .font(Pixel.font(10))
                            .tracking(0.6)
                            .foregroundStyle(Theme.secondary.opacity(0.7))
                            .padding(.leading, 20)
                            .padding(.top, 12)
                            .padding(.bottom, 4)
                        ForEach(GamePage.allCases.filter { $0.group == group }) { item in
                            SidebarItem(
                                icon: item.icon,
                                title: item.title,
                                selected: page == item,
                                badge: item == .inbox ? store.inboxBadge : 0
                            ) { onPage(item) }
                        }
                    }
                }
            }
            Spacer(minLength: 0)
            SidebarItem(icon: "save", title: "Save", selected: false, badge: 0) {
                Task { await store.manualSave() }
            }
            SidebarItem(icon: "settings", title: "Settings", selected: false, badge: 0, action: onSettings)
            SidebarItem(icon: "quit", title: "Main menu", selected: false, badge: 0) {
                Task { await store.exitToMenu() }
            }
        }
        .padding(.vertical, 14)
        .background(Theme.panel)
    }
}

private struct SidebarItem: View {
    let icon: String
    let title: String
    let selected: Bool
    let badge: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Pixel.small
                    .fill(selected ? Theme.gold : Color.clear)
                    .frame(width: 3, height: 18)
                PixelIcon(icon, size: 16)
                    .foregroundStyle(selected ? Theme.gold : Theme.secondary)
                    .frame(width: 20)
                Text(title.uppercased())
                    .font(Pixel.font(12))
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                    .accessibilityLabel(title)
                    .foregroundStyle(selected ? Theme.primaryText : Theme.secondary)
                Spacer(minLength: 4)
                if badge > 0 { SignalPill(text: "\(badge)", tint: Theme.warning) }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(selected ? Theme.panelRaised : Color.clear, in: Pixel.shape)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 8)
        .padding(.vertical, 1)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// Which company the screens act on, from the companies you control.
private struct CompanySwitcher: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        let firm = store.selectedFirmDto
        let controlled = store.controlledFirms
        Menu {
            ForEach(controlled, id: \.handle) { item in
                Button(item.name) { Task { await store.selectFirm(item.handle) } }
            }
        } label: {
            HStack(spacing: 8) {
                if let firm { SectorBadge(sector: firm.industry, size: 24) }
                VStack(alignment: .leading, spacing: 1) {
                    Text(firm?.name ?? "No company yet")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(Theme.primaryText)
                        .lineLimit(1)
                    Text(firm.map { "\(Sector.name($0.industry)) · \($0.region)" } ?? "Found one in the CEO Office")
                        .font(.caption2)
                        .foregroundStyle(Theme.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 2)
                if controlled.count > 1 {
                    PixelIcon("sort", size: 12)
                        .foregroundStyle(Theme.secondary)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .background(Theme.panelRaised, in: Pixel.shape)
        }
        .disabled(controlled.count < 2)
        .accessibilityLabel("Company: \(firm?.name ?? "none")")
    }
}

// MARK: - Status bar

/// Cash, profit, outlook, share price, net worth and the economy, with action search and the
/// time controls, always in view.
private struct DesktopStatusBar: View {
    @EnvironmentObject private var store: AppStore
    var onSearch: (String) -> Void
    @State private var text = ""

    var body: some View {
        let overview = store.overview
        let profit = overview?.quarterNetIncomeCents ?? 0
        let weeks = Int(overview?.quarterWeeks ?? 0)
        let cash = overview?.firmCashCents ?? 0
        let forecast = overview?.forecastFourWeekCents ?? 0
        let primary = overview?.primaryCurrency ?? ""
        // The figures scroll; the week controls stay pinned on the right at every width.
        HStack(spacing: 12) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 20) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(overview?.weekLabel ?? "")
                            .font(.headline)
                            .foregroundStyle(Theme.primaryText)
                        if let economy = store.economy { PhasePill(phase: economy.phase) }
                    }
                    figure("Company cash", Format.moneyCompact(cash, currency: primary))
                    figure(weeks >= 13 ? "Profit, quarter" : "Profit, \(weeks) wk", Format.moneyCompact(profit, currency: primary),
                           tone: profit >= 0 ? Theme.positive : Theme.negative)
                    figure("Cash in 4 wk", Format.moneyCompact(forecast, currency: primary),
                           tone: forecast - cash >= 0 ? Theme.primaryText : Theme.negative)
                    if let firm = store.selectedFirmDto {
                        figure("Share price", Format.price(firm.markCents, currency: Format.numeraire))
                    }
                    figure("Net worth", Format.moneyCompact(overview?.investorNetWorthCents ?? 0))
                    if let economy = store.economy {
                        figure("Rate", String(format: "%.2f%%", Double(economy.policyRateBps) / 100))
                        figure("Inflation", String(format: "%.1f%%", Double(economy.inflationBps) / 100),
                               tone: economy.inflationBps > economy.targetInflationBps + 100 ? Theme.warning : Theme.primaryText)
                    }
                    searchField
                }
                .padding(.leading, 16)
                .padding(.vertical, 8)
            }
            Button {
                Task { await store.advance(weeks: 1) }
            } label: {
                Text("Next week").font(.callout.weight(.bold)).lineLimit(1).fixedSize().padding(.horizontal, 6)
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
            .disabled(store.isBusy || store.runSpeed > 0)
            SpeedControl()
                .padding(.trailing, 16)
        }
        .background(Theme.panel)
    }

    private func figure(_ label: String, _ value: String, tone: Color = Theme.primaryText) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label.uppercased())
                .font(.caption2.weight(.semibold))
                .tracking(0.6)
                .foregroundStyle(Theme.secondary)
            Text(value)
                .font(.callout.weight(.semibold).monospacedDigit())
                .foregroundStyle(tone)
                .lineLimit(1)
        }
        .fixedSize()
        .accessibilityElement(children: .combine)
    }

    private var searchField: some View {
        HStack(spacing: 6) {
            PixelIcon("search", size: 14).foregroundStyle(Theme.secondary)
            TextField("Search actions", text: $text)
                .textFieldStyle(.plain)
                .font(.footnote)
                .autocorrectionDisabled()
                .onSubmit {
                    onSearch(text)
                    text = ""
                }
            Text("Cmd K").font(.caption2).foregroundStyle(Theme.secondary.opacity(0.6))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .frame(width: 220)
        .background(Theme.ink, in: Pixel.shape)
        .overlay(Pixel.shape.stroke(Theme.stroke, lineWidth: 1))
    }
}

// MARK: - Multi-pane screens

/// The CEO Office beside the company's results.
private struct DesktopOffice: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        if store.selectedFirmDto == nil && store.controlledFirms.isEmpty {
            ScrollView {
                FoundCompanyPanel()
                    .padding(20)
                    .frame(maxWidth: 1000)
                    .frame(maxWidth: .infinity)
            }
        } else {
            HStack(spacing: 0) {
                CEOOfficeView()
                Rectangle().fill(Theme.stroke).frame(width: 1)
                NavigationStack { StatsView() }
                    .frame(width: 460)
            }
        }
    }
}

/// The economy, the stock exchange and the commodity board side by side, with the region boards
/// one tab over.
private struct DesktopMarkets: View {
    private enum Mode: String, CaseIterable, Identifiable {
        case exchange = "Exchange"
        case regions = "Regions"
        case structure = "Structure"
        var id: String { rawValue }
    }

    @EnvironmentObject private var store: AppStore
    @State private var mode = Mode.exchange

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Markets")
                    .font(.title2.weight(.bold))
                    .foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                Text(store.overview?.weekLabel ?? "")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(Theme.secondary)
                Spacer()
                PixelChoices(options: Mode.allCases.map { ($0, $0.rawValue) }, selection: $mode, label: "View")
                    .frame(maxWidth: 260)
                .frame(width: 330)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            switch mode {
            case .exchange:
                HStack(alignment: .top, spacing: 12) {
                    ScrollView { EconomyCard(collapsible: false).padding(.bottom, 24) }
                        .frame(width: 400)
                    ScrollView { StocksSection().padding(.bottom, 24) }
                    ScrollView { CommoditiesSection().padding(.bottom, 24) }
                }
                .padding(.horizontal, 16)
            case .structure:
                ScrollView {
                    StructureSection()
                        .padding(16)
                        .frame(maxWidth: 900)
                        .frame(maxWidth: .infinity)
                }
            case .regions:
                ScrollView {
                    RegionMarketsSection()
                        .padding(16)
                        .frame(maxWidth: 900)
                        .frame(maxWidth: .infinity)
                }
            }
        }
        .background(Theme.ink)
        .onAppear { store.tutorial.visited(.markets) }
    }
}
