import SwiftUI

/// The home screen: the world map, framed on the scenario, with the company HUD over it. Tap a region to
/// open its board; build or raid from there. On iPad the board docks beside the map.
struct MapHomeView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// True inside the iPad and Mac shell, where the status bar replaces the HUD and an outliner
    /// sits left of the map.
    var embedded = false
    @State private var layer: MapLayer = .presence
    @State private var selected: String?
    @State private var building: String?

    private var wide: Bool { sizeClass == .regular }

    var body: some View {
        ZStack(alignment: .top) {
            HStack(spacing: 0) {
                if embedded {
                    MapOutliner()
                        .frame(width: 300)
                        .background(Theme.panel)
                        .overlay(alignment: .trailing) { Rectangle().fill(Theme.stroke).frame(width: 1) }
                }
                WorldMapView(shading: shading, layer: layer, divisions: divisionMarkers, lanes: layer == .freight ? store.lanes : [], regionCodes: store.boards.map(\.code), selected: $selected)
                    .ignoresSafeArea(edges: .bottom)
                if wide, let code = selected, let board = store.boards.first(where: { $0.code == code }) {
                    RegionBoardView(board: board, onBuild: { building = code }, onClose: { selected = nil })
                        .frame(width: 380)
                        .background(Theme.panel)
                        .overlay(alignment: .leading) { Rectangle().fill(Theme.stroke).frame(width: 1) }
                        .transition(.move(edge: .trailing))
                }
            }
            if layer == .freight {
                VStack {
                    Spacer()
                    FreightPanelView(focus: selected)
                        .frame(maxHeight: 300)
                        .padding(.horizontal, 12)
                        .padding(.bottom, 56)
                }
            }
            VStack(spacing: 10) {
                if !embedded { CompanyHUD() }
                LayerChips(layer: $layer)
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.top, 6)
            .padding(.bottom, wide ? 16 : 12)
            .padding(.trailing, wide && selected != nil ? 380 : 0)
        }
        .background(Theme.ink)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.2), value: selected)
        .sheet(item: compactSelection) { item in
            if let board = store.boards.first(where: { $0.code == item.id }) {
                RegionBoardView(board: board, onBuild: { building = item.id }, onClose: { selected = nil })
                    .presentationDetents([.fraction(0.5), .large])
                    .presentationBackgroundInteraction(.enabled(upThrough: .fraction(0.5)))
                    .presentationBackground(Theme.panel)
            }
        }
        .sheet(item: buildSelection) { item in
            BuildFlowView(regionCode: item.id)
                .environmentObject(store)
                .presentationBackground(Theme.panel)
        }
    }

    private struct Code: Identifiable { let id: String }

    private var compactSelection: Binding<Code?> {
        Binding(
            get: { (!wide && building == nil) ? selected.map(Code.init) : nil },
            set: { if $0 == nil && !wide && building == nil { selected = nil } }
        )
    }

    private var buildSelection: Binding<Code?> {
        Binding(get: { building.map(Code.init) }, set: { building = $0?.id })
    }

    /// The player's divisions: one circle per owned sector type in a region, merged across the
    /// player's companies.
    private var divisionMarkers: [DivisionMarker] {
        let grouped = Dictionary(grouping: store.mapDivisions) { $0.region + "/" + $0.industry }
        return grouped.values.compactMap { items in
            guard let first = items.first else { return nil }
            // A plant is one block; a site can hold several.
            return DivisionMarker(region: first.region, sector: first.industry, plants: Int(items.reduce(UInt32(0)) { $0 + $1.blocks }))
        }
    }

    /// 0..1 intensity per region for the active layer.
    private var shading: [String: Double] {
        switch layer {
        case .presence:
            let blocks = Dictionary(grouping: store.mapDivisions, by: \.region)
                .mapValues { Double($0.reduce(UInt32(0)) { $0 + $1.blocks }) }
            let top = blocks.values.max() ?? 1
            return blocks.mapValues { $0 / max(top, 1) }
        case .room:
            let values = Dictionary(uniqueKeysWithValues: store.boards.map { board in
                (board.code, Double(board.room.reduce(Int64(0)) { $0 + $1.importLots * $1.importPriceCents }))
            })
            let top = values.values.max() ?? 1
            return values.mapValues { top > 0 ? $0 / top : 0 }
        case .wages:
            let values = store.boards.map { Double($0.wageIndexBps) }
            let low = values.min() ?? 0
            let high = values.max() ?? 1
            return Dictionary(uniqueKeysWithValues: store.boards.map { ($0.code, (Double($0.wageIndexBps) - low) / max(high - low, 1)) })
        case .skill:
            return Dictionary(uniqueKeysWithValues: store.boards.map { ($0.code, Double($0.skillBps) / 10_000) })
        case .freight:
            // Lanes are drawn over the land instead of shading it.
            return [:]
        }
    }
}

