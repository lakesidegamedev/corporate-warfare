import Foundation
import SwiftUI

/// The bundled corridor map: public-domain US Census and Natural Earth geometry, pre-projected
/// (Albers conic) into a 10000-unit-wide view box with integer coordinates. Built by
/// `tools/map/compact_map.py`.
struct CorridorMapFile: Decodable {
    struct Area: Decodable {
        let code: String
        let name: String
        let play: Bool
        let rings: [[Int]]
    }

    struct County: Decodable {
        let fips: String
        let name: String
        let state: String
        let rings: [[Int]]
    }

    struct Water: Decodable {
        let name: String
        let rings: [[Int]]
    }

    struct Line: Decodable {
        let name: String
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
    let viewbox: [Double]
    let states: [Area]
    let counties: [County]
    let lakes: [Water]
    let rivers: [Line]
    let interstates: [Line]
    let cities: [City]
    let hubs: [String: [Double]]
}

/// Geometry turned into ready-to-draw paths once, shared by every map view.
final class CorridorMap {
    static let shared = CorridorMap()

    let size: CGSize
    let attribution: String
    let playStates: [(code: String, name: String, path: Path, bounds: CGRect)]
    let contextLand: Path
    let counties: Path
    let lakes: Path
    let rivers: Path
    let interstates: Path
    let cities: [CorridorMapFile.City]
    let hubs: [String: CGPoint]

    private init() {
        let file: CorridorMapFile? = Bundle.main.url(forResource: "corridor-map", withExtension: "json")
            .flatMap { try? Data(contentsOf: $0) }
            .flatMap { try? JSONDecoder().decode(CorridorMapFile.self, from: $0) }
        guard let file, file.viewbox.count == 2 else {
            size = CGSize(width: 10_000, height: 8_000)
            attribution = ""
            playStates = []
            contextLand = Path()
            counties = Path()
            lakes = Path()
            rivers = Path()
            interstates = Path()
            cities = []
            hubs = [:]
            return
        }
        size = CGSize(width: file.viewbox[0], height: file.viewbox[1])
        attribution = file.attribution
        var states: [(String, String, Path, CGRect)] = []
        var context = Path()
        for area in file.states {
            let path = CorridorMap.polygons(area.rings)
            if area.play {
                states.append((area.code, area.name, path, path.boundingRect))
            } else {
                context.addPath(path)
            }
        }
        playStates = states.map { (code: $0.0, name: $0.1, path: $0.2, bounds: $0.3) }
        contextLand = context
        var countyPath = Path()
        for county in file.counties {
            countyPath.addPath(CorridorMap.polygons(county.rings))
        }
        counties = countyPath
        var lakePath = Path()
        for lake in file.lakes {
            lakePath.addPath(CorridorMap.polygons(lake.rings))
        }
        lakes = lakePath
        rivers = CorridorMap.polylines(file.rivers.flatMap(\.lines))
        interstates = CorridorMap.polylines(file.interstates.flatMap(\.lines))
        cities = file.cities
        hubs = file.hubs.compactMapValues { $0.count == 2 ? CGPoint(x: $0[0], y: $0[1]) : nil }
    }

    private static func polygons(_ rings: [[Int]]) -> Path {
        var path = Path()
        for ring in rings where ring.count >= 6 {
            path.move(to: CGPoint(x: ring[0], y: ring[1]))
            var index = 2
            while index + 1 < ring.count {
                path.addLine(to: CGPoint(x: ring[index], y: ring[index + 1]))
                index += 2
            }
            path.closeSubpath()
        }
        return path
    }

    private static func polylines(_ lines: [[Int]]) -> Path {
        var path = Path()
        for line in lines where line.count >= 4 {
            path.move(to: CGPoint(x: line[0], y: line[1]))
            var index = 2
            while index + 1 < line.count {
                path.addLine(to: CGPoint(x: line[index], y: line[index + 1]))
                index += 2
            }
        }
        return path
    }

    /// The playable state under a point in map units.
    func state(at point: CGPoint) -> String? {
        playStates.first { $0.bounds.contains(point) && $0.path.contains(point, eoFill: true) }?.code
    }

    /// The bounding rectangle of the playable states, for the initial framing.
    var playBounds: CGRect {
        playStates.map(\.bounds).reduce(CGRect.null) { $0.union($1) }
    }

    /// Cities inside a playable state, largest first.
    func cities(in stateName: String) -> [CorridorMapFile.City] {
        cities.filter { $0.region == stateName && $0.country == "USA" }
    }
}
