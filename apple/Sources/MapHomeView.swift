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
                if let objective = store.objective {
                    BoardGoalStrip(objective: objective)
                }
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
        // One row where it fits; on a phone the name keeps its own line above the figures.
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
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
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
            label: "EBIT/wk",
            value: Format.moneyCompact(store.overview?.lastWeekEbitCents ?? 0),
            tone: (store.overview?.lastWeekEbitCents ?? 0) >= 0 ? Theme.positive : Theme.negative
        )
    }

    private var controls: some View {
        HStack(spacing: 6) {
            Button {
                Task { await store.advance(weeks: 1) }
            } label: {
                Image(systemName: "forward.frame.fill")
            }
            .accessibilityLabel("Step one week")
            Button {
                store.toggleRun()
            } label: {
                Image(systemName: store.runSpeed > 0 ? "pause.fill" : "play.fill")
            }
            .accessibilityLabel(store.runSpeed > 0 ? "Pause" : "Play")
        }
        .buttonStyle(.borderedProminent)
        .tint(Theme.gold)
        .foregroundStyle(Theme.ink)
        .disabled(store.isBusy)
    }
}

struct HUDFigure: View {
    let label: String
    let value: String
    var tone: Color = Theme.primaryText

    var body: some View {
        VStack(alignment: .trailing, spacing: 2) {
            Eyebrow(text: label)
            Text(value).font(.subheadline.weight(.semibold).monospacedDigit()).foregroundStyle(tone)
        }
        .accessibilityElement(children: .combine)
    }
}

struct LayerChips: View {
    @Binding var layer: MapLayer

    var body: some View {
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
        }
    }
}

struct MapLegend: View {
    let layer: MapLayer

    private var text: String {
        switch layer {
        case .presence: return "Each circle is one of your sector divisions in a region; the badge counts its plants. Brighter regions hold more of your capacity."
        case .room: return "Brighter regions import more than local plants make: demand you could build into."
        case .wages: return "Brighter regions pay more, from 2000 county incomes. Thin lines are regions, thick lines states."
        case .skill: return "Workforce skill. High skill saves inputs in skilled sectors; low skill cuts their output."
        }
    }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Circle().fill(layer.tint).frame(width: 10, height: 10).padding(.top, 3)
            Text(text).font(.caption).foregroundStyle(Theme.secondary)
            Spacer(minLength: 0)
        }
        .padding(10)
        .background(Theme.panel.opacity(0.92), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .frame(maxWidth: 520, alignment: .leading)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The board's quarterly goal, one line over the map.
struct BoardGoalStrip: View {
    let objective: ObjectiveDto

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "target").foregroundStyle(Theme.gold)
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(objective.title).font(.caption.weight(.semibold)).foregroundStyle(Theme.primaryText).lineLimit(1)
                    Spacer(minLength: 6)
                    Text("\(objective.weeksLeft) wk").font(.caption2.monospacedDigit()).foregroundStyle(Theme.secondary)
                }
                ProgressView(value: Double(objective.progressBps) / 10_000).tint(Theme.gold)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Theme.panel.opacity(0.92), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Board goal: \(objective.title), \(objective.progressBps / 100) percent, \(objective.weeksLeft) weeks left")
    }
}
