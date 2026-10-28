import Foundation
import SwiftUI

/// The bundled world map: public-domain Natural Earth countries plus US Census detail for the
/// scenario's states, in ten-thousandths of a degree. Built by `tools/map/build_map.sh`. Regions
/// are whole counties dissolved by the engine's region table. The view projects it at draw time.
struct WorldMapFile: Decodable {
    struct Country: Decodable {
        let code: String
        let name: String
        let rings: [[Int]]
    }

    struct Area: Decodable {
        let code: String
        let name: String
        let play: Bool
        let rings: [[Int]]
        /// The scenario a state was added for, when its code could match another place
        /// (DE is also Delaware); none means the default scenario's.
        let scenario: String?
    }

    /// The outline of a scenario's whole region, lit up on the main menu globe.
    struct ScenarioArea: Decodable {
        let id: String
        let rings: [[Int]]
    }

    struct RegionArea: Decodable {
        let code: String
        let state: String
        let name: String
        let rings: [[Int]]
    }

    struct County: Decodable {
        let state: String?
        let rings: [[Int]]
    }

    struct Water: Decodable {
        let rings: [[Int]]
    }

    struct Line: Decodable {
        let lines: [[Int]]
        /// The scenario a layer was added for; none means the default scenario's.
        let scenario: String?
    }

    /// The states one scenario plays in. The first scenario in the file is the default.
    struct ScenarioStates: Decodable {
        let id: String
        let states: [String]
    }

    struct City: Decodable {
        let name: String
        let region: String
        let country: String?
        let x: Int
        let y: Int
        let population: Int
        let capital: Bool
        let scenario: String?
    }

    let attribution: String
    let countries: [Country]
    let provinces: [[Int]]?
    let roads: [[Int]]?
    // The key is `scenario_areas` in the file, so the property keeps that spelling.
    let scenario_areas: [ScenarioArea]?
    let states: [Area]
    let regions: [RegionArea]
    let counties: [County]
    let lakes: [Water]
    let rivers: [Line]
    let interstates: [Line]
    let cities: [City]
    let hubs: [String: [Double]]
    let scenarios: [ScenarioStates]?
}

/// A point on the sphere in radians, with its sine and cosine of latitude kept for projection.
struct GeoPoint {
    let lon: Double
    let lat: Double
    let sinLat: Double
    let cosLat: Double

    init(e4 lon: Int, _ lat: Int) {
        self.init(lonDegrees: Double(lon) / 10_000, latDegrees: Double(lat) / 10_000)
    }

    init(lonDegrees: Double, latDegrees: Double) {
        lon = lonDegrees * .pi / 180
        lat = latDegrees * .pi / 180
        sinLat = sin(lat)
        cosLat = cos(lat)
    }

    /// Unit vector, for angular distances.
    var unit: (Double, Double, Double) { (cosLat * cos(lon), cosLat * sin(lon), sinLat) }
}

/// One ring or line, with a bounding cap (centre and angular radius) for culling.
struct GeoRing {
    let points: [GeoPoint]
    let center: (Double, Double, Double)
    let radius: Double

    init(_ flat: [Int]) {
        var points: [GeoPoint] = []
        points.reserveCapacity(flat.count / 2)
        var index = 0
        while index + 1 < flat.count {
            points.append(GeoPoint(e4: flat[index], flat[index + 1]))
            index += 2
        }
        var sum = (0.0, 0.0, 0.0)
        for point in points {
            let u = point.unit
            sum = (sum.0 + u.0, sum.1 + u.1, sum.2 + u.2)
        }
        let norm = max((sum.0 * sum.0 + sum.1 * sum.1 + sum.2 * sum.2).squareRoot(), 1e-9)
        let center = (sum.0 / norm, sum.1 / norm, sum.2 / norm)
        var radius = 0.0
        for point in points {
            let u = point.unit
            radius = max(radius, acos(min(max(u.0 * center.0 + u.1 * center.1 + u.2 * center.2, -1), 1)))
        }
        self.points = points
        self.center = center
        self.radius = radius
    }
}

/// Geometry turned into sphere points once, shared by every map view. A map is for one scenario:
/// its playable states, regions, counties, roads, rivers and cities.
final class WorldMap {
    /// The default scenario's map, for views that have no career yet.
    static let shared = WorldMap.forScenario(nil)

