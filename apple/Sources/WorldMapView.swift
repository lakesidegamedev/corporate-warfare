import SwiftUI
import UIKit

/// One raised region: its ground footprint, top, lit walls (dark, mid, lit) and height in points,
/// and how far its top stands above its base on screen.
struct RegionSolid {
    let code: String
    let ground: Path
    let top: Path
    let walls: [Path]
    let height: Double
    let lift: Double
    let bounds: CGRect

    init(code: String, ground: Path, top: Path, walls: [Path], height: Double, lift: Double) {
        self.code = code
        self.ground = ground
        self.top = top
        self.walls = walls
        self.height = height
        self.lift = lift
        bounds = top.boundingRect
    }
}

/// A building standing on the map: untilted screen point, floor and top in points.
struct Building {
    let sector: String
    let plants: Int
    let gx: Double
    let gy: Double
    let half: Double
    let floor: Double
    let top: Double
    let geo: GeoPoint
    let key: String
}

/// The top of a city's tallest tower: untilted point and height.
struct TopPoint {
    let x: Double
    let y: Double
    let h: Double
}

/// A lane's ends as untilted points, with its mode and weight.
struct LaneTrack {
    let a: CGPoint
    let b: CGPoint
    let mode: String
    let weight: Double
    let length: Double
    let seed: Double
}

/// Where a building's beacon, smoke and ripple sit on screen.
struct BuildingSpot {
    let base: CGPoint
    let top: CGPoint
    let color: Color
    let smoke: Bool
    let seed: Double
}

/// What the moving layer needs for the current camera, built once per camera and data.
struct OverlayGeometry {
    let lanes: [LaneTrack]
    let roads: [MapPolyline]
    let buildings: [BuildingSpot]
    let selectedTop: Path?
    let scale: Double
    let plinth: Double
    /// Untilted ground points of the biggest cities, where planes fly between.
    let airports: [CGPoint]
}

/// The map projected for one camera, view size and tilt. Each path is built the first time it is
/// drawn and kept, so a redraw that moves nothing (a new week, a selection) projects nothing.
/// Raised tiles, buildings and the moving layer's tracks are kept for the last heights and data.
final class MapGeometry {
    let map: WorldMap
    let camera: MapCamera
    let size: CGSize
    let tilt: Double
    let detail: Int
    let projection: MapProjection
    /// How big 3D things are drawn: 1 at the scenario's framing, larger closer in.
    let scale: Double

    init(map: WorldMap, camera: MapCamera, size: CGSize, tilt: Double, detail: Int) {
        self.map = map
        self.camera = camera
        self.size = size
        self.tilt = tilt
        self.detail = detail
        projection = MapProjection(camera: camera, size: size, tilt: tilt)
        let framed = MapCamera.scenario(in: size, map: map).zoom
        scale = min(max(camera.zoom / max(framed, 1e-6), 0.6), 2.5)
    }

    /// The height of the scenario's base in points.
    var plinth: Double { 8 * scale * projection.t }
    var center: CGPoint { CGPoint(x: projection.cx, y: projection.cy) }

    lazy var graticule: Path = projection.lines(MapGeometry.graticuleRings)
    lazy var land: Path = {
        var path = Path()
        for country in map.countries {
            for ring in country.rings { projection.addPolygon(ring, to: &path) }
        }
        return path
    }()
    lazy var biomes: [Path] = Biomes.paths(map, projection)
    lazy var provinces: Path = projection.lines(map.provinces)
    lazy var roads: Path = projection.lines(map.roads)
    lazy var regionPaths: [(code: String, path: Path)] = map.regions.map { (code: $0.code, path: projection.path($0.rings)) }
    lazy var counties: Path = projection.lines(map.counties)
    lazy var lakes: Path = projection.path(map.lakes)
    lazy var rivers: Path = projection.lines(map.rivers)
    lazy var interstates: Path = projection.lines(map.interstates)
    lazy var playStates: [Path] = map.playStates.map { projection.path($0.rings) }
    /// The small soft-light layers for this camera.
    let backLight = LightLayer()
    let frontLight = LightLayer()

    /// Meridians and parallels every 15 degrees, for the globe.
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

    /// Points high for a region at a layer intensity, flattening towards the base up close.
    func height(_ intensity: Double) -> Double {
        let close = detail < 2 ? 0 : min(max((camera.zoom - 7) / 5, 0), 1)
        return (8 + 34 * min(max(intensity, 0), 1) * (1 - 0.75 * close)) * scale * projection.t
    }

    /// How high a region's tile stands; nil heights put everything on the ground.
    func regionLift(_ code: String, _ heights: [String: Double]?) -> Double {
        guard let heights else { return 0 }
        return height(heights[code] ?? 0)
    }

    /// The height of the tile a city stands on.
    func cityFloor(_ index: Int, _ heights: [String: Double]?) -> Double {
        guard heights != nil, index < map.regionIndex.cityRegion.count else { return 0 }
        let region = map.regionIndex.cityRegion[index]
        return region >= 0 ? regionLift(map.regions[region].code, heights) : 0
    }

    private var nightKey: MapSun?
    private var nightPaths: [Path] = []

    func night(_ sun: MapSun) -> [Path] {
        if sun != nightKey {
            nightPaths = GlobeLighting.nightRings(sun, projection).map { projection.path([$0]) }
            nightKey = sun
        }
        return nightPaths
    }

    private var raisedKey: [String: Double]?
    private var raisedHeld: [RegionSolid] = []

    /// The scenario's regions as raised tiles, far ones first.
    func raised(_ heights: [String: Double]) -> [RegionSolid] {
        if raisedKey == heights { return raisedHeld }
        let p = projection
        let base = p.lift(center, 0).y
        var built: [(y: CGFloat, solid: RegionSolid)] = []
        for (index, region) in map.regions.enumerated() {
            let h = height(heights[region.code] ?? 0)
            let anchorY = map.anchor(of: region.code).map { p.project($0).0.y } ?? 0
            let solid = RegionSolid(code: region.code, ground: regionPaths[index].path, top: p.path(region.rings, height: h),
                                    walls: p.walls(region.rings, height: h), height: h, lift: Double(base - p.lift(center, h).y))
            built.append((y: anchorY, solid: solid))
        }
        raisedHeld = built.sorted { $0.y < $1.y }.map(\.solid)
        raisedKey = heights
        return raisedHeld
    }

