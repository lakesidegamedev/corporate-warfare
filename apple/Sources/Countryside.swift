import ImageIO
import SwiftUI

/// The diorama up close: small voxel props scattered over the satellite ground by what the ground
/// is. Forest gets conifers in the north and leafy trees further south, farmland gets crop rows and
/// the odd barn, dry land a few rocks, and houses gather round each city, spreading further in a
/// boom and pulling in during a recession. Each prop has a summer, autumn and winter look, so the
/// countryside follows the seasons with the Earth beneath it. The props come from the scenario's
/// satellite crop, read once in the background; only the current scenario's are kept.
enum Countryside {
    /// A prop on the ground: where (radians), what, and how far into a city's edge a house sits.
    struct Prop {
        let lon: Double
        let lat: Double
        let latDeg: Double
        let kind: String
        let city: Double
        /// Turns the prop its own way, so a field of trees never looks stamped.
        let seed: Int
    }

    static let ready = Notification.Name("Countryside.ready")
    private static var held: (id: String, props: [Prop])?
    private static var working: String?
    private static let lock = NSLock()

    /// The scenario's props, or nil while they are worked out in the background.
    static func props(_ map: WorldMap) -> [Prop]? {
        guard let id = map.scenarioId else { return [] }
        lock.lock()
        if let held, held.id == id {
            lock.unlock()
            return held.props
        }
        if working == id {
            lock.unlock()
            return nil
        }
        working = id
        lock.unlock()
        DispatchQueue.global(qos: .utility).async {
            let props = build(map, id)
            lock.lock()
            held = (id, props)
            working = nil
            lock.unlock()
            DispatchQueue.main.async { NotificationCenter.default.post(name: ready, object: nil) }
        }
        return nil
    }

    /// Works the props out now, for tests.
    static func prepare(_ map: WorldMap) {
        guard let id = map.scenarioId else { return }
        let props = build(map, id)
        lock.lock()
        held = (id, props)
        lock.unlock()
    }