    private static let file: WorldMapFile? = Bundle.main.url(forResource: "world-map", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(WorldMapFile.self, from: $0) }
    private static let lock = NSLock()
    private static var built: [String: WorldMap] = [:]

    /// One scenario's map, built once. nil is the default scenario, the first in the file.
    static func forScenario(_ id: String?) -> WorldMap {
        let key = id ?? file?.scenarios?.first?.id ?? ""
        lock.lock()
        defer { lock.unlock() }
        if let map = built[key] { return map }
        let map = WorldMap(file: file, scenario: key.isEmpty ? nil : key)
        built[key] = map
        return map
    }

    /// The map of the scenario whose regions these codes belong to, found from their state prefix.
    static func forRegions(_ codes: [String]) -> WorldMap {
        guard let state = codes.first?.split(separator: "-").first.map(String.init),
              let id = file?.scenarios?.first(where: { $0.states.contains(state) })?.id
        else { return shared }
        return forScenario(id)
    }

    struct Feature {
        let code: String
        let name: String
        let rings: [GeoRing]
    }

    struct RegionFeature {
        let code: String
        let state: String
        let name: String
        let rings: [GeoRing]
    }

    let attribution: String
    let countries: [Feature]
    /// Every state and province border in the world, as shared lines.
    let provinces: [GeoRing]
    /// Main highways worldwide, drawn grey outside the scenario.
    let roads: [GeoRing]
    let playStates: [Feature]
    let contextStates: [Feature]
    let regions: [RegionFeature]
    let counties: [GeoRing]
    let lakes: [GeoRing]
    let rivers: [GeoRing]
    let interstates: [GeoRing]
    let cities: [WorldMapFile.City]
    let hubs: [String: GeoPoint]
    /// Each scenario's outline, for the main menu globe.
    let scenarioAreas: [(id: String, rings: [GeoRing])]
    /// The scenario's playable states, in degrees: west, south, east, north.
    let playBounds: (west: Double, south: Double, east: Double, north: Double)

    /// Every region's name in the file, whichever scenario it belongs to, for feeds that name a
    /// region the current map does not draw.
    private let names: [String: String]

    /// Every city in the file, whichever scenario it belongs to, largest first. The main menu
    /// globe lights and links them.
    static let worldCities: [WorldMapFile.City] = (file?.cities ?? []).sorted { $0.population > $1.population }

    /// Which region each line, lake and city falls in, built the first time a raised map needs it.
    lazy var regionIndex = RegionIndex(map: self)