    private var shadowKey: [String: Double]?
    private var shadowHeld = Path()

    /// Every tile's shadow in one path, cast down and right of its footprint.
    func shadows(_ heights: [String: Double]) -> Path {
        if shadowKey == heights { return shadowHeld }
        var path = Path()
        for solid in raised(heights) {
            path.addPath(solid.ground, transform: CGAffineTransform(translationX: 0.25 * solid.lift, y: 0.18 * solid.lift))
        }
        shadowHeld = path
        shadowKey = heights
        return path
    }

    private var buildingsKey: (divisions: [DivisionMarker], heights: [String: Double]?)?
    private var buildingsHeld: [Building] = []

    /// The player's divisions as buildings round their region's hub, far ones first.
    func buildings(_ divisions: [DivisionMarker], _ heights: [String: Double]?) -> [Building] {
        if let key = buildingsKey, key.divisions == divisions, key.heights == heights { return buildingsHeld }
        let p = projection
        let sorted = divisions.sorted {
            $0.region != $1.region ? $0.region < $1.region
                : (Sector.order.firstIndex(of: $0.sector) ?? 99) < (Sector.order.firstIndex(of: $1.sector) ?? 99)
        }
        var perRegion: [String: Int] = [:]
        var placed: [Building] = []
        for division in sorted {
            guard let anchor = map.anchor(of: division.region),
                  let ground = p.ground(lon: anchor.lon, lat: anchor.lat) else { continue }
            let i = perRegion[division.region, default: 0]
            perRegion[division.region] = i + 1
            let gx = Double(ground.x) + Double(i % 3 - 1) * 24 * scale
            let gy = Double(ground.y) + (16 + Double(i / 3) * 24) * scale
            let floor = regionLift(division.region, heights)
            let tall = (12 + 7 * Double(max(division.plants, 0)).squareRoot()) * scale
            placed.append(Building(sector: division.sector, plants: division.plants, gx: gx, gy: gy, half: 7.5 * scale,
                                   floor: floor, top: floor + tall, geo: anchor, key: division.id))
        }
        buildingsHeld = placed.sorted { $0.gy < $1.gy }
        buildingsKey = (divisions: divisions, heights: heights)
        return buildingsHeld
    }

    private var skylineBuilt = false
    private var skylineKey: [String: Double]?
    private var skylineHeld: [Building] = []
    private var skylineTops: [Int: TopPoint] = [:]

    /// Skylines for the biggest cities on screen, each a small cluster of towers.
    func skylines(_ heights: [String: Double]?) -> [Building] {
        if skylineBuilt && skylineKey == heights { return skylineHeld }
        let p = projection
        skylineTops.removeAll()
        var boxes: [Building] = []
        let bounds = CGRect(origin: .zero, size: size).insetBy(dx: -40, dy: -40)
        let order = map.cities.indices.sorted { map.cities[$0].population > map.cities[$1].population }
        var shown = 0
        for index in order {
            if shown >= 30 { break }
            let city = map.cities[index]
            let geo = GeoPoint(e4: city.x, city.y)
            guard let ground = p.ground(lon: geo.lon, lat: geo.lat), bounds.contains(p.lift(ground, 0)) else { continue }
            shown += 1
            let hash = stableHash(city.name)
            let count = 3 + hash % 5
            let floor = cityFloor(index, heights)
            let tall = (6 + 4 * log10(max(Double(city.population) / 1e5, 1))) * scale
            var best = 0.0
            var bestAt = TopPoint(x: Double(ground.x), y: Double(ground.y), h: floor)
            for k in 0..<count {
                let h2 = stableHash(city.name + "\(k)")
                let dx = Double(h2 % 17 - 8) * 1.3 * scale
                let dy = Double((h2 / 17) % 13 - 6) * 1.1 * scale
                let h = tall * (0.35 + 0.65 * Double((h2 / 221) % 100) / 100) * (k == 0 ? 1.25 : 1)
                let gx = Double(ground.x) + dx
                let gy = Double(ground.y) + dy
                boxes.append(Building(sector: "", plants: 0, gx: gx, gy: gy, half: (1.6 + Double(h2 % 3)) * scale,
                                      floor: floor, top: floor + h, geo: geo, key: city.name))
                if h > best {
                    best = h
                    bestAt = TopPoint(x: gx, y: gy, h: floor + h)
                }
            }
            skylineTops[index] = bestAt
        }
        skylineHeld = boxes.sorted { $0.gy < $1.gy }
        skylineKey = heights
        skylineBuilt = true
        return skylineHeld
    }

    /// The top of a city's tallest tower, once its skyline has been built.
    func skylineTop(_ index: Int) -> TopPoint? { skylineTops[index] }

    private struct OverlayKey: Equatable {
        let lanes: Int
        let divisions: [DivisionMarker]
        let selected: String?
        let heights: [String: Double]
        let look: MapLook
        let freight: Bool
    }

    private var overlayKey: OverlayKey?
    private var overlayHeld: OverlayGeometry?