/// Company name, cash, forecast and the clock, always visible over the map.
struct CompanyHUD: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        // One row where it fits; on a phone the name keeps its own line above the figures. The
        // board's goal is the HUD's last line rather than a separate strip over the map.
        VStack(alignment: .leading, spacing: 8) {
            layout
            if let objective = store.objective {
                Divider().overlay(Theme.stroke)
                GoalLine(objective: objective)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
    }

    private var layout: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 14) {
                title
                Spacer(minLength: 8)
                figures
                controls
            }
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 10) {
                    title
                    Spacer(minLength: 8)
                    controls
                }
                HStack(spacing: 18) {
                    figures
                    Spacer(minLength: 0)
                }
            }
        }
    }

    private var title: some View {
        VStack(alignment: .leading, spacing: 2) {
            Eyebrow(text: store.overview?.weekLabel ?? "")
            Text(store.overview?.primaryName.isEmpty == false ? store.overview!.primaryName : "No company yet")
                .font(.headline)
                .foregroundStyle(Theme.primaryText)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
        }
    }

    @ViewBuilder private var figures: some View {
        HUDFigure(label: "Cash", value: Format.moneyCompact(store.overview?.firmCashCents ?? 0, currency: store.overview?.primaryCurrency ?? ""))
        HUDFigure(
            label: (store.overview?.quarterWeeks ?? 0) >= 13 ? "Profit/qtr" : "Profit so far",
            value: Format.moneyCompact(store.overview?.quarterNetIncomeCents ?? 0, currency: store.overview?.primaryCurrency ?? ""),
            tone: (store.overview?.quarterNetIncomeCents ?? 0) >= 0 ? Theme.positive : Theme.negative
        )
    }

    private var controls: some View {
        HStack(spacing: 6) {
            TimeBar()
            GameMenuButton()
        }
    }
}

/// Time controls folded into one pill: the speed it is running at, tap to open Next week and the
/// speed buttons, tap again to fold.
struct TimeBar: View {
    @EnvironmentObject private var store: AppStore
    @State private var open = false

    var body: some View {
        HStack(spacing: 6) {
            if open {
                Button {
                    Task { await store.advance(weeks: 1) }
                } label: {
                    PixelIcon("next_week")
                }
                .buttonStyle(.borderedProminent)
                .tint(Theme.gold)
                .foregroundStyle(Theme.ink)
                .disabled(store.isBusy || store.runSpeed > 0)
                .accessibilityLabel("Next week")
                SpeedControl(compact: true)
            }
            Button {
                withAnimation(.easeOut(duration: 0.15)) { open.toggle() }
            } label: {
                HStack(spacing: 4) {
                    PixelIcon(store.runSpeed > 0 ? "play" : "pause")
                    if store.runSpeed > 0 { Text("\(Int(store.runSpeed))x").monospacedDigit() }
                    PixelIcon(open ? "chevron_right" : "chevron_left", size: 12)
                }
                .font(.caption.weight(.bold))
                .foregroundStyle(Theme.primaryText)
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .background(Theme.panelRaised, in: Capsule())
                .overlay(Capsule().stroke(Theme.stroke, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(open ? "Hide time controls" : "Show time controls")
        }
    }
}

struct HUDFigure: View {
    let label: String
    let value: String
    var tone: Color = Theme.primaryText

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Eyebrow(text: label)
            Text(value).font(.subheadline.weight(.semibold).monospacedDigit()).foregroundStyle(tone)
        }
        .accessibilityElement(children: .combine)
    }
}

struct LayerChips: View {
    @Binding var layer: MapLayer

    var body: some View {
        chips
            // The trailing fade says the row scrolls when the last chip is cut off.
            .mask(
                HStack(spacing: 0) {
                    Color.black
                    LinearGradient(colors: [.black, .black.opacity(0)], startPoint: .leading, endPoint: .trailing)
                        .frame(width: 28)
                }
            )
    }

