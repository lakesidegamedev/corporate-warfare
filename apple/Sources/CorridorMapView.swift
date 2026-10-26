import SwiftUI

/// What the map shades the playable states by.
enum MapLayer: String, CaseIterable, Identifiable {
    case presence = "Your plants"
    case room = "Room to build"
    case wages = "Wages"
    case skill = "Workforce skill"

    var id: String { rawValue }

    var symbol: String {
        switch self {
        case .presence: return "building.2.fill"
        case .room: return "arrow.down.right.and.arrow.up.left.circle.fill"
        case .wages: return "dollarsign.circle.fill"
        case .skill: return "graduationcap.fill"
        }
    }

    var tint: Color {
        switch self {
        case .presence: return Theme.gold
        case .room: return Theme.positive
        case .wages: return Theme.copper
        case .skill: return Color(red: 0.259, green: 0.647, blue: 0.961)
        }
    }
}

/// One plant marker on the map.
struct MapMarker: Identifiable {
    let id: UInt64
    let state: String
    let sector: String
    let player: Bool
    let label: String
}

/// The real map of the Great Lakes and Ohio Valley, drawn from bundled public-domain geometry
/// with pan, pinch zoom and tap-to-select. Shading, markers and selection come from the caller.
struct CorridorMapView: View {
    let shading: [String: Double]
    let layer: MapLayer
    let markers: [MapMarker]
    @Binding var selected: String?

    @State private var zoom: CGFloat = 1
    @State private var pan: CGSize = .zero
    @GestureState private var pinch: CGFloat = 1
    @GestureState private var drag: CGSize = .zero

    private let map = CorridorMap.shared