    /// What the moving layer needs, rebuilt only when the camera or the data under it changes.
    func overlay(lanes: [LaneDto], divisions: [DivisionMarker], selected: String?, heights: [String: Double],
                 look: MapLook, freightLayer: Bool) -> OverlayGeometry {
        var hasher = Hasher()
        for lane in lanes {
            hasher.combine(lane.id)
            hasher.combine(lane.capacityLots)
            hasher.combine(lane.outsideLots)
        }
        let key = OverlayKey(lanes: hasher.finalize(), divisions: divisions, selected: selected, heights: heights,
                             look: look, freight: freightLayer)
        if let overlayHeld, overlayKey == key { return overlayHeld }
        let p = projection
        let raised = look.extrude && p.t > 0.05
        let ground: [String: Double]? = raised ? heights : nil
        let laneCount = look.traffic >= 2 || freightLayer ? 120 : look.traffic == 1 ? 60 : 0
        let tracks = p.t > 0.5 && laneCount > 0 ? laneTracks(lanes, count: laneCount) : []
        let roadLines = look.traffic > 0 && detail >= 2 && camera.zoom > 9
            ? MapPolyline.build(p, map.interstates, lift: raised ? plinth : 0, size: size) : []
        let spots: [BuildingSpot] = look.buildings > 0 && camera.zoom > 3 && p.t > 0.5
            ? buildings(divisions, ground).map { b in
                BuildingSpot(base: p.lift(CGPoint(x: b.gx, y: b.gy), b.floor), top: p.lift(CGPoint(x: b.gx, y: b.gy), b.top),
                             color: Sector.color(b.sector), smoke: smokes(b.sector), seed: Double(stableHash(b.key) % 1000) / 1000)
            } : []
        var top: Path?
        if let selected, let region = map.regions.first(where: { $0.code == selected }) {
            top = p.path(region.rings, height: regionLift(selected, ground))
        }
        let airports: [CGPoint] = look.traffic > 0 && p.t > 0.5
            ? map.cities.sorted { $0.population > $1.population }.prefix(14).compactMap { city in
                let geo = GeoPoint(e4: city.x, city.y)
                return p.ground(lon: geo.lon, lat: geo.lat)
            } : []
        let held = OverlayGeometry(lanes: tracks, roads: roadLines, buildings: spots, selectedTop: top, scale: scale,
                                   plinth: raised ? plinth : 0, airports: airports)
        overlayHeld = held
        overlayKey = key
        return held
    }

    /// The busiest lanes as untilted tracks, seeded so particles never line up between lanes.
    private func laneTracks(_ lanes: [LaneDto], count: Int) -> [LaneTrack] {
        let busiest = lanes.sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }.prefix(count)
        guard let first = busiest.first else { return [] }
        let top = Double(max(first.capacityLots + first.outsideLots, 1))
        let p = projection
        return busiest.compactMap { lane -> LaneTrack? in
            guard let a = map.anchor(of: lane.a).flatMap({ p.ground(lon: $0.lon, lat: $0.lat) }),
                  let b = map.anchor(of: lane.b).flatMap({ p.ground(lon: $0.lon, lat: $0.lat) }) else { return nil }
            let length = max(Double(hypot(b.x - a.x, b.y - a.y)), 8)
            return LaneTrack(a: a, b: b, mode: lane.modeId, weight: Double(lane.capacityLots + lane.outsideLots) / top,
                             length: length, seed: Double(stableHash(lane.a + lane.b) % 1000) / 1000)
        }
    }
}

/// Holds the last `MapGeometry`. A reference type so the canvas can refill it while drawing.
final class MapGeometryCache {
    private var held: MapGeometry?

    func of(map: WorldMap, camera: MapCamera, size: CGSize, tilt: Double = 0, detail: Int = 2) -> MapGeometry {
        if let held, held.map === map, held.camera == camera, held.size == size, held.tilt == tilt, held.detail == detail {
            return held
        }
        let fresh = MapGeometry(map: map, camera: camera, size: size, tilt: tilt, detail: detail)
        held = fresh
        return fresh
    }
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

    var tint: Color { tintRGB.color() }

    /// `tint` as components, for shading raised tiles.
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
struct DivisionMarker: Identifiable, Equatable {
    let region: String
    let sector: String
    let plants: Int
    var id: String { region + "/" + sector }
}

/// The graphics options the map reads, gathered once per update.
struct MapLook: Equatable {
    var detail: Int
    /// The tilt setting in radians.
    var tilt: Double
    var extrude: Bool
    var lighting: Int
    var dayNight: Bool
    var cityLights: Bool
    var traffic: Int
    var buildings: Int
    var frameRate: Int
    var animations: Bool
    var showFps: Bool
    var ultra: Bool

    /// Whether anything on the map moves on its own, so the frame clock needs to run.
    var moves: Bool { animations && (traffic > 0 || buildings > 0 || dayNight || lighting == 2) }
    /// Heavy industry smokes with busy traffic or on Ultra.
    var smoke: Bool { traffic >= 2 || ultra }
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

/// Everything one draw of the map needs: what is shown and how. The still parts are drawn by
/// `drawStatic` and cached as a layer; the moving parts by `drawOverlay` each frame, over it.
/// The main menu's globe draws the same still layer.
struct MapScene {
    let map: WorldMap
    let camera: MapCamera
    let look: MapLook
    let tint: RGB
    let shading: [String: Double]
    /// Intensities the tiles stand at, easing towards `shading`.
    let heights: [String: Double]
    let selected: String?
    let divisions: [DivisionMarker]
    var lanes: [LaneDto] = []
    var freightLayer = false
    var pickedLane: String?
    /// The moving sun, nil with day and night off.
    let sun: MapSun?
    var space = true
    /// Pixels per point, for the half-resolution light layers.
    var displayScale = 2.0

    static let spaceColor = RGB(hex: 0x05090E)
    private static let outsideLand = RGB(hex: 0x25282B)
    private static let outsideBorder = RGB(hex: 0x55595D)
    private static let outsideRoad = RGB(hex: 0x6A6E72)
    private static let regionLand = RGB(hex: 0x33475A)
    private static let skyline = RGB(hex: 0x4A5A6C)
    private static let modeColors: [String: RGB] = [
        "road": RGB(hex: 0xE0A040), "truck": RGB(hex: 0xE0A040), "rail": RGB(hex: 0x7FD3FF), "sea": RGB(hex: 0x9AF0E0),
    ]
    private static let modeSpeed: [String: Double] = ["road": 40, "truck": 40, "rail": 55, "sea": 22]

