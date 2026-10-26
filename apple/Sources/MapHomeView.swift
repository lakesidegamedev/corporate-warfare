import SwiftUI

/// The home screen: the real map of the corridor with the company HUD over it. Tap a state to
/// open its board; build from there. On iPad the board docks as an inspector beside the map.
struct MapHomeView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var layer: MapLayer = .presence
    @State private var selected: String?
    @State private var building: String?

    private var wide: Bool { sizeClass == .regular }

    var body: some View {
        ZStack(alignment: .top) {
            HStack(spacing: 0) {
                CorridorMapView(shading: shading, layer: layer, markers: markers, selected: $selected)
                    .ignoresSafeArea(edges: .bottom)
                if wide, let code = selected, let state = store.states.first(where: { $0.code == code }) {
                    StateBoardView(state: state, onBuild: { building = code }, onClose: { selected = nil })
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
        .animation(.easeOut(duration: 0.2), value: selected)
        .sheet(item: compactSelection) { item in
            if let state = store.states.first(where: { $0.code == item.id }) {
                StateBoardView(state: state, onBuild: { building = item.id }, onClose: { selected = nil })
                    .presentationDetents([.fraction(0.5), .large])
                    .presentationBackgroundInteraction(.enabled(upThrough: .fraction(0.5)))
                    .presentationBackground(Theme.panel)
            }
        }
        .sheet(item: buildSelection) { item in
            BuildFlowView(stateCode: item.id)
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

    private var markers: [MapMarker] {
        store.mapPlants.map {
            MapMarker(id: $0.handle, state: $0.region, sector: $0.industry, player: $0.player, label: $0.firmName)
        }
    }

    /// 0..1 intensity per state for the active layer.
    private var shading: [String: Double] {
        switch layer {
        case .presence:
            let counts = Dictionary(grouping: store.mapPlants.filter(\.player), by: \.region).mapValues { Double($0.count) }
            let top = counts.values.max() ?? 1
            return counts.mapValues { $0 / max(top, 1) }
        case .room:
            let values = Dictionary(uniqueKeysWithValues: store.states.map { state in
                (state.code, Double(state.room.reduce(Int64(0)) { $0 + $1.importLots * $1.importPriceCents }))
            })
            let top = values.values.max() ?? 1
            return values.mapValues { top > 0 ? $0 / top : 0 }
        case .wages:
            let values = store.states.map { Double($0.wageIndexBps) }
            let low = values.min() ?? 0
            let high = values.max() ?? 1
            return Dictionary(uniqueKeysWithValues: store.states.map { ($0.code, (Double($0.wageIndexBps) - low) / max(high - low, 1)) })
        case .skill:
            return Dictionary(uniqueKeysWithValues: store.states.map { ($0.code, Double($0.skillBps) / 10_000) })
        }
    }
}

/// Company name, cash, forecast and the clock, always visible over the map.
struct CompanyHUD: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Eyebrow(text: store.overview?.weekLabel ?? "")
                Text(store.overview?.primaryName.isEmpty == false ? store.overview!.primaryName : "No company yet")
                    .font(.headline)
                    .foregroundStyle(Theme.primaryText)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            HUDFigure(label: "Cash", value: Format.moneyCompact(store.overview?.firmCashCents ?? 0))
            HUDFigure(
                label: "EBIT/wk",
                value: Format.moneyCompact(store.overview?.lastWeekEbitCents ?? 0),
                tone: (store.overview?.lastWeekEbitCents ?? 0) >= 0 ? Theme.positive : Theme.negative
            )
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
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
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
        case .presence: return "Brighter states hold more of your plants. Gold rings mark your plants; other markers are rivals, coloured by sector."
        case .room: return "Brighter states import more than local plants make: demand you could build into."
        case .wages: return "Brighter states pay more. Census 2000 populations; wages are rounded circa-2000 indices."
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
