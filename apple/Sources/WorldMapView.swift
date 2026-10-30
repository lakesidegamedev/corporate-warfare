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

/// How big things stand on the ground, in metres. A voxel is this wide on every model, so a building covers
/// the same ground at every zoom, tilt and turn; only the minimum keeps one from shrinking to nothing when
/// the map is far out. Lots are fixed offsets from a region's hub, so a building never moves once built.
enum WorldScale {
    static let earthRadius = 6_371_000.0
    static let playerVoxel = 2600.0
    static let cityVoxel = 1950.0
    static let rivalVoxel = 1560.0
    /// The smallest a voxel is drawn, in device pixels.
    static let minUnitPixels = 1.0

    /// A division's lot beside its region's hub, east and north in metres: its industry's place in the list decides.
    static func playerLot(_ sector: String) -> (east: Double, north: Double) {
        let i = Sector.order.firstIndex(of: sector) ?? Sector.order.count
        return (Double(i % 5 - 2) * 60_000, -(35_000 + Double(i / 5) * 55_000))
    }

    /// A rival's lot north of the hub: slot 0 to 7, four to a row.
    static func rivalLot(_ slot: Int) -> (east: Double, north: Double) {
        ((Double(slot % 4) - 1.5) * 50_000, 35_000 + Double(slot / 4) * 35_000)
    }

