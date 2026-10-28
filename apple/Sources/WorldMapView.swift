import SwiftUI
import UIKit

/// The map projected for one camera, view size and tilt. Each path is built the first time it is
/// drawn and kept, so a redraw that moves nothing (a new week, a selection) projects nothing.
final class MapGeometry {
    let map: WorldMap
    let camera: MapCamera
    let size: CGSize
    let tilt: Double
    let projection: MapProjection

    init(map: WorldMap, camera: MapCamera, size: CGSize, tilt: Double) {
        self.map = map
        self.camera = camera
        self.size = size
        self.tilt = tilt
        self.projection = MapProjection(camera: camera, size: size, tilt: tilt)
    }

    lazy var graticule: Path = projection.lines(WorldMapView.graticuleRings)
    lazy var land: Path = {
        var path = Path()
        for country in map.countries {
            for ring in country.rings { projection.addPolygon(ring, to: &path) }
        }
        return path
    }()
    lazy var provinces: Path = projection.lines(map.provinces)
    lazy var roads: Path = projection.lines(map.roads)
    lazy var regionPaths: [(code: String, path: Path)] = map.regions.map { (code: $0.code, path: projection.path($0.rings)) }
    lazy var counties: Path = projection.lines(map.counties)
    lazy var lakes: Path = projection.path(map.lakes)
    lazy var rivers: Path = projection.lines(map.rivers)
    lazy var interstates: Path = projection.lines(map.interstates)
    lazy var playStates: [Path] = map.playStates.map { projection.path($0.rings) }

    private var raisedHeights: [Double]?
    private var raisedHeld: [RaisedRegion] = []

    /// The scenario's regions as raised tiles at these heights (one per region), farthest first.
    func raised(_ heights: [Double]) -> [RaisedRegion] {
        if let raisedHeights, raisedHeights == heights { return raisedHeld }
        var built: [RaisedRegion] = []
        for (index, region) in map.regions.enumerated() {
            let h = index < heights.count ? heights[index] : 0
            let ground = regionPaths[index].path
            built.append(RaisedRegion(index: index, region: region, height: h, ground: ground, map: map, projection: projection))
        }
        built.sort { $0.sortY < $1.sortY }
        raisedHeights = heights
        raisedHeld = built
        return built
    }

    private var highwayHeights: [Double]?
    private var highwayHeld: [MapPolyline] = []

    /// Highways as screen polylines with running lengths, for the cars along them. Each point
    /// sits on its own region's tile.
    func highways(_ heights: [Double]) -> [MapPolyline] {
        if let highwayHeights, highwayHeights == heights { return highwayHeld }
        let p = projection
        let bounds = CGRect(origin: .zero, size: size).insetBy(dx: -40, dy: -40)
        let regionOf: [[Int]] = heights.isEmpty ? [] : map.regionIndex.interstatePoints
        var lines: [MapPolyline] = []
        let jump = p.r * 0.5
        for (ringIndex, ring) in map.interstates.enumerated() where p.mayShow(ring) {
            var current: [CGPoint] = []
            func finish() {
                if current.count >= 2 { lines.append(MapPolyline(current)) }
                current.removeAll(keepingCapacity: true)
            }
            for (pointIndex, point) in ring.points.enumerated() {
                let (flatPoint, visible) = p.flat(point)
                var h = 0.0
                if !regionOf.isEmpty {
                    let region = regionOf[ringIndex][pointIndex]
                    if region >= 0, region < heights.count { h = heights[region] }
                }
                let screen = p.lift(flatPoint, h)
                guard visible, bounds.contains(screen) else {
                    finish()
                    continue
                }
                if let last = current.last, abs(screen.x - last.x) + abs(screen.y - last.y) >= jump { finish() }
                current.append(screen)
            }
            finish()
        }
        highwayHeights = heights
        highwayHeld = lines
        return lines
    }
}

/// Holds the last `MapGeometry`. A reference type so the canvas can refill it while drawing.
final class MapGeometryCache {
    private var held: MapGeometry?

    func of(map: WorldMap, camera: MapCamera, size: CGSize, tilt: Double = 0) -> MapGeometry {
        if let held, held.map === map, held.camera == camera, held.size == size, held.tilt == tilt { return held }
        let fresh = MapGeometry(map: map, camera: camera, size: size, tilt: tilt)
        held = fresh
        return fresh
    }
}

/// A polyline on screen with the running length at each point.
struct MapPolyline {
    let points: [CGPoint]
    let lengths: [Double]
    var total: Double { lengths.last ?? 0 }

    init(_ points: [CGPoint]) {
        self.points = points
        var lengths: [Double] = [0]
        lengths.reserveCapacity(points.count)
        for index in 1..<points.count {
            let a = points[index - 1]
            let b = points[index]
            lengths.append(lengths[index - 1] + Double(hypot(b.x - a.x, b.y - a.y)))
        }
        self.lengths = lengths
    }

    /// The point a distance along the line and the direction of travel there.
    func point(at distance: Double) -> (CGPoint, CGVector) {
        var low = 0
        var high = lengths.count - 1
        while high - low > 1 {
            let mid = (low + high) / 2
            if lengths[mid] <= distance { low = mid } else { high = mid }
        }
        let a = points[low]
        let b = points[high]
        let span = max(lengths[high] - lengths[low], 1e-6)
        let f = min(max((distance - lengths[low]) / span, 0), 1)
        let dx = Double(b.x - a.x) / span
        let dy = Double(b.y - a.y) / span
        return (CGPoint(x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f), CGVector(dx: dx, dy: dy))
    }
}

/// One scenario region standing up as a tile: its footprint, its top face, its walls in three
/// shades of light, and the detail drawn on its top.
final class RaisedRegion {
    /// Wall light levels, darkest first.
    static let levels: [Double] = [0.5 + 0.4 / 6, 0.5 + 0.4 / 2, 0.5 + 0.4 * 5 / 6]

    let index: Int
    let code: String
    let height: Double
    let ground: Path
    let top: Path
    let walls: [Path]
    let sortY: CGFloat
    private let rings: [GeoRing]
    private let map: WorldMap
    private let projection: MapProjection

