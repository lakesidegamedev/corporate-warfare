import SwiftUI

/// What the map shades the regions by.
enum MapLayer: String, CaseIterable, Identifiable {
    case presence = "Your divisions"
    case room = "Room to build"
    case wages = "Wages"
    case skill = "Workforce skill"

    var id: String { rawValue }

    var icon: String {
        switch self {
        case .presence: return "layer_presence"
        case .room: return "layer_room"
        case .wages: return "layer_wages"
        case .skill: return "layer_skill"
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


/// The world map: a globe when zoomed out that flattens into a map as the player zooms in on the
/// scenario. Drag to turn or pan, pinch or use the buttons to zoom, double-tap to fly between the
/// globe and the scenario, tap a region to select it. Shading, markers and selection come from
/// the caller.
struct WorldMapView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @AppStorage(GraphicsKeys.mapDetail) private var detailLevel = MapDetailLevel.high.rawValue
    @AppStorage(GraphicsKeys.animations) private var animations = true
    let shading: [String: Double]
    let layer: MapLayer
    let divisions: [DivisionMarker]
    @Binding var selected: String?

    @State private var camera: MapCamera?
    @State private var lastDrag: CGSize = .zero
    @State private var lastPinch: CGFloat = 1
    @State private var flight: Task<Void, Never>?

    private let map = WorldMap.shared
    private static let space = Color(red: 0.020, green: 0.035, blue: 0.055)
    private static let outsideLand = Color(red: 0.145, green: 0.157, blue: 0.169)
    private static let outsideBorder = Color(red: 0.333, green: 0.349, blue: 0.365)
    private static let outsideRoad = Color(red: 0.416, green: 0.431, blue: 0.447)
    private static let graticule: [GeoRing] = {
        var rings: [GeoRing] = []
        for lon in stride(from: -180, to: 180, by: 15) {
            rings.append(GeoRing(stride(from: -80, through: 80, by: 4).flatMap { [lon * 10_000, $0 * 10_000] }))
        }
        for lat in stride(from: -75, through: 75, by: 15) {
            rings.append(GeoRing(stride(from: -180, through: 180, by: 4).flatMap { [$0 * 10_000, lat * 10_000] }))
        }
        return rings
    }()

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            let current = camera ?? MapCamera.scenario(in: size)
            Canvas { context, size in
                draw(in: &context, projection: MapProjection(camera: current, size: size), camera: current)
            }
            .background(WorldMapView.space)
            .contentShape(Rectangle())
            .gesture(
                SimultaneousGesture(
                    MagnifyGesture()
                        .onChanged { value in
                            flight?.cancel()
                            let factor = value.magnification / lastPinch
                            lastPinch = value.magnification
                            camera = (camera ?? current).zoomed(by: Double(factor))
                        }
                        .onEnded { _ in lastPinch = 1 },
                    DragGesture(minimumDistance: 6)
                        .onChanged { value in
                            flight?.cancel()
                            let delta = CGSize(width: value.translation.width - lastDrag.width,
                                               height: value.translation.height - lastDrag.height)
                            lastDrag = value.translation
                            camera = (camera ?? current).panned(by: delta, in: size)
                        }
                        .onEnded { _ in lastDrag = .zero }
                )
            )
            .onTapGesture(count: 2) {
                let target = current.zoom > MapCamera.flatStart
                    ? MapCamera(lon: current.lon, lat: current.lat, zoom: 1)
                    : MapCamera.scenario(in: size)
                fly(from: current, to: target)
            }
            .onTapGesture(coordinateSpace: .local) { location in
                let projection = MapProjection(camera: current, size: size)
                let hit = map.regions.first { projection.contains($0.rings, location) }?.code
                selected = hit == selected ? nil : hit
            }
            .overlay(alignment: .bottomTrailing) {
                zoomControls(current: current, size: size)
                    .padding(.trailing, 12)
                    .padding(.bottom, 40)
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("World map. The scenario's regions can be selected.")
            .accessibilityValue(selected.map { map.regionName($0) } ?? "No region selected")
            .accessibilityAction(named: "Show the whole globe") {
                fly(from: current, to: MapCamera(lon: current.lon, lat: current.lat, zoom: 1))
            }
            .accessibilityAction(named: "Show the scenario") { fly(from: current, to: MapCamera.scenario(in: size)) }
        }
    }

    private func zoomControls(current: MapCamera, size: CGSize) -> some View {
        VStack(spacing: 0) {
            mapButton("plus", label: "Zoom in") { fly(from: current, to: current.zoomed(by: 1.8)) }
            Divider().frame(width: 36).overlay(Theme.stroke)
            mapButton("minus", label: "Zoom out") { fly(from: current, to: current.zoomed(by: 1 / 1.8)) }
            Divider().frame(width: 36).overlay(Theme.stroke)
            mapButton(current.zoom > MapCamera.flatStart ? "globe" : "scope",
                      label: current.zoom > MapCamera.flatStart ? "Show the whole globe" : "Show the scenario") {
                let target = current.zoom > MapCamera.flatStart
                    ? MapCamera(lon: current.lon, lat: current.lat, zoom: 1)
                    : MapCamera.scenario(in: size)
                fly(from: current, to: target)
            }
        }
        .background(Theme.panel.opacity(0.94), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.stroke))
    }