    private var chips: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(MapLayer.allCases) { option in
                    Button {
                        layer = option
                    } label: {
                        PixelLabel(option.rawValue, icon: option.icon)
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 12)
                            .padding(.vertical, 7)
                            .foregroundStyle(layer == option ? Theme.ink : Theme.primaryText)
                            .background(layer == option ? option.tint : Theme.panel.opacity(0.9), in: Capsule())
                            .overlay(Capsule().stroke(Theme.stroke, lineWidth: layer == option ? 0 : 1))
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(layer == option ? .isSelected : [])
                }
            }
            .padding(.trailing, 28)
        }
    }
}

/// The board's quarterly goal as one line inside the company HUD.
struct GoalLine: View {
    let objective: ObjectiveDto

    var body: some View {
        HStack(spacing: 8) {
            PixelIcon("target", size: 14).foregroundStyle(Theme.gold)
            Text(objective.title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(Theme.primaryText)
                .lineLimit(1)
            ProgressView(value: Double(objective.progressBps) / 10_000)
                .tint(Theme.gold)
                .frame(maxWidth: 80)
            Spacer(minLength: 4)
            Text("\(objective.weeksLeft) wk left")
                .font(.caption2.monospacedDigit())
                .foregroundStyle(Theme.secondary)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Board goal: \(objective.title), \(objective.progressBps / 100) percent, \(objective.weeksLeft) weeks left")
    }
}


/// The in-game menu: save, settings, and back to the main menu.
struct GameMenuButton: View {
    @EnvironmentObject private var store: AppStore
    @State private var showingSettings = false

    var body: some View {
        Menu {
            Button {
                Task { await store.manualSave() }
            } label: {
                PixelLabel("Save now", icon: "save")
            }
            Button {
                showingSettings = true
            } label: {
                PixelLabel("Settings", icon: "settings")
            }
            Divider()
            Button {
                Task { await store.exitToMenu() }
            } label: {
                PixelLabel("Save and exit to main menu", icon: "quit")
            }
        } label: {
            PixelIcon("menu", size: 12)
                .foregroundStyle(Theme.primaryText)
                .frame(width: 30, height: 30)
                .background(Theme.panelRaised, in: Circle())
        }
        .accessibilityLabel("Menu")
        .sheet(isPresented: $showingSettings) {
            SettingsView().environmentObject(store)
        }
    }
}

/// The desktop map's left column: what needs attention, the board's goal, your divisions and
/// what rivals did lately.
struct MapOutliner: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                if let alerts = store.overview?.alerts, !alerts.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Eyebrow(text: "Alerts")
                        ForEach(alerts, id: \.self) { alert in
                            CalloutRow(icon: "warning", text: alert, tint: Theme.warning)
                        }
                    }
                }
                if let objective = store.objective {
                    VStack(alignment: .leading, spacing: 6) {
                        Eyebrow(text: "Board goal")
                        GoalLine(objective: objective)
                    }
                }
                VStack(alignment: .leading, spacing: 6) {
                    Eyebrow(text: "Your divisions")
                    if store.divisions.isEmpty {
                        Text("No divisions yet. Pick a region with room to build.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondary)
                    }
                    ForEach(store.divisions) { division in
                        HStack(spacing: 8) {
                            SectorBadge(sector: division.industry, size: 24)
                            VStack(alignment: .leading, spacing: 1) {
                                Text("\(Sector.name(division.industry)) in \(division.regionName)")
                                    .font(.footnote.weight(.semibold))
                                    .foregroundStyle(Theme.primaryText)
                                    .lineLimit(1)
                                Text("\(division.strategyName) · \(division.blocks) plants · \(division.shareBps / 100)% share")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondary)
                                    .lineLimit(1)
                            }
                        }
                    }
                }
                VStack(alignment: .leading, spacing: 6) {
                    Eyebrow(text: "Rival moves")
                    if store.recentRaids.isEmpty {
                        Text("No raids lately.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondary)
                    }
                    ForEach(Array(store.recentRaids.prefix(8).enumerated()), id: \.offset) { _, raid in
                        Text("\(ResultBuckets.weekLabel(raid.week)): \(raid.attacker)\(raid.attackerCeo.isEmpty ? "" : " (\(raid.attackerCeo), \(temperamentLabel(raid.attackerTemperament)))") \(raid.success ? "took" : "failed to take") \(Sector.name(raid.industry)) in \(raid.region) from \(raid.defender).")
                            .font(.caption)
                            .foregroundStyle(Theme.primaryText)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
            .padding(14)
        }
    }
}