    private func clamp(_ x: Double) -> Double { min(max(x, 0), 1) }

    // MARK: Still layer

    /// Space, the lit globe, land, the raised scenario, night, buildings, cities and labels.
    func drawStatic(in context: inout GraphicsContext, geometry g: MapGeometry) {
        let p = g.projection
        let t = p.t
        let size = g.size
        let zoom = camera.zoom
        let light = sun ?? MapSun.fixed(p)
        let lit = look.lighting > 0
        let whole = Path(CGRect(origin: .zero, size: size))
        if space {
            context.fill(whole, with: .color(MapScene.spaceColor.color()))
            if look.lighting == 2 && t < 1 {
                StarField.shared.draw(in: &context, size: size, seconds: 0, drift: 0, twinkle: false)
            }
        }
        let lightKey: [AnyHashable] = [light.lon, light.lat, look.lighting]
        if lit && t < 1 {
            g.backLight.draw(in: &context, size: size, displayScale: displayScale, key: lightKey) { layer, _ in
                GlobeLighting.atmosphere(in: &layer, projection: p, sun: light, lighting: look.lighting)
            }
        }
        GlobeLighting.back(in: &context, projection: p, sun: light, lighting: look.lighting, size: size)
        if t < 1 { context.stroke(g.graticule, with: .color(Theme.primaryText.opacity(0.06 * (1 - t))), lineWidth: 0.6) }
        // Outside the scenario the world is muted: climate-tinted land on the globe that greys as
        // the map flattens, grey borders and roads. The scenario's regions are drawn over it.
        if lit {
            // Shallow water: a lighter band of sea hugging every coast, under the land.
            let shallows = min(max(2 + zoom * 0.35, 2), 9)
            for path in g.biomes { context.stroke(path, with: .color(RGB(hex: 0x2F6F9A).color(0.30)), lineWidth: shallows) }
            Biomes.draw(g.biomes, in: &context, grey: 0.25 + 0.6 * t)
        } else {
            context.fill(g.land, with: .color(MapScene.outsideLand.color()), style: FillStyle(eoFill: true))
        }
        let provinces = look.detail == 0 ? 0 : clamp((zoom - 1.4) / 1.2)
        if provinces > 0 { context.stroke(g.provinces, with: .color(MapScene.outsideBorder.color(0.7 * provinces)), lineWidth: 0.5) }
        let roads = look.detail < 2 ? 0 : clamp((zoom - 2.4) / 1.6)
        if roads > 0 { context.stroke(g.roads, with: .color(MapScene.outsideRoad.color(0.6 * roads)), lineWidth: 0.8) }
        if lit {
            for path in g.biomes { context.stroke(path, with: .color(MapScene.outsideBorder.color()), lineWidth: 0.8) }
        } else {
            context.stroke(g.land, with: .color(MapScene.outsideBorder.color()), lineWidth: 0.8)
        }
        // Lit and flat, the world outside the scenario falls into shade so the scenario stands out.
        if lit && t > 0.5 { context.fill(whole, with: .color(.black.opacity(0.18 * t))) }

        let detail = look.detail < 2 ? 0 : clamp((zoom - 7) / 5)
        let raised = look.extrude && t > 0.05
        // How far up the base of the raised scenario sits on screen, for lines drawn on it.
        let plinth = raised ? g.plinth : 0
        let up = Double(p.lift(g.center, plinth).y - p.lift(g.center, 0).y)
        let water = Theme.water
        let ground: [String: Double]? = raised ? heights : nil
        let even = FillStyle(eoFill: true)
        if raised {
            context.fill(g.lakes, with: .color(water), style: even)
            let solids = g.raised(heights)
            // Shadows first, so no tile's shadow falls across a tile drawn before it.
            if lit { context.fill(g.shadows(heights), with: .color(.black.opacity(0.22)), style: even) }
            for solid in solids {
                let intensity = shading[solid.code] ?? 0
                let top = intensity > 0 ? MapScene.regionLand.mixed(with: tint, min(0.15 + 0.6 * intensity, 1)) : MapScene.regionLand
                context.fill(solid.walls[0], with: .color(top.scaled(0.50).color()))
                context.fill(solid.walls[1], with: .color(top.scaled(0.68).color()))
                context.fill(solid.walls[2], with: .color(top.scaled(0.88).color()))
                context.fill(solid.top, with: .color(top.color()), style: even)
                // Tops catch the light from the upper left and fall off away from it.
                if lit {
                    let b = solid.bounds
                    context.fill(solid.top, with: .linearGradient(Gradient(stops: [
                        .init(color: .white.opacity(0.09), location: 0),
                        .init(color: .clear, location: 0.5),
                        .init(color: .black.opacity(0.12), location: 1),
                    ]), startPoint: CGPoint(x: b.minX, y: b.minY), endPoint: CGPoint(x: b.maxX, y: b.maxY)), style: even)
                }
                if solid.code == selected { context.fill(solid.top, with: .color(Theme.gold.opacity(0.35)), style: even) }
                context.stroke(solid.top, with: .color(lit ? RGB(hex: 0x9FB4C8).color(0.55) : Theme.stroke), lineWidth: 1)
            }
            if detail > 0 {
                var onTop = context
                onTop.translateBy(x: 0, y: up)
                onTop.stroke(g.counties, with: .color(Theme.stroke.opacity(0.45 * detail)), lineWidth: 0.5)
            }
        } else {
            for region in g.regionPaths {
                context.fill(region.path, with: .color(MapScene.regionLand.color()), style: even)
                let intensity = shading[region.code] ?? 0
                if intensity > 0 { context.fill(region.path, with: .color(tint.color(0.15 + 0.6 * intensity)), style: even) }
            }
            if detail > 0 { context.stroke(g.counties, with: .color(Theme.stroke.opacity(0.45 * detail)), lineWidth: 0.5) }
            context.fill(g.lakes, with: .color(water), style: even)
        }
        var lifted = context
        lifted.translateBy(x: 0, y: up)
        if detail > 0 {
            lifted.stroke(g.rivers, with: .color(water.opacity(0.9 * detail)), lineWidth: 1.4)
            lifted.stroke(g.interstates, with: .color(RGB(hex: 0xC68A3D).color(0.35 * detail)), lineWidth: 1)
        }
        if !raised {
            for region in g.regionPaths {
                if region.code == selected { lifted.fill(region.path, with: .color(Theme.gold.opacity(0.35)), style: even) }
                lifted.stroke(region.path, with: .color(Theme.stroke), lineWidth: 0.9)
            }
        }
        for state in g.playStates {
            // A cool rim light along the scenario's states when lit.
            if lit && t > 0.5 { lifted.stroke(state, with: .color(RGB(hex: 0x8EC5FF).color(0.10)), lineWidth: 4) }
            lifted.stroke(state, with: .color(RGB(hex: 0xD2DEEA).color(0.85)), lineWidth: zoom > 3 ? 2 : 1)
        }
        // Night falls on the globe whenever it is lit, and on the flat map with day and night on.
        // With the globe's shading, the haze and the vignette it is soft, so it all goes in one
        // small light layer, clipped to the globe while there is one so its edge stays sharp.
        if lit {
            let night = t < 1 || sun != nil
            var front = context
            if t < 0.5 { front.clip(to: disc(g.center, p.r)) }
            let lighting = look.lighting
            g.frontLight.draw(in: &front, size: size, displayScale: displayScale, key: lightKey + [night]) { layer, full in
                if night { GlobeLighting.night(g.night(light), in: &layer) }
                GlobeLighting.front(in: &layer, projection: p, sun: light, lighting: lighting)
                if t > 0.5 { MapScene.flatLight(in: &layer, projection: p, size: full) }
            }
        }
        if t < 1 && !lit {
            // The globe's rim, fading as it unrolls.
            let c = g.center
            context.stroke(disc(c, p.r), with: .color(water.opacity(0.9 * (1 - t))), lineWidth: 1.5)
            context.stroke(disc(c, p.r + 4), with: .color(water.opacity(0.3 * (1 - t))), lineWidth: 6)
        }
        var lanesLayer = context
        lanesLayer.translateBy(x: 0, y: up)
        if freightLayer && !lanes.isEmpty {
            drawLanes(in: &lanesLayer, projection: p)
        } else if look.traffic > 0 && !lanes.isEmpty && t > 0.5 {
            drawFaintLanes(in: &lanesLayer, projection: p)
        }

        var labels = context
        labels.addFilter(.shadow(color: .black.opacity(0.85), radius: 1.5, x: 0, y: 1))
        if zoom > 3 {
            let close = zoom > 14
            let nightNow = sun != nil
            // City skylines, then the player's divisions as buildings, nearest drawn last.
            if look.buildings >= 2 && zoom > 6 && t > 0.5 {
                for box in g.skylines(ground) {
                    let windows = nightNow && light.facing(box.geo) < 0 ? 0.75 : look.lighting == 2 ? 0.22 : 0
                    drawBox(&context, projection: p, gx: box.gx, gy: box.gy, half: box.half, floor: box.floor, top: box.top,
                            color: MapScene.skyline, windows: windows)
                }
            }
            for (index, city) in map.cities.prefix(close ? 40 : 14).enumerated() {
                if !close && city.country != "USA" { continue }
                let geo = GeoPoint(e4: city.x, city.y)
                guard let at = p.at(lon: geo.lon, lat: geo.lat, height: g.cityFloor(index, ground)) else { continue }
                let above = g.skylineTop(index).map { p.lift(CGPoint(x: $0.x, y: $0.y), $0.h) }
                context.fill(disc(at, 2.5), with: .color(Theme.primaryText.opacity(0.85)))
                labels.draw(
                    Text(city.name).font(.system(size: 10, weight: .semibold)).foregroundStyle(Theme.primaryText.opacity(0.9)),
                    at: CGPoint(x: at.x + 6, y: min(above?.y ?? at.y, at.y)), anchor: .leading
                )
            }
            if look.buildings > 0 && t > 0.5 {
                for b in g.buildings(divisions, ground) {
                    let windows = nightNow && light.facing(b.geo) < 0 ? 0.85 : look.lighting == 2 ? 0.30 : 0
                    drawBox(&context, projection: p, gx: b.gx, gy: b.gy, half: b.half, floor: b.floor, top: b.top,
                            color: MapScene.sectorRGB(b.sector), windows: windows, outline: Theme.gold.opacity(0.9))
                    let topAt = p.lift(CGPoint(x: b.gx, y: b.gy), b.top)
                    let badge = CGPoint(x: topAt.x, y: topAt.y - 12)
                    let count = context.resolve(Text("\(b.plants)").font(.system(size: 10, weight: .bold)).foregroundStyle(Theme.ink))
                    let measured = count.measure(in: CGSize(width: 200, height: 50))
                    context.fill(disc(badge, max(measured.width, measured.height) / 2 + 3), with: .color(Theme.gold))
                    context.draw(count, at: badge)
                }
            } else {
                drawDivisions(in: &context, projection: p, lift: plinth)
            }
            if look.cityLights && lit && (sun != nil || t < 1) {
                GlobeLighting.cityLights(in: &context, cities: map.cities, projection: p, sun: light, lift: plinth, scale: 1)
            }
        } else if look.cityLights && lit && t < 1 {
            GlobeLighting.cityLights(in: &context, cities: map.cities, projection: p, sun: light, lift: 0, scale: 0.7)
        }
        if zoom > 14 {
            for region in map.regions {
                guard let anchor = map.anchor(of: region.code),
                      let at = p.at(lon: anchor.lon, lat: anchor.lat, height: g.regionLift(region.code, ground)) else { continue }
                labels.draw(
                    Text(region.name.uppercased()).font(.system(size: 9, weight: .semibold)).tracking(0.8)
                        .foregroundStyle(Theme.secondary.opacity(0.8)),
                    at: CGPoint(x: at.x, y: at.y + 30), anchor: .top
                )
            }
        }
    }