    private init(file: WorldMapFile?, scenario: String?) {
        guard let file else {
            names = [:]
            attribution = ""
            countries = []
            provinces = []
            roads = []
            playStates = []
            contextStates = []
            regions = []
            counties = []
            lakes = []
            rivers = []
            interstates = []
            cities = []
            hubs = [:]
            scenarioAreas = []
            playBounds = (-90, 37, -74, 46)
            return
        }
        attribution = file.attribution
        names = Dictionary(file.regions.map { ($0.code, $0.name) }, uniquingKeysWith: { first, _ in first })
        let scenarioStates: Set<String>? = scenario.flatMap { id in
            file.scenarios?.first { $0.id == id }.map { Set($0.states) }
        }
        let defaultScenario = file.scenarios?.first?.id
        func isPlay(_ area: WorldMapFile.Area) -> Bool {
            guard let states = scenarioStates else { return area.play }
            // A tagged state counts only in its own scenario; an untagged one is the default's.
            return states.contains(area.code) && (area.scenario.map { $0 == scenario } ?? area.play)
        }
        func showsState(_ area: WorldMapFile.Area) -> Bool {
            scenarioStates == nil || area.scenario == nil || area.scenario == scenario
        }
        // A layer tagged for another scenario is hidden; an untagged one belongs to the default.
        func owns(_ tag: String?) -> Bool { scenarioStates == nil || (tag ?? defaultScenario) == scenario }
        countries = file.countries.map { Feature(code: $0.code, name: $0.name, rings: $0.rings.map(GeoRing.init)) }
        provinces = (file.provinces ?? []).map(GeoRing.init)
        roads = (file.roads ?? []).map(GeoRing.init)
        func feature(_ area: WorldMapFile.Area) -> Feature {
            Feature(code: area.code, name: area.name, rings: area.rings.map(GeoRing.init))
        }
        playStates = file.states.filter { showsState($0) && isPlay($0) }.map(feature)
        contextStates = file.states.filter { showsState($0) && !isPlay($0) }.map(feature)
        regions = file.regions.filter { scenarioStates?.contains($0.state) ?? true }.map {
            RegionFeature(code: $0.code, state: $0.state, name: $0.name, rings: $0.rings.map(GeoRing.init))
        }
        counties = file.counties
            .filter { county in scenarioStates.map { states in county.state.map { states.contains($0) } ?? false } ?? true }
            .flatMap { $0.rings.map(GeoRing.init) }
        lakes = file.lakes.flatMap { $0.rings.map(GeoRing.init) }
        rivers = file.rivers.filter { owns($0.scenario) }.flatMap { $0.lines.map(GeoRing.init) }
        interstates = file.interstates.filter { owns($0.scenario) }.flatMap { $0.lines.map(GeoRing.init) }
        cities = file.cities.filter { owns($0.scenario) }
        scenarioAreas = (file.scenario_areas ?? []).map { (id: $0.id, rings: $0.rings.map(GeoRing.init)) }
        hubs = file.hubs.compactMapValues { $0.count == 2 ? GeoPoint(lonDegrees: $0[0] / 10_000, latDegrees: $0[1] / 10_000) : nil }
        var bounds = (west: 180.0, south: 90.0, east: -180.0, north: -90.0)
        for state in file.states where isPlay(state) {
            for ring in state.rings {
                var index = 0
                while index + 1 < ring.count {
                    let lon = Double(ring[index]) / 10_000
                    let lat = Double(ring[index + 1]) / 10_000
                    bounds = (min(bounds.west, lon), min(bounds.south, lat), max(bounds.east, lon), max(bounds.north, lat))
                    index += 2
                }
            }
        }
        playBounds = bounds
    }

    /// A region's display name.
    func regionName(_ code: String) -> String {
        names[code] ?? code
    }

    /// A label point for a region: its anchor hub, or its first vertex.
    func anchor(of code: String) -> GeoPoint? {
        if let hub = hubs[code] { return hub }
        return regions.first { $0.code == code }?.rings.first?.points.first
    }

    /// Cities inside a playable state, largest first.
    func cities(in stateName: String) -> [WorldMapFile.City] {
        cities.filter { $0.region == stateName && $0.country == "USA" }
    }
}

/// Where the map looks: a centre in degrees and a zoom where 1 fits the whole globe.
struct MapCamera: Equatable {
    var lon: Double
    var lat: Double
    var zoom: Double
    /// How much of the player's tilt setting applies: 1 tilted, 0 straight down. The 3D button
    /// flies between the two.
    var tilt: Double = 1

    static let minZoom = 0.8
    static let maxZoom = 90.0
    /// The globe flattens between these zooms.
    static let flatStart = 2.4
    static let flatEnd = 6.0

    /// 0 shows the globe, 1 the flat map, smoothly between.
    var flatness: Double {
        let x = min(max((zoom - MapCamera.flatStart) / (MapCamera.flatEnd - MapCamera.flatStart), 0), 1)
        return x * x * (3 - 2 * x)
    }

    func radius(in size: CGSize) -> Double {
        zoom * 0.45 * Double(min(size.width, size.height))
    }

    /// Moves the centre by a screen drag, so the ground follows the finger. A tilted map
    /// foreshortens vertical movement by the cosine of its pitch.
    func panned(by delta: CGSize, in size: CGSize, pitch: Double = 0) -> MapCamera {
        let r = radius(in: size)
        let t = flatness
        let latRad = lat * .pi / 180
        let east = 1 + t * (max(cos(latRad), 0.2) - 1)
        var next = self
        next.lon -= Double(delta.width) / (r * east) * 180 / .pi
        next.lat += Double(delta.height) / max(cos(pitch), 0.2) / r * 180 / .pi
        next.lat = min(max(next.lat, -80), 80)
        next.lon = (next.lon + 540).truncatingRemainder(dividingBy: 360) - 180
        return next
    }

