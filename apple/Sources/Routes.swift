import Foundation

/// Freight follows the real networks: trucks the interstates, trains the railways (Natural Earth,
/// in the bundled map). Each network is a graph whose nodes are the points of its lines snapped to
/// a hundredth of a degree, so lines that cross or meet join; a lane's route is the shortest path
/// between the network points nearest its two hubs. Routes are worked out once per map and kept. A
/// lane with no sensible route on the network has none, and shows no trucks or trains.
final class RoadNetwork {
    private var lon: [Double] = []
    private var lat: [Double] = []
    private var edges: [[(to: Int, d: Double)]] = []
    private var ids: [Int64: Int] = [:]

    init(_ rings: [GeoRing]) {
        for ring in rings {
            var previous = -1
            for point in ring.points {
                let node = node(point.lon, point.lat)
                if previous >= 0 && previous != node {
                    let d = distance(previous, node)
                    edges[previous].append((node, d))
                    edges[node].append((previous, d))
                }
                previous = node
            }
        }
    }

    private func node(_ lonRad: Double, _ latRad: Double) -> Int {
        let key = Int64((lonRad * 180 / .pi * 100).rounded()) * 100_000 + Int64((latRad * 180 / .pi * 100).rounded())
        if let id = ids[key] { return id }
        lon.append(lonRad)
        lat.append(latRad)
        edges.append([])
        ids[key] = lon.count - 1
        return lon.count - 1
    }

    private func distance(_ a: Int, _ b: Int) -> Double {
        let dx = (lon[b] - lon[a]) * cos((lat[a] + lat[b]) / 2)
        let dy = lat[b] - lat[a]
        return (dx * dx + dy * dy).squareRoot()
    }

    private func nearest(_ lonRad: Double, _ latRad: Double) -> Int {
        var best = -1
        var bestD = Double.greatestFiniteMagnitude
        let c = cos(latRad)
        for i in lon.indices where !edges[i].isEmpty {
            let dx = (lon[i] - lonRad) * c
            let dy = lat[i] - latRad
            let d = dx * dx + dy * dy
            if d < bestD {
                bestD = d
                best = i
            }
        }
        // Further than about two degrees from the network, there is no road to take.
        let reach = 2 * Double.pi / 180
        return bestD < reach * reach ? best : -1
    }

    /// The route from one place to another as longitudes and latitudes in radians, or nil.
    func route(from a: GeoPoint, to b: GeoPoint) -> [GeoPoint]? {
        let start = nearest(a.lon, a.lat)
        let goal = nearest(b.lon, b.lat)
        guard start >= 0, goal >= 0, start != goal else { return nil }
        var dist = [Double](repeating: .greatestFiniteMagnitude, count: lon.count)
        var back = [Int](repeating: -1, count: lon.count)
        var queue = MinHeap()
        dist[start] = 0
        queue.push(0, start)
        while let next = queue.pop() {
            let (d, at) = next
            if at == goal { break }
            if d > dist[at] { continue }
            for edge in edges[at] {
                let nd = d + edge.d
                if nd < dist[edge.to] {
                    dist[edge.to] = nd
                    back[edge.to] = at
                    queue.push(nd, edge.to)
                }
            }
        }
        guard back[goal] >= 0 else { return nil }
        // A route far longer than the straight line means the network is broken between them.
        let dx = (b.lon - a.lon) * cos((a.lat + b.lat) / 2)
        let dy = b.lat - a.lat
        if dist[goal] > 3 * (dx * dx + dy * dy).squareRoot() + 0.02 { return nil }
        var path: [Int] = []
        var at = goal
        while at >= 0 {
            path.append(at)
            at = back[at]
        }
        return [a] + path.reversed().map { GeoPoint(lonRadians: lon[$0], latRadians: lat[$0]) } + [b]
    }

    /// A binary min-heap of (distance, node), for the shortest path.
    private struct MinHeap {
        private var items: [(Double, Int)] = []

        mutating func push(_ d: Double, _ node: Int) {
            items.append((d, node))
            var i = items.count - 1
            while i > 0 {
                let parent = (i - 1) / 2
                if items[parent].0 <= items[i].0 { break }
                items.swapAt(parent, i)
                i = parent
            }
        }

        mutating func pop() -> (Double, Int)? {
            guard let first = items.first else { return nil }
            let last = items.removeLast()
            if !items.isEmpty {
                items[0] = last
                var i = 0
                while true {
                    let l = 2 * i + 1
                    let r = l + 1
                    var m = i
                    if l < items.count && items[l].0 < items[m].0 { m = l }
                    if r < items.count && items[r].0 < items[m].0 { m = r }
                    if m == i { break }
                    items.swapAt(i, m)
                    i = m
                }
            }
            return first
        }
    }
}

/// Each lane's route on the real network for its mode, kept per scenario and pair of hubs.
enum Routes {
    private static var networks: [String: RoadNetwork] = [:]
    private static var routes: [String: [GeoPoint]?] = [:]
    private static let lock = NSLock()

    private static func network(_ map: WorldMap, _ mode: String) -> RoadNetwork? {
        let key = "\(map.scenarioId ?? "")/\(mode)"
        if let held = networks[key] { return held }
        let built: RoadNetwork
        switch mode {
        case "road", "truck": built = RoadNetwork(map.interstates)
        case "rail":
            guard !map.railroads.isEmpty else { return nil }
            built = RoadNetwork(map.railroads)
        default: return nil
        }
        networks[key] = built
        return built
    }

    /// The route for a lane between two regions by its mode, or nil when there is none.
    static func of(_ map: WorldMap, mode: String, _ a: String, _ b: String) -> [GeoPoint]? {
        let key = "\(map.scenarioId ?? "")/\(mode)/\(a)>\(b)"
        lock.lock()
        defer { lock.unlock() }
        if let held = routes[key] { return held }
        var found: [GeoPoint]?
        if let from = map.anchor(of: a), let to = map.anchor(of: b) {
            found = network(map, mode)?.route(from: from, to: to)
        }
        routes[key] = .some(found)
        return found
    }
}
