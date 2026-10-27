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
        let rings: [[Int]]
    }

    struct Water: Decodable {
        let rings: [[Int]]
    }

    struct Line: Decodable {
        let lines: [[Int]]
    }

    struct City: Decodable {
        let name: String
        let region: String
        let country: String?
        let x: Int
        let y: Int
        let population: Int
        let capital: Bool
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

/// Geometry turned into sphere points once, shared by every map view.
final class WorldMap {
    static let shared = WorldMap()

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

    private init() {
        let file: WorldMapFile? = Bundle.main.url(forResource: "world-map", withExtension: "json")
            .flatMap { try? Data(contentsOf: $0) }
            .flatMap { try? JSONDecoder().decode(WorldMapFile.self, from: $0) }
        guard let file else {
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
        countries = file.countries.map { Feature(code: $0.code, name: $0.name, rings: $0.rings.map(GeoRing.init)) }
        provinces = (file.provinces ?? []).map(GeoRing.init)
        roads = (file.roads ?? []).map(GeoRing.init)
        func feature(_ area: WorldMapFile.Area) -> Feature {
            Feature(code: area.code, name: area.name, rings: area.rings.map(GeoRing.init))
        }
        playStates = file.states.filter { $0.play }.map(feature)
        contextStates = file.states.filter { !$0.play }.map(feature)
        regions = file.regions.map {
            RegionFeature(code: $0.code, state: $0.state, name: $0.name, rings: $0.rings.map(GeoRing.init))
        }
        counties = file.counties.flatMap { $0.rings.map(GeoRing.init) }
        lakes = file.lakes.flatMap { $0.rings.map(GeoRing.init) }
        rivers = file.rivers.flatMap { $0.lines.map(GeoRing.init) }
        interstates = file.interstates.flatMap { $0.lines.map(GeoRing.init) }
        cities = file.cities
        scenarioAreas = (file.scenario_areas ?? []).map { (id: $0.id, rings: $0.rings.map(GeoRing.init)) }
        hubs = file.hubs.compactMapValues { $0.count == 2 ? GeoPoint(lonDegrees: $0[0] / 10_000, latDegrees: $0[1] / 10_000) : nil }
        var bounds = (west: 180.0, south: 90.0, east: -180.0, north: -90.0)
        for state in file.states where state.play {
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
        regions.first { $0.code == code }?.name ?? code
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

    /// Moves the centre by a screen drag, so the ground follows the finger.
    func panned(by delta: CGSize, in size: CGSize) -> MapCamera {
        let r = radius(in: size)
        let t = flatness
        let latRad = lat * .pi / 180
        let east = 1 + t * (max(cos(latRad), 0.2) - 1)
        var next = self
        next.lon -= Double(delta.width) / (r * east) * 180 / .pi
        next.lat += Double(delta.height) / r * 180 / .pi
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
            zoom: exp(log(a.zoom) + (log(b.zoom) - log(a.zoom)) * f)
        )
    }
}

/// Orthographic globe blended into a flat equirectangular map around the camera centre. The two
/// agree at the centre, so the blend reads as the globe unrolling rather than a cut.
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

    init(camera: MapCamera, size: CGSize) {
        r = camera.radius(in: size)
        t = camera.flatness
        cx = Double(size.width) / 2
        cy = Double(size.height) / 2
        lon0 = camera.lon * .pi / 180
        lat0 = camera.lat * .pi / 180
        sin0 = sin(lat0)
        cos0 = cos(lat0)
        view = (cos0 * cos(lon0), cos0 * sin(lon0), sin0)
        let halfDiagonal = (Double(size.width * size.width + size.height * size.height)).squareRoot() / 2
        reach = min(.pi / 2 + 0.05 + t, halfDiagonal / r * 1.4 + 0.05)
    }

    /// Whether any of a ring could be on screen.
    func mayShow(_ ring: GeoRing) -> Bool {
        let c = ring.center
        let d = acos(min(max(c.0 * view.0 + c.1 * view.1 + c.2 * view.2, -1), 1))
        return d - ring.radius < reach
    }

    /// Screen point and whether it faces the viewer. Hidden points sit on the globe's edge so a
    /// filled shape that crosses the horizon is clipped to it.
    func project(_ p: GeoPoint, dl given: Double? = nil) -> (CGPoint, Bool) {
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

    /// A filled ring. Longitude is unwrapped along the ring so the flat map never tears it.
    func addPolygon(_ ring: GeoRing, to path: inout Path) {
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
            let (screen, _) = project(point, dl: dl)
            if index == 0 { path.move(to: screen) } else { path.addLine(to: screen) }
        }
        path.closeSubpath()
    }

    /// A stroked line, broken where it passes behind the globe or across the date line.
    func addLine(_ ring: GeoRing, to path: inout Path) {
        guard ring.points.count >= 2, mayShow(ring) else { return }
        var drawing = false
        var last = CGPoint.zero
        let jump = r * 0.5
        for point in ring.points {
            let (screen, visible) = project(point)
            if !visible {
                drawing = false
                continue
            }
            if drawing, abs(screen.x - last.x) + abs(screen.y - last.y) < jump {
                path.addLine(to: screen)
            } else {
                path.move(to: screen)
                drawing = true
            }
            last = screen
        }
    }

    func path(_ rings: [GeoRing]) -> Path {
        var path = Path()
        for ring in rings { addPolygon(ring, to: &path) }
        return path
    }

    func lines(_ rings: [GeoRing]) -> Path {
        var path = Path()
        for ring in rings { addLine(ring, to: &path) }
        return path
    }

    /// Even-odd test of a screen point against a feature's projected rings.
    func contains(_ rings: [GeoRing], _ location: CGPoint) -> Bool {
        path(rings).contains(location, eoFill: true)
    }
}