    private func mapButton(_ icon: String, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            PixelIcon(icon, size: 24)
                .foregroundStyle(Theme.primaryText)
                .frame(width: 40, height: 40)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    /// Animates the camera to a target, easing in and out, about 60 frames a second.
    private func fly(from start: MapCamera, to target: MapCamera) {
        flight?.cancel()
        if reduceMotion || !animations {
            camera = target
            return
        }
        flight = Task { @MainActor in
            let frames = 42
            for frame in 1...frames {
                if Task.isCancelled { return }
                let x = Double(frame) / Double(frames)
                camera = MapCamera.interpolate(start, target, x * x * (3 - 2 * x))
                try? await Task.sleep(nanoseconds: 16_000_000)
            }
        }
    }

    private func draw(in context: inout GraphicsContext, projection p: MapProjection, camera: MapCamera) {
        let t = p.t
        let center = CGPoint(x: p.cx, y: p.cy)
        // Ocean: the globe's disc, growing to fill the view as the map flattens.
        let oceanRadius = p.r / max(1 - t, 0.03)
        let disc = CGRect(x: center.x - oceanRadius, y: center.y - oceanRadius, width: oceanRadius * 2, height: oceanRadius * 2)
        context.fill(Path(ellipseIn: disc), with: .color(Theme.water.opacity(0.55)))
        if t < 1 {
            context.stroke(p.lines(WorldMapView.graticule), with: .color(Theme.primaryText.opacity(0.06 * (1 - t))), lineWidth: 0.6)
        }

        // Outside the scenario the world is greyed out: neutral land, grey borders and roads.
        // The scenario's regions are drawn over it in colour.
        var land = Path()
        for country in map.countries {
            for ring in country.rings { p.addPolygon(ring, to: &land) }
        }
        context.fill(land, with: .color(WorldMapView.outsideLand), style: FillStyle(eoFill: true))
        let provinces = detailLevel == 0 ? 0 : min(max((camera.zoom - 1.4) / 1.2, 0), 1)
        if provinces > 0 {
            context.stroke(p.lines(map.provinces), with: .color(WorldMapView.outsideBorder.opacity(0.7 * provinces)), lineWidth: 0.5)
        }
        let roads = detailLevel < 2 ? 0 : min(max((camera.zoom - 2.4) / 1.6, 0), 1)
        if roads > 0 {
            context.stroke(p.lines(map.roads), with: .color(WorldMapView.outsideRoad.opacity(0.6 * roads)), lineWidth: 0.8)
        }
        context.stroke(land, with: .color(WorldMapView.outsideBorder), lineWidth: 0.8)

        let detail = detailLevel < 2 ? 0 : min(max((camera.zoom - 7) / 5, 0), 1)
        let regionPaths = map.regions.map { (code: $0.code, path: p.path($0.rings)) }
        for region in regionPaths {
            context.fill(region.path, with: .color(Theme.land))
            let value = shading[region.code] ?? 0
            if value > 0 {
                context.fill(region.path, with: .color(layer.tint.opacity(0.12 + 0.55 * min(value, 1))))
            }
        }
        if detail > 0 {
            context.stroke(p.lines(map.counties), with: .color(Theme.stroke.opacity(0.45 * detail)), lineWidth: 0.5)
        }
        context.fill(p.path(map.lakes), with: .color(Theme.water))
        if detail > 0 {
            context.stroke(p.lines(map.rivers), with: .color(Theme.water.opacity(0.9 * detail)), lineWidth: 1.4)
            context.stroke(p.lines(map.interstates), with: .color(Theme.road.opacity(0.4 * detail)), lineWidth: 1)
        }
        // Region borders, then state borders over them, then the selected region on top.
        for region in regionPaths {
            context.stroke(region.path, with: .color(Theme.secondary.opacity(0.45)), lineWidth: 0.9)
        }
        for state in map.playStates {
            context.stroke(p.path(state.rings), with: .color(Theme.primaryText.opacity(0.7)), lineWidth: camera.zoom > 3 ? 1.8 : 1)
        }
        if let selected, let region = regionPaths.first(where: { $0.code == selected }) {
            context.stroke(region.path, with: .color(Theme.gold), lineWidth: 3)
        }
        if t < 1 {
            // The globe's rim, fading as it unrolls.
            let rim = CGRect(x: center.x - p.r, y: center.y - p.r, width: p.r * 2, height: p.r * 2)
            context.stroke(Path(ellipseIn: rim), with: .color(Theme.water.opacity(0.9 * (1 - t))), lineWidth: 1.5)
            context.stroke(Path(ellipseIn: rim.insetBy(dx: -4, dy: -4)), with: .color(Theme.water.opacity(0.3 * (1 - t))), lineWidth: 6)
        }
        if camera.zoom > 3 {
            if layer == .presence {
                drawDivisions(in: &context, projection: p)
            }
            drawCities(in: &context, projection: p, zoom: camera.zoom)
        }
        if camera.zoom > 14 {
            drawRegionNames(in: &context, projection: p)
        }
    }

    /// Region names once the player has zoomed in far enough to read them.
    private func drawRegionNames(in context: inout GraphicsContext, projection p: MapProjection) {
        for region in map.regions {
            guard let anchor = map.anchor(of: region.code) else { continue }
            let (point, visible) = p.project(anchor)
            guard visible else { continue }
            context.draw(
                Text(region.name.uppercased())
                    .font(.system(size: 9, weight: .semibold))
                    .tracking(0.8)
                    .foregroundStyle(Theme.secondary.opacity(0.8)),
                at: CGPoint(x: point.x, y: point.y + 14)
            )
        }
    }

    /// The player's divisions: one sector badge per owned sector type in each region, in a row
    /// below the region's anchor city, with a badge counting its plants.
    private func drawDivisions(in context: inout GraphicsContext, projection p: MapProjection) {
        let radius: CGFloat = 11
        let byRegion = Dictionary(grouping: divisions, by: \.region)
        for (code, owned) in byRegion {
            guard let anchor = map.anchor(of: code) else { continue }
            let (origin, visible) = p.project(anchor)
            guard visible else { continue }
            let sorted = owned.sorted { (Sector.order.firstIndex(of: $0.sector) ?? 99) < (Sector.order.firstIndex(of: $1.sector) ?? 99) }
            let spacing = radius * 2 + 5
            let width = CGFloat(sorted.count - 1) * spacing
            for (index, division) in sorted.enumerated() {
                let center = CGPoint(x: origin.x - width / 2 + CGFloat(index) * spacing, y: origin.y + radius + 8)
                let rect = CGRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect.insetBy(dx: -2, dy: -2)), with: .color(Theme.ink))
                context.fill(Path(ellipseIn: rect), with: .color(Sector.color(division.sector)))
                context.stroke(Path(ellipseIn: rect.insetBy(dx: -1.5, dy: -1.5)), with: .color(Theme.gold), lineWidth: 2)
                if let art = SectorIcons.all[division.sector] {
                    PixelIcons.draw(
                        rows: art.rows, palette: art.palette, gridSize: art.rows.count,
                        in: &context, rect: rect.insetBy(dx: 5, dy: 5), tint: Theme.ink
                    )
                }
                let badge = CGRect(x: rect.maxX - 7, y: rect.minY - 5, width: 14, height: 14)
                context.fill(Path(ellipseIn: badge), with: .color(Theme.gold))
                context.draw(
                    Text("\(division.plants)").font(.system(size: 9, weight: .bold)).foregroundStyle(Theme.ink),
                    at: CGPoint(x: badge.midX, y: badge.midY)
                )
            }
        }
    }

    private func drawCities(in context: inout GraphicsContext, projection p: MapProjection, zoom: Double) {
        let close = zoom > 14
        let shown = close ? 40 : 14
        for city in map.cities.prefix(shown) where city.country == "USA" || close {
            let (point, visible) = p.project(GeoPoint(e4: city.x, city.y))
            guard visible else { continue }
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
