import SwiftUI

/// The player's own position: personal wealth and the companies it rests on, where cash is
/// heading, the one recommended move, the clock, and game settings at the end.
struct OverviewView: View {
    @EnvironmentObject private var store: AppStore
    @State private var showingBridge = false
    @State private var showingSettings = false

    var body: some View {
        TabScreen(title: "Portfolio", subtitle: store.overview?.weekLabel) {
            Button {
                showingSettings = true
            } label: {
                PixelIcon("settings")
                    .font(.title3)
                    .foregroundStyle(Theme.secondary)
                    .frame(width: 44, height: 44)
            }
            .accessibilityLabel("Settings")
        } content: {
            if let overview = store.overview {
                if overview.pause == "five_year_score" {
                    fiveYearScore
                }
                wealth(overview)
                if !overview.nextActionKey.isEmpty {
                    CalloutRow(icon: "bulb", text: overview.nextActionReason)
                }
                ForEach(overview.alerts, id: \.self) { alert in
                    CalloutRow(icon: "warning", text: alert, tint: Theme.warning)
                }
                RunControls()
                Text("Next week settles one week. The speed control plays a week a second at 1x, two at 2x and four at 4x, and stops for any decision.")
                    .font(.caption2)
                    .foregroundStyle(Theme.secondary)
                groupPanel
                ShareholdingsPanel()
            } else {
                ProgressView("Opening your world…")
            }
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView().environmentObject(store)
        }
        .sheet(isPresented: $showingBridge) {
            CashBridgeView()
                .environmentObject(store)
        }
    }

    /// Net worth leads; cash on both sides of the investor/company line, the quarter's profit and
    /// where company cash is heading over four weeks.
    private func wealth(_ overview: OverviewDto) -> some View {
        let outlook = overview.forecastFourWeekCents - overview.firmCashCents
        let weeks = Int(overview.quarterWeeks)
        return VStack(alignment: .leading, spacing: 8) {
            StatGroup(
                headline: Stat(label: "Net worth", value: Format.moneyCompact(overview.investorNetWorthCents),
                               note: "your holdings at fair value plus cash"),
                stats: [
                    Stat(label: "Your cash", value: Format.moneyCompact(overview.investorCashCents)),
                    Stat(label: "Company cash", value: Format.moneyCompact(overview.firmCashCents, currency: overview.primaryCurrency)),
                    Stat(label: weeks >= 13 ? "Profit, quarter" : "Profit, \(weeks) wk",
                         value: Format.moneyCompact(overview.quarterNetIncomeCents, currency: overview.primaryCurrency),
                         tone: overview.quarterNetIncomeCents >= 0 ? Theme.positive : Theme.negative),
                ]
            )
            Button {
                showingBridge = true
            } label: {
                HStack(spacing: 10) {
                    PixelIcon(outlook >= 0 ? "trend_up" : "trend_down")
                        .foregroundStyle(outlook >= 0 ? Theme.positive : Theme.negative)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Company cash in four weeks: \(Format.moneyCompact(overview.forecastFourWeekCents, currency: overview.primaryCurrency))")
                            .font(.callout.weight(.medium))
                            .foregroundStyle(Theme.primaryText)
                        Text("\(Format.signedMoney(outlook)) after payroll and loan principal due · see the cash bridge")
                            .font(.caption2.monospacedDigit())
                            .foregroundStyle(outlook >= 0 ? Theme.secondary : Theme.negative)
                    }
                    Spacer(minLength: 0)
                    PixelIcon("chevron_right", size: 14).foregroundStyle(Theme.secondary)
                }
                .padding(12)
                .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder private var fiveYearScore: some View {
        if let score = store.score {
            SectionCard(title: "Five-year score") {
                Text("\(score.total) / 100")
                    .font(.largeTitle.monospacedDigit())
                DetailRow(title: "Growth (45%)", value: "\(score.returnComponent)")
                DetailRow(title: "Expansion (25%)", value: "\(score.expansionComponent)")
                DetailRow(title: "Leadership (10%)", value: "\(score.leadershipComponent)")
                DetailRow(title: "Resilience (10%)", value: "\(score.resilienceComponent)")
                DetailRow(title: "Conduct (10%)", value: "\(score.conductComponent)")
                Text("Growth compares personal net worth with the start: 20 when flat, 100 at double. Expansion counts divisions beyond your first and states beyond your home. Leadership counts markets where your group has the most capacity. Resilience is the share of weeks without past-due liabilities. Conduct deducts unresolved regulatory breaches and failed worker settlements. Holding your starting company scores about a third.")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondary)
                if score.assisted {
                    Text("Assisted recovery: this score is labelled.")
                        .font(.caption)
                        .foregroundStyle(Theme.warning)
                }
            }
        }
        Button {
            Task { await store.continueCareer() }
        } label: {
            PixelLabel("Continue this career in sandbox", icon: "arrow_right")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
    }

    private var groupPanel: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Eyebrow(text: "Your companies")
                if store.groupTree.isEmpty {
                    Text("You do not control a company. Found one from the map.")
                        .font(.callout)
                        .foregroundStyle(Theme.secondary)
                }
                ForEach(store.groupTree, id: \.handle) { node in
                    Button {
                        Task { await store.selectFirm(node.handle) }
                    } label: {
                        HStack(spacing: 10) {
                            if node.depth > 0 {
                                PixelIcon("indent", size: 12)
                                    .foregroundStyle(Theme.secondary)
                                    .accessibilityHidden(true)
                            }
                            VStack(alignment: .leading, spacing: 1) {
                                HStack(spacing: 6) {
                                    Text(node.name).font(.callout.weight(.medium)).foregroundStyle(Theme.primaryText).lineLimit(1)
                                    if store.selectedFirm == node.handle {
                                        PixelIcon("check_circle", size: 14).foregroundStyle(Theme.positive)
                                    }
                                }
                                Text("\(node.industry) · \(node.region) · control \(Format.percentBps(node.controlBps))")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondary)
                                    .lineLimit(1)
                            }
                            Spacer(minLength: 8)
                            Text(Format.moneyCompact(node.cashCents))
                                .font(.callout.monospacedDigit())
                                .foregroundStyle(Theme.primaryText)
                        }
                        .frame(minHeight: 44)
                        .padding(.leading, CGFloat(min(Int(node.depth), 4)) * 12)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("\(node.name), \(node.directlyControlled ? "direct holding" : "subsidiary"), control \(Format.percentBps(node.controlBps)), look-through \(Format.percentBps(node.lookThroughBps))")
                    .accessibilityHint("Makes this company the selected operating context.")
                }
            }
        }
    }
}

