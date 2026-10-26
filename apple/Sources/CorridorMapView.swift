import SwiftUI

/// What the map shades the regions by.
enum MapLayer: String, CaseIterable, Identifiable {
    case presence = "Your divisions"
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

/// One of the player's divisions on the map: a sector type the player runs in a region.
struct DivisionMarker: Identifiable {
    let region: String
    let sector: String
    let plants: Int
    var id: String { region + "/" + sector }
}

/// The real map of the Great Lakes and Ohio Valley, drawn from bundled public-domain geometry
/// with pan, pinch zoom and tap-to-select a region. Shading, division markers and selection
/// come from the caller.
struct CorridorMapView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let shading: [String: Double]
    let layer: MapLayer
    let divisions: [DivisionMarker]
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
                let hit = map.region(at: point)
                selected = hit == selected ? nil : hit
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Map of the playable regions")
            .accessibilityValue(selected.map { map.regionName($0) } ?? "No region selected")
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
        context.fill(map.contextLand.applying(transform), with: .color(Theme.landDim))
        for region in map.regions {
            let path = region.path.applying(transform)
            context.fill(path, with: .color(Theme.land))
            let value = shading[region.code] ?? 0
            if value > 0 {
                context.fill(path, with: .color(layer.tint.opacity(0.12 + 0.55 * min(value, 1))))
            }
        }
        context.stroke(map.counties.applying(transform), with: .color(Theme.stroke.opacity(0.45)), lineWidth: 0.5)
        context.fill(map.lakes.applying(transform), with: .color(Theme.water))
        context.stroke(map.rivers.applying(transform), with: .color(Theme.water.opacity(0.9)), lineWidth: 1.4)
        context.stroke(map.interstates.applying(transform), with: .color(Theme.road.opacity(0.4)), lineWidth: 1)
        // Region borders, then state borders over them, then the selected region on top.
        for region in map.regions {
            context.stroke(region.path.applying(transform), with: .color(Theme.secondary.opacity(0.45)), lineWidth: 0.9)
        }
        for state in map.playStates {
            context.stroke(state.path.applying(transform), with: .color(Theme.primaryText.opacity(0.7)), lineWidth: 1.8)
        }
        if let selected, let region = map.regions.first(where: { $0.code == selected }) {
            context.stroke(region.path.applying(transform), with: .color(Theme.gold), lineWidth: 3)
        }
        if layer == .presence {
            drawDivisions(in: &context, frame: frame)
        }
        drawCities(in: &context, frame: frame)
        if zoom * pinch > 1.8 {
            drawRegionNames(in: &context, frame: frame)
        }
    }

    /// Region names once the player has zoomed in far enough to read them.
    private func drawRegionNames(in context: inout GraphicsContext, frame: Framing) {
        for region in map.regions {
            let point = frame.toScreen(CGPoint(x: region.bounds.midX, y: region.bounds.midY))
            context.draw(
                Text(region.name.uppercased())
                    .font(.system(size: 9, weight: .semibold))
                    .tracking(0.8)
                    .foregroundStyle(Theme.secondary.opacity(0.8)),
                at: CGPoint(x: point.x, y: point.y + 14)
            )
        }
    }

    /// The player's divisions: one sector circle per owned sector type in each region, in a row
    /// beside the region's anchor city, with a badge counting its plants.
    private func drawDivisions(in context: inout GraphicsContext, frame: Framing) {
        let radius: CGFloat = 11
        let byRegion = Dictionary(grouping: divisions, by: \.region)
        for (code, owned) in byRegion {
            guard let anchor = map.anchor(of: code) else { continue }
            let origin = frame.toScreen(anchor)
            let sorted = owned.sorted { (Sector.order.firstIndex(of: $0.sector) ?? 99) < (Sector.order.firstIndex(of: $1.sector) ?? 99) }
            let spacing = radius * 2 + 5
            let width = CGFloat(sorted.count - 1) * spacing
            for (index, division) in sorted.enumerated() {
                // Below the anchor so the city label to its right stays clear.
                let center = CGPoint(x: origin.x - width / 2 + CGFloat(index) * spacing, y: origin.y + radius + 8)
                let rect = CGRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect.insetBy(dx: -2, dy: -2)), with: .color(Theme.ink))
                context.fill(Path(ellipseIn: rect), with: .color(Sector.color(division.sector)))
                context.stroke(Path(ellipseIn: rect.insetBy(dx: -1.5, dy: -1.5)), with: .color(Theme.gold), lineWidth: 2)
                var symbol = context.resolve(Image(systemName: Sector.symbol(division.sector)))
                symbol.shading = .color(Theme.ink)
                context.draw(symbol, in: rect.insetBy(dx: 5, dy: 5))
                // Plant count badge.
                let badge = CGRect(x: rect.maxX - 7, y: rect.minY - 5, width: 14, height: 14)
                context.fill(Path(ellipseIn: badge), with: .color(Theme.gold))
                context.draw(
                    Text("\(division.plants)").font(.system(size: 9, weight: .bold)).foregroundStyle(Theme.ink),
                    at: CGPoint(x: badge.midX, y: badge.midY)
                )
            }
        }
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
}