    /// Every prop for a scenario, from its satellite crop's colours and its cities, exactly as
    /// the Android map works them out.
    private static func build(_ map: WorldMap, _ id: String) -> [Prop] {
        guard let region = CloudRegion.all[id],
              let url = Bundle.main.url(forResource: "earth-\(id)", withExtension: "jpg"),
              let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return [] }
        let width = image.width
        let height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let drawn = pixels.withUnsafeMutableBytes { raw -> Bool in
            guard let context = CGContext(data: raw.baseAddress, width: width, height: height, bitsPerComponent: 8,
                                          bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                                          bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else { return [] }
        // The land mask from the game's own outlines, so nothing stands in a lake.
        var mask: (width: Int, height: Int, alpha: [UInt8])?
        if let url = Bundle.main.url(forResource: "earth-land-\(id)", withExtension: "png"),
           let source = CGImageSourceCreateWithURL(url as CFURL, nil),
           let land = CGImageSourceCreateImageAtIndex(source, 0, nil) {
            var alpha = [UInt8](repeating: 0, count: land.width * land.height)
            let ok = alpha.withUnsafeMutableBytes { raw -> Bool in
                guard let context = CGContext(data: raw.baseAddress, width: land.width, height: land.height, bitsPerComponent: 8,
                                              bytesPerRow: land.width, space: CGColorSpaceCreateDeviceGray(),
                                              bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue) else { return false }
                context.draw(land, in: CGRect(x: 0, y: 0, width: land.width, height: land.height))
                return true
            }
            if ok { mask = (land.width, land.height, alpha) }
        }
        func onLand(_ lon: Double, _ lat: Double) -> Bool {
            guard let mask else { return true }
            let u = Int((lon - region.west) / (region.east - region.west) * Double(mask.width))
            let v = Int((region.north - lat) / (region.north - region.south) * Double(mask.height))
            return u >= 0 && u < mask.width && v >= 0 && v < mask.height && mask.alpha[v * mask.width + u] > 128
        }
        let b = map.playBounds
        let west = b.west - 3
        let east = b.east + 3
        let south = b.south - 3
        let north = b.north + 3
        let step = 0.14
        let cities = map.cities.map { (lon: Double($0.x) / 10_000, lat: Double($0.y) / 10_000, pop: Double($0.population)) }
        var out: [Prop] = []
        var lat = south
        var row = 0
        while lat < north {
            var lon = west
            var col = 0
            while lon < east {
                let seed = stableHash("\(id)/\(row)/\(col)")
                let jx = (Double(seed % 1000) / 1000 - 0.5) * step * 0.9
                let jy = (Double((seed / 1000) % 1000) / 1000 - 0.5) * step * 0.9
                let x = lon + jx
                let y = lat + jy
                let roll = (seed / 7) % 100
                if !onLand(x, y) {
                    lon += step
                    col += 1
                    continue
                }
                // Near a city: houses, thicker towards the middle.
                var near = 9.0
                if !cities.isEmpty {
                    near = .infinity
                    for c in cities {
                        let dx = (x - c.lon) * cos(y * .pi / 180)
                        let d = (dx * dx + (y - c.lat) * (y - c.lat)).squareRoot()
                        near = min(near, d / (0.06 * max((c.pop / 1e5).squareRoot(), 1)))
                    }
                }
                var kind: String?
                if near < 1 {
                    if roll < 45 { kind = "house" }
                } else {
                    let u = Int((x - region.west) / (region.east - region.west) * Double(width))
                    let v = Int((region.north - y) / (region.north - region.south) * Double(height))
                    if u >= 0 && u < width && v >= 0 && v < height {
                        let i = (v * width + u) * 4
                        let r = Int(pixels[i])
                        let g = Int(pixels[i + 1])
                        let bl = Int(pixels[i + 2])
                        let lum = (r + g + bl) / 3
                        if bl > r + 8 && bl >= g {
                            kind = nil // water
                        } else if lum > 200 {
                            kind = nil // snow, ice or cloud in the source
                        } else if g > r + 6 && lum < 34 {
                            // Forest is the darkest, greenest ground; farmland is a little lighter.
                            kind = roll < 18 ? (y > 44 || roll < 6 ? "conifer" : "broadleaf") : nil
                        } else if g >= bl && lum >= 34 && lum <= 150 {
                            kind = roll < 11 ? "field" : roll < 13 ? "barn" : roll < 15 ? "broadleaf" : nil
                        } else if r > g && lum > 100 {
                            kind = roll < 8 ? "rock" : nil
                        } else {
                            kind = roll < 4 ? "broadleaf" : nil
                        }
                    }
                }
                if let kind {
                    out.append(Prop(lon: x * .pi / 180, lat: y * .pi / 180, latDeg: y, kind: kind, city: near < 1 ? near : 0,
                                    seed: Int(seed % 4)))
                }
                lon += step
                col += 1
            }
            lat += step
            row += 1
        }
        return out
    }

    /// Draws the props on screen for the current camera, far ones first, at most `limit`. Houses
    /// further from a city than the economy supports are left out: towns grow in a boom.
    static func draw(in context: inout GraphicsContext, projection p: MapProjection, map: WorldMap, week: Int, unit: Double,
                     lift: Double, economyPhase: String?, size: CGSize, limit: Int = 500) {
        guard let props = props(map) else { return }
        let reach: Double
        switch economyPhase {
        case "boom": reach = 1
        case "expansion": reach = 0.9
        case "recovery": reach = 0.8
        case "slowdown": reach = 0.7
        case "recession": reach = 0.6
        default: reach = 0.85
        }
        var shown: [(at: CGPoint, ground: CGPoint, prop: Prop)] = []
        for prop in props {
            if prop.kind == "house" && prop.city > reach { continue }
            guard let g = p.ground(lon: prop.lon, lat: prop.lat) else { continue }
            let at = p.lift(g, lift)
            if at.x < -20 || at.y < -20 || at.x > size.width + 20 || at.y > size.height + 20 { continue }
            shown.append((at, g, prop))
            if shown.count >= limit { break }
        }
        for item in shown.sorted(by: { $0.at.y < $1.at.y }) {
            let prop = item.prop
            VoxelModels.draw("\(prop.kind)/\(Earth.season(week, prop.latDeg))", projection: p, ground: item.ground, floor: lift,
                             unit: unit, heading: Double(prop.seed) * .pi / 2, ink: false, in: &context)
        }
    }
}