    /// Haze towards the horizon when tilted, and a light vignette that holds the eye in the middle.
    static func flatLight(in context: inout GraphicsContext, projection p: MapProjection, size: CGSize) {
        // Kept faint: depth without washing the map out.
        let haze = 0.07 * sin(p.pitch) / sin(48 * Double.pi / 180)
        if haze > 0.01 {
            context.fill(Path(CGRect(x: 0, y: 0, width: size.width, height: size.height * 0.25)), with: .linearGradient(Gradient(stops: [
                .init(color: RGB(hex: 0x9CC3E6).color(haze), location: 0),
                .init(color: .clear, location: 1),
            ]), startPoint: .zero, endPoint: CGPoint(x: 0, y: size.height * 0.25)))
        }
        context.fill(Path(CGRect(origin: .zero, size: size)), with: .radialGradient(Gradient(stops: [
            .init(color: .clear, location: 0),
            .init(color: .clear, location: 0.8),
            .init(color: .black.opacity(0.18), location: 1),
        ]), center: CGPoint(x: size.width / 2, y: size.height / 2), startRadius: 0,
            endRadius: max(size.width, size.height) * 0.75))
    }

    static func sectorRGB(_ sector: String) -> RGB {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        guard UIColor(Sector.color(sector)).getRed(&red, green: &green, blue: &blue, alpha: &alpha) else { return regionLand }
        return RGB(Double(red), Double(green), Double(blue))
    }