    func zoomed(by factor: Double) -> MapCamera {
        var next = self
        next.zoom = min(max(zoom * factor, MapCamera.minZoom), MapCamera.maxZoom)
        return next
    }

    /// The camera that frames the scenario's playable states.
    static func scenario(in size: CGSize, map: WorldMap = .shared) -> MapCamera {
        let b = map.playBounds
        let lat = (b.south + b.north) / 2
        let lon = (b.west + b.east) / 2
        let dLon = (b.east - b.west) * .pi / 180 * cos(lat * .pi / 180) * 1.12
        let dLat = (b.north - b.south) * .pi / 180 * 1.12
        // A portrait phone fitting the whole width leaves most of the screen empty, so it frames
        // the middle of the scenario at a readable size and the player pans east or west.
        let portrait = size.height > size.width * 1.2
        let widthShown = portrait ? dLon * 0.58 : dLon
        let r = min(Double(size.width) / max(widthShown, 1e-6), Double(size.height) / max(dLat, 1e-6))
        let zoom = r / (0.45 * Double(max(min(size.width, size.height), 1)))
        return MapCamera(lon: lon, lat: lat, zoom: min(max(zoom, flatEnd), maxZoom))
    }

    /// Steps between two cameras for a fly-to: shortest way round in longitude, even in zoom.
    static func interpolate(_ a: MapCamera, _ b: MapCamera, _ f: Double) -> MapCamera {
        var dLon = b.lon - a.lon
        if dLon > 180 { dLon -= 360 }
        if dLon < -180 { dLon += 360 }
        return MapCamera(
            lon: a.lon + dLon * f,
            lat: a.lat + (b.lat - a.lat) * f,
            zoom: exp(log(a.zoom) + (log(b.zoom) - log(a.zoom)) * f),
            tilt: a.tilt + (b.tilt - a.tilt) * f
        )
    }
}

/// Orthographic globe blended into a flat equirectangular map around the camera centre. The two
/// agree at the centre, so the blend reads as the globe unrolling rather than a cut. A pitch then
/// tilts the flat result away from the viewer, and heights rise off the ground; `flat` gives the
/// untilted point and `lift` tilts it, so every path, anchor and hit test shares one transform.
struct MapProjection {
    let r: Double
    let t: Double
    let cx: Double
    let cy: Double
    let lon0: Double
    let lat0: Double
    let sin0: Double
    let cos0: Double
    let view: (Double, Double, Double)
    /// Angular radius that can reach the screen; anything farther is culled.
    let reach: Double
    /// Radians the ground is tilted, 0 on the globe and the full setting once flat.
    let pitch: Double
    let cosPitch: Double
    let sinPitch: Double
    /// Focal length in points for the tilt's perspective.
    let focal: Double
    /// Screen rise per point of height: the pitch's sine, or an oblique view on an untilted map.
    let rise: Double

    /// `tilt` is the player's tilt setting in radians; the camera's own tilt and the flatness
    /// scale it.
    init(camera: MapCamera, size: CGSize, tilt: Double = 0) {
        r = camera.radius(in: size)
        t = camera.flatness
        cx = Double(size.width) / 2
        cy = Double(size.height) / 2
        lon0 = camera.lon * .pi / 180
        lat0 = camera.lat * .pi / 180
        sin0 = sin(lat0)
        cos0 = cos(lat0)
        view = (cos0 * cos(lon0), cos0 * sin(lon0), sin0)
        pitch = tilt * min(max(camera.tilt, 0), 1) * t
        cosPitch = cos(pitch)
        sinPitch = sin(pitch)
        focal = 1.6 * Double(max(size.width, size.height))
        rise = max(sinPitch, 0.40 * t)
        let halfDiagonal = (Double(size.width * size.width + size.height * size.height)).squareRoot() / 2
        // A tilted view sees farther up the screen, so the cull reaches farther with it.
        reach = min(.pi / 2 + 0.05 + t, halfDiagonal / r * 1.4 * (1 + 1.5 * sinPitch) + 0.05)
    }