/// Saving and analytics: game settings, not company business.
struct GameSettingsPanel: View {
    @EnvironmentObject private var store: AppStore
    @State private var analytics = Telemetry.enabled

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: "Game")
                HStack {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Save").font(.callout).foregroundStyle(Theme.primaryText)
                        Text("The game also saves itself every four weeks.").font(.caption2).foregroundStyle(Theme.secondary)
                    }
                    Spacer()
                    Button("Save now") { Task { await store.manualSave() } }
                        .font(.callout.weight(.semibold))
                        .tint(Theme.gold)
                }
                Divider().overlay(Theme.stroke)
                Toggle(isOn: $analytics) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Share anonymous market data").font(.callout).foregroundStyle(Theme.primaryText)
                        Text("A market summary every five weeks with a random id; no names, account or device data.")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary)
                    }
                }
                .tint(Theme.gold)
                .onChange(of: analytics) { _, value in Telemetry.enabled = value }
                if !store.statusLine.isEmpty {
                    Text(store.statusLine).font(.caption2).foregroundStyle(Theme.secondary)
                }
            }
        }
    }
}

/// The anonymous-data switch on its own, for the main menu's settings.
struct AnalyticsPanel: View {
    @State private var analytics = Telemetry.enabled

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: "Game")
                Toggle(isOn: $analytics) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Share anonymous market data").font(.callout).foregroundStyle(Theme.primaryText)
                        Text("A market summary every five weeks with a random id; no names, account or device data.")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary)
                    }
                }
                .tint(Theme.gold)
                .onChange(of: analytics) { _, value in Telemetry.enabled = value }
            }
        }
    }
}