    private var busiestLanes: [LaneDto] {
        Array(lanes.sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }.prefix(90))
    }

    private func laneEnds(_ lane: LaneDto, _ p: MapProjection) -> (CGPoint, CGPoint)? {
        guard let a = map.anchor(of: lane.a), let b = map.anchor(of: lane.b) else { return nil }
        let endA = p.project(a)
        let endB = p.project(b)
        guard endA.1, endB.1 else { return nil }
        return (endA.0, endB.0)
    }

    /// The lane drawn nearest a tap, within a finger's width.
    func laneAt(_ tap: CGPoint, geometry g: MapGeometry) -> LaneDto? {
        let up: CGFloat = look.extrude && g.projection.t > 0.05
            ? g.projection.lift(g.center, g.plinth).y - g.projection.lift(g.center, 0).y : 0
        var best: LaneDto?
        var bestDistance: CGFloat = 22
        for lane in busiestLanes {
            guard let ends = laneEnds(lane, g.projection) else { continue }
            let (a, b) = ends
            let dx = b.x - a.x
            let dy = b.y - a.y
            let len2 = dx * dx + dy * dy
            let y = tap.y - up
            let f = len2 <= 0 ? 0 : min(max(((tap.x - a.x) * dx + (y - a.y) * dy) / len2, 0), 1)
            let d = hypot(tap.x - (a.x + f * dx), y - (a.y + f * dy))
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
    private func drawLanes(in context: inout GraphicsContext, projection p: MapProjection) {
        let busiest = busiestLanes
        let top = CGFloat(max(busiest.map { Int($0.capacityLots + $0.outsideLots) }.max() ?? 1, 1))
        for lane in busiest {
            guard let ends = laneEnds(lane, p) else { continue }
            let (a, b) = ends
            var path = Path()
            path.move(to: a)
            path.addLine(to: b)
            let weight = CGFloat(lane.capacityLots + lane.outsideLots) / top
            let dim = (selected != nil && lane.a != selected && lane.b != selected && pickedLane == nil) ? 0.35 : 0.9
            let width = 1.5 + 5 * weight
            let dash: [CGFloat] = lane.modeId == "sea" ? [12, 9] : []
            if lane.id == pickedLane {
                context.stroke(path, with: .color(.white.opacity(0.9)), style: StrokeStyle(lineWidth: width + 7, dash: dash))
            } else if laneIsMine(lane) {
                context.stroke(path, with: .color(Theme.gold.opacity(0.75)), style: StrokeStyle(lineWidth: width + 4, dash: dash))
            }
            context.stroke(path, with: .color(laneColor(lane).opacity(dim)), style: StrokeStyle(lineWidth: width, dash: dash))
        }
    }

    /// Off the Freight layer, the lanes freight moves along show faintly.
    private func drawFaintLanes(in context: inout GraphicsContext, projection p: MapProjection) {
        for lane in busiestLanes.prefix(60) {
            guard let ends = laneEnds(lane, p) else { continue }
            let (a, b) = ends
            var path = Path()
            path.move(to: a)
            path.addLine(to: b)
            context.stroke(path, with: .color(laneColor(lane).opacity(0.10)), lineWidth: 1)
        }
    }

    /// The player's divisions as flat badges: one per owned sector type in each region, in a row
    /// below the region's anchor city, with a badge counting its plants.
    private func drawDivisions(in context: inout GraphicsContext, projection p: MapProjection, lift: Double) {
        let radius: CGFloat = 11
        let byRegion = Dictionary(grouping: divisions, by: \.region)
        for (code, owned) in byRegion {
            guard let anchor = map.anchor(of: code) else { continue }
            let (origin, visible) = p.project(anchor, height: lift)
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

    // MARK: Moving layer

    /// Everything that moves on the map, for one frame.
    func drawOverlay(in context: inout GraphicsContext, geometry g: MapGeometry, seconds: Double, rippleAge: Double) {
        let p = g.projection
        let o = g.overlay(lanes: lanes, divisions: divisions, selected: selected, heights: heights, look: look,
                          freightLayer: freightLayer)
        let zoom = camera.zoom
        // Freight along the lanes, both ways.
        if look.traffic > 0 || freightLayer {
            let per = look.traffic >= 2 ? 9.0 : 4.0
            let boost = freightLayer ? 2.0 : 1.0
            var glow = context
            glow.blendMode = .plusLighter
            var drawn = 0
            laneLoop: for lane in o.lanes {
                let count = 1 + Int(lane.weight * per)
                let color = MapScene.modeColors[lane.mode] ?? RGB(hex: 0xE8E8E8)
                let speed = MapScene.modeSpeed[lane.mode] ?? 40
                var cores = Path()
                for k in 0..<count {
                    for dir in 0..<2 {
                        if drawn >= 600 { break laneLoop }
                        var f = fract(seconds * speed / lane.length + lane.seed + Double(k) / Double(count) + Double(dir) * 0.37)
                        if dir == 1 { f = 1 - f }
                        let gx = Double(lane.a.x) + Double(lane.b.x - lane.a.x) * f
                        let gy = Double(lane.a.y) + Double(lane.b.y - lane.a.y) * f
                        let at = p.lift(CGPoint(x: gx, y: gy), o.plinth + 2)
                        stamp(&glow, .glow, at: at, radius: 4 * boost, color: color, alpha: 0.35 * min(boost, 1.5))
                        cores.addPath(disc(at, 1.6 * boost))
                        drawn += 1
                    }
                }
                context.fill(cores, with: .color(color.color()))
            }
        }
        // Cars on the highways once close, each direction in its own lane.
        if look.traffic > 0 && look.detail >= 2 && zoom > 9 && !o.roads.isEmpty {
            let spacing = look.traffic >= 2 ? 35.0 : 70.0
            let detailFade = clamp((zoom - 7) / 5)
            let fade = detailFade * clamp((zoom - 9) / 3)
            var heads = Path()
            var tails = Path()
            var cars = 0
            roadLoop: for road in o.roads {
                let n = max(Int(road.total / spacing), 1)
                for k in 0..<n {
                    if cars >= 400 { break roadLoop }
                    let forward = k % 2 == 0
                    var d = (Double(k) * spacing + (forward ? 1 : -1) * seconds * 18).truncatingRemainder(dividingBy: road.total)
                    if d < 0 { d += road.total }
                    let at = road.point(at: d)
                    let ahead = road.point(at: min(max(d + (forward ? 3 : -3), 0), road.total))
                    let dx = ahead.x - at.x
                    let dy = ahead.y - at.y
                    let len = max(hypot(dx, dy), 1e-3)
                    let sign: CGFloat = forward ? 1 : -1
                    let sx = -dy / len * 1.6 * sign
                    let sy = dx / len * 1.6 * sign
                    heads.addPath(disc(CGPoint(x: ahead.x + sx, y: ahead.y + sy), 1.2))
                    tails.addPath(disc(CGPoint(x: at.x + sx, y: at.y + sy), 1.1))
                    cars += 1
                }
            }
            context.fill(heads, with: .color(RGB(hex: 0xFFF4D8).color(0.95 * fade)))
            context.fill(tails, with: .color(RGB(hex: 0xFF4A3A).color(0.85 * fade)))
        }
        // Building beacons, smoke and the week's ripple.
        if look.buildings > 0 {
            for b in o.buildings {
                let phase = fract(seconds / 2.4 + b.seed)
                context.stroke(disc(b.base, (6 + 20 * phase) * o.scale), with: .color(b.color.opacity(0.45 * (1 - phase))), lineWidth: 1.4)
                // An aircraft warning light blinks on every rooftop.
                if sin(seconds * 3 + b.seed * 20) > 0.55 {
                    var beacon = context
                    beacon.blendMode = .plusLighter
                    stamp(&beacon, .glow, at: b.top, radius: 5, color: RGB(hex: 0xFF4A3A), alpha: 0.7)
                    context.fill(disc(b.top, 1.3), with: .color(RGB(hex: 0xFF6A5A).color()))
                }
                if b.smoke && look.smoke {
                    for k in 0..<3 {
                        let f = fract(seconds / 3 + Double(k) / 3 + b.seed)
                        let at = CGPoint(x: b.top.x + 4 * sin(f * 5 + b.seed * 9), y: b.top.y - 30 * f * o.scale)
                        stamp(&context, .cloud, at: at, radius: (2 + 5 * f) * o.scale, color: RGB(hex: 0x9AA3AB), alpha: 0.55 * (1 - f))
                    }
                }
                if rippleAge >= 0 && rippleAge <= 1.2 {
                    let f = rippleAge / 1.2
                    context.stroke(disc(b.base, 60 * f * o.scale), with: .color(Theme.gold.opacity(0.6 * (1 - f))), lineWidth: 2)
                }
            }
        }
        // Planes between the big cities, high over the map, trailing contrails with shadows below.
        if look.traffic > 0 && o.airports.count >= 2 {
            let planes = look.traffic >= 2 ? 10 : 5
            let n = o.airports.count
            let life = 14.0
            for k in 0..<planes {
                let local = seconds + Double(k) * 3.1
                let cycle = Int(floor(local / life))
                let f = (local - Double(cycle) * life) / life
                let hash = stableHash("plane\(k)/\(cycle)")
                let a = o.airports[hash % n]
                let b = o.airports[(hash % n + 1 + (hash / n) % (n - 1)) % n]
                let fade = min(min(f, 1 - f) * 8, 1)
                for trail in stride(from: 12, through: 0, by: -1) {
                    let ft = min(max(f - Double(trail) * 0.006, 0), 1)
                    let ground = CGPoint(x: a.x + (b.x - a.x) * ft, y: a.y + (b.y - a.y) * ft)
                    let at = p.lift(ground, (40 + 50 * sin(.pi * ft)) * o.scale)
                    if trail == 0 {
                        let shadow = p.lift(ground, o.plinth)
                        context.fill(disc(CGPoint(x: shadow.x + 3, y: shadow.y + 2), 1.8), with: .color(.black.opacity(0.25 * fade)))
                        context.fill(disc(at, 1.7), with: .color(.white.opacity(fade)))
                        if fract(seconds * 2 + Double(k)) < 0.15 {
                            context.fill(disc(CGPoint(x: at.x, y: at.y + 2.5), 1.2), with: .color(RGB(hex: 0xFF5040).color(fade)))
                        }
                    } else {
                        context.fill(disc(at, 1.1 + Double(trail) * 0.08),
                                     with: .color(.white.opacity(0.22 * fade * (1 - Double(trail) / 13))))
                    }
                }
            }
        }
        // The selected region breathes.
        if let top = o.selectedTop {
            let pulse = 0.5 + 0.5 * sin(2 * Double.pi * seconds / 1.6)
            context.stroke(top, with: .color(Theme.gold.opacity(0.18)), lineWidth: 8)
            context.stroke(top, with: .color(Theme.gold.opacity(0.95)), lineWidth: 2 + 1.5 * pulse)
        }
    }

    /// With nothing moving, only the selected region's outline.
    func drawSelection(in context: inout GraphicsContext, geometry g: MapGeometry) {
        let o = g.overlay(lanes: lanes, divisions: divisions, selected: selected, heights: heights, look: look,
                          freightLayer: freightLayer)
        if let top = o.selectedTop { context.stroke(top, with: .color(Theme.gold), lineWidth: 2.5) }
    }
}

/// The world map: a globe when zoomed out that flattens into a map as the player zooms in on the
/// scenario. Drag to turn or pan, pinch or use the buttons to zoom, double-tap to fly between the
/// globe and the scenario, tap a region to select it. Shading, markers and selection come from
/// the caller. A still layer holds the land, regions and buildings; a moving layer over it holds
/// traffic, planes and pulses, and only runs while something on it moves.
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
    @State private var meter = FrameMeter()

    private var map: WorldMap { WorldMap.forRegions(regionCodes) }

    private var look: MapLook {
        MapLook(
            detail: detailLevel, tilt: GraphicsSettings.pitch(forTilt: tiltSetting), extrude: extrude,
            lighting: lighting, dayNight: dayNight, cityLights: cityLights, traffic: traffic,
            buildings: buildings, frameRate: frameRate, animations: animations, showFps: showFps,
            ultra: preset == GraphicsPreset.ultra.rawValue
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
            let interval = GraphicsSettings.frameInterval(look.frameRate)
            let running = clockAllowed && (look.moves || look.showFps)
            ZStack(alignment: .topLeading) {
                TimelineView(.animation(minimumInterval: easing ? interval : 1.0 / 3.0,
                                        paused: !(easing || (look.dayNight && clockAllowed)))) { timeline in
                    Canvas { context, canvasSize in
                        let now = MapClock.seconds(timeline.date)
                        let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                        scene(camera: current, look: look, now: now).drawStatic(in: &context, geometry: g)
                    }
                    .drawingGroup()
                }
                if look.moves || look.showFps || selected != nil {
                    TimelineView(.animation(minimumInterval: interval, paused: !running)) { timeline in
                        Canvas { context, canvasSize in
                            let now = MapClock.seconds(timeline.date)
                            let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                            let moving = scene(camera: current, look: look, now: now)
                            if look.moves {
                                moving.drawOverlay(in: &context, geometry: g, seconds: now, rippleAge: now - rippleStart)
                            } else {
                                moving.drawSelection(in: &context, geometry: g)
                            }
                            if look.showFps {
                                drawFrameRate(in: &context, size: canvasSize, time: now, running: running)
                            }
                        }
                        .drawingGroup()
                    }
                    .allowsHitTesting(false)
                }
            }
            .background(MapScene.spaceColor.color())
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
                let now = MapClock.now
                let g = geometry.of(map: map, camera: current, size: size, tilt: look.tilt, detail: look.detail)
                let tapped = scene(camera: current, look: look, now: now)
                if layer == .freight, let onLane {
                    if let lane = tapped.laneAt(location, geometry: g) {
                        onLane(lane.id == pickedLane ? nil : lane.id)
                        return
                    }
                    onLane(nil)
                }
                var hit: String?
                if look.extrude && g.projection.t > 0.05 {
                    // Nearest tile first, on its raised top face.
                    hit = g.raised(tapped.heights).reversed().first { $0.top.contains(location, eoFill: true) }?.code
                }
                if hit == nil {
                    hit = map.regions.first { g.projection.contains($0.rings, location) }?.code
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
            ease.retarget(from: old, to: new, now: MapClock.now, animate: animate && extrude)
            guard animate && extrude else { return }
            easing = true
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: 650_000_000)
                easing = false
            }
        }
        .onChange(of: week) { _, _ in
            rippleStart = MapClock.now
        }
    }

    /// The scene for one camera and moment.
    private func scene(camera: MapCamera, look: MapLook, now: Double) -> MapScene {
        MapScene(
            map: map, camera: camera, look: look, tint: layer.tintRGB, shading: shading,
            heights: layer == .freight ? [:] : ease.value(at: now, target: shading),
            selected: selected, divisions: divisions, lanes: lanes, freightLayer: layer == .freight, pickedLane: pickedLane,
            sun: look.dayNight ? MapSun.moving(now) : nil
        )
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

    /// The optional frame counter, bottom left.
    private func drawFrameRate(in context: inout GraphicsContext, size: CGSize, time: Double, running: Bool) {
        let label: String
        if running {
            label = "\(Int(meter.tick(time))) fps"
        } else {
            meter.reset()
            label = "Still"
        }
        let text = context.resolve(Text(label).font(.system(size: 11, weight: .bold).monospacedDigit()).foregroundStyle(Theme.positive))
        let measured = text.measure(in: CGSize(width: 200, height: 40))
        let box = CGRect(x: 6, y: size.height - measured.height - 14, width: measured.width + 12, height: measured.height + 8)
        context.fill(Path(box), with: .color(.black.opacity(0.55)))
        context.draw(text, at: CGPoint(x: 12, y: size.height - measured.height - 10), anchor: .topLeading)
    }
}