    /// Whether any of a ring could be on screen.
    func mayShow(_ ring: GeoRing) -> Bool {
        let c = ring.center
        let d = acos(min(max(c.0 * view.0 + c.1 * view.1 + c.2 * view.2, -1), 1))
        return d - ring.radius < reach
    }

    /// The untilted screen point and whether it faces the viewer. Hidden points sit on the
    /// globe's edge so a filled shape that crosses the horizon is clipped to it.
    func flat(_ p: GeoPoint, dl given: Double? = nil) -> (CGPoint, Bool) {
        var dl = given ?? (p.lon - lon0)
        if given == nil {
            if dl > .pi { dl -= 2 * .pi }
            if dl < -.pi { dl += 2 * .pi }
        }
        let cosDl = cos(dl)
        let cosc = sin0 * p.sinLat + cos0 * p.cosLat * cosDl
        var ox = p.cosLat * sin(dl)
        var oy = cos0 * p.sinLat - sin0 * p.cosLat * cosDl
        let front = cosc >= 0
        if !front {
            let n = (ox * ox + oy * oy).squareRoot()
            if n > 1e-9 {
                ox /= n
                oy /= n
            }
        }
        let fx = dl * cos0
        let fy = p.lat - lat0
        let x = cx + r * (ox + t * (fx - ox))
        let y = cy - r * (oy + t * (fy - oy))
        return (CGPoint(x: x, y: y), front || t > 0.5)
    }

    /// Perspective scale at an untilted screen height: 1 at the centre, smaller farther away.
    func scale(atFlatY y: CGFloat) -> Double {
        if pitch == 0 { return 1 }
        let z = -(Double(y) - cy) * sinPitch
        return focal / max(focal + z, 0.25 * focal)
    }

    /// Tilts an untilted screen point and raises it `h` points off the ground.
    func lift(_ point: CGPoint, _ h: Double = 0) -> CGPoint {
        if pitch == 0 && (h == 0 || rise == 0) { return point }
        let dx = Double(point.x) - cx
        let dy = Double(point.y) - cy
        let y1 = dy * cosPitch - h * rise
        let s = scale(atFlatY: point.y)
        return CGPoint(x: cx + dx * s, y: cy + y1 * s)
    }

    /// Screen point on the ground and whether it faces the viewer.
    func project(_ p: GeoPoint, dl given: Double? = nil) -> (CGPoint, Bool) {
        let (point, visible) = flat(p, dl: given)
        return (lift(point), visible)
    }

    /// Screen point raised `h` points off the ground.
    func project(_ p: GeoPoint, height h: Double) -> (CGPoint, Bool) {
        let (point, visible) = flat(p)
        return (lift(point, h), visible)
    }

    /// A ring's untilted points, longitude unwrapped along the ring so the flat map never tears
    /// it. nil when the ring cannot be on screen.
    func flatRing(_ ring: GeoRing) -> [CGPoint]? {
        guard ring.points.count >= 3, mayShow(ring) else { return nil }
        var points: [CGPoint] = []
        points.reserveCapacity(ring.points.count)
        var previous = 0.0
        for (index, point) in ring.points.enumerated() {
            var dl = point.lon - lon0
            if index == 0 {
                if dl > .pi { dl -= 2 * .pi }
                if dl < -.pi { dl += 2 * .pi }
            } else {
                while dl - previous > .pi { dl -= 2 * .pi }
                while dl - previous < -.pi { dl += 2 * .pi }
            }
            previous = dl
            points.append(flat(point, dl: dl).0)
        }
        return points
    }

    /// A filled ring, raised `height` points off the ground.
    func addPolygon(_ ring: GeoRing, to path: inout Path, height: Double = 0) {
        guard ring.points.count >= 3, mayShow(ring) else { return }
        var previous = 0.0
        for (index, point) in ring.points.enumerated() {
            var dl = point.lon - lon0
            if index == 0 {
                if dl > .pi { dl -= 2 * .pi }
                if dl < -.pi { dl += 2 * .pi }
            } else {
                while dl - previous > .pi { dl -= 2 * .pi }
                while dl - previous < -.pi { dl += 2 * .pi }
            }
            previous = dl
            let screen = lift(flat(point, dl: dl).0, height)
            if index == 0 { path.move(to: screen) } else { path.addLine(to: screen) }
        }
        path.closeSubpath()
    }