/// Keys for the graphics settings, read by the map and the main menu. The same keys on every
/// platform.
enum GraphicsKeys {
    static let preset = "graphics.preset"
    static let mapDetail = "graphics.mapDetail"
    static let tilt = "graphics.tilt"
    static let extrude = "graphics.extrude"
    static let lighting = "graphics.lighting"
    static let dayNight = "graphics.dayNight"
    static let cityLights = "graphics.cityLights"
    static let clouds = "graphics.clouds"
    static let traffic = "graphics.traffic"
    static let buildings = "graphics.buildings"
    static let frameRate = "graphics.frameRate"
    static let animations = "graphics.animations"
    static let showFps = "graphics.showFps"
}

/// How much the map draws: 0 low, 1 medium, 2 high.
enum MapDetailLevel: Int, CaseIterable, Identifiable {
    case low = 0, medium, high
    var id: Int { rawValue }
    var title: String { ["Low", "Medium", "High"][rawValue] }
    var note: String {
        ["Countries and the scenario only", "Adds state and province borders", "Adds highways, counties and rivers"][rawValue]
    }
}

/// Which events stop the clock. Decisions that need an answer always do.
struct PauseSettingsPanel: View {
    @AppStorage(Engine.pauseOnWarningsKey) private var onWarnings = false

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: "Time")
                Toggle(isOn: $onWarnings) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Stop for warnings too").font(.callout).foregroundStyle(Theme.primaryText)
                        Text("Time always stops for a decision that needs your answer. Turn this on to also stop when a company runs short of grace, or workers strike.")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary)
                    }
                }
                .tint(Theme.gold)
            }
        }
    }
}