    init(index: Int, region: WorldMap.RegionFeature, height: Double, ground: Path, map: WorldMap, projection p: MapProjection) {
        self.index = index
        code = region.code
        self.height = height
        self.ground = ground
        rings = region.rings
        self.map = map
        projection = p
        top = height > 0 ? p.path(region.rings, height: height) : ground
        var walls = [Path(), Path(), Path()]
        if height > 0.5 {
            // Light from the upper left of the screen.
            let lx = -0.55 / (0.55 * 0.55 + 0.83 * 0.83).squareRoot()
            let ly = -0.83 / (0.55 * 0.55 + 0.83 * 0.83).squareRoot()
            for ring in region.rings {
                guard let flat = p.flatRing(ring) else { continue }
                let bottom = flat.map { p.lift($0, 0) }
                let upper = flat.map { p.lift($0, height) }
                let n = bottom.count
                var area = 0.0
                for i in 0..<n {
                    let a = bottom[i]
                    let b = bottom[(i + 1) % n]
                    area += Double(a.x * b.y - b.x * a.y)
                }
                for i in 0..<n {
                    let j = (i + 1) % n
                    let ex = Double(bottom[j].x - bottom[i].x)
                    let ey = Double(bottom[j].y - bottom[i].y)
                    let length = (ex * ex + ey * ey).squareRoot()
                    guard length > 0.01 else { continue }
                    var nx = ey / length
                    var ny = -ex / length
                    if area < 0 {
                        nx = -nx
                        ny = -ny
                    }
                    // Only walls facing the viewer.
                    guard ny > -0.05 else { continue }
                    let k = 0.50 + 0.40 * max(0, -(nx * lx + ny * ly))
                    let bucket = min(max(Int((k - 0.5) / 0.4 * 3), 0), 2)
                    walls[bucket].move(to: bottom[i])
                    walls[bucket].addLine(to: bottom[j])
                    walls[bucket].addLine(to: upper[j])
                    walls[bucket].addLine(to: upper[i])
                    walls[bucket].closeSubpath()
                }
            }
        }
        self.walls = walls
        if let anchor = map.anchor(of: region.code) {
            sortY = p.project(anchor).0.y
        } else {
            sortY = ground.boundingRect.midY
        }
    }

    private func lines(_ indices: [Int], _ source: [GeoRing]) -> Path {
        var path = Path()
        for index in indices { projection.addLine(source[index], to: &path, height: height) }
        return path
    }

    lazy var counties: Path = lines(map.regionIndex.counties[index], map.counties)
    lazy var rivers: Path = lines(map.regionIndex.rivers[index], map.rivers)
    lazy var interstates: Path = lines(map.regionIndex.interstates[index], map.interstates)
    lazy var lakes: Path = {
        var path = Path()
        for i in map.regionIndex.lakes[index] { projection.addPolygon(map.lakes[i], to: &path, height: height) }
        return path
    }()
    lazy var states: Path = {
        var path = Path()
        for i in map.regionIndex.states[index] {
            for ring in map.playStates[i].rings { projection.addPolygon(ring, to: &path, height: height) }
        }
        return path
    }()
}

/// What the map shades the regions by.
enum MapLayer: String, CaseIterable, Identifiable {
    case presence = "Your divisions"
    case room = "Room to build"
    case wages = "Wages"
    case skill = "Workforce skill"
    case freight = "Freight"

    var id: String { rawValue }

    var icon: String {
        switch self {
        case .presence: return "layer_presence"
        case .room: return "layer_room"
        case .wages: return "layer_wages"
        case .skill: return "layer_skill"
        case .freight: return "layer_freight"
        }
    }

    var tint: Color {
        switch self {
        case .presence: return Theme.gold
        case .room: return Theme.positive
        case .wages: return Theme.copper
        case .skill: return Color(red: 0.259, green: 0.647, blue: 0.961)
        case .freight: return Theme.gold
        }
    }