    /// A stroked line, broken where it passes behind the globe or across the date line.
    func addLine(_ ring: GeoRing, to path: inout Path, height: Double = 0) {
        guard ring.points.count >= 2, mayShow(ring) else { return }
        var drawing = false
        var last = CGPoint.zero
        let jump = r * 0.5
        for point in ring.points {
            let (flatPoint, visible) = flat(point)
            if !visible {
                drawing = false
                continue
            }
            let screen = lift(flatPoint, height)
            if drawing, abs(screen.x - last.x) + abs(screen.y - last.y) < jump {
                path.addLine(to: screen)
            } else {
                path.move(to: screen)
                drawing = true
            }
            last = screen
        }
    }

    func path(_ rings: [GeoRing], height: Double = 0) -> Path {
        var path = Path()
        for ring in rings { addPolygon(ring, to: &path, height: height) }
        return path
    }

    func lines(_ rings: [GeoRing], height: Double = 0) -> Path {
        var path = Path()
        for ring in rings { addLine(ring, to: &path, height: height) }
        return path
    }

    /// Even-odd test of a screen point against a feature's projected rings.
    func contains(_ rings: [GeoRing], _ location: CGPoint, height: Double = 0) -> Bool {
        path(rings, height: height).contains(location, eoFill: true)
    }

    /// A geo direction in view space: x right, y up, z towards the viewer.
    func toView(_ p: (Double, Double, Double)) -> (Double, Double, Double) {
        let sinL = sin(lon0)
        let cosL = cos(lon0)
        let east = (-sinL, cosL, 0.0)
        let north = (-sin0 * cosL, -sin0 * sinL, cos0)
        return (p.0 * east.0 + p.1 * east.1 + p.2 * east.2,
                p.0 * north.0 + p.1 * north.1 + p.2 * north.2,
                p.0 * view.0 + p.1 * view.1 + p.2 * view.2)
    }

    /// A view-space direction back in geo space.
    func toGeo(_ v: (Double, Double, Double)) -> (Double, Double, Double) {
        let sinL = sin(lon0)
        let cosL = cos(lon0)
        let east = (-sinL, cosL, 0.0)
        let north = (-sin0 * cosL, -sin0 * sinL, cos0)
        return (v.0 * east.0 + v.1 * north.0 + v.2 * view.0,
                v.0 * east.1 + v.1 * north.1 + v.2 * view.1,
                v.0 * east.2 + v.1 * north.2 + v.2 * view.2)
    }

    /// The night side for a sun at `sunLon`, `sunLat` degrees: everything where the sun's
    /// direction dotted with the point is below `offset`. A ring along the terminator from one
    /// side of the view to the other, closed round the night pole, ready for `addPolygon`.
    static func nightRing(sunLon: Double, sunLat: Double, offset c: Double, centerLon lon0: Double) -> GeoRing {
        let ls = sunLon * .pi / 180
        var dsDeg = sunLat
        if abs(dsDeg) < 2 { dsDeg = dsDeg < 0 ? -2 : 2 }
        let ds = dsDeg * .pi / 180
        let nightPole = ds > 0 ? -89.9 : 89.9
        let dayPole = -nightPole
        var flat: [Int] = []
        let first = Int((lon0 - 180).rounded())
        let last = Int((lon0 + 180).rounded())
        for lonDeg in stride(from: first, through: last, by: 3) {
            let lon = Double(lonDeg) * .pi / 180
            let a = cos(ds) * cos(lon - ls)
            let b = sin(ds)
            let rr = (a * a + b * b).squareRoot()
            let lat: Double
            if abs(c) > rr {
                // The whole column is on one side: night when even the sunniest point is below c.
                lat = c > rr ? dayPole : nightPole
            } else {
                let alpha = atan2(b, a)
                let spread = acos(c / rr)
                var roots: [Double] = []
                for candidate in [alpha + spread, alpha - spread] {
                    var deg = candidate * 180 / .pi
                    while deg > 180 { deg -= 360 }
                    while deg <= -180 { deg += 360 }
                    if deg >= -90 && deg <= 90 { roots.append(deg) }
                }
                if roots.isEmpty {
                    // Neither root is on the globe: the column is all night or all day.
                    lat = a < c ? dayPole : nightPole
                } else {
                    // Night lies between the boundary and the night pole: take the root nearest it.
                    lat = ds > 0 ? roots.min() ?? nightPole : roots.max() ?? nightPole
                }
            }
            flat.append(lonDeg * 10_000)
            flat.append(Int((min(max(lat, -89.9), 89.9) * 10_000).rounded()))
        }
        for lonDeg in stride(from: last, through: first, by: -3) {
            flat.append(lonDeg * 10_000)
            flat.append(Int((nightPole * 10_000).rounded()))
        }
        return GeoRing(flat)
    }
}

