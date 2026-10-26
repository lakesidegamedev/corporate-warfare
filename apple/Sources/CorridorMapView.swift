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
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
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
                withAnimation(reduceMotion ? nil : .easeOut(duration: 0.25)) {
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
        // The eight states are wider than tall. A portrait phone fitting the whole width leaves
        // most of the screen empty, so it frames the middle of the corridor at a readable size
        // and the player pans east or west.
        let portrait = size.height > size.width * 1.2
        let widthShown = portrait ? bounds.width * 0.58 : bounds.width
        let fit = min(size.width / max(widthShown, 1), size.height / max(bounds.height, 1))
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
        drawMarkers(in: &context, frame: frame)
        drawCities(in: &context, frame: frame)
        _ = hairline
    }

    private func drawCities(in context: inout GraphicsContext, frame: Framing) {
        let shown = zoom * pinch > 2 ? 40 : 14
        for city in map.cities.prefix(shown) where city.country == "USA" || zoom * pinch > 2 {
            let point = frame.toScreen(CGPoint(x: city.x, y: city.y))
            let dot = CGRect(x: point.x - 2.5, y: point.y - 2.5, width: 5, height: 5)
            context.fill(Path(ellipseIn: dot), with: .color(Theme.primaryText.opacity(0.85)))
            // Labels sit above plant markers with an ink halo so a busy city stays readable.
            let at = CGPoint(x: point.x + 6, y: point.y)
            let halo = context.resolve(Text(city.name).font(.system(size: 10, weight: .semibold)).foregroundStyle(Theme.ink))
            for dx in [-1.0, 1.0] {
                for dy in [-1.0, 1.0] {
                    context.draw(halo, at: CGPoint(x: at.x + dx, y: at.y + dy), anchor: .leading)
                }
            }
            context.draw(
                Text(city.name).font(.system(size: 10, weight: .semibold)).foregroundStyle(Theme.primaryText.opacity(0.9)),
                at: at,
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
            // City labels run to the right of their dot, so plants fan out on the left half:
            // three slots in the first ring, five in each wider ring.
            let plantsByCity = Dictionary(grouping: plants.sorted(by: { $0.id < $1.id }).enumerated().map { $0 }) { $0.offset % anchors.count }
            for (city, group) in plantsByCity {
                for (slot, entry) in group.enumerated() {
                    let anchor = frame.toScreen(anchors[city])
                    let (ring, angle) = Self.slot(slot)
                    let distance = radius * (2.2 + 2.1 * CGFloat(ring))
                    let center = CGPoint(x: anchor.x + CGFloat(cos(angle)) * distance, y: anchor.y + CGFloat(sin(angle)) * distance)
                    drawMarker(entry.element, at: center, radius: radius, in: &context)
                }
            }
        }
    }

    /// Ring and angle for the n-th plant at one city, all on the side away from its label.
    private static func slot(_ n: Int) -> (Int, Double) {
        if n < 3 {
            return (0, Double.pi * (2.0 / 3.0 + Double(n) / 3.0))
        }
        let k = n - 3
        return (1 + k / 5, Double.pi * (0.5 + Double(k % 5) / 4.0))
    }

    private func drawMarker(_ plant: MapMarker, at center: CGPoint, radius: CGFloat, in context: inout GraphicsContext) {
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