    /// Rival slots by stable hash of each key, a taken slot giving way to the next free one.
    static func rivalSlots(_ keys: [String]) -> [String: Int] {
        var taken = [Bool](repeating: false, count: 8)
        var out: [String: Int] = [:]
        for key in keys.sorted(by: { stableHash($0) != stableHash($1) ? stableHash($0) < stableHash($1) : $0 < $1 }) {
            var slot = stableHash(key) % 8
            var tries = 0
            while taken[slot] && tries < 8 { slot = (slot + 1) % 8; tries += 1 }
            taken[slot] = true
            out[key] = slot
        }
        return out
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

/// A city drawn as a voxel block: its index in the map's cities, untilted ground point, the
/// tile it stands on, and a stand-in plant count that picks the block size.
struct CityBlock {
    let index: Int
    let gx: Double
    let gy: Double
    let floor: Double
    let plants: Int
}

/// A lane's ends as untilted points, with its mode and weight.
struct LaneTrack {
    let a: CGPoint
    let b: CGPoint
    let mode: String
    let weight: Double
    let length: Double
    let seed: Double
    /// The lane's length on the ground, so freight keeps its pace at any zoom.
    let km: Double
    /// The player owns or ships on it, so its vehicles wear the gold livery.
    var mine = false
    /// Who moves the goods: the player's gold, a rival's sector colour, or the outside carriers' grey.
    var owner = FreightLook.outside
    /// Whether each of `LaneWater.samples` points from a to b is over water, so ships never sail
    /// over land and trucks never drive on a lake.
    var water: [Bool]?
    /// The route on the untilted ground when the lane follows a real road or railway, and the
    /// distance along it at each point as a fraction of its length; nil for a straight line.
    var route: [CGPoint]?
    var along: [Double]?

    /// True when the lane runs on a real network rather than a straight line.
    var routed: Bool { along != nil }

    /// The lane following `points` on the ground.
    func following(_ points: [CGPoint]) -> LaneTrack {
        guard points.count >= 2 else { return self }
        var cum = [Double](repeating: 0, count: points.count)
        for i in 1..<points.count {
            cum[i] = cum[i - 1] + Double(hypot(points[i].x - points[i - 1].x, points[i].y - points[i - 1].y))
        }
        let total = max(cum[cum.count - 1], 1e-3)
        var next = self
        next.route = points
        next.along = cum.map { $0 / total }
        return next
    }

    /// The untilted ground point `f` of the way along the lane.
    func at(_ f: Double) -> CGPoint {
        guard let along, let route else {
            return CGPoint(x: Double(a.x) + Double(b.x - a.x) * f, y: Double(a.y) + Double(b.y - a.y) * f)
        }
        var low = 0
        var high = along.count - 1
        while high - low > 1 {
            let mid = (low + high) / 2
            if along[mid] <= f { low = mid } else { high = mid }
        }
        let t = min(max((f - along[low]) / max(along[high] - along[low], 1e-6), 0), 1)
        return CGPoint(x: Double(route[low].x) + Double(route[high].x - route[low].x) * t,
                       y: Double(route[low].y) + Double(route[high].y - route[low].y) * t)
    }
}

/// Where a building's rooftop light and smoke sit on screen.
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
    /// Zoomed in past the framing: freight, cars and planes are drawn as voxel vehicles.
    var closeUp = false
    /// Points a voxel for the vehicles, a step under the player's buildings.
    var vehicleUnit = 1.0
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
    /// The zoom against the scenario's framing, unclamped: 1 at the framing, under 1 further out.
    let zoomRatio: Double

    /// Zoomed in past the scenario's framing: only then do divisions and cities stand as voxel
    /// models. Around the framing divisions are small badges; further out nothing marks them.
    var closeUp: Bool { zoomRatio >= 1.4 }

    init(map: WorldMap, camera: MapCamera, size: CGSize, tilt: Double, detail: Int) {
        self.map = map
        self.camera = camera
        self.size = size
        self.tilt = tilt
        self.detail = detail
        projection = MapProjection(camera: camera, size: size, tilt: tilt)
        let framed = MapCamera.scenario(in: size, map: map).zoom
        zoomRatio = camera.zoom / max(framed, 1e-6)
        scale = min(max(zoomRatio, 0.6), 2.5)
    }

    /// Points one metre of ground covers at this camera.
    var pointsPerMetre: Double { projection.r / WorldScale.earthRadius }

    /// Points a voxel of [metres] covers on the ground, never under the minimum that keeps a building visible.
    func unit(_ metres: Double, displayScale: Double) -> Double {
        max(metres * pointsPerMetre, WorldScale.minUnitPixels / max(displayScale, 1))
    }

    /// The untilted screen point of a spot [east] and [north] metres from a hub: the same spot at every camera.
    func lot(_ hub: GeoPoint, east: Double, north: Double) -> CGPoint? {
        projection.ground(lon: hub.lon + east / (WorldScale.earthRadius * cos(hub.lat)), lat: hub.lat + north / WorldScale.earthRadius)
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
    /// The interstates' gold centre line: dashes cut on the ground, projected like the roads and
    /// stroked solid, so they stay painted on the road as the camera moves.
    lazy var centreLines: Path = projection.lines(map.centreDashLines)
    var railroadLines: [GeoRing] { map.railroads }
    private var railsHeld: (lift: Double, path: Path)?
    private var sleepersHeld: (lift: Double, path: Path)?

    /// The real railways `lift` points up, kept for the last height.
    func railroads(lift: Double) -> Path {
        if let railsHeld, railsHeld.lift == lift { return railsHeld.path }
        let path = projection.lines(map.railroads, height: lift)
        railsHeld = (lift, path)
        return path
    }

    /// The railways' sleepers `lift` points up, cut on the ground, kept for the last height.
    func sleepers(lift: Double) -> Path {
        if let sleepersHeld, sleepersHeld.lift == lift { return sleepersHeld.path }
        let path = projection.lines(map.sleeperDashes, height: lift)
        sleepersHeld = (lift, path)
        return path
    }
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

    private var tileRoadsKey: [String: Double]?
    private var tileRoadsHeld: [String: Path] = [:]

    /// The interstates on each raised tile, projected at that tile's own height so they lie on its
    /// top whatever the camera does. A single screen offset for every tile would leave them hanging
    /// between the tiles' tops, and sliding as the perspective changes across the screen.
    func tileRoads(_ heights: [String: Double]) -> [String: Path] {
        if tileRoadsKey == heights { return tileRoadsHeld }
        let index = map.regionIndex
        var out: [String: Path] = [:]
        for (i, region) in map.regions.enumerated() where i < index.regionInterstates.count {
            let lines = index.regionInterstates[i].map { map.interstates[$0] }
            if lines.isEmpty { continue }
            out[region.code] = projection.lines(lines, height: height(heights[region.code] ?? 0))
        }
        tileRoadsHeld = out
        tileRoadsKey = heights
        return out
    }

    private var tileDashesKey: [String: Double]?
    private var tileDashesHeld: [String: Path] = [:]

    /// The centre-line dashes on each raised tile, at that tile's own height like its roads.
    func tileDashes(_ heights: [String: Double]) -> [String: Path] {
        if tileDashesKey == heights { return tileDashesHeld }
        let index = map.regionIndex
        var out: [String: Path] = [:]
        for (i, region) in map.regions.enumerated() where i < index.regionInterstates.count {
            let dashes = index.regionInterstates[i].flatMap { map.centreDashes[$0] }
            if dashes.isEmpty { continue }
            out[region.code] = projection.lines(dashes, height: height(heights[region.code] ?? 0))
        }
        tileDashesHeld = out
        tileDashesKey = heights
        return out
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
        let sorted = divisions.sorted {
            $0.region != $1.region ? $0.region < $1.region
                : (Sector.order.firstIndex(of: $0.sector) ?? 99) < (Sector.order.firstIndex(of: $1.sector) ?? 99)
        }
        var placed: [Building] = []
        let u = unit(WorldScale.playerVoxel, displayScale: 1)
        for division in sorted {
            guard let anchor = map.anchor(of: division.region) else { continue }
            // A fixed lot beside the hub, in metres, so it stands still as the map zooms, tilts and turns.
            let lot = WorldScale.playerLot(division.sector)
            guard let ground = self.lot(anchor, east: lot.east, north: lot.north) else { continue }
            let floor = regionLift(division.region, heights)
            let tall = (10 + 6 * Double(max(division.plants, 0)).squareRoot()) * u
            placed.append(Building(sector: division.sector, plants: division.plants, gx: Double(ground.x), gy: Double(ground.y), half: 4 * u,
                                   floor: floor, top: floor + tall, geo: anchor, key: division.id))
        }
        buildingsHeld = placed.sorted { $0.gy < $1.gy }
        buildingsKey = (divisions: divisions, heights: heights)
        return buildingsHeld
    }

    private var rivalsKey: (plants: [MapPlantDto], heights: [String: Double]?)?
    private var rivalsHeld: [Building] = []

    /// Rivals' plants as buildings: each rival's plants in an industry and region together, the
    /// four biggest per region, set in a row behind the region's hub (north of it, where the
    /// player's own stand to the south), far ones first.
    func rivalBuildings(_ plants: [MapPlantDto], _ heights: [String: Double]?) -> [Building] {
        if let key = rivalsKey, key.plants == plants, key.heights == heights { return rivalsHeld }
        var blocks: [String: (region: String, industry: String, firm: UInt64, blocks: Int)] = [:]
        for plant in plants where !plant.player {
            let key = "\(plant.region)/\(plant.industry)/\(plant.firm)"
            blocks[key, default: (plant.region, plant.industry, plant.firm, 0)].blocks += Int(plant.blocks)
        }
        var placed: [Building] = []
        let u = unit(WorldScale.rivalVoxel, displayScale: 1)
        for (region, firms) in Dictionary(grouping: blocks, by: { $0.value.region }) {
            guard let anchor = map.anchor(of: region) else { continue }
            let floor = regionLift(region, heights)
            let biggest = Array(firms.sorted { $0.value.blocks != $1.value.blocks ? $0.value.blocks > $1.value.blocks : $0.key < $1.key }.prefix(4))
            let slots = WorldScale.rivalSlots(biggest.map(\.key))
            for firm in biggest {
                let lot = WorldScale.rivalLot(slots[firm.key] ?? 0)
                guard let ground = self.lot(anchor, east: lot.east, north: lot.north) else { continue }
                placed.append(Building(sector: firm.value.industry, plants: firm.value.blocks, gx: Double(ground.x), gy: Double(ground.y), half: 3 * u,
                                       floor: floor, top: floor + 10 * u, geo: anchor, key: firm.key))
            }
        }
        rivalsHeld = placed.sorted { $0.gy < $1.gy }
        rivalsKey = (plants: plants, heights: heights)
        return rivalsHeld
    }

    private var blocksBuilt = false
    private var blocksKey: [String: Double]?
    private var blocksHeld: [CityBlock] = []

    /// The biggest cities on screen as voxel city blocks, far ones first: thirty, or sixteen
    /// around the scenario's framing so the map stays readable. Below a million people a town
    /// block, below four million a city, above that a metropolis.
    func cityBlocks(_ heights: [String: Double]?) -> [CityBlock] {
        if blocksBuilt && blocksKey == heights { return blocksHeld }
        let p = projection
        let bounds = CGRect(origin: .zero, size: size).insetBy(dx: -60, dy: -60)
        let order = map.cities.indices.sorted { map.cities[$0].population > map.cities[$1].population }
        let most = camera.zoom > 10 ? 30 : 16
        var blocks: [CityBlock] = []
        for index in order {
            if blocks.count >= most { break }
            let city = map.cities[index]
            let geo = GeoPoint(e4: city.x, city.y)
            guard let ground = p.ground(lon: geo.lon, lat: geo.lat), bounds.contains(p.lift(ground, 0)) else { continue }
            let stand = city.population < 1_000_000 ? 2 : city.population < 4_000_000 ? 5 : 12
            blocks.append(CityBlock(index: index, gx: Double(ground.x), gy: Double(ground.y),
                                    floor: cityFloor(index, heights), plants: stand))
        }
        blocksHeld = blocks.sorted { $0.gy < $1.gy }
        blocksKey = heights
        blocksBuilt = true
        return blocksHeld
    }

    private struct OverlayKey: Equatable {
        let lanes: Int
        let divisions: [DivisionMarker]
        let selected: String?
        let heights: [String: Double]
        let look: MapLook
        let freight: Bool
        let displayScale: Double
        let routes: Int
        let firmSector: [UInt64: String]
    }

    private var overlayKey: OverlayKey?
    private var overlayHeld: OverlayGeometry?

    /// What the moving layer needs, rebuilt only when the camera or the data under it changes.
    func overlay(lanes: [LaneDto], divisions: [DivisionMarker], selected: String?, heights: [String: Double],
                 look: MapLook, freightLayer: Bool, displayScale: Double = 2,
                 firmSector: [UInt64: String] = [:]) -> OverlayGeometry {
        var hasher = Hasher()
        for lane in lanes {
            hasher.combine(lane.id)
            hasher.combine(lane.capacityLots)
            hasher.combine(lane.outsideLots)
        }
        let key = OverlayKey(lanes: hasher.finalize(), divisions: divisions, selected: selected, heights: heights,
                             look: look, freight: freightLayer, displayScale: displayScale, routes: Routes.generation,
                             firmSector: firmSector)
        if let overlayHeld, overlayKey == key { return overlayHeld }
        let p = projection
        let raised = look.raised(p.t)
        let ground: [String: Double]? = raised ? heights : nil
        let laneCount = look.traffic >= 2 || freightLayer ? 120 : look.traffic == 1 ? 60 : 0
        let tracks = p.t > 0.5 && laneCount > 0 ? laneTracks(lanes, count: laneCount, firmSector: firmSector) : []
        let roadLines = look.traffic > 0 && detail >= 2 && camera.zoom > 9
            ? MapPolyline.build(p, map.interstates, lift: raised ? plinth : 0, size: size) : []
        // Beacons and smoke only where the models stand.
        let spots: [BuildingSpot] = look.buildings > 0 && closeUp && p.t > 0.5
            ? buildings(divisions, ground).map { b in
                let base = p.lift(CGPoint(x: b.gx, y: b.gy), b.floor)
                // The rooftop light and smoke sit on the model's top.
                let name = VoxelModels.sized(b.sector, plants: b.plants)
                let unit = self.unit(WorldScale.playerVoxel, displayScale: displayScale)
                let top = p.lift(CGPoint(x: b.gx, y: b.gy),
                                 VoxelModels.has(name) ? b.floor + VoxelModels.lift(name, unit: unit) : b.top)
                return BuildingSpot(base: base, top: top, color: Sector.color(b.sector), smoke: smokes(b.sector),
                                    seed: Double(stableHash(b.key) % 1000) / 1000)
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
                                   plinth: raised ? plinth : 0, airports: airports, closeUp: closeUp && p.t > 0.5,
                                   vehicleUnit: VoxelModels.unit(scale, displayScale: displayScale) * 0.6)
        overlayHeld = held
        overlayKey = key
        return held
    }

    /// The busiest lanes as untilted tracks, seeded so particles never line up between lanes.
    private func laneTracks(_ lanes: [LaneDto], count: Int, firmSector: [UInt64: String]) -> [LaneTrack] {
        let busiest = lanes.sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }.prefix(count)
        guard let first = busiest.first else { return [] }
        let top = Double(max(first.capacityLots + first.outsideLots, 1))
        let p = projection
        var missing: [Routes.Request] = []
        let tracks = busiest.compactMap { lane -> LaneTrack? in
            guard let a = map.anchor(of: lane.a).flatMap({ p.ground(lon: $0.lon, lat: $0.lat) }),
                  let b = map.anchor(of: lane.b).flatMap({ p.ground(lon: $0.lon, lat: $0.lat) }) else { return nil }
            // Trucks and trains follow the real interstates and railways where there is a route.
            // Routes are worked out in the background; a lane waits for its own.
            let request = Routes.Request(mode: lane.modeId, a: lane.a, b: lane.b)
            guard let known = Routes.cached(map, request), let water = LaneWater.cached(map, lane.a, lane.b) else {
                missing.append(request)
                return nil
            }
            var points: [CGPoint]?
            if let route = known {
                let projected = route.compactMap { p.ground(lon: $0.lon, lat: $0.lat) }
                if projected.count == route.count { points = projected }
            }
            var length = Double(hypot(b.x - a.x, b.y - a.y))
            if let points {
                length = zip(points, points.dropFirst()).reduce(0) { $0 + Double(hypot($1.1.x - $1.0.x, $1.1.y - $1.0.y)) }
            }
            var track = LaneTrack(a: a, b: b, mode: lane.modeId, weight: Double(lane.capacityLots + lane.outsideLots) / top,
                                  length: max(length, 8), seed: Double(stableHash(lane.a + lane.b) % 1000) / 1000,
                                  km: max(Double(lane.km), 20), mine: laneIsMine(lane), water: water)
            track.owner = FreightLook.ownerColor(lane, firmSector: firmSector)
            return points.map { track.following($0) } ?? track
        }
        if !missing.isEmpty { Routes.prewarm(map, missing) }
        return tracks
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

/// Which points along a lane's straight line are over water, kept per pair of places since it
/// never changes with the camera.
enum LaneWater {
    static let samples = 48
    private static var held: [String: [Bool]] = [:]
    private static let lock = NSLock()

    /// The crossings if already worked out, without working them out.
    static func cached(_ map: WorldMap, _ a: String, _ b: String) -> [Bool]? {
        lock.lock()
        defer { lock.unlock() }
        return held["\(map.scenarioId ?? "")/\(a)>\(b)"]
    }

    /// Whether each of `samples` points between two places is in a lake or outside every country.
    static func along(_ map: WorldMap, _ a: String, _ b: String) -> [Bool]? {
        let key = "\(map.scenarioId ?? "")/\(a)>\(b)"
        if let hit = cached(map, a, b) { return hit }
        guard let from = map.anchor(of: a), let to = map.anchor(of: b) else { return nil }
        let lon0 = from.lon, lat0 = from.lat, lon1 = to.lon, lat1 = to.lat
        // Only the rings whose box the lane passes through.
        let box = (min(lon0, lon1), max(lon0, lon1), min(lat0, lat1), max(lat0, lat1))
        func overlaps(_ ring: GeoRing) -> Bool {
            guard ring.points.count >= 3 else { return false }
            var w = Double.infinity, e = -Double.infinity, s = Double.infinity, n = -Double.infinity
            for point in ring.points {
                w = min(w, point.lon)
                e = max(e, point.lon)
                s = min(s, point.lat)
                n = max(n, point.lat)
            }
            return e >= box.0 && w <= box.1 && n >= box.2 && s <= box.3
        }
        let lakes = map.lakes.filter(overlaps)
        let land = map.countries.flatMap(\.rings).filter(overlaps)
        let out = (0..<samples).map { i -> Bool in
            let t = Double(i) / Double(samples - 1)
            let lon = lon0 + (lon1 - lon0) * t
            let lat = lat0 + (lat1 - lat0) * t
            return lakes.contains { inside($0, lon, lat) } || !land.contains { inside($0, lon, lat) }
        }
        lock.lock()
        held[key] = out
        lock.unlock()
        return out
    }

    /// Even-odd test of a place against one ring, on longitude and latitude.
    private static func inside(_ ring: GeoRing, _ lon: Double, _ lat: Double) -> Bool {
        let points = ring.points
        guard points.count >= 3 else { return false }
        var inside = false
        var j = points.count - 1
        for i in 0..<points.count {
            let yi = points[i].lat
            let yj = points[j].lat
            if (yi > lat) != (yj > lat) && lon < (points[j].lon - points[i].lon) * (lat - yi) / (yj - yi) + points[i].lon {
                inside.toggle()
            }
            j = i
        }
        return inside
    }
}

/// How a division's plants stand, for the map.
enum PlantState {
    case site, growing, running, mothballed

    /// Each division's state from its plants, keyed region/industry (and /firm for rivals): a
    /// building site while every plant is still going up, growing while some are built or
    /// retooled, mothballed when every plant is, running otherwise.
    static func of(_ plants: [MapPlantDto], byFirm: Bool = false) -> [String: PlantState] {
        let grouped = Dictionary(grouping: plants) { byFirm ? "\($0.region)/\($0.industry)/\($0.firm)" : "\($0.region)/\($0.industry)" }
        return grouped.mapValues { items in
            let building = items.filter { $0.status.hasPrefix("Building") }.count
            let retooling = items.filter { $0.status.hasPrefix("Retooling") }.count
            let mothballed = items.filter { $0.status.hasPrefix("Mothballed") }.count
            if building == items.count { return .site }
            if mothballed == items.count { return .mothballed }
            return building + retooling > 0 ? .growing : .running
        }
    }
}

/// Remembers who owned each plant, so a takeover can be marked on the map for a while.
final class TakeoverLog {
    private var owner: [UInt64: Bool] = [:]
    private var since: [String: Int] = [:]

    /// Divisions (region/industry) with a plant that was a rival's and is the player's now, for
    /// six weeks after.
    func update(_ plants: [MapPlantDto], week: Int) -> Set<String> {
        for plant in plants {
            if owner[plant.handle] == false && plant.player { since["\(plant.region)/\(plant.industry)"] = week }
            owner[plant.handle] = plant.player
        }
        since = since.filter { week - $0.value <= 6 }
        return Set(since.keys)
    }
}

/// Which part of the still map to draw: the ground, the structures over the traffic, or both.
enum MapPhase {
    /// Space and the ocean alone, under the satellite Earth's layer.
    case sky
    case ground, structures, all
}

/// Which moving things to draw: traffic (under the buildings), effects (over them), or both.
enum OverlayPart {
    case traffic, effects, all
}

/// Time for the world on the map: it stands still while the game is paused and runs one, two or
/// four times as fast at the three speeds, so moving things follow the game rather than the wall.
final class WorldClock {
    private var last = Double.nan
    private var seconds = 0.0

    func advance(wall: Double, speed: Double) -> Double {
        // Several layers read the clock each frame; only time that has really passed counts.
        if last.isNaN || wall > last {
            if !last.isNaN && speed > 0 { seconds += min(max(wall - last, 0), 0.5) * min(max(speed, 1), 4) }
            last = wall
        }
        return seconds
    }
}

/// What the map shades the regions by, with the words for each end of its legend. The same order,
/// titles, tints and words as the other platforms.
enum MapLayer: String, CaseIterable, Identifiable {
    case presence = "Your divisions"
    case share = "Your share"
    case competition = "Competition"
    case room = "Room to build"
    case demand = "Demand"
    case wages = "Wages"
    case skill = "Workforce skill"
    case infrastructure = "Infrastructure"
    case land = "Land price"
    case population = "Population"
    case freight = "Freight"

    var id: String { rawValue }

    var icon: String {
        switch self {
        case .presence: return "layer_presence"
        case .share: return "percent"
        case .competition: return "target"
        case .room: return "layer_room"
        case .demand: return "trend_up"
        case .wages: return "layer_wages"
        case .skill: return "layer_skill"
        case .infrastructure: return "rail"
        case .land: return "tag"
        case .population: return "people"
        case .freight: return "layer_freight"
        }
    }

    /// The legend's word for the faint end.
    var low: String {
        switch self {
        case .presence, .share: return "None"
        case .competition: return "Open"
        case .room: return "Little"
        case .wages, .land: return "Cheap"
        case .infrastructure: return "Poor"
        case .population: return "Small"
        case .demand, .skill, .freight: return "Low"
        }
    }

    /// The legend's word for the full end.
    var high: String {
        switch self {
        case .presence: return "Most plants"
        case .share: return "Leading"
        case .competition: return "Crowded"
        case .room: return "Most imports"
        case .wages, .land: return "Dear"
        case .infrastructure: return "Good"
        case .population: return "Large"
        case .demand, .skill, .freight: return "High"
        }
    }

    var tint: Color { tintRGB.color() }

    /// `tint` as components, for shading raised tiles.
    var tintRGB: RGB {
        switch self {
        case .presence, .share, .freight: return .gold
        case .competition: return RGB(hex: 0xE5605E)
        case .room: return .positive
        case .demand: return RGB(hex: 0x7E57C2)
        case .wages: return RGB(hex: 0x42A5F5)
        case .skill: return RGB(hex: 0x26C6DA)
        case .infrastructure: return RGB(hex: 0x90A4AE)
        case .land: return RGB(hex: 0xFF8A65)
        case .population: return RGB(hex: 0xEC407A)
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
    var clouds: Int
    var traffic: Int
    var buildings: Int
    var frameRate: Int
    var animations: Bool
    var showFps: Bool
    var ultra: Bool
    /// The satellite Earth under the map; the regions then lie flat and see-through over it.
    var earth = false
    /// Trees, fields, barns and houses on the ground up close.
    var countryside = true
    /// Rivals' plants as buildings up close.
    var rivals = true
    /// The drawn interstates.
    var interstates = true
    /// Names on the map: 0 none, 1 cities, 2 cities and regions.
    var labels = 2

    /// Whether the regions stand as raised tiles at a flatness: not over the satellite Earth.
    func raised(_ t: Double) -> Bool { extrude && t > 0.05 && !earth }
    /// Whether anything on the map moves on its own, so the frame clock needs to run.
    var moves: Bool { animations && (traffic > 0 || clouds > 0 || buildings > 0 || dayNight || lighting == 2) }
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
    /// Every plant on the map, the player's and rivals', for rivals' buildings and plant states.
    var plants: [MapPlantDto] = []
    /// The economy's phase ("boom", "recession" and so on), shown in the cities and towns.
    var economyPhase: String?
    /// Divisions (region/industry) the player has just taken over, which fly a flag.
    var acquired: Set<String> = []
    /// Week of the year 1..52, for the seasons.
    var week = 28
    var space = true
    /// Pixels per point, for the half-resolution light layers.
    var displayScale = 2.0
    /// Remembers which labels showed last frame, so they hold steady while the map moves.
    var labelPlanner: LabelPlanner?

    static let spaceColor = RGB(hex: 0x05090E)
    private static let outsideLand = RGB(hex: 0x25282B)
    private static let outsideBorder = RGB(hex: 0x55595D)
    private static let outsideRoad = RGB(hex: 0x6A6E72)
    private static let regionLand = RGB(hex: 0x33475A)
    private static let modeColors: [String: RGB] = [
        "road": RGB(hex: 0xE0A040), "truck": RGB(hex: 0xE0A040), "rail": RGB(hex: 0x7FD3FF), "sea": RGB(hex: 0x9AF0E0),
    ]
    private static let modeSpeed: [String: Double] = ["road": 40, "truck": 40, "rail": 55, "sea": 22]

    private func clamp(_ x: Double) -> Double { min(max(x, 0), 1) }

    // MARK: Still layer

    /// Space, the lit globe, land, the raised scenario, night, buildings, cities and labels. The map
    /// draws the ground and the structures as two layers with the traffic and clouds between them.
    func drawStatic(in context: inout GraphicsContext, geometry g: MapGeometry, phase: MapPhase = .all) {
        let p = g.projection
        let t = p.t
        let size = g.size
        let zoom = camera.zoom
        let light = sun ?? MapSun.fixed(p)
        let lit = look.lighting > 0
        let whole = Path(CGRect(origin: .zero, size: size))
        let detail = look.detail < 2 ? 0 : clamp((zoom - 7) / 5)
        let raised = look.raised(t)
        // How far up the base of the raised scenario sits on screen, for lines drawn on it.
        let plinth = raised ? g.plinth : 0
        let up = Double(p.lift(g.center, plinth).y - p.lift(g.center, 0).y)
        let ground: [String: Double]? = raised ? heights : nil
        if phase == .sky {
            drawSky(in: &context, geometry: g, light: light, whole: whole)
            return
        }
        if phase != .structures {
            // With the satellite Earth the sky is its own layer under the Earth's.
            if !look.earth { drawSky(in: &context, geometry: g, light: light, whole: whole) }
            drawGround(in: &context, geometry: g, light: light, whole: whole, detail: detail, raised: raised, up: up)
        }
        if phase == .ground { return }
        var labels = context
        labels.addFilter(.shadow(color: .black.opacity(0.85), radius: 1.5, x: 0, y: 1))
        if zoom > 3 {
            let close = zoom > 14
            let nightNow = sun != nil
            // Each big city on screen is a voxel city block sized by its population, a step smaller than the
            // player's buildings so those stay the stars, shaped by what the city is known for (a port, a steel
            // town, a car town); then rivals' plants a step smaller again, then the player's divisions.
            // Voxels in true 3D: points a voxel for the player's buildings, a step down for the rest.
            let unit = g.unit(WorldScale.playerVoxel, displayScale: displayScale)
            let e = p.east
            let planner = labelPlanner ?? LabelPlanner()
            let box = VoxelModels.Bounds()
            var cityBox: [Int: CGRect] = [:]
            // Buildings and badges are reserved first, so a label that would cross one is the label that goes.
            planner.begin()
            defer { planner.end() }
            func reserved() { if let r = box.rect { planner.reserve(r) } }
            // The diorama up close: trees, fields, barns, rocks and the houses round each city.
            if look.countryside && look.buildings >= 2 && look.earth && g.closeUp && t > 0.5 {
                Countryside.draw(in: &context, projection: p, map: map, week: week, unit: VoxelModels.unit(g.scale, displayScale: displayScale) * 0.6,
                                 lift: plinth, economyPhase: economyPhase, size: size)
            }
            if look.buildings >= 2 && g.closeUp && t > 0.5 {
                // The economy shows in the cities: cranes rise in a boom, lights go out in a recession.
                let tone: VoxelModels.Tone = economyPhase == "recession" || economyPhase == "slowdown" ? .dark : .plain
                let cityUnit = g.unit(WorldScale.cityVoxel, displayScale: displayScale)
                let ink = VoxelModels.inked(cityUnit, displayScale: displayScale)
                for (i, block) in g.cityBlocks(ground).enumerated() {
                    let at = CGPoint(x: block.gx, y: block.gy)
                    var kind = CityKinds.of(map.cities[block.index].name)
                    if !VoxelModels.has(VoxelModels.sized(kind, plants: block.plants)) { kind = "city" }
                    box.reset()
                    VoxelModels.draw(VoxelModels.sized(kind, plants: block.plants), projection: p, ground: at,
                                     floor: block.floor, unit: cityUnit, tone: tone, ink: ink, bounds: box, in: &context)
                    if let r = box.rect { cityBox[block.index] = r }
                    reserved()
                    if (economyPhase == "boom" || economyPhase == "expansion" && i % 3 == 0) && i < 12 {
                        let k = 9 * cityUnit
                        box.reset()
                        VoxelModels.draw(VoxelModels.sized("crane", plants: economyPhase == "boom" ? 8 : 2), projection: p,
                                         ground: CGPoint(x: block.gx + e.dx * k, y: block.gy + e.dy * k), floor: block.floor,
                                         unit: cityUnit, ink: ink, bounds: box, in: &context)
                        reserved()
                    }
                }
            }
            if look.buildings > 0 && g.closeUp && t > 0.5 {
                // Rivals' plants first, in their sector's colour with a dark roof and about 60 percent of the
                // player's size, so the player's own stand over them.
                let rivalUnit = g.unit(WorldScale.rivalVoxel, displayScale: displayScale)
                let rivalInk = VoxelModels.inked(rivalUnit, displayScale: displayScale)
                let rivalStates = PlantState.of(plants.filter { !$0.player }, byFirm: true)
                let rivalRoof = RGB(hex: 0x2A2F36)
                for b in look.rivals ? g.rivalBuildings(plants, ground) : [] {
                    let at = CGPoint(x: b.gx, y: b.gy)
                    let state = rivalStates[b.key]
                    let name = state == .site ? VoxelModels.sized("site", plants: b.plants) : VoxelModels.sized(b.sector, plants: b.plants)
                    let tone: VoxelModels.Tone = state == .site ? .muted : state == .mothballed ? .dark : .sector(FreightLook.sectorColor(b.sector))
                    box.reset()
                    VoxelModels.draw(name, projection: p, ground: at, floor: b.floor, unit: rivalUnit, tone: tone, ink: rivalInk,
                                     roof: state == .site || state == .mothballed ? nil : rivalRoof, bounds: box, in: &context)
                    reserved()
                }
                let ink = VoxelModels.inked(unit, displayScale: displayScale)
                let states = PlantState.of(plants.filter(\.player))
                for b in g.buildings(divisions, ground) {
                    // Each division is a voxel building for its industry, shown as it stands: a
                    // building site while its plants go up, a crane beside it while one is added or
                    // retooled, dark when mothballed, a gold flag when just taken over. A plain box
                    // if the model is missing. The player's roofs are gold.
                    let at = CGPoint(x: b.gx, y: b.gy)
                    let base = p.lift(at, b.floor)
                    let state = states[b.key] ?? .running
                    let spriteTop: CGFloat?
                    box.reset()
                    switch state {
                    case .site:
                        spriteTop = VoxelModels.draw(VoxelModels.sized("site", plants: b.plants), projection: p, ground: at,
                                                     floor: b.floor, unit: unit, ink: ink, bounds: box, in: &context)
                    case .mothballed:
                        spriteTop = VoxelModels.draw(VoxelModels.sized(b.sector, plants: b.plants), projection: p, ground: at,
                                                     floor: b.floor, unit: unit, tone: .dark, ink: ink, bounds: box, in: &context)
                    default:
                        spriteTop = VoxelModels.draw(VoxelModels.sized(b.sector, plants: b.plants), projection: p, ground: at,
                                                     floor: b.floor, unit: unit, ink: ink, roof: RGB.gold, bounds: box, in: &context)
                    }
                    reserved()
                    if state == .growing {
                        let k = 9 * unit
                        box.reset()
                        VoxelModels.draw("crane/0", projection: p, ground: CGPoint(x: b.gx + e.dx * k, y: b.gy + e.dy * k),
                                         floor: b.floor, unit: unit, ink: ink, bounds: box, in: &context)
                        reserved()
                    }
                    if acquired.contains(b.key), let spriteTop {
                        MapScene.drawFlag(in: &context, at: CGPoint(x: base.x + 4 * unit, y: spriteTop - 2 / max(displayScale, 1)),
                                          unit: unit)
                    }
                    let topAt: CGPoint
                    if let spriteTop {
                        topAt = CGPoint(x: base.x, y: spriteTop)
                    } else {
                        let windows = nightNow && light.facing(b.geo) < 0 ? 0.85 : look.lighting == 2 ? 0.30 : 0
                        drawBox(&context, projection: p, gx: b.gx, gy: b.gy, half: b.half, floor: b.floor, top: b.top,
                                color: MapScene.sectorRGB(b.sector), windows: windows, outline: Theme.gold.opacity(0.9))
                        topAt = p.lift(CGPoint(x: b.gx, y: b.gy), b.top)
                    }
                    let badge = CGPoint(x: topAt.x, y: topAt.y - 12)
                    let count = context.resolve(Text("\(b.plants)").font(.system(size: 10, weight: .bold)).foregroundStyle(Theme.ink))
                    let measured = count.measure(in: CGSize(width: 200, height: 50))
                    let radius = max(measured.width, measured.height) / 2 + 3
                    context.fill(disc(badge, radius), with: .color(Theme.gold))
                    context.draw(count, at: badge)
                    planner.reserve(CGRect(x: badge.x - radius, y: badge.y - radius, width: radius * 2, height: radius * 2))
                }
            } else if t > 0.9 && g.zoomRatio >= 0.7 {
                // Around the scenario's framing: a small badge per division. Further out, nothing.
                drawDivisions(in: &context, projection: p, lift: plinth)
            }
            if look.cityLights && lit && (sun != nil || t < 1) {
                GlobeLighting.cityLights(in: &context, cities: map.cities, projection: p, sun: light, lift: plinth, scale: 1)
            }
            // City names over every building. Sentence case, one weight. A name sits beside its block (right,
            // left, above, below); one that fits nowhere is dropped, and the biggest cities choose first.
            // A city above the town size always keeps its name while its block is on screen: it chooses before the
            // smaller ones, tries further slots with a leader line, and is never dropped for a region name.
            var named = look.labels >= 1 ? Array(map.cities.enumerated().prefix(close ? 40 : 14)) : []
            if look.labels >= 1 {
                let listed = Set(named.map(\.offset))
                for (index, city) in map.cities.enumerated() where !listed.contains(index) && cityBox[index] != nil
                    && Int(city.population) >= LabelPlanner.townPopulation { named.append((offset: index, element: city)) }
            }
            named.sort { $0.element.population > $1.element.population }
            for (index, city) in named {
                if !close && city.country != "USA" { continue }
                let geo = GeoPoint(e4: city.x, city.y)
                guard let at = p.at(lon: geo.lon, lat: geo.lat, height: g.cityFloor(index, ground)) else { continue }
                context.fill(disc(at, 2.5), with: .color(Theme.primaryText.opacity(0.85)))
                let text = context.resolve(Text(city.name).font(.system(size: 10, weight: .medium)).foregroundStyle(Theme.primaryText.opacity(0.9)))
                let size = text.measure(in: CGSize(width: 400, height: 60))
                let slots: [CGPoint]
                if let b = cityBox[index] {
                    let mid = b.midY - size.height / 2
                    let cx = b.midX - size.width / 2
                    slots = [CGPoint(x: b.maxX + 3, y: mid), CGPoint(x: b.minX - 3 - size.width, y: mid),
                             CGPoint(x: cx, y: b.minY - 3 - size.height), CGPoint(x: cx, y: b.maxY + 3)]
                } else {
                    slots = [CGPoint(x: at.x + 6, y: at.y - size.height / 2), CGPoint(x: at.x - 6 - size.width, y: at.y - size.height / 2),
                             CGPoint(x: at.x - size.width / 2, y: at.y - size.height - 5), CGPoint(x: at.x - size.width / 2, y: at.y + 5)]
                }
                if let b = cityBox[index], Int(city.population) >= LabelPlanner.townPopulation {
                    let put = planner.placeCity("c:\(city.name)", size: size, slots: slots, center: CGPoint(x: b.midX, y: b.midY),
                                                reach: max(b.width, b.height) / 2 + 6, view: size)
                    if put.leader {
                        // One point line from the name's nearest edge to the block it belongs to.
                        var line = Path()
                        line.move(to: CGPoint(x: min(max(b.midX, put.corner.x), put.corner.x + size.width),
                                              y: min(max(b.midY, put.corner.y), put.corner.y + size.height)))
                        line.addLine(to: CGPoint(x: b.midX, y: b.midY))
                        context.stroke(line, with: .color(Theme.primaryText.opacity(0.7)), lineWidth: 1)
                    }
                    labels.draw(text, at: put.corner, anchor: .topLeading)
                } else if let corner = planner.place("c:\(city.name)", size: size, slots: slots) {
                    labels.draw(text, at: corner, anchor: .topLeading)
                }
            }
            // Region names last and lowest in priority: small caps with tracking, and only these.
            if zoom > 14 && look.labels >= 2 {
                for region in map.regions {
                    guard let anchor = map.anchor(of: region.code),
                          let at = p.at(lon: anchor.lon, lat: anchor.lat, height: g.regionLift(region.code, ground)) else { continue }
                    let text = context.resolve(Text(region.name.uppercased()).font(.system(size: 9, weight: .medium)).tracking(0.8)
                        .foregroundStyle(Theme.secondary.opacity(0.8)))
                    let size = text.measure(in: CGSize(width: 400, height: 60))
                    let slots = [CGPoint(x: at.x - size.width / 2, y: at.y + 30), CGPoint(x: at.x - size.width / 2, y: at.y - 30 - size.height),
                                 CGPoint(x: at.x - size.width / 2, y: at.y + 34 + size.height)]
                    if let corner = planner.place("r:\(region.code)", size: size, slots: slots) {
                        labels.draw(text, at: corner, anchor: .topLeading)
                    }
                }
            }
        } else if look.cityLights && lit && t < 1 {
            GlobeLighting.cityLights(in: &context, cities: map.cities, projection: p, sun: light, lift: 0, scale: 0.7)
        }
    }

    /// Space, the stars, the atmosphere and the ocean: what lies under the land.
    private func drawSky(in context: inout GraphicsContext, geometry g: MapGeometry, light: MapSun, whole: Path) {
        let p = g.projection
        let t = p.t
        let size = g.size
        if space {
            context.fill(whole, with: .color(MapScene.spaceColor.color()))
            if look.lighting == 2 && t < 1 {
                StarField.shared.draw(in: &context, size: size, seconds: 0, drift: 0, twinkle: false)
            }
        }
        if look.lighting > 0 && t < 1 {
            g.backLight.draw(in: &context, size: size, displayScale: displayScale, key: [light.lon, light.lat, look.lighting]) { layer, _ in
                GlobeLighting.atmosphere(in: &layer, projection: p, sun: light, lighting: look.lighting)
            }
        }
        GlobeLighting.back(in: &context, projection: p, sun: light, lighting: look.lighting, size: size)
        if t < 1 { context.stroke(g.graticule, with: .color(Theme.primaryText.opacity(0.06 * (1 - t))), lineWidth: 0.6) }
    }

    /// The ground: everything that lies flat, under the traffic and the buildings.
    private func drawGround(in context: inout GraphicsContext, geometry g: MapGeometry, light: MapSun, whole: Path,
                            detail: Double, raised: Bool, up: Double) {
        let p = g.projection
        let t = p.t
        let size = g.size
        let zoom = camera.zoom
        let lit = look.lighting > 0
        let lightKey: [AnyHashable] = [light.lon, light.lat, look.lighting]
        // Outside the scenario the world is muted: climate-tinted land on the globe that greys as
        // the map flattens, grey borders and roads. The scenario's regions are drawn over it. The
        // satellite Earth, when on, is its own layer below and stands in for the painted land.
        if look.earth {
            // Nothing painted: the real ground shows through.
        } else if lit {
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

        let water = Theme.water
        let even = FillStyle(eoFill: true)
        let highways = look.detail == 0 || !look.interstates || t <= 0.5 ? 0 : smoothstep(0.85, 1.2, g.zoomRatio)
        let roadWidth = 1.6 + 1.2 * min(g.scale, 2.5)
        // The centre line's dashes are cut on the ground and stroked solid: a screen dash pattern
        // would crawl along the road as the camera moves.
        func strokeRoads(_ path: Path, dashes: Path?, in layer: inout GraphicsContext) {
            layer.stroke(path, with: .color(RGB(hex: 0x1C2228).color(0.9 * highways)),
                         style: StrokeStyle(lineWidth: roadWidth + 2, lineCap: .square))
            layer.stroke(path, with: .color(RGB(hex: 0x6E757D).color(highways)), style: StrokeStyle(lineWidth: roadWidth, lineCap: .square))
            if g.closeUp, let dashes {
                layer.stroke(dashes, with: .color(Theme.gold.opacity(0.85 * highways)), style: StrokeStyle(lineWidth: 1, lineCap: .butt))
            }
        }
        if raised {
            let tileRoads = highways > 0 ? g.tileRoads(heights) : [:]
            let tileDashes = highways > 0 && g.closeUp ? g.tileDashes(heights) : [:]
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
                // The tile's own interstates on its top, before any nearer tile is drawn over them.
                if let roads = tileRoads[solid.code] {
                    var onTile = context
                    onTile.clip(to: solid.top, style: even)
                    strokeRoads(roads, dashes: tileDashes[solid.code], in: &onTile)
                }
            }
            if detail > 0 {
                var onTop = context
                onTop.translateBy(x: 0, y: up)
                onTop.stroke(g.counties, with: .color(Theme.stroke.opacity(0.45 * detail)), lineWidth: 0.5)
            }
        } else {
            for region in g.regionPaths {
                context.fill(region.path, with: .color(MapScene.regionLand.color(look.earth ? 0.32 : 1)), style: even)
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
        // Interstates over the regions and their shading, under the traffic and the buildings: an
        // 8-bit road with a dark edge, grey asphalt and, up close, a dashed gold centre line. On
        // raised tiles each tile lays its own above; flat, they are projected on the ground itself.
        // The real railways once close, so trains have track to run on: a thin grey rail over dark
        // sleepers cut on the ground, on the ground or the scenario's base.
        if g.closeUp && t > 0.5 && look.traffic > 0 && !g.railroadLines.isEmpty {
            let lift = raised ? g.plinth : 0
            context.stroke(g.sleepers(lift: lift), with: .color(RGB(hex: 0x3A3028).color(0.9)), style: StrokeStyle(lineWidth: 3, lineCap: .butt))
            context.stroke(g.railroads(lift: lift), with: .color(RGB(hex: 0x9AA1A8).color(0.85)), lineWidth: 1)
        }
        if highways > 0 && !raised {
            strokeRoads(g.interstates, dashes: g.closeUp ? g.centreLines : nil, in: &context)
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
        // Freight routes show only in the Freight mode, over a darkened map so they read clearly.
        if freightLayer && !lanes.isEmpty {
            context.fill(whole, with: .color(.black.opacity(0.35)))
            drawLanes(in: &context, projection: p, plinth: raised ? g.plinth : 0)
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

    /// A small gold pennant on a pole, planted on a building the player has just taken over.
    /// `unit` is one sprite pixel in points.
    static func drawFlag(in context: inout GraphicsContext, at: CGPoint, unit k: Double) {
        context.fill(Path(CGRect(x: at.x - k, y: at.y - 12 * k, width: 2 * k, height: 12 * k)), with: .color(Theme.ink))
        context.fill(Path(CGRect(x: at.x - k / 2, y: at.y - 12 * k, width: k, height: 12 * k)), with: .color(RGB(hex: 0xC8CED4).color()))
        var pennant = Path()
        pennant.move(to: CGPoint(x: at.x + k / 2, y: at.y - 12 * k))
        pennant.addLine(to: CGPoint(x: at.x + 8 * k, y: at.y - 9.5 * k))
        pennant.addLine(to: CGPoint(x: at.x + k / 2, y: at.y - 7 * k))
        pennant.closeSubpath()
        context.fill(pennant, with: .color(Theme.gold))
        context.stroke(pennant, with: .color(Theme.ink), lineWidth: 1)
    }

    static func sectorRGB(_ sector: String) -> RGB {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        guard UIColor(Sector.color(sector)).getRed(&red, green: &green, blue: &blue, alpha: &alpha) else { return regionLand }
        return RGB(Double(red), Double(green), Double(blue))
    }

    /// The busiest lanes as arcs between their hubs, each with a stripe per company.
    private func drawLanes(in context: inout GraphicsContext, projection p: MapProjection, plinth: Double) {
        FreightArcs.draw(lanes, map: map, projection: p, plinth: plinth, focus: selected, picked: pickedLane, in: &context)
    }

    /// The lane drawn nearest a tap, within a finger's width.
    func laneAt(_ tap: CGPoint, geometry g: MapGeometry) -> LaneDto? {
        FreightArcs.at(tap, lanes: lanes, map: map, projection: g.projection,
                       plinth: look.raised(g.projection.t) ? g.plinth : 0)
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

    /// Everything that moves on the map, for one frame: the traffic (under the buildings), the
    /// effects (over them), or both. `seconds` is game time.
    func drawOverlay(in context: inout GraphicsContext, geometry g: MapGeometry, seconds: Double,
                     part: OverlayPart = .all) {
        let p = g.projection
        let o = g.overlay(lanes: lanes, divisions: divisions, selected: selected, heights: heights, look: look,
                          freightLayer: freightLayer, displayScale: displayScale, firmSector: FreightLook.firmSectors(plants))
        let zoom = camera.zoom
        let traffic = part != .effects
        let effects = part != .traffic
        // Up close the freight, cars and planes are little voxel vehicles; further out, dots.
        var vehicles: [VehicleSpot]? = o.closeUp && traffic ? [] : nil
        // Freight along the lanes, both ways, at kilometres a second of game time. In the Freight
        // mode the arcs tell it; moving freight would only clutter them.
        if traffic && look.traffic > 0 && !freightLayer {
            let per = look.traffic >= 2 ? 9.0 : 4.0
            var glow = context
            glow.blendMode = .plusLighter
            var drawn = 0
            laneLoop: for lane in o.lanes {
                // Trucks and trains run only where a real road or railway carries them; a lane
                // with no route shows nothing rather than a line across country.
                if lane.mode != "sea" && !lane.routed { continue }
                // Up close a lane holds only as many vehicles as keep two vehicle lengths between neighbours.
                let vehicleLength = o.vehicleUnit * (lane.mode == "rail" ? 21 : lane.mode == "sea" ? 12 : 7)
                var count = 1 + Int(lane.weight * per)
                if vehicles != nil { count = min(count, FreightLook.maxPerLane(laneLength: lane.length, vehicleLength: vehicleLength)) }
                let color = lane.owner
                let speed = MapScene.modeSpeed[lane.mode] ?? 40
                var cores = Path()
                for k in 0..<count {
                    for dir in 0..<2 {
                        if drawn >= 600 { break laneLoop }
                        var f = fract(seconds * speed / lane.km + lane.seed + Double(k) / Double(count) + Double(dir) * 0.37)
                        if dir == 1 { f = 1 - f }
                        if vehicles != nil {
                            if vehicles!.count >= MapScene.maxFreightVehicles { break laneLoop }
                            MapScene.laneVehicles(lane, f: f, forward: dir == 0, unit: o.vehicleUnit, into: &vehicles!)
                            drawn += 1
                            continue
                        }
                        if lane.mode == "sea", let water = lane.water, !water[Int((f * Double(LaneWater.samples - 1)).rounded())] {
                            continue
                        }
                        let at = p.lift(lane.at(f), o.plinth + 2)
                        stamp(&glow, .glow, at: at, radius: 4, color: color, alpha: 0.35)
                        cores.addPath(disc(at, 1.6))
                        drawn += 1
                    }
                }
                context.fill(cores, with: .color(color.color()))
            }
        }
        // Cars on the highways once close, each direction in its own lane. Spacing and speed are on
        // the ground, in degrees, so cars hold their pace and places while the camera moves; closer
        // in they simply look further apart and faster. The Freight mode hides them with the freight.
        if traffic && look.traffic > 0 && look.detail >= 2 && zoom > 9 && !o.roads.isEmpty && !freightLayer {
            // Close in, never nearer than two car lengths (a car is four voxels long) however far in the camera is.
            let spacing = vehicles != nil ? max(look.traffic >= 2 ? 0.12 : 0.24, 8 * o.vehicleUnit / max(p.r * .pi / 180, 1e-3))
                : look.traffic >= 2 ? 0.035 : 0.07
            let pace = 0.012
            let step = 3 / max(p.r * .pi / 180, 1e-3)
            let detailFade = clamp((zoom - 7) / 5)
            let fade = detailFade * clamp((zoom - 9) / 3)
            var heads = Path()
            var tails = Path()
            var cars = 0
            roadLoop: for road in o.roads {
                let n = max(Int(road.total / spacing), 1)
                for k in 0..<n {
                    if cars >= 400 || (vehicles?.count ?? 0) >= MapScene.maxVehicles { break roadLoop }
                    let forward = k % 2 == 0
                    var d = (Double(k) * spacing + (forward ? 1 : -1) * seconds * pace).truncatingRemainder(dividingBy: road.total)
                    if d < 0 { d += road.total }
                    let sign: CGFloat = forward ? 1 : -1
                    if vehicles != nil {
                        // On the ground, so the car heads along the road whatever the camera does.
                        let at = road.ground(at: d)
                        let ahead = road.ground(at: min(max(d + (forward ? step : -step), 0), road.total))
                        let dx = ahead.x - at.x
                        let dy = ahead.y - at.y
                        let len = max(hypot(dx, dy), 1e-3)
                        if len > 0.01 && fade > 0.5 {
                            let side = CGFloat(2.4 * o.vehicleUnit) * sign
                            vehicles!.append(VehicleSpot(type: "car", mine: false,
                                                         ground: CGPoint(x: at.x - dy / len * side, y: at.y + dx / len * side),
                                                         dir: CGVector(dx: dx, dy: dy), tone: .livery(FreightLook.outside)))
                        }
                    } else {
                        let at = road.point(at: d)
                        let ahead = road.point(at: min(max(d + (forward ? step : -step), 0), road.total))
                        let dx = ahead.x - at.x
                        let dy = ahead.y - at.y
                        let len = max(hypot(dx, dy), 1e-3)
                        let sx = -dy / len * 1.6 * sign
                        let sy = dx / len * 1.6 * sign
                        heads.addPath(disc(CGPoint(x: ahead.x + sx, y: ahead.y + sy), 1.2))
                        tails.addPath(disc(CGPoint(x: at.x + sx, y: at.y + sy), 1.1))
                    }
                    cars += 1
                }
            }
            context.fill(heads, with: .color(RGB(hex: 0xFFF4D8).color(0.95 * fade)))
            context.fill(tails, with: .color(RGB(hex: 0xFF4A3A).color(0.85 * fade)))
        }
        if let vehicles {
            // Nearer vehicles over farther ones, each a 3D model turned along its road.
            let ordered = vehicles.map { (depth: p.lift($0.ground, 0).y, spot: $0) }.sorted { $0.depth < $1.depth }
            for v in ordered { drawVehicle(v.spot, overlay: o, projection: p, in: &context) }
        }
        if !effects { return }
        // Rooftop lights and smoke. No rings or pulses: they read as cheap.
        if look.buildings > 0 {
            for b in o.buildings {
                // A steady aircraft warning light on every rooftop.
                context.fill(disc(b.top, 1.3), with: .color(RGB(hex: 0xFF5A4A).color()))
                if b.smoke && look.smoke {
                    for k in 0..<3 {
                        let f = fract(seconds / 3 + Double(k) / 3 + b.seed)
                        let at = CGPoint(x: b.top.x + 4 * sin(f * 5 + b.seed * 9), y: b.top.y - 30 * f * o.scale)
                        stamp(&context, .cloud, at: at, radius: (2 + 5 * f) * o.scale, color: RGB(hex: 0x9AA3AB), alpha: 0.55 * (1 - f))
                    }
                }
            }
        }
        // Planes between the big cities, high over the map, trailing contrails with shadows below.
        // Hidden in the Freight mode with the rest of the moving traffic.
        if look.traffic > 0 && o.airports.count >= 2 && !freightLayer {
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
                if o.closeUp {
                    drawCargoPlane(in: &context, from: a, to: b, f: f, fade: fade, overlay: o, projection: p)
                    continue
                }
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
        // The selected region: a steady gold outline.
        if let top = o.selectedTop { context.stroke(top, with: .color(Theme.gold), lineWidth: 2.5) }
    }

    /// Sprites a frame at most: freight first, then the highway cars fill what is left.
    static let maxVehicles = 250
    static let maxFreightVehicles = 190

    /// The vehicles for one load on a lane at fraction `f`: a truck, a ship, or a train of a
    /// locomotive and two wagons, on the untilted ground. Ships keep to water.
    static func laneVehicles(_ lane: LaneTrack, f: Double, forward: Bool, unit: Double, into out: inout [VehicleSpot]) {
        let sign = forward ? 1.0 : -1.0
        let step = 2 / max(lane.length, 1)
        let at = lane.at(f)
        let ahead = lane.at(min(max(f + sign * step, 0), 1))
        let behind = lane.at(min(max(f - sign * step, 0), 1))
        let dir = CGVector(dx: ahead.x - behind.x, dy: ahead.y - behind.y)
        let dl = hypot(Double(dir.dx), Double(dir.dy))
        guard dl >= 1e-3 else { return }
        let ux = Double(dir.dx) / dl
        let uy = Double(dir.dy) / dl
        // To the right of the direction of travel.
        let rx = -uy * sign
        let ry = ux * sign
        // A straight lane between two hubs can cross a lake; a routed one keeps to its road.
        let wet: Bool? = lane.routed ? false : lane.water.map { $0[Int((f * Double(LaneWater.samples - 1)).rounded())] }
        let tone: VoxelModels.Tone = lane.mine ? .plain : .livery(lane.owner)
        switch lane.mode {
        case "rail":
            guard wet != true else { return }
            // Wagons trail the locomotive along the track, spaced by a vehicle's length on the ground.
            let gap = 7 * unit / max(lane.length, 1)
            out.append(VehicleSpot(type: "loco", mine: lane.mine, ground: at, dir: dir, tone: tone))
            for w in 1...2 {
                let t = f - sign * gap * Double(w)
                if t >= 0 && t <= 1 { out.append(VehicleSpot(type: "wagon", mine: lane.mine, ground: lane.at(t), dir: dir, tone: tone)) }
            }
        case "sea":
            // Ships keep to their own side of the channel.
            guard wet != false else { return }
            out.append(VehicleSpot(type: "ship", mine: lane.mine,
                                   ground: CGPoint(x: Double(at.x) + rx * 4 * unit, y: Double(at.y) + ry * 4 * unit), dir: dir, tone: tone))
        default:
            guard wet != true else { return }
            out.append(VehicleSpot(type: "truck", mine: lane.mine,
                                   ground: CGPoint(x: Double(at.x) + rx * 2.2 * unit, y: Double(at.y) + ry * 2.2 * unit), dir: dir, tone: tone))
        }
    }

    /// A vehicle as a 3D voxel model turned along its direction of travel on the ground.
    private func drawVehicle(_ v: VehicleSpot, overlay o: OverlayGeometry, projection p: MapProjection, alpha: Double = 1,
                             in context: inout GraphicsContext) {
        let name = "\(v.type)/\(v.mine ? "player" : "neutral")"
        VoxelModels.draw(name, projection: p, ground: v.ground, floor: o.plinth + v.height, unit: o.vehicleUnit,
                         heading: VoxelModels.heading(v.dir, projection: p), tone: v.tone,
                         ink: VoxelModels.inked(o.vehicleUnit, displayScale: displayScale), alpha: alpha, in: &context)
    }

    /// A cargo plane up close: the voxel plane high over the map, its shadow on the ground and a
    /// short faint contrail.
    private func drawCargoPlane(in context: inout GraphicsContext, from a: CGPoint, to b: CGPoint, f: Double, fade: Double,
                                overlay o: OverlayGeometry, projection p: MapProjection) {
        func flat(_ t: Double) -> CGPoint { CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t) }
        func height(_ t: Double) -> Double { (40 + 50 * sin(.pi * t)) * o.scale }
        func air(_ t: Double) -> CGPoint { p.lift(flat(t), o.plinth + height(t)) }
        for trail in stride(from: 6, through: 1, by: -1) {
            context.fill(disc(air(max(f - Double(trail) * 0.005, 0)), 1),
                         with: .color(.white.opacity(0.12 * fade * (1 - Double(trail) / 7))))
        }
        let shadow = p.lift(flat(f), o.plinth)
        let k = o.vehicleUnit
        context.fill(Path(ellipseIn: CGRect(x: shadow.x - 4 * k, y: shadow.y - 1.5 * k, width: 8 * k, height: 3 * k)),
                     with: .color(.black.opacity(0.22 * fade)))
        drawVehicle(VehicleSpot(type: "plane", mine: false, ground: flat(f), dir: CGVector(dx: b.x - a.x, dy: b.y - a.y),
                                height: height(f)), overlay: o, projection: p, alpha: fade, in: &context)
    }

    /// With nothing moving, only the selected region's outline.
    func drawSelection(in context: inout GraphicsContext, geometry g: MapGeometry) {
        let o = g.overlay(lanes: lanes, divisions: divisions, selected: selected, heights: heights, look: look,
                          freightLayer: freightLayer, displayScale: displayScale, firmSector: FreightLook.firmSectors(plants))
        if let top = o.selectedTop { context.stroke(top, with: .color(Theme.gold), lineWidth: 2.5) }
    }
}

/// The world map: a globe when zoomed out that flattens into a map as the player zooms in on the
/// scenario. Drag to turn or pan, pinch or use the buttons to zoom, double-tap to fly between the
/// globe and the scenario, tap a region to select it. Shading, markers and selection come from
/// the caller. A still layer holds the land, regions and buildings; a moving layer over it holds
/// traffic, planes and smoke, and only runs while something on it moves.
struct WorldMapView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.displayScale) private var displayScale
    @AppStorage(GraphicsKeys.mapDetail) private var detailLevel = MapDetailLevel.high.rawValue
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @AppStorage(GraphicsKeys.preset) private var preset = GraphicsPreset.high.rawValue
    @AppStorage(GraphicsKeys.tilt) private var tiltSetting = 1
    @AppStorage(GraphicsKeys.extrude) private var extrude = true
    @AppStorage(GraphicsKeys.lighting) private var lighting = 2
    @AppStorage(GraphicsKeys.dayNight) private var dayNight = true
    @AppStorage(GraphicsKeys.cityLights) private var cityLights = true
    @AppStorage(GraphicsKeys.earth) private var earth = true
    @AppStorage(GraphicsKeys.clouds) private var clouds = 1
    @AppStorage(GraphicsKeys.traffic) private var traffic = 1
    @AppStorage(GraphicsKeys.buildings) private var buildings = 2
    @AppStorage(GraphicsKeys.frameRate) private var frameRate = 60
    @AppStorage(GraphicsKeys.showFps) private var showFps = false
    @AppStorage(GraphicsKeys.countryside) private var countryside = true
    @AppStorage(GraphicsKeys.rivals) private var rivals = true
    @AppStorage(GraphicsKeys.interstates) private var interstates = true
    @AppStorage(GraphicsKeys.seasons) private var seasons = true
    @AppStorage(GraphicsKeys.labels) private var labels = 2
    let shading: [String: Double]
    let layer: MapLayer
    let divisions: [DivisionMarker]
    var lanes: [LaneDto] = []
    /// The lane picked on the Freight layer, and what to do when a lane is tapped.
    var pickedLane: String?
    var onLane: ((String?) -> Void)?
    /// The career's region codes, which say which scenario's map to draw.
    var regionCodes: [String] = []
    /// The game's week count, for how long a takeover flag flies.
    var week: UInt32 = 0
    /// The week's label, such as "2000 W10", which sets the season.
    var weekLabel: String?
    /// Every plant on the map, the player's and rivals'.
    var plants: [MapPlantDto] = []
    /// The economy's phase, shown in the cities.
    var economyPhase: String?
    /// Puts the zoom controls at the top right, for the desktop shell where the tutorial card
    /// sits in the bottom corner.
    var controlsAtTop = false
    /// Weeks a second the game runs at, 0 while paused: the world's clock follows it.
    var gameSpeed: Double = 0
    @Binding var selected: String?

    @State private var camera: MapCamera?
    @State private var lastDrag: CGSize = .zero
    @State private var lastPinch: CGFloat = 1
    @State private var lastTurn: Angle = .zero
    @State private var flight: Task<Void, Never>?
    @State private var geometry = MapGeometryCache()
    @State private var ease = HeightEase()
    @State private var easing = false
    @State private var meter = FrameMeter()
    @State private var world = WorldClock()
    @State private var takeovers = TakeoverLog()
    @State private var labelPlanner = LabelPlanner()
    @State private var acquired: Set<String> = []
    @State private var countrysideReady = 0

    private var map: WorldMap { WorldMap.forRegions(regionCodes) }

    private var look: MapLook {
        MapLook(
            detail: detailLevel, tilt: GraphicsSettings.pitch(forTilt: tiltSetting), extrude: extrude,
            lighting: lighting, dayNight: dayNight, cityLights: cityLights, clouds: clouds, traffic: traffic,
            buildings: buildings, frameRate: frameRate, animations: animations, showFps: showFps,
            ultra: preset == GraphicsPreset.ultra.rawValue, earth: earth, countryside: countryside, rivals: rivals,
            interstates: interstates, labels: labels
        )
    }

    /// The week of the year for the seasons: midsummer with seasons off.
    private var seasonWeek: Int { seasons ? Earth.week(of: weekLabel) : 28 }

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
            // Layers, bottom to top: the ground, traffic on it, clouds, buildings and labels, then
            // effects. Traffic runs under the buildings; buildings stand clear of the clouds.
            // Moving things follow the world clock, which stops while the game is paused.
            let sunMoves = easing || (look.dayNight && clockAllowed && gameSpeed > 0)
            ZStack(alignment: .topLeading) {
                if look.earth {
                    // Space and the ocean, then the satellite Earth over them, under the rest of the ground.
                    TimelineView(.animation(minimumInterval: easing ? interval : 1.0 / 3.0, paused: !sunMoves)) { timeline in
                        Canvas { context, canvasSize in
                            let now = MapClock.seconds(timeline.date)
                            let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                            MainStalls.time("map.sky") { scene(camera: current, look: look, now: now).drawStatic(in: &context, geometry: g, phase: .sky) }
                        }
                        .drawingGroup()
                    }
                    EarthLayer(view: EarthView(camera: current, size: size, tilt: look.tilt, week: seasonWeek,
                                               bounds: map.playBounds, scenario: map.scenarioId))
                        .allowsHitTesting(false)
                }
                TimelineView(.animation(minimumInterval: easing ? interval : 1.0 / 3.0, paused: !sunMoves)) { timeline in
                    Canvas { context, canvasSize in
                        let now = MapClock.seconds(timeline.date)
                        let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                        MainStalls.time("map.ground") { scene(camera: current, look: look, now: now).drawStatic(in: &context, geometry: g, phase: .ground) }
                    }
                    .drawingGroup()
                }
                if look.moves {
                    TimelineView(.animation(minimumInterval: interval, paused: !running || gameSpeed <= 0)) { timeline in
                        // Redrawn when the lanes' routes arrive from the background.
                        let ready = countrysideReady
                        Canvas { context, canvasSize in
                            _ = ready
                            let now = MapClock.seconds(timeline.date)
                            let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                            MainStalls.time("map.traffic") {
                                scene(camera: current, look: look, now: now)
                                    .drawOverlay(in: &context, geometry: g, seconds: gameSeconds(now),
                                                 part: .traffic)
                            }
                        }
                        .drawingGroup()
                    }
                    .allowsHitTesting(false)
                }
                // Clouds drift a fraction of a degree a second, so their layer is redrawn eight
                // times a second rather than every frame; the camera still moves them at once. They
                // step aside while the Freight mode is open.
                if look.clouds > 0 && layer != .freight {
                    TimelineView(.animation(minimumInterval: 1.0 / 8, paused: !running || gameSpeed <= 0)) { timeline in
                        let game = floor(gameSeconds(MapClock.seconds(timeline.date)) * 8) / 8
                        let g = geometry.of(map: map, camera: current, size: size, tilt: look.tilt, detail: look.detail)
                        CloudLayer(sky: CloudSky(
                            camera: current, size: size, tilt: look.tilt,
                            sun: look.dayNight ? MapSun.moving(game) : MapSun.fixed(g.projection),
                            seconds: game, amount: look.clouds, bounds: map.playBounds,
                            altitude: 46 * g.scale * g.projection.t, zoom: current.zoom, shadows: look.lighting > 0,
                            scenario: map.scenarioId
                        ))
                    }
                    .allowsHitTesting(false)
                }
                TimelineView(.animation(minimumInterval: easing ? interval : 1.0 / 3.0, paused: !sunMoves)) { timeline in
                    // Redrawn once the countryside for the scenario is worked out.
                    let ready = countrysideReady
                    Canvas { context, canvasSize in
                        _ = ready
                        let now = MapClock.seconds(timeline.date)
                        let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                        MainStalls.time("map.structures") { scene(camera: current, look: look, now: now).drawStatic(in: &context, geometry: g, phase: .structures) }
                    }
                    .drawingGroup()
                }
                .allowsHitTesting(false)
                if look.moves || look.showFps || selected != nil {
                    TimelineView(.animation(minimumInterval: interval, paused: !running)) { timeline in
                        Canvas { context, canvasSize in
                            let now = MapClock.seconds(timeline.date)
                            let g = geometry.of(map: map, camera: current, size: canvasSize, tilt: look.tilt, detail: look.detail)
                            let moving = scene(camera: current, look: look, now: now)
                            if look.moves {
                                moving.drawOverlay(in: &context, geometry: g, seconds: gameSeconds(now),
                                                   part: .effects)
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
            // A right drag (a Mac, or an iPad with a mouse) turns the map.
            .background(SecondaryDrag { dx in
                flight?.cancel()
                camera = (camera ?? current).rotated(by: Double(dx) * 0.4)
            })
            .contentShape(Rectangle())
            .gesture(
                SimultaneousGesture(
                    SimultaneousGesture(
                        MagnifyGesture()
                            .onChanged { value in
                                flight?.cancel()
                                let factor = value.magnification / lastPinch
                                lastPinch = value.magnification
                                camera = (camera ?? current).zoomed(by: Double(factor))
                            }
                            .onEnded { _ in lastPinch = 1 },
                        // A two-finger twist turns the map; sprites and labels stay upright.
                        RotateGesture()
                            .onChanged { value in
                                flight?.cancel()
                                let turn = value.rotation - lastTurn
                                lastTurn = value.rotation
                                camera = (camera ?? current).rotated(by: turn.degrees)
                            }
                            .onEnded { _ in lastTurn = .zero }
                    ),
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
                if look.raised(g.projection.t) {
                    // Nearest tile first, on its raised top face.
                    hit = g.raised(tapped.heights).reversed().first { $0.top.contains(location, eoFill: true) }?.code
                }
                if hit == nil {
                    hit = map.regions.first { g.projection.contains($0.rings, location) }?.code
                }
                selected = hit == selected ? nil : hit
            }
            .overlay(alignment: .bottomLeading) {
                // What the colours mean for the active mode.
                if layer == .freight {
                    FreightLegend(lanes: lanes)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .frame(width: 190, alignment: .leading)
                        .pixelFrame(Theme.panel.opacity(0.94))
                        .padding(.leading, 12)
                        .padding(.bottom, controlsAtTop ? 24 : 40)
                        .allowsHitTesting(false)
                } else {
                    MapLegend(layer: layer)
                        .padding(.leading, 12)
                        .padding(.bottom, controlsAtTop ? 24 : 40)
                        .allowsHitTesting(false)
                }
            }
            .overlay(alignment: controlsAtTop ? .topTrailing : .bottomTrailing) {
                // Up top they sit below the layer chips.
                zoomControls(current: current, size: size)
                    .padding(.trailing, 12)
                    .padding(controlsAtTop ? .top : .bottom, controlsAtTop ? 56 : 40)
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("World map. The scenario's regions can be selected.")
            .accessibilityValue(selected.map { map.regionName($0) } ?? "No region selected")
            .accessibilityAction(named: "Show the whole globe") {
                fly(from: current, to: MapCamera(lon: current.lon, lat: current.lat, zoom: 1, tilt: current.tilt))
            }
            .accessibilityAction(named: "Show the scenario") { fly(from: current, to: MapCamera.scenario(in: size, map: map)) }
        }
        // Divisions the player has just taken over carry a flag for six weeks: a plant that was a
        // rival's last week and is the player's now.
        .onAppear { acquired = takeovers.update(plants, week: Int(week)) }
        .onReceive(NotificationCenter.default.publisher(for: Countryside.ready)) { _ in countrysideReady += 1 }
        .onReceive(NotificationCenter.default.publisher(for: VoxelModels.ready)) { _ in countrysideReady += 1 }
        .onReceive(NotificationCenter.default.publisher(for: Routes.ready)) { _ in countrysideReady += 1 }
        .onChange(of: plants) { _, new in acquired = takeovers.update(new, week: Int(week)) }
        .onChange(of: shading) { old, new in
            ease.retarget(from: old, to: new, now: MapClock.now, animate: animate && extrude)
            guard animate && extrude else { return }
            easing = true
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: 650_000_000)
                easing = false
            }
        }
    }

    /// The world's time at a wall-clock moment: still while paused, faster at higher speeds.
    private func gameSeconds(_ now: Double) -> Double {
        world.advance(wall: now, speed: clockAllowed ? gameSpeed : 0)
    }

    /// The scene for one camera and moment. The sun moves on game time; the height easing on the wall.
    private func scene(camera: MapCamera, look: MapLook, now: Double) -> MapScene {
        MapScene(
            map: map, camera: camera, look: look, tint: layer.tintRGB, shading: shading,
            heights: layer == .freight ? [:] : ease.value(at: now, target: shading),
            selected: selected, divisions: divisions, lanes: lanes, freightLayer: layer == .freight, pickedLane: pickedLane,
            sun: look.dayNight ? MapSun.moving(gameSeconds(now)) : nil, plants: plants, economyPhase: economyPhase,
            acquired: acquired, week: seasonWeek, displayScale: displayScale, labelPlanner: labelPlanner
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
            Divider().frame(width: 36).overlay(Theme.stroke)
            cloudButton
            if current.turned {
                // Turned away from north: one tap turns it back.
                Divider().frame(width: 36).overlay(Theme.stroke)
                Button {
                    var target = current
                    target.bearing = 0
                    fly(from: current, to: target)
                } label: {
                    Text("N")
                        .font(Pixel.font(14, bold: true))
                        .foregroundStyle(Theme.gold)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Turn the map back to north")
            }
            if tiltSetting > 0 {
                Divider().frame(width: 36).overlay(Theme.stroke)
                Button {
                    var target = current
                    target.tilt = current.tilt > 0.5 ? 0 : 1
                    fly(from: current, to: target)
                } label: {
                    Text("3D")
                        .font(Pixel.font(14, bold: true))
                        .foregroundStyle(current.tilt > 0.5 ? Theme.gold : Theme.primaryText)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(current.tilt > 0.5 ? "Look straight down" : "Tilt the map")
            }
        }
        .pixelFrame(Theme.panel)
    }

    /// Clouds in one tap: heavy, light, thin, off, and round again.
    private var cloudButton: some View {
        let level = min(max(clouds, 0), 3)
        return Button {
            clouds = (level + 3) % 4
            // A mix that happens to match a preset shows as that preset.
            let now = GraphicsSettings(mapDetail: detailLevel, tilt: tiltSetting, extrude: extrude, lighting: lighting,
                                       dayNight: dayNight, cityLights: cityLights, earth: earth, clouds: clouds, traffic: traffic,
                                       buildings: buildings, frameRate: frameRate, countryside: countryside, rivals: rivals,
                                       interstates: interstates, seasons: seasons, labels: labels)
            preset = (GraphicsPreset.choices.first { $0.values == now } ?? .custom).rawValue
        } label: {
            Group {
                if level == 0 {
                    PixelIcon("cloud_off", size: 22, colored: true)
                } else {
                    PixelIcon("cloud", size: 22)
                        .foregroundStyle(Theme.primaryText.opacity(0.4 + 0.2 * Double(level)))
                }
            }
            .frame(width: 40, height: 40)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Clouds: \(["Off", "Thin", "Light", "Heavy"][level])")
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

/// Turns the map with a secondary-button drag. The recogniser sits on the window, so the map's own
/// SwiftUI gestures keep every other touch, and it answers only drags that start over this view.
private struct SecondaryDrag: UIViewRepresentable {
    let onTurn: (CGFloat) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onTurn: onTurn) }

    func makeUIView(context: Context) -> Host {
        let view = Host()
        view.isUserInteractionEnabled = false
        view.backgroundColor = .clear
        view.coordinator = context.coordinator
        return view
    }

    func updateUIView(_ view: Host, context: Context) {
        context.coordinator.onTurn = onTurn
    }

    static func dismantleUIView(_ view: Host, coordinator: Coordinator) {
        coordinator.detach()
    }

    final class Host: UIView {
        weak var coordinator: Coordinator?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            coordinator?.attach(to: window, over: self)
        }
    }

    final class Coordinator: NSObject, UIGestureRecognizerDelegate {
        var onTurn: (CGFloat) -> Void
        private weak var area: UIView?
        private var pan: UIPanGestureRecognizer?
        private var last: CGFloat = 0

        init(onTurn: @escaping (CGFloat) -> Void) {
            self.onTurn = onTurn
        }

        func attach(to window: UIWindow?, over view: UIView) {
            detach()
            guard let window else { return }
            let pan = UIPanGestureRecognizer(target: self, action: #selector(dragged(_:)))
            pan.allowedScrollTypesMask = []
            pan.cancelsTouchesInView = false
            pan.delegate = self
            window.addGestureRecognizer(pan)
            self.pan = pan
            area = view
        }

        func detach() {
            if let pan { pan.view?.removeGestureRecognizer(pan) }
            pan = nil
        }

        @objc private func dragged(_ pan: UIPanGestureRecognizer) {
            let x = pan.translation(in: pan.view).x
            switch pan.state {
            case .began:
                last = x
            case .changed:
                onTurn(x - last)
                last = x
            default:
                last = 0
            }
        }

        /// Only drags with the secondary button down.
        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive event: UIEvent) -> Bool {
            event.type != .touches || event.buttonMask.contains(.secondary)
        }

        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
            guard let area, area.window != nil else { return false }
            return area.bounds.contains(touch.location(in: area))
        }

        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                               shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool {
            true
        }
    }
}

/// The active mode's name over a strip from its low colour to its high one.
struct MapLegend: View {
    let layer: MapLayer

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(layer.rawValue.uppercased())
                .font(Pixel.font(10))
                .foregroundStyle(Theme.primaryText)
                .lineLimit(1)
            // Stepped, like the rest of the 8-bit look: five blocks from faint to full.
            HStack(spacing: 2) {
                ForEach(0..<5, id: \.self) { k in
                    Rectangle()
                        .fill(RGB(hex: 0x33475A).mixed(with: layer.tintRGB, 0.15 + 0.6 * Double(k) / 4).color())
                }
            }
            .frame(height: 10)
            HStack(spacing: 4) {
                Text(layer.low)
                Spacer(minLength: 0)
                Text(layer.high)
            }
            .font(.system(size: 10))
            .foregroundStyle(Theme.secondary)
            .lineLimit(1)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .frame(width: 167, alignment: .leading)
        .pixelFrame(Theme.panel.opacity(0.94))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(layer.rawValue) legend, \(layer.low) to \(layer.high)")
    }
}
