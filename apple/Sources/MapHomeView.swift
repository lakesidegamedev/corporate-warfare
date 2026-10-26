import SwiftUI

/// The home screen: the real map of the corridor with the company HUD over it. Tap a region to
/// open its board; build or raid from there. On iPad the board docks beside the map.
struct MapHomeView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var layer: MapLayer = .presence
    @State private var selected: String?
    @State private var building: String?

    private var wide: Bool { sizeClass == .regular }

    var body: some View {
        ZStack(alignment: .top) {
            HStack(spacing: 0) {
                CorridorMapView(shading: shading, layer: layer, divisions: divisionMarkers, selected: $selected)
                    .ignoresSafeArea(edges: .bottom)
                if wide, let code = selected, let board = store.boards.first(where: { $0.code == code }) {
                    RegionBoardView(board: board, onBuild: { building = code }, onClose: { selected = nil })
                        .frame(width: 380)
                        .background(Theme.panel)
                        .overlay(alignment: .leading) { Rectangle().fill(Theme.stroke).frame(width: 1) }
                        .transition(.move(edge: .trailing))
                }
            }
            VStack(spacing: 10) {
                CompanyHUD()
                LayerChips(layer: $layer)
                Spacer()
                MapLegend(layer: layer)
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
        HUDFigure(label: "Cash", value: Format.moneyCompact(store.overview?.firmCashCents ?? 0))
        HUDFigure(
            label: (store.overview?.quarterWeeks ?? 0) >= 13 ? "Profit/qtr" : "Profit so far",
            value: Format.moneyCompact(store.overview?.quarterNetIncomeCents ?? 0),
            tone: (store.overview?.quarterNetIncomeCents ?? 0) >= 0 ? Theme.positive : Theme.negative
        )
    }

    private var controls: some View {
        HStack(spacing: 6) {
            Button {
                Task { await store.advance(weeks: 1) }
            } label: {
                Image(systemName: "forward.frame.fill")
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
            .disabled(store.isBusy || store.runSpeed > 0)
            .accessibilityLabel("Next week")
            SpeedControl(compact: true)
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
                        Label(option.rawValue, systemImage: option.symbol)
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

struct MapLegend: View {
    let layer: MapLayer
    @State private var expanded = false

    private var short: String {
        switch layer {
        case .presence: return "Circles are your divisions; the badge counts plants."
        case .room: return "Brighter regions import more: demand to build into."
        case .wages: return "Brighter regions pay higher wages."
        case .skill: return "Brighter regions have more skilled workers."
        }
    }

    private var long: String {
        switch layer {
        case .presence: return "Each circle is one of your sector divisions in a region; the badge counts its plants. Brighter regions hold more of your capacity."
        case .room: return "Brighter regions import more than local plants make: demand you could build into."
        case .wages: return "Brighter regions pay more, from 2000 county incomes. Thin lines are regions, thick lines states."
        case .skill: return "Workforce skill. High skill saves inputs in skilled sectors; low skill cuts their output."
        }
    }

    var body: some View {
        Button {
            expanded.toggle()
        } label: {
            HStack(alignment: .top, spacing: 8) {
                Circle().fill(layer.tint).frame(width: 8, height: 8).padding(.top, 4)
                Text(expanded ? long : short)
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                Image(systemName: expanded ? "chevron.down" : "info.circle")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(Theme.panel.opacity(0.92), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .buttonStyle(.plain)
        .frame(maxWidth: 520, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityHint(expanded ? "Shows less" : "Explains this map layer")
    }
}

/// The board's quarterly goal as one line inside the company HUD.
struct GoalLine: View {
    let objective: ObjectiveDto

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "target").font(.caption).foregroundStyle(Theme.gold)
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