/// The graphics options: a quality preset that sets everything, then each option on its own.
struct GraphicsSettingsPanel: View {
    @AppStorage(GraphicsKeys.preset) private var preset = GraphicsPreset.high.rawValue
    @AppStorage(GraphicsKeys.mapDetail) private var detail = MapDetailLevel.high.rawValue
    @AppStorage(GraphicsKeys.tilt) private var tilt = 1
    @AppStorage(GraphicsKeys.extrude) private var extrude = true
    @AppStorage(GraphicsKeys.lighting) private var lighting = 2
    @AppStorage(GraphicsKeys.dayNight) private var dayNight = true
    @AppStorage(GraphicsKeys.cityLights) private var cityLights = true
    @AppStorage(GraphicsKeys.clouds) private var clouds = 1
    @AppStorage(GraphicsKeys.traffic) private var traffic = 1
    @AppStorage(GraphicsKeys.buildings) private var buildings = 2
    @AppStorage(GraphicsKeys.frameRate) private var frameRate = 60
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @AppStorage(GraphicsKeys.showFps) private var showFps = false

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: "Graphics")
                VStack(alignment: .leading, spacing: 4) {
                    Text("Quality").font(.callout).foregroundStyle(Theme.primaryText)
                    Picker("Quality", selection: presetBinding) {
                        ForEach(GraphicsPreset.choices) { Text($0.title).tag($0.rawValue) }
                    }
                    .pickerStyle(.segmented)
                    Text(preset == GraphicsPreset.custom.rawValue
                         ? "Custom. Picking a preset sets every option below."
                         : "Picking a preset sets every option below. Changing any one shows Custom.")
                        .font(.caption2)
                        .foregroundStyle(Theme.secondary)
                }
                Divider().overlay(Theme.stroke)
                choice("Map detail", custom($detail), MapDetailLevel.allCases.map(\.title),
                       (MapDetailLevel(rawValue: detail) ?? .high).note)
                choice("3D tilt", custom($tilt), ["Off", "Subtle", "Full"],
                       "Tilts the map as you zoom in, so the ground recedes into the distance.")
                toggle("Raised regions", custom($extrude),
                       "Regions stand up as tiles, taller where the active layer is stronger.")
                choice("Lighting", custom($lighting), ["Off", "Standard", "Cinematic"],
                       "Sunlight, shadows and the atmosphere around the globe.")
                toggle("Day and night", custom($dayNight), "The sun moves across the world and night falls on the map.")
                toggle("City lights", custom($cityLights), "Cities glow on the night side.")
                choice("Clouds", custom($clouds), ["Off", "Few", "Many"], "Clouds drift over the map and cast shadows.")
                choice("Traffic", custom($traffic), ["Off", "Light", "Busy"],
                       "Freight moves along the lanes and cars along the highways.")
                choice("Buildings", custom($buildings), ["Flat", "Your divisions", "Divisions and cities"],
                       "Shows your divisions as buildings, and city skylines.")
                VStack(alignment: .leading, spacing: 4) {
                    Text("Frame rate").font(.callout).foregroundStyle(Theme.primaryText)
                    Picker("Frame rate", selection: custom($frameRate)) {
                        Text("30").tag(30)
                        Text("60").tag(60)
                        Text("Unlimited").tag(0)
                    }
                    .pickerStyle(.segmented)
                    Text("Lower saves battery.").font(.caption2).foregroundStyle(Theme.secondary)
                }
                Divider().overlay(Theme.stroke)
                toggle("Animations", $animations, "Camera flights, the menu and moving effects.")
                toggle("Show frame rate", $showFps, "A frame counter in the corner of the map.")
            }
        }
    }

    /// Picking a preset writes every option it covers.
    private var presetBinding: Binding<String> {
        Binding(
            get: { preset },
            set: { id in
                guard let chosen = GraphicsPreset(rawValue: id), let values = chosen.values else { return }
                detail = values.mapDetail
                tilt = values.tilt
                extrude = values.extrude
                lighting = values.lighting
                dayNight = values.dayNight
                cityLights = values.cityLights
                clouds = values.clouds
                traffic = values.traffic
                buildings = values.buildings
                frameRate = values.frameRate
                preset = chosen.rawValue
            }
        )
    }

    /// An option binding that marks the preset Custom when the player changes it.
    private func custom<Value: Equatable>(_ binding: Binding<Value>) -> Binding<Value> {
        Binding(
            get: { binding.wrappedValue },
            set: { value in
                guard value != binding.wrappedValue else { return }
                binding.wrappedValue = value
                preset = GraphicsPreset.custom.rawValue
            }
        )
    }

    private func choice(_ title: String, _ selection: Binding<Int>, _ labels: [String], _ note: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.callout).foregroundStyle(Theme.primaryText)
            Picker(title, selection: selection) {
                ForEach(Array(labels.enumerated()), id: \.offset) { index, label in
                    Text(label).tag(index)
                }
            }
            .pickerStyle(.segmented)
            Text(note).font(.caption2).foregroundStyle(Theme.secondary)
        }
    }

    private func toggle(_ title: String, _ isOn: Binding<Bool>, _ note: String) -> some View {
        Toggle(isOn: isOn) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.callout).foregroundStyle(Theme.primaryText)
                Text(note).font(.caption2).foregroundStyle(Theme.secondary)
            }
        }
        .tint(Theme.gold)
    }
}

/// Why the clock is stopped and the one button that resumes it: Answer opens the exact card,
/// every other pause has Continue. Shown beside the time controls on every screen.
struct PauseBanner: View {
    @EnvironmentObject private var store: AppStore
    @State private var showingCard = false

    private var event: EventDto? {
        guard let id = store.overview?.pauseInfo?.eventId else { return nil }
        return store.events.first { $0.id == id }
    }

    var body: some View {
        if let info = store.overview?.pauseInfo {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 10) {
                    PixelIcon("pause")
                    Text(info.title).font(.callout.weight(.semibold))
                    Spacer(minLength: 6)
                    if info.action == "answer", event != nil {
                        Button(info.actionLabel) { showingCard = true }
                            .buttonStyle(.borderedProminent)
                            .tint(Theme.gold)
                            .foregroundStyle(Theme.ink)
                            .disabled(store.isBusy)
                    } else {
                        Button("Continue") { Task { await store.continueCareer() } }
                            .buttonStyle(.borderedProminent)
                            .tint(Theme.gold)
                            .foregroundStyle(Theme.ink)
                            .disabled(store.isBusy)
                    }
                }
                Text(info.detail)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.9))
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(12)
            .foregroundStyle(.white)
            .background(info.kind == "decision" ? Theme.warning : Theme.panelRaised)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .sheet(isPresented: Binding(get: { showingCard && event != nil }, set: { showingCard = $0 })) {
                if let event {
                    ScrollView {
                        DecisionCard(event: event).padding(16)
                    }
                    .background(Theme.ink)
                    .presentationDetents([.medium, .large])
                }
            }
        }
    }
}