    /// `tint` as components, for shading raised walls.
    var tintRGB: RGB {
        switch self {
        case .presence, .freight: return .gold
        case .room: return .positive
        case .wages: return .copper
        case .skill: return .blue
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

/// The graphics options the map reads, gathered once per update.
struct MapLook {
    var detail: Int
    var tilt: Double
    var extrude: Bool
    var lighting: Int
    var dayNight: Bool
    var cityLights: Bool
    var clouds: Int
    var traffic: Int
    var buildings: Int
    var frameRate: Int
    var showFps: Bool
    var smoke: Bool
}

/// Everything a draw shares for one camera: the projection, the cached geometry and how high
/// each region stands.
struct MapFrame {
    let projection: MapProjection
    let geometry: MapGeometry
    /// Size scale against the scenario's own framing, for buildings and heights.
    let zs: Double
    /// Height per region index; empty on a flat map.
    let heights: [Double]
    let raised: [RaisedRegion]
    private let byCode: [String: Double]
    private let map: WorldMap

    init(map: WorldMap, geometry: MapGeometry, zs: Double, heights: [Double]) {
        self.map = map
        self.geometry = geometry
        projection = geometry.projection
        self.zs = zs
        self.heights = heights
        if heights.isEmpty {
            raised = []
            byCode = [:]
        } else {
            raised = geometry.raised(heights)
            var codes: [String: Double] = [:]
            for (index, region) in map.regions.enumerated() where index < heights.count { codes[region.code] = heights[index] }
            byCode = codes
        }
    }

    func height(of code: String) -> Double { byCode[code] ?? 0 }

    func cityHeight(_ index: Int) -> Double {
        guard !heights.isEmpty else { return 0 }
        let region = map.regionIndex.cityRegion[index]
        return region >= 0 && region < heights.count ? heights[region] : 0
    }

    /// A region's anchor on top of its tile.
    func anchor(of code: String) -> (CGPoint, Bool)? {
        guard let anchor = map.anchor(of: code) else { return nil }
        return projection.project(anchor, height: height(of: code))
    }
}

/// Region heights ease to new values when the layer or the data changes.
final class HeightEase {
    private var from: [String: Double] = [:]
    private var start = -1.0

    func retarget(from old: [String: Double], to new: [String: Double], now: Double, animate: Bool) {
        if animate {
            from = value(at: now, target: old)
            start = now
        } else {
            start = -1
        }
    }

    func value(at now: Double, target: [String: Double]) -> [String: Double] {
        guard start >= 0 else { return target }
        let x = min(max((now - start) / 0.6, 0), 1)
        if x >= 1 {
            start = -1
            return target
        }
        let e = x * x * (3 - 2 * x)
        var out: [String: Double] = [:]
        for key in Set(from.keys).union(target.keys) {
            let a = from[key] ?? 0
            out[key] = a + ((target[key] ?? 0) - a) * e
        }
        return out
    }
}

/// A lane with moving freight: how many particles run each way and how they look.
struct FlowLane {
    let lane: LaneDto
    let perWay: Int
    let style: Int
    let speed: Double
}

/// A division drawn as a building, placed for one frame.
struct PlacedBuilding {
    let id: String
    let sector: String
    let plants: Int
    let box: MapBox
    let base: CGPoint
    let geo: GeoPoint
}

/// The world map: a globe when zoomed out that flattens into a map as the player zooms in on the
/// scenario. Drag to turn or pan, pinch or use the buttons to zoom, double-tap to fly between the
/// globe and the scenario, tap a region to select it. Shading, markers and selection come from
/// the caller. A still layer holds the land, regions and buildings; a moving layer over it holds
/// traffic, clouds and pulses, and only runs while something on it moves.
struct WorldMapView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage(GraphicsKeys.mapDetail) private var detailLevel = MapDetailLevel.high.rawValue
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @AppStorage(GraphicsKeys.preset) private var preset = GraphicsPreset.high.rawValue
    @AppStorage(GraphicsKeys.tilt) private var tiltSetting = 1
    @AppStorage(GraphicsKeys.extrude) private var extrude = true
    @AppStorage(GraphicsKeys.lighting) private var lighting = 2
    @AppStorage(GraphicsKeys.dayNight) private var dayNight = true
    @AppStorage(GraphicsKeys.cityLights) private var cityLights = true
    @AppStorage(GraphicsKeys.clouds) private var clouds = 1
    @AppStorage(GraphicsKeys.traffic) private var traffic = 1
    @AppStorage(GraphicsKeys.buildings) private var buildings = 2
    @AppStorage(GraphicsKeys.frameRate) private var frameRate = 60
    @AppStorage(GraphicsKeys.showFps) private var showFps = false
    let shading: [String: Double]
    let layer: MapLayer
    let divisions: [DivisionMarker]
    var lanes: [LaneDto] = []
    /// The lane picked on the Freight layer, and what to do when a lane is tapped.
    var pickedLane: String?
    var onLane: ((String?) -> Void)?
    /// The career's region codes, which say which scenario's map to draw.
    var regionCodes: [String] = []
    /// The game week; a new week sends a ripple out from each building.
    var week: UInt32 = 0
    @Binding var selected: String?

    @State private var camera: MapCamera?
    @State private var lastDrag: CGSize = .zero
    @State private var lastPinch: CGFloat = 1
    @State private var flight: Task<Void, Never>?
    @State private var geometry = MapGeometryCache()
    @State private var ease = HeightEase()
    @State private var easing = false
    @State private var rippleStart = -10.0
    @State private var rippling = false
    @State private var meter = FrameMeter()

    private var map: WorldMap { WorldMap.forRegions(regionCodes) }
    private static let space = Color(red: 0.020, green: 0.035, blue: 0.055)
    private static let outsideLand = Color(red: 0.145, green: 0.157, blue: 0.169)
    private static let outsideBorder = Color(red: 0.333, green: 0.349, blue: 0.365)
    private static let outsideRoad = Color(red: 0.416, green: 0.431, blue: 0.447)
    /// Freight colours: road, rail, sea, other.
    private static let flowColors: [RGB] = [RGB(hex: 0xE0A040), RGB(hex: 0x7FD3FF), RGB(hex: 0x9AF0E0), RGB(hex: 0xE8E8E8)]
    private static let windowColor = RGB(hex: 0xFFD27A)
    private static let skylineSide = RGB(hex: 0x3A4A5C)
    private static let skylineTop = RGB(hex: 0x56687C)
    private static let haze = RGB(hex: 0x9CC3E6)
    /// Heavy industry, which smokes.
    private static let smoky: Set<String> = ["extraction", "energy", "chemical_industries", "manufacturing"]
    static let graticuleRings: [GeoRing] = {
        var rings: [GeoRing] = []
        for lon in stride(from: -180, to: 180, by: 15) {
            rings.append(GeoRing(stride(from: -80, through: 80, by: 4).flatMap { [lon * 10_000, $0 * 10_000] }))
        }
        for lat in stride(from: -75, through: 75, by: 15) {
            rings.append(GeoRing(stride(from: -180, through: 180, by: 4).flatMap { [$0 * 10_000, lat * 10_000] }))
        }
        return rings
    }()

    private var look: MapLook {
        MapLook(
            detail: detailLevel, tilt: GraphicsSettings.pitch(forTilt: tiltSetting), extrude: extrude,
            lighting: lighting, dayNight: dayNight, cityLights: cityLights, clouds: clouds, traffic: traffic,
            buildings: buildings, frameRate: frameRate, showFps: showFps,
            smoke: preset == GraphicsPreset.ultra.rawValue || traffic == 2
        )
    }

    private var animate: Bool { animations && !reduceMotion }
    /// Whether moving effects may run at all.
    private var clockAllowed: Bool { animate && scenePhase == .active }

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            let current = camera ?? MapCamera.scenario(in: size, map: map)
            let look = self.look
            let flow = flowLanes(look)
            let moving = overlayMoves(look, camera: current)
            let interval = GraphicsSettings.frameInterval(look.frameRate)
            ZStack(alignment: .topLeading) {
                TimelineView(.animation(minimumInterval: easing ? interval : 0.5,
                                        paused: !(easing || (look.dayNight && clockAllowed)))) { timeline in
                    Canvas { context, canvasSize in
                        let f = mapFrame(camera: current, size: canvasSize, look: look, now: Date().timeIntervalSinceReferenceDate)
                        drawStatic(in: &context, frame: f, camera: current, look: look, flow: flow,
                                   time: timeline.date.timeIntervalSinceReferenceDate)
                    }
                    .drawingGroup()
                }
                if moving || look.showFps {
                    TimelineView(.animation(minimumInterval: interval, paused: !(clockAllowed && moving))) { timeline in
                        Canvas { context, canvasSize in
                            let now = timeline.date.timeIntervalSinceReferenceDate
                            let f = mapFrame(camera: current, size: canvasSize, look: look, now: Date().timeIntervalSinceReferenceDate)
                            drawOverlay(in: &context, frame: f, camera: current, look: look, flow: flow, time: now)
                            if look.showFps {
                                drawFrameRate(in: &context, time: now, running: clockAllowed && moving)
                            }
                        }
                        .drawingGroup()
                    }
                    .allowsHitTesting(false)
                }
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
                            let base = camera ?? current
                            let pitch = MapProjection(camera: base, size: size, tilt: look.tilt).pitch
                            camera = base.panned(by: delta, in: size, pitch: pitch)
                        }
                        .onEnded { _ in lastDrag = .zero }
                )
            )
            .onTapGesture(count: 2) {
                let target = current.zoom > MapCamera.flatStart
                    ? MapCamera(lon: current.lon, lat: current.lat, zoom: 1, tilt: current.tilt)
                    : MapCamera.scenario(in: size, map: map)
                fly(from: current, to: target)
            }
            .onTapGesture(coordinateSpace: .local) { location in
                let f = mapFrame(camera: current, size: size, look: look, now: Date().timeIntervalSinceReferenceDate)
                if layer == .freight, let onLane {
                    if let lane = laneAt(location, frame: f) {
                        onLane(lane.id == pickedLane ? nil : lane.id)
                        return
                    }
                    onLane(nil)
                }
                let hit: String?
                if f.raised.isEmpty {
                    hit = map.regions.first { f.projection.contains($0.rings, location) }?.code
                } else {
                    // Nearest tile first, on its raised top face.
                    hit = f.raised.reversed().first { $0.top.contains(location) }?.code
                }
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
                fly(from: current, to: MapCamera(lon: current.lon, lat: current.lat, zoom: 1, tilt: current.tilt))
            }
            .accessibilityAction(named: "Show the scenario") { fly(from: current, to: MapCamera.scenario(in: size, map: map)) }
        }
        .onChange(of: shading) { old, new in
            ease.retarget(from: old, to: new, now: Date().timeIntervalSinceReferenceDate, animate: animate && extrude)
            guard animate && extrude else { return }
            easing = true
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: 650_000_000)
                easing = false
            }
        }
        .onChange(of: week) { _, _ in
            guard animate, buildings > 0 else { return }
            rippleStart = Date().timeIntervalSinceReferenceDate
            rippling = true
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: 1_250_000_000)
                rippling = false
            }
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
                    ? MapCamera(lon: current.lon, lat: current.lat, zoom: 1, tilt: current.tilt)
                    : MapCamera.scenario(in: size, map: map)
                fly(from: current, to: target)
            }
            if tiltSetting > 0 {
                Divider().frame(width: 36).overlay(Theme.stroke)
                Button {
                    var target = current
                    target.tilt = current.tilt > 0.5 ? 0 : 1
                    fly(from: current, to: target)
                } label: {
                    Text("3D")
                        .font(.system(size: 14, weight: .heavy, design: .rounded))
                        .foregroundStyle(current.tilt > 0.5 ? Theme.gold : Theme.primaryText)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(current.tilt > 0.5 ? "Look straight down" : "Tilt the map")
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

    // MARK: - Frame

    /// The projection, geometry and region heights for one camera and moment.
    private func mapFrame(camera: MapCamera, size: CGSize, look: MapLook, now: Double) -> MapFrame {
        let map = self.map
        let geometry = self.geometry.of(map: map, camera: camera, size: size, tilt: look.tilt)
        let t = geometry.projection.t
        let scenarioZoom = MapCamera.scenario(in: size, map: map).zoom
        let zs = min(max(camera.zoom / max(scenarioZoom, 1e-6), 0.6), 2.5)
        var heights: [Double] = []
        if look.extrude && t > 0.01 && !map.regions.isEmpty {
            let intensity = ease.value(at: now, target: shading)
            heights = map.regions.map { region in
                let value = layer == .freight ? 0 : min(max(intensity[region.code] ?? 0, 0), 1)
                // Rounded to a tenth of a point so a still map reuses its cached tiles.
                return (((5 + 34 * value) * zs * t) * 10).rounded() / 10
            }
        }
        return MapFrame(map: map, geometry: geometry, zs: zs, heights: heights)
    }

    /// Whether anything on the moving layer would move for this camera.
    private func overlayMoves(_ look: MapLook, camera: MapCamera) -> Bool {
        if look.traffic > 0 && (!lanes.isEmpty || (look.detail >= 2 && camera.zoom > 9)) { return true }
        if look.clouds > 0 { return true }
        if look.buildings > 0 && layer == .presence && !divisions.isEmpty && camera.zoom > 3 { return true }
        if look.extrude && selected != nil { return true }
        if look.lighting == 2 && camera.flatness < 1 { return true }
        return rippling
    }

    /// The busiest lanes that carry moving freight, and how many particles run on each.
    private func flowLanes(_ look: MapLook) -> [FlowLane] {
        guard look.traffic > 0, !lanes.isEmpty else { return [] }
        let busiest = lanes
            .sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }
            .prefix(look.traffic >= 2 ? 120 : 60)
        let top = Double(max(busiest.map { Int($0.capacityLots + $0.outsideLots) }.max() ?? 1, 1))
        let per = busiest.map { 1 + Int((Double($0.capacityLots + $0.outsideLots) / top * (look.traffic >= 2 ? 9 : 4)).rounded()) }
        let total = per.reduce(0, +) * 2
        let scale = total > 600 ? 600 / Double(total) : 1
        return zip(busiest, per).map { lane, count -> FlowLane in
            let style: Int
            let speed: Double
            switch lane.modeId {
            case "truck", "road": style = 0; speed = 40
            case "rail": style = 1; speed = 55
            case "sea": style = 2; speed = 22
            default: style = 3; speed = 40
            }
            return FlowLane(lane: lane, perWay: max(1, Int(Double(count) * scale)), style: style, speed: speed)
        }
    }

    /// How dark the night side is: always on the globe with lighting, on the flat map only with
    /// day and night.
    private func nightWeight(_ look: MapLook, t: Double) -> Double {
        look.dayNight ? 1 : (look.lighting > 0 ? 1 - t : 0)
    }

    /// The colour of a region's top face: land tinted by the layer's value there.
    private func topColor(_ code: String) -> RGB {
        let value = shading[code] ?? 0
        guard value > 0 else { return .land }
        return RGB.land.mixed(with: layer.tintRGB, 0.12 + 0.55 * min(value, 1))
    }

    // MARK: - Still layer

    private func drawStatic(in context: inout GraphicsContext, frame f: MapFrame, camera: MapCamera, look: MapLook,
                            flow: [FlowLane], time: Double) {
        let projected = f.geometry
        let p = f.projection
        let t = p.t
        let globe = 1 - t
        let center = CGPoint(x: p.cx, y: p.cy)
        let size = projected.size
        let sun = MapSun.at(time: time, dayNight: look.dayNight, projection: p)
        let night = nightWeight(look, t: t)

        // Ocean: the globe's disc, growing to fill the view as the map flattens. Once flat it
        // fills the whole view so a tilted plane never shows its edge.
        if t >= 0.999 {
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(Theme.water.opacity(0.55)))
        } else {
            if look.lighting == 2 {
                let spin = camera.lon
                Starfield.draw(in: &context, size: size, range: 40..<220,
                               shift: (-spin * 0.6, -spin * 1.5, -spin * 3), time: nil, weight: globe)
            }
            if look.lighting > 0 {
                GlobeLighting.atmosphere(in: &context, center: center, radius: p.r, sun: sun, weight: globe, cinematic: look.lighting == 2)
            }
            let oceanRadius = p.r / max(1 - t, 0.03)
            let disc = CGRect(x: center.x - oceanRadius, y: center.y - oceanRadius, width: oceanRadius * 2, height: oceanRadius * 2)
            context.fill(Path(ellipseIn: disc), with: .color(Theme.water.opacity(0.55)))
            if look.lighting > 0 {
                GlobeLighting.ocean(in: &context, center: center, radius: p.r, sun: sun, weight: globe)
            }
            context.stroke(projected.graticule, with: .color(Theme.primaryText.opacity(0.06 * (1 - t))), lineWidth: 0.6)
        }

        // Outside the scenario the world is greyed out: neutral land, grey borders and roads.
        // The scenario's regions are drawn over it in colour.
        let land = projected.land
        context.fill(land, with: .color(WorldMapView.outsideLand), style: FillStyle(eoFill: true))
        let provinces = look.detail == 0 ? 0 : min(max((camera.zoom - 1.4) / 1.2, 0), 1)
        if provinces > 0 {
            context.stroke(projected.provinces, with: .color(WorldMapView.outsideBorder.opacity(0.7 * provinces)), lineWidth: 0.5)
        }
        let roads = look.detail < 2 ? 0 : min(max((camera.zoom - 2.4) / 1.6, 0), 1)
        if roads > 0 {
            context.stroke(projected.roads, with: .color(WorldMapView.outsideRoad.opacity(0.6 * roads)), lineWidth: 0.8)
        }
        context.stroke(land, with: .color(WorldMapView.outsideBorder), lineWidth: 0.8)

        let detail = look.detail < 2 ? 0 : min(max((camera.zoom - 7) / 5, 0), 1)
        let stateWidth: CGFloat = camera.zoom > 3 ? 1.8 : 1
        if f.raised.isEmpty {
            let regionPaths = projected.regionPaths
            for region in regionPaths {
                context.fill(region.path, with: .color(Theme.land))
                let value = shading[region.code] ?? 0
                if value > 0 {
                    context.fill(region.path, with: .color(layer.tint.opacity(0.12 + 0.55 * min(value, 1))))
                }
            }
            if detail > 0 {
                context.stroke(projected.counties, with: .color(Theme.stroke.opacity(0.45 * detail)), lineWidth: 0.5)
            }
            context.fill(projected.lakes, with: .color(Theme.water))
            if detail > 0 {
                context.stroke(projected.rivers, with: .color(Theme.water.opacity(0.9 * detail)), lineWidth: 1.4)
                context.stroke(projected.interstates, with: .color(Theme.road.opacity(0.4 * detail)), lineWidth: 1)
            }
            // Region borders, then state borders over them, then the selected region on top.
            for region in regionPaths {
                context.stroke(region.path, with: .color(Theme.secondary.opacity(0.45)), lineWidth: 0.9)
            }
            for state in projected.playStates {
                context.stroke(state, with: .color(Theme.primaryText.opacity(0.7)), lineWidth: stateWidth)
            }
            if let selected, let region = regionPaths.first(where: { $0.code == selected }) {
                context.stroke(region.path, with: .color(Theme.gold), lineWidth: 3)
            }
        } else {
            // Lakes outside the scenario stay on the ground; the tiles cover the rest.
            context.fill(projected.lakes, with: .color(Theme.water))
            for region in f.raised {
                drawRaised(region, in: &context, look: look, detail: detail, stateWidth: stateWidth)
            }
        }

        if layer == .freight {
            drawLanes(in: &context, frame: f)
        } else if !flow.isEmpty {
            drawFaintLanes(in: &context, frame: f, flow: flow)
        }

        if night > 0 {
            GlobeLighting.night(in: &context, projection: p, sun: sun, weight: night)
            if look.cityLights {
                GlobeLighting.cityLights(in: &context, projection: p, sun: sun, cities: map.cities, weight: night,
                                         height: { f.cityHeight($0) })
            }
        }
        if t < 1 {
            if look.lighting > 0 {
                GlobeLighting.surface(in: &context, center: center, radius: p.r, sun: sun, weight: globe, cinematic: look.lighting == 2)
            }
            // The globe's rim, fading as it unrolls.
            let rim = CGRect(x: center.x - p.r, y: center.y - p.r, width: p.r * 2, height: p.r * 2)
            context.stroke(Path(ellipseIn: rim), with: .color(Theme.water.opacity(0.9 * (1 - t))), lineWidth: 1.5)
            context.stroke(Path(ellipseIn: rim.insetBy(dx: -4, dy: -4)), with: .color(Theme.water.opacity(0.3 * (1 - t))), lineWidth: 6)
        }

        if camera.zoom > 3 {
            var labelAt: [Int: CGPoint] = [:]
            if look.buildings == 2 && camera.zoom > 6 {
                labelAt = drawSkylines(in: &context, frame: f, look: look, sun: sun, night: night)
            }
            if layer == .presence {
                if look.buildings > 0 {
                    drawBuildings(in: &context, frame: f, look: look, sun: sun, night: night)
                } else {
                    drawDivisions(in: &context, frame: f)
                }
            }
            drawCities(in: &context, frame: f, zoom: camera.zoom, labelAt: labelAt)
        }
        if camera.zoom > 14 {
            drawRegionNames(in: &context, frame: f)
        }

        if look.lighting > 0 && t > 0 {
            GlobeLighting.vignette(in: &context, size: size, strength: t)
        }
        if p.pitch > 0.001 {
            // Haze towards the horizon of a tilted map.
            let alpha = 0.22 * p.sinPitch / sin(48 * Double.pi / 180)
            context.fill(Path(CGRect(x: 0, y: 0, width: size.width, height: size.height * 0.45)), with: .linearGradient(
                Gradient(colors: [WorldMapView.haze.color(alpha), WorldMapView.haze.color(0)]),
                startPoint: .zero, endPoint: CGPoint(x: 0, y: size.height * 0.45)
            ))
        }
    }

    /// One raised tile: its shadow, the walls facing the viewer, its top face and the detail on
    /// it, then its border.
    private func drawRaised(_ region: RaisedRegion, in context: inout GraphicsContext, look: MapLook, detail: Double, stateWidth: CGFloat) {
        let h = region.height
        let color = topColor(region.code)
        if h > 0.5 {
            if look.lighting > 0 {
                context.fill(region.ground.offsetBy(dx: 0.5 * h, dy: 0.35 * h), with: .color(.black.opacity(0.30)))
            }
            for (level, wall) in region.walls.enumerated() where !wall.isEmpty {
                context.fill(wall, with: .color(color.scaled(RaisedRegion.levels[level]).color()))
            }
            context.stroke(region.ground, with: .color(.black.opacity(0.08)), lineWidth: 1)
        }
        context.fill(region.top, with: .color(color.color()))
        context.stroke(region.top, with: .color(.white.opacity(0.10)), lineWidth: 1)
        var top = context
        top.clip(to: region.top)
        if detail > 0 {
            top.stroke(region.counties, with: .color(Theme.stroke.opacity(0.45 * detail)), lineWidth: 0.5)
        }
        top.fill(region.lakes, with: .color(Theme.water))
        if detail > 0 {
            top.stroke(region.rivers, with: .color(Theme.water.opacity(0.9 * detail)), lineWidth: 1.4)
            top.stroke(region.interstates, with: .color(Theme.road.opacity(0.4 * detail)), lineWidth: 1)
        }
        // Clipped to the tile, so only the inner half of a state border shows: draw it twice as wide.
        top.stroke(region.states, with: .color(Theme.primaryText.opacity(0.7)), lineWidth: stateWidth * 2)
        context.stroke(region.top, with: .color(Theme.secondary.opacity(0.45)), lineWidth: 0.9)
        if region.code == selected {
            context.stroke(region.top, with: .color(Theme.gold), lineWidth: 3)
        }
    }

    // MARK: - Lanes

    private var busiestLanes: [LaneDto] {
        Array(lanes.sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }.prefix(90))
    }

    /// The lane drawn nearest a tap, within a finger's width.
    private func laneAt(_ tap: CGPoint, frame f: MapFrame) -> LaneDto? {
        var best: LaneDto?
        var bestDistance: CGFloat = 22
        for lane in busiestLanes {
            guard let endA = f.anchor(of: lane.a), let endB = f.anchor(of: lane.b), endA.1, endB.1 else { continue }
            let a = endA.0
            let b = endB.0
            let dx = b.x - a.x
            let dy = b.y - a.y
            let len2 = dx * dx + dy * dy
            let t = len2 <= 0 ? 0 : min(max(((tap.x - a.x) * dx + (tap.y - a.y) * dy) / len2, 0), 1)
            let d = hypot(tap.x - (a.x + t * dx), tap.y - (a.y + t * dy))
            if d < bestDistance {
                bestDistance = d
                best = lane
            }
        }
        return best
    }

    /// The busiest lanes between region anchors: thicker with more freight, coloured by how full
    /// they are, dashed for sea. Lanes the player owns or ships on get a gold edge; the lane
    /// picked for the panel gets a white one.
    private func drawLanes(in context: inout GraphicsContext, frame f: MapFrame) {
        let busiest = busiestLanes
        let top = CGFloat(max(busiest.map { Int($0.capacityLots + $0.outsideLots) }.max() ?? 1, 1))
        for lane in busiest {
            guard let endA = f.anchor(of: lane.a), let endB = f.anchor(of: lane.b), endA.1, endB.1 else { continue }
            let a = endA.0
            let b = endB.0
            var path = Path()
            path.move(to: a)
            path.addLine(to: b)
            let weight = CGFloat(lane.capacityLots + lane.outsideLots) / top
            let dim = (selected != nil && lane.a != selected && lane.b != selected && pickedLane == nil) ? 0.35 : 0.9
            let width = 1.5 + 5 * weight
            let dash: [CGFloat] = lane.modeId == "sea" ? [12, 9] : []
            if lane.id == pickedLane {
                context.stroke(path, with: .color(.white.opacity(0.9)), style: StrokeStyle(lineWidth: width + 7, lineCap: .round, dash: dash))
            } else if laneIsMine(lane) {
                context.stroke(path, with: .color(Theme.gold.opacity(0.75)), style: StrokeStyle(lineWidth: width + 4, lineCap: .round, dash: dash))
            }
            context.stroke(path, with: .color(laneColor(lane).opacity(dim)), style: StrokeStyle(lineWidth: width, lineCap: .round, dash: dash))
        }
    }

    /// Off the Freight layer, the lanes freight moves along show faintly.
    private func drawFaintLanes(in context: inout GraphicsContext, frame f: MapFrame, flow: [FlowLane]) {
        var paths = [Path(), Path(), Path(), Path()]
        for item in flow {
            guard let endA = f.anchor(of: item.lane.a), let endB = f.anchor(of: item.lane.b), endA.1, endB.1 else { continue }
            let a = endA.0
            let b = endB.0
            paths[item.style].move(to: a)
            paths[item.style].addLine(to: b)
        }
        for (style, path) in paths.enumerated() where !path.isEmpty {
            context.stroke(path, with: .color(WorldMapView.flowColors[style].color(0.10)), lineWidth: 1)
        }
    }

    // MARK: - Markers

    /// Region names once the player has zoomed in far enough to read them.
    private func drawRegionNames(in context: inout GraphicsContext, frame f: MapFrame) {
        for region in map.regions {
            guard let end = f.anchor(of: region.code), end.1 else { continue }
            let point = end.0
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
    private func drawDivisions(in context: inout GraphicsContext, frame f: MapFrame) {
        let radius: CGFloat = 11
        let byRegion = Dictionary(grouping: divisions, by: \.region)
        for (code, owned) in byRegion {
            guard let end = f.anchor(of: code), end.1 else { continue }
            let origin = end.0
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

    /// Each division as a building on its region's tile, in slots around the anchor, taller with
    /// more plants. Farthest first.
    private func placedBuildings(_ f: MapFrame) -> [PlacedBuilding] {
        let p = f.projection
        var placed: [PlacedBuilding] = []
        let byRegion = Dictionary(grouping: divisions, by: \.region)
        for (code, owned) in byRegion {
            guard let anchor = map.anchor(of: code) else { continue }
            let (origin, visible) = p.flat(anchor)
            guard visible else { continue }
            let ground = f.height(of: code)
            let sorted = owned.sorted { (Sector.order.firstIndex(of: $0.sector) ?? 99) < (Sector.order.firstIndex(of: $1.sector) ?? 99) }
            for (index, division) in sorted.enumerated() {
                let x = Double(origin.x) + Double(index % 3 - 1) * 24 * f.zs
                let y = Double(origin.y) + (16 + Double(index / 3) * 24) * f.zs
                let half = 7.5 * f.zs
                let corners = [
                    CGPoint(x: x - half, y: y - half), CGPoint(x: x + half, y: y - half),
                    CGPoint(x: x + half, y: y + half), CGPoint(x: x - half, y: y + half),
                ]
                let tall = (12 + 7 * Double(max(division.plants, 0)).squareRoot()) * f.zs
                let box = MapBox(base: corners.map { p.lift($0, ground) }, top: corners.map { p.lift($0, ground + tall) })
                placed.append(PlacedBuilding(id: division.id, sector: division.sector, plants: division.plants, box: box,
                                             base: p.lift(CGPoint(x: x, y: y), ground), geo: anchor))
            }
        }
        return placed.sorted { $0.base.y < $1.base.y }
    }

    private func drawBuildings(in context: inout GraphicsContext, frame f: MapFrame, look: MapLook, sun: MapSun, night: Double) {
        var lit = Path()
        var dim = Path()
        for building in placedBuildings(f) {
            let color = sectorRGB(building.sector)
            let glowing = look.lighting == 2 || (night > 0 && sun.nightLight(building.geo) * night > 0)
            if glowing {
                building.box.draw(in: &context, side: color.scaled(0.80), front: color.scaled(0.58), roof: color.scaled(1.15),
                                  viewCenterX: f.projection.cx, windows: &lit, outline: Theme.gold)
            } else {
                building.box.draw(in: &context, side: color.scaled(0.80), front: color.scaled(0.58), roof: color.scaled(1.15),
                                  viewCenterX: f.projection.cx, windows: &dim, outline: Theme.gold)
            }
            // The plant count floats over the roof, upright.
            let roof = building.box.top
            let top = CGPoint(x: (roof[0].x + roof[1].x + roof[2].x + roof[3].x) / 4,
                              y: min(roof[0].y, roof[1].y, roof[2].y, roof[3].y) - 6 - 7)
            let badge = CGRect(x: top.x - 7, y: top.y - 7, width: 14, height: 14)
            context.fill(Path(ellipseIn: badge.insetBy(dx: -1, dy: -1)), with: .color(Theme.ink))
            context.fill(Path(ellipseIn: badge), with: .color(Theme.gold))
            context.draw(
                Text("\(building.plants)").font(.system(size: 9, weight: .bold)).foregroundStyle(Theme.ink),
                at: CGPoint(x: badge.midX, y: badge.midY)
            )
        }
        context.fill(lit, with: .color(WorldMapView.windowColor.color(0.85)))
        context.fill(dim, with: .color(WorldMapView.windowColor.color(0.25)))
    }

    private func sectorRGB(_ sector: String) -> RGB {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        guard UIColor(Sector.color(sector)).getRed(&red, green: &green, blue: &blue, alpha: &alpha) else { return .land }
        return RGB(Double(red), Double(green), Double(blue))
    }

    /// City skylines: a cluster of towers for the largest cities on screen. Returns where each
    /// city's label should sit, over its tallest tower.
    private func drawSkylines(in context: inout GraphicsContext, frame f: MapFrame, look: MapLook, sun: MapSun, night: Double) -> [Int: CGPoint] {
        let p = f.projection
        let bounds = CGRect(origin: .zero, size: f.geometry.size).insetBy(dx: -30, dy: -30)
        var labels: [Int: CGPoint] = [:]
        var lit = Path()
        var dim = Path()
        var shown = 0
        for index in map.regionIndex.byPopulation {
            if shown >= 30 { break }
            let city = map.cities[index]
            let geo = GeoPoint(e4: city.x, city.y)
            let (origin, visible) = p.flat(geo)
            guard visible else { continue }
            let ground = f.cityHeight(index)
            guard bounds.contains(p.lift(origin, ground)) else { continue }
            shown += 1
            let scale = (6 + 4 * log10(max(Double(city.population), 1e5) / 1e5)) * f.zs
            let count = 3 + min(Int(MapNoise.unit(city.name, 1) * 5), 4)
            var boxes: [(y: Double, box: MapBox)] = []
            for b in 0..<count {
                let x = Double(origin.x) + (MapNoise.unit(city.name, 10 + b) - 0.5) * 28 * f.zs
                let y = Double(origin.y) + (MapNoise.unit(city.name, 20 + b) - 0.5) * 18 * f.zs
                let half = (2.5 + MapNoise.unit(city.name, 30 + b) * 1.5) * f.zs
                let tall = scale * (0.5 + MapNoise.unit(city.name, 40 + b))
                let corners = [
                    CGPoint(x: x - half, y: y - half), CGPoint(x: x + half, y: y - half),
                    CGPoint(x: x + half, y: y + half), CGPoint(x: x - half, y: y + half),
                ]
                boxes.append((y: y, box: MapBox(base: corners.map { p.lift($0, ground) }, top: corners.map { p.lift($0, ground + tall) })))
            }
            boxes.sort { $0.y < $1.y }
            let glowing = look.lighting == 2 || (night > 0 && sun.nightLight(geo) * night > 0)
            var highest = CGPoint(x: p.lift(origin, ground).x, y: .greatestFiniteMagnitude)
            for item in boxes {
                if glowing {
                    item.box.draw(in: &context, side: WorldMapView.skylineSide.scaled(0.85), front: WorldMapView.skylineSide,
                                  roof: WorldMapView.skylineTop, viewCenterX: p.cx, windows: &lit)
                } else {
                    item.box.draw(in: &context, side: WorldMapView.skylineSide.scaled(0.85), front: WorldMapView.skylineSide,
                                  roof: WorldMapView.skylineTop, viewCenterX: p.cx, windows: &dim)
                }
                let roofY = item.box.top.map(\.y).min() ?? highest.y
                if roofY < highest.y { highest = CGPoint(x: item.box.top[0].x, y: roofY) }
            }
            labels[index] = highest
        }
        context.fill(lit, with: .color(WorldMapView.windowColor.color(0.85)))
        context.fill(dim, with: .color(WorldMapView.windowColor.color(0.25)))
        return labels
    }

    private func drawCities(in context: inout GraphicsContext, frame f: MapFrame, zoom: Double, labelAt: [Int: CGPoint]) {
        let close = zoom > 14
        let shown = close ? 40 : 14
        for (index, city) in map.cities.prefix(shown).enumerated() where city.country == "USA" || close {
            let (flatPoint, visible) = f.projection.flat(GeoPoint(e4: city.x, city.y))
            guard visible else { continue }
            let point = f.projection.lift(flatPoint, f.cityHeight(index))
            let dot = CGRect(x: point.x - 2.5, y: point.y - 2.5, width: 5, height: 5)
            context.fill(Path(ellipseIn: dot), with: .color(Theme.primaryText.opacity(0.85)))
            // Labels sit above plant markers with an ink halo so a busy city stays readable. A
            // city with a skyline is named over its tallest tower.
            let at = labelAt[index].map { CGPoint(x: $0.x + 6, y: $0.y - 6) } ?? CGPoint(x: point.x + 6, y: point.y)
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

    // MARK: - Moving layer

    private func drawOverlay(in context: inout GraphicsContext, frame f: MapFrame, camera: MapCamera, look: MapLook,
                             flow: [FlowLane], time: Double) {
        let p = f.projection
        let t = p.t
        let size = f.geometry.size
        if look.lighting == 2 && t < 1 {
            // Twinkling stars, kept off the globe's face.
            var stars = context
            var outside = Path(CGRect(origin: .zero, size: size))
            outside.addEllipse(in: CGRect(x: p.cx - p.r, y: p.cy - p.r, width: p.r * 2, height: p.r * 2))
            stars.clip(to: outside, style: FillStyle(eoFill: true))
            let spin = camera.lon
            Starfield.draw(in: &stars, size: size, range: 0..<40, shift: (-spin * 0.6, -spin * 1.5, -spin * 3), time: time, weight: 1 - t)
        }
        if !flow.isEmpty {
            drawFreight(in: &context, frame: f, flow: flow, time: time)
        }
        if look.traffic > 0 && look.detail >= 2 && camera.zoom > 9 {
            drawCars(in: &context, frame: f, look: look, time: time)
        }
        if layer == .presence && look.buildings > 0 && camera.zoom > 3 && !divisions.isEmpty {
            drawBuildingEffects(in: &context, frame: f, look: look, time: time)
        }
        if look.extrude, let selected, let region = f.raised.first(where: { $0.code == selected }) {
            let pulse = 0.5 + 0.5 * sin(2 * Double.pi * time / 1.6)
            context.stroke(region.top, with: .color(Theme.gold.opacity(0.18)), lineWidth: 8)
            context.stroke(region.top, with: .color(Theme.gold), lineWidth: 2 + 1.5 * pulse)
        }
        if look.clouds > 0 {
            drawClouds(in: &context, frame: f, camera: camera, look: look, time: time)
        }
    }

    /// Freight particles running both ways along the busiest lanes.
    private func drawFreight(in context: inout GraphicsContext, frame f: MapFrame, flow: [FlowLane], time: Double) {
        let boost = layer == .freight ? 2.0 : 1.0
        var halos = [Path(), Path(), Path(), Path()]
        var cores = [Path(), Path(), Path(), Path()]
        let halo = 4 * boost
        let core = 1.6 * boost
        for (laneIndex, item) in flow.enumerated() {
            guard let endA = f.anchor(of: item.lane.a), let endB = f.anchor(of: item.lane.b), endA.1, endB.1 else { continue }
            let a = endA.0
            let b = endB.0
            let length = max(Double(hypot(b.x - a.x, b.y - a.y)), 1)
            for way in 0..<2 {
                for n in 0..<item.perWay {
                    var u = fract(time * item.speed / length + MapNoise.unit(laneIndex, n, way))
                    if way == 1 { u = 1 - u }
                    let x = Double(a.x) + Double(b.x - a.x) * u
                    let y = Double(a.y) + Double(b.y - a.y) * u
                    halos[item.style].addEllipse(in: CGRect(x: x - halo, y: y - halo, width: halo * 2, height: halo * 2))
                    cores[item.style].addEllipse(in: CGRect(x: x - core, y: y - core, width: core * 2, height: core * 2))
                }
            }
        }
        for style in 0..<4 where !cores[style].isEmpty {
            let color = WorldMapView.flowColors[style]
            context.fill(halos[style], with: .color(color.color(min(0.25 * boost, 1))))
            context.fill(cores[style], with: .color(color.color(min(0.95 * boost, 1))))
        }
    }

    /// Cars along the highways, headlights ahead and tail lights behind.
    private func drawCars(in context: inout GraphicsContext, frame f: MapFrame, look: MapLook, time: Double) {
        let lines = f.geometry.highways(f.heights)
        let spacing = look.traffic >= 2 ? 35.0 : 70.0
        let wanted = lines.reduce(0.0) { $0 + floor($1.total / spacing) }
        guard wanted > 0 else { return }
        let scale = min(1, 400 / wanted)
        var heads = Path()
        var tails = Path()
        for (lineIndex, line) in lines.enumerated() {
            let count = Int(floor(line.total / spacing) * scale)
            guard count > 0 else { continue }
            for n in 0..<count {
                var along = fract(time * 40 / line.total + MapNoise.unit(lineIndex, n, 7)) * line.total
                let backward = n % 2 == 1
                if backward { along = line.total - along }
                let (point, direction) = line.point(at: along)
                let sign = backward ? -1.0 : 1.0
                let dx = direction.dx * 1.6 * sign
                let dy = direction.dy * 1.6 * sign
                heads.addRect(CGRect(x: point.x + dx - 0.6, y: point.y + dy - 0.6, width: 1.2, height: 1.2))
                tails.addRect(CGRect(x: point.x - dx - 0.6, y: point.y - dy - 0.6, width: 1.2, height: 1.2))
            }
        }
        context.fill(heads, with: .color(.white.opacity(0.95)))
        context.fill(tails, with: .color(Color(red: 1, green: 0.25, blue: 0.2).opacity(0.9)))
    }

    /// Beacons from each building, a ripple when the week turns, and smoke from heavy industry.
    private func drawBuildingEffects(in context: inout GraphicsContext, frame f: MapFrame, look: MapLook, time: Double) {
        let squash = f.projection.pitch > 0 ? f.projection.cosPitch : 1
        let rippleAge = time - rippleStart
        for building in placedBuildings(f) {
            let color = Sector.color(building.sector)
            let phase = fract(time / 2.4 + MapNoise.unit(building.id))
            let radius = 6 + 20 * phase
            context.stroke(Path(ellipseIn: CGRect(x: building.base.x - radius, y: building.base.y - radius * squash,
                                                  width: radius * 2, height: radius * 2 * squash)),
                           with: .color(color.opacity(0.45 * (1 - phase))), lineWidth: 1.5)
            if rippleAge >= 0 && rippleAge < 1.2 {
                let e = rippleAge / 1.2
                let ripple = 60 * f.zs * e
                context.stroke(Path(ellipseIn: CGRect(x: building.base.x - ripple, y: building.base.y - ripple * squash,
                                                      width: ripple * 2, height: ripple * 2 * squash)),
                               with: .color(Theme.gold.opacity(0.6 * (1 - e))), lineWidth: 2)
            }
            if look.smoke && WorldMapView.smoky.contains(building.sector) {
                let roof = building.box.top
                let stack = CGPoint(x: (roof[0].x + roof[1].x) / 2, y: min(roof[0].y, roof[1].y))
                for puff in 0..<3 {
                    let age = fract(time / 3 + Double(puff) / 3 + MapNoise.unit(building.id, puff))
                    let size = 2 + 5 * age
                    let at = CGPoint(x: stack.x + 4 * sin(age * 3 + Double(puff)), y: stack.y - 30 * age)
                    context.fill(Path(ellipseIn: CGRect(x: at.x - size, y: at.y - size, width: size * 2, height: size * 2)),
                                 with: .color(Color(white: 0.75).opacity(0.45 * (1 - age))))
                }
            }
        }
    }

    /// Clouds drifting east over the scenario, with shadows on the ground below.
    private func drawClouds(in context: inout GraphicsContext, frame f: MapFrame, camera: MapCamera, look: MapLook, time: Double) {
        let p = f.projection
        let b = map.playBounds
        let field = CloudField.field(count: look.clouds >= 2 ? 80 : 36,
                                     west: b.west - 6, south: b.south - 6, east: b.east + 6, north: b.north + 6)
        let fade = min(max(1.4 - camera.zoom / 30, 0.25), 1)
        let altitude = 46 * f.zs * p.t
        let night = nightWeight(look, t: p.t)
        let sun = night > 0 ? MapSun.at(time: time, dayNight: look.dayNight, projection: p) : nil
        let shadowColor = Color.black.opacity(0.16 * fade)
        for puff in field.puffs {
            let geo = GeoPoint(lonDegrees: field.lon(of: puff, time: time), latDegrees: puff.lat)
            let (flatPoint, visible) = p.flat(geo)
            guard visible else { continue }
            if p.t < 0.5 {
                let u = geo.unit
                guard u.0 * p.view.0 + u.1 * p.view.1 + u.2 * p.view.2 > 0 else { continue }
            }
            let s = p.scale(atFlatY: flatPoint.y)
            let radius = puff.radius * Double.pi / 180 * p.r * s
            guard radius > 0.5 else { continue }
            var alpha = fade
            if let sun { alpha *= 1 - 0.6 * sun.nightLight(geo) * night }
            let top = p.lift(flatPoint, altitude)
            let ground = p.lift(flatPoint, 0)
            let shadowAt = CGPoint(x: ground.x + 12 * f.zs * p.t, y: ground.y + 8 * f.zs * p.t)
            for part in puff.parts {
                CloudField.blob(in: &context, at: CGPoint(x: shadowAt.x + part.dx * radius, y: shadowAt.y + part.dy * radius),
                                radius: radius * part.size, color: shadowColor)
            }
            for part in puff.parts {
                CloudField.blob(in: &context, at: CGPoint(x: top.x + part.dx * radius, y: top.y + part.dy * radius),
                                radius: radius * part.size, color: .white.opacity(0.42 * alpha))
            }
        }
    }

    /// The optional frame counter, top left.
    private func drawFrameRate(in context: inout GraphicsContext, time: Double, running: Bool) {
        let label: String
        if running {
            label = "\(Int(meter.tick(time).rounded())) fps"
        } else {
            meter.reset()
            label = "Still"
        }
        let text = context.resolve(Text(label).font(.system(size: 11, weight: .semibold).monospacedDigit()).foregroundStyle(Theme.primaryText))
        let measured = text.measure(in: CGSize(width: 200, height: 40))
        let box = CGRect(x: 8, y: 8, width: measured.width + 12, height: measured.height + 6)
        context.fill(Path(roundedRect: box, cornerRadius: 6), with: .color(Theme.ink.opacity(0.75)))
        context.draw(text, at: CGPoint(x: box.minX + 6, y: box.midY), anchor: .leading)
    }
}