/// Which region each county, lake, river, highway, state border and city of a map belongs to,
/// so a raised region can draw its own detail on its top face. Built once per map.
final class RegionIndex {
    /// Region bounding boxes in radians.
    private let boxes: [(west: Double, south: Double, east: Double, north: Double)]
    private let regions: [WorldMap.RegionFeature]
    let counties: [[Int]]
    let lakes: [[Int]]
    let rivers: [[Int]]
    let interstates: [[Int]]
    let states: [[Int]]
    /// The region index each city sits in, or -1 outside the scenario.
    let cityRegion: [Int]
    /// City indices, most populous first.
    let byPopulation: [Int]
    private let interstateRings: [GeoRing]
    /// The region index of every highway point, for traffic on raised regions.
    lazy var interstatePoints: [[Int]] = interstateRings.map { ring in ring.points.map { region(at: $0) } }

    init(map: WorldMap) {
        regions = map.regions
        func box(_ rings: [GeoRing]) -> (west: Double, south: Double, east: Double, north: Double) {
            var b = (west: Double.infinity, south: Double.infinity, east: -Double.infinity, north: -Double.infinity)
            for ring in rings {
                for p in ring.points {
                    b = (min(b.west, p.lon), min(b.south, p.lat), max(b.east, p.lon), max(b.north, p.lat))
                }
            }
            return b
        }
        let regionBoxes = map.regions.map { box($0.rings) }
        boxes = regionBoxes
        func assign(_ rings: [GeoRing]) -> [[Int]] {
            var result = Array(repeating: [Int](), count: regionBoxes.count)
            for (index, ring) in rings.enumerated() {
                let b = box([ring])
                for (r, rb) in regionBoxes.enumerated()
                where b.west <= rb.east && b.east >= rb.west && b.south <= rb.north && b.north >= rb.south {
                    result[r].append(index)
                }
            }
            return result
        }
        counties = assign(map.counties)
        lakes = assign(map.lakes)
        rivers = assign(map.rivers)
        interstates = assign(map.interstates)
        // A state's border belongs to the regions inside it.
        states = map.regions.map { region in
            map.playStates.indices.filter { map.playStates[$0].code == region.state }
        }
        var cities: [Int] = []
        cities.reserveCapacity(map.cities.count)
        for city in map.cities {
            cities.append(RegionIndex.find(GeoPoint(e4: city.x, city.y), boxes: regionBoxes, regions: map.regions))
        }
        cityRegion = cities
        byPopulation = map.cities.indices.sorted { map.cities[$0].population > map.cities[$1].population }
        interstateRings = map.interstates
    }

    /// The region a point falls in, or -1.
    func region(at point: GeoPoint) -> Int {
        Self.find(point, boxes: boxes, regions: regions)
    }

    private static func find(_ p: GeoPoint, boxes: [(west: Double, south: Double, east: Double, north: Double)],
                             regions: [WorldMap.RegionFeature]) -> Int {
        for (index, b) in boxes.enumerated() where p.lon >= b.west && p.lon <= b.east && p.lat >= b.south && p.lat <= b.north {
            var inside = false
            for ring in regions[index].rings {
                let pts = ring.points
                var j = pts.count - 1
                for i in 0..<pts.count {
                    let a = pts[i]
                    let c = pts[j]
                    if (a.lat > p.lat) != (c.lat > p.lat) {
                        let x = (c.lon - a.lon) * (p.lat - a.lat) / (c.lat - a.lat) + a.lon
                        if p.lon < x { inside.toggle() }
                    }
                    j = i
                }
            }
            if inside { return index }
        }
        return -1
    }
}