struct DecisionBanner: View {
    let text: String

    var body: some View {
        HStack(spacing: 12) {
            PixelIcon("hand")
            Text(text).font(.callout.weight(.medium))
            Spacer()
        }
        .padding(12)
        .foregroundStyle(.white)
        .background(Theme.warning)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

struct CashBridgeView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if let finance = store.finance {
                    row("Opening cash", finance.bridgeOpeningCents)
                    row("Receipts", finance.bridgeReceiptsCents)
                    row("Operating payments", finance.bridgeOperatingCents)
                    row("Investment", finance.bridgeInvestmentCents)
                    row("Financing", finance.bridgeFinancingCents)
                    row("Tax", finance.bridgeTaxCents)
                    row("Closing cash", finance.bridgeClosingCents, bold: true)
                    Section("Why cash differs from profit") {
                        Text("Revenue is recognised when a sale settles. Investment in facilities moves cash into assets rather than expense, and depreciation is an expense with no cash movement.")
                            .font(.footnote)
                    }
                } else {
                    Text("Select a company to see its bridge.")
                }
            }
            .navigationTitle("Cash bridge")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }

    private func row(_ title: String, _ cents: Int64, bold: Bool = false) -> some View {
        HStack {
            Text(title).fontWeight(bold ? .semibold : .regular)
            Spacer()
            Text(Format.money(cents)).monospacedDigit().fontWeight(bold ? .semibold : .regular)
        }
    }
}

/// Game settings in one place: saving, analytics, the walkthrough and build details. Company
/// business stays on the other tabs.
struct SettingsView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var showingSaves = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    if store.sessionReady {
                        GameSettingsPanel()
                    } else {
                        AnalyticsPanel()
                    }
                    if store.sessionReady, let fx = store.economy?.fx {
                        Panel(padding: 12) {
                            VStack(alignment: .leading, spacing: 10) {
                                Eyebrow(text: "Currency")
                                Text("Show my cash and net worth in").font(.callout).foregroundStyle(Theme.primaryText)
                                CurrencyPicker(fx: fx)
                                Text("Each company keeps its own currency. This only changes how your own figures are shown.")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondary)
                            }
                        }
                    }
                    PauseSettingsPanel()
                    GraphicsSettingsPanel()
                    Panel(padding: 12) {
                        VStack(alignment: .leading, spacing: 10) {
                            Eyebrow(text: "Help")
                            Button("Saves and recovery") { showingSaves = true }
                                .tint(Theme.gold)
                            Button("Restart the tutorial") { store.tutorial.restart() }
                                .tint(Theme.gold)
                            if store.sessionReady {
                                Button("Replay the guided walkthrough") {
                                    dismiss()
                                    store.showOnboarding = true
                                }
                                .tint(Theme.gold)
                                Button("Save and exit to main menu") {
                                    dismiss()
                                    Task { await store.exitToMenu() }
                                }
                                .tint(Theme.gold)
                            }
                        }
                    }
                    Panel(padding: 12) {
                        VStack(alignment: .leading, spacing: 6) {
                            Eyebrow(text: "About")
                            let info = engineInfo()
                            Text("Corporate Warfare \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "") · engine \(info.version)")
                                .font(.caption.monospaced())
                                .foregroundStyle(Theme.secondary)
                            Text("Institutions are simplified game rules, not legal, tax or investment advice.")
                                .font(.caption)
                                .foregroundStyle(Theme.secondary)
                        }
                    }
                }
                .padding(16)
            }
            .background(Theme.ink)
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .sheet(isPresented: $showingSaves) { SavesView().environmentObject(store) }
        }
    }
}