    var body: some View {
        GeometryReader { proxy in
            let frame = framing(in: proxy.size)
            Canvas { context, size in
                draw(in: &context, size: size, frame: frame)
            }
            .background(Theme.ink)
            .contentShape(Rectangle())
            .gesture(
                SimultaneousGesture(
                    MagnifyGesture()
                        .updating($pinch) { value, state, _ in state = value.magnification }
                        .onEnded { value in zoom = min(max(zoom * value.magnification, 1), 8) },
                    DragGesture(minimumDistance: 6)
                        .updating($drag) { value, state, _ in state = value.translation }
                        .onEnded { value in
                            pan.width += value.translation.width
                            pan.height += value.translation.height
                        }
                )
            )
            .onTapGesture(count: 2) {
                withAnimation(.easeOut(duration: 0.25)) {
                    zoom = 1
                    pan = .zero
                }
            }
            .onTapGesture(coordinateSpace: .local) { location in
                let point = frame.toMap(location)
                let hit = map.state(at: point)
                selected = hit == selected ? nil : hit
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Map of the playable states")
            .accessibilityValue(selected.flatMap { code in map.playStates.first { $0.code == code }?.name } ?? "No state selected")
        }
    }

    /// Map-to-screen framing: fit the playable states, then apply zoom and pan.
    struct Framing {
        let scale: CGFloat
        let origin: CGPoint
        let offset: CGPoint

        func toScreen(_ point: CGPoint) -> CGPoint {
            CGPoint(x: (point.x - origin.x) * scale + offset.x, y: (point.y - origin.y) * scale + offset.y)
        }

        func toMap(_ point: CGPoint) -> CGPoint {
            CGPoint(x: (point.x - offset.x) / scale + origin.x, y: (point.y - offset.y) / scale + origin.y)
        }

        var transform: CGAffineTransform {
            CGAffineTransform(translationX: offset.x, y: offset.y)
                .scaledBy(x: scale, y: scale)
                .translatedBy(x: -origin.x, y: -origin.y)
        }
    }

    private func framing(in size: CGSize) -> Framing {
        let bounds = map.playBounds.isNull ? CGRect(origin: .zero, size: map.size) : map.playBounds.insetBy(dx: -150, dy: -150)
        let fit = min(size.width / max(bounds.width, 1), size.height / max(bounds.height, 1))
        let scale = fit * zoom * pinch
        let center = CGPoint(x: bounds.midX, y: bounds.midY)
        return Framing(
            scale: scale,
            origin: center,
            offset: CGPoint(x: size.width / 2 + pan.width + drag.width, y: size.height / 2 + pan.height + drag.height)
        )
    }

    private func draw(in context: inout GraphicsContext, size: CGSize, frame: Framing) {
        let transform = frame.transform
        let hairline = 1 / frame.scale
        context.fill(map.contextLand.applying(transform), with: .color(Theme.landDim))
        for state in map.playStates {
            let path = state.path.applying(transform)
            context.fill(path, with: .color(Theme.land))
            let value = shading[state.code] ?? 0
            if value > 0 {
                context.fill(path, with: .color(layer.tint.opacity(0.12 + 0.55 * min(value, 1))))
            }
        }
        context.stroke(map.counties.applying(transform), with: .color(Theme.stroke.opacity(0.9)), lineWidth: 0.6)
        context.fill(map.lakes.applying(transform), with: .color(Theme.water))
        context.stroke(map.rivers.applying(transform), with: .color(Theme.water.opacity(0.9)), lineWidth: 1.4)
        context.stroke(map.interstates.applying(transform), with: .color(Theme.road.opacity(0.45)), lineWidth: 1)
        for state in map.playStates {
            let path = state.path.applying(transform)
            let isSelected = state.code == selected
            context.stroke(path, with: .color(isSelected ? Theme.gold : Theme.secondary.opacity(0.7)), lineWidth: isSelected ? 3 : 1.2)
        }
        drawCities(in: &context, frame: frame)
        drawMarkers(in: &context, frame: frame)
        _ = hairline
    }

    private func drawCities(in context: inout GraphicsContext, frame: Framing) {
        let shown = zoom * pinch > 2 ? 40 : 14
        for city in map.cities.prefix(shown) where city.country == "USA" || zoom * pinch > 2 {
            let point = frame.toScreen(CGPoint(x: city.x, y: city.y))
            let dot = CGRect(x: point.x - 2.5, y: point.y - 2.5, width: 5, height: 5)
            context.fill(Path(ellipseIn: dot), with: .color(Theme.primaryText.opacity(0.85)))
            context.draw(
                Text(city.name).font(.system(size: 10, weight: .medium)).foregroundStyle(Theme.primaryText.opacity(0.75)),
                at: CGPoint(x: point.x + 5, y: point.y),
                anchor: .leading
            )
        }
    }

    /// Plants sit around their state's cities, a few per city, so a busy state reads as busy.
    private func drawMarkers(in context: inout GraphicsContext, frame: Framing) {
        let byState = Dictionary(grouping: markers, by: \.state)
        for (code, plants) in byState {
            guard let name = map.playStates.first(where: { $0.code == code })?.name else { continue }
            var anchors = map.cities(in: name).prefix(6).map { CGPoint(x: $0.x, y: $0.y) }
            if anchors.isEmpty, let hub = map.hubs[code] { anchors = [hub] }
            guard !anchors.isEmpty else { continue }
            let radius: CGFloat = 9
            for (index, plant) in plants.sorted(by: { $0.id < $1.id }).enumerated() {
                let anchor = frame.toScreen(anchors[index % anchors.count])
                let ring = index / anchors.count
                let angle = Double(ring) * 1.9 + Double(index) * 0.7
                let distance = CGFloat(ring) * radius * 1.9 + radius * 1.4
                let center = CGPoint(x: anchor.x + CGFloat(cos(angle)) * distance, y: anchor.y + CGFloat(sin(angle)) * distance)
                let rect = CGRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect), with: .color(Sector.color(plant.sector)))
                context.stroke(
                    Path(ellipseIn: rect.insetBy(dx: -1.5, dy: -1.5)),
                    with: .color(plant.player ? Theme.gold : Theme.ink),
                    lineWidth: plant.player ? 3 : 1.5
                )
                var symbol = context.resolve(Image(systemName: Sector.symbol(plant.sector)))
                symbol.shading = .color(Theme.ink)
                context.draw(symbol, in: rect.insetBy(dx: 4.5, dy: 4.5))
            }
        }
    }
}
