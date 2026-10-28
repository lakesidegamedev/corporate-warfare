import SwiftUI

/// A colour as components, so walls and faces can be shaded by multiplying it.
struct RGB {
    var r: Double
    var g: Double
    var b: Double

    init(_ r: Double, _ g: Double, _ b: Double) {
        self.r = r
        self.g = g
        self.b = b
    }

    init(hex: UInt32) {
        r = Double((hex >> 16) & 0xFF) / 255
        g = Double((hex >> 8) & 0xFF) / 255
        b = Double(hex & 0xFF) / 255
    }

    func scaled(_ k: Double) -> RGB { RGB(min(r * k, 1), min(g * k, 1), min(b * k, 1)) }
    func mixed(with other: RGB, _ amount: Double) -> RGB {
        RGB(r + (other.r - r) * amount, g + (other.g - g) * amount, b + (other.b - b) * amount)
    }
    func color(_ alpha: Double = 1) -> Color { Color(red: r, green: g, blue: b).opacity(alpha) }

    static let land = RGB(0.169, 0.231, 0.290)
    static let gold = RGB(0.886, 0.690, 0.290)
    static let positive = RGB(0.247, 0.749, 0.541)
    static let copper = RGB(0.788, 0.502, 0.290)
    static let blue = RGB(0.259, 0.647, 0.961)
}

/// Deterministic noise, the same on every launch and platform (Swift's own hashing is seeded per
/// launch).
enum MapNoise {
    /// 0..1 from a string and a salt.
    static func unit(_ text: String, _ salt: Int = 0) -> Double {
        var hash: UInt64 = 0xcbf29ce484222325 ^ UInt64(bitPattern: Int64(salt))
        for byte in text.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x100000001b3
        }
        return mix(hash)
    }

    /// 0..1 from integers.
    static func unit(_ a: Int, _ b: Int, _ c: Int = 0) -> Double {
        let seed = UInt64(bitPattern: Int64(a)) &* 0x9E3779B97F4A7C15
            ^ UInt64(bitPattern: Int64(b)) &* 0xC2B2AE3D27D4EB4F
            ^ UInt64(bitPattern: Int64(c)) &* 0x165667B19E3779F9
        return mix(seed)
    }

    private static func mix(_ value: UInt64) -> Double {
        var z = value &+ 0x9E3779B97F4A7C15
        z = (z ^ (z >> 30)) &* 0xBF58476D1CE4E5B9
        z = (z ^ (z >> 27)) &* 0x94D049BB133111EB
        z ^= z >> 31
        return Double(z >> 11) / Double(1 << 53)
    }

    /// The same linear congruential stream the menu has always used for its stars.
    struct Stream {
        var seed: UInt64
        mutating func next() -> Double {
            seed = seed &* 6364136223846793005 &+ 1442695040888963407
            return Double(seed >> 33) / 2147483648.0
        }
    }
}

func fract(_ x: Double) -> Double { x - floor(x) }

func smoothstep(_ edge0: Double, _ edge1: Double, _ x: Double) -> Double {
    let t = min(max((x - edge0) / (edge1 - edge0), 0), 1)
    return t * t * (3 - 2 * t)
}

/// Where the sun is: a unit vector in geo space and the same direction in view space.
struct MapSun {
    let geo: (Double, Double, Double)
    let view: (Double, Double, Double)
    let lonDegrees: Double
    let latDegrees: Double

    /// With day and night on the sun crosses the world every four minutes of real time,
    /// quantized to half a degree so the map redraws rarely. Otherwise it stays fixed over the
    /// viewer's upper left.
    static func at(time: Double, dayNight: Bool, projection p: MapProjection) -> MapSun {
        if dayNight {
            var lon = -90 - 360 * time / 240
            lon = (lon / 0.5).rounded() * 0.5
            lon = (lon + 180).truncatingRemainder(dividingBy: 360)
            if lon < 0 { lon += 360 }
            lon -= 180
            let lat = 18.0
            let lonR = lon * .pi / 180
            let latR = lat * .pi / 180
            let geo = (cos(latR) * cos(lonR), cos(latR) * sin(lonR), sin(latR))
            return MapSun(geo: geo, view: p.toView(geo), lonDegrees: lon, latDegrees: lat)
        }
        let n = (0.25 + 0.3025 + 0.4489).squareRoot()
        let view = (-0.50 / n, 0.55 / n, 0.67 / n)
        let geo = p.toGeo(view)
        let lon = atan2(geo.1, geo.0) * 180 / .pi
        let lat = asin(min(max(geo.2, -1), 1)) * 180 / .pi
        return MapSun(geo: geo, view: view, lonDegrees: lon, latDegrees: lat)
    }

    /// The sun's height over a point: positive by day, negative by night.
    func dot(_ p: GeoPoint) -> Double {
        let u = p.unit
        return u.0 * geo.0 + u.1 * geo.1 + u.2 * geo.2
    }

    /// How lit a city is at night, 0 by day.
    func nightLight(_ p: GeoPoint) -> Double {
        smoothstep(0.05, -0.15, dot(p))
    }
}

/// Sunlight, atmosphere and night drawn around and over the globe. Shared by the map and the
/// main menu so both look the same. `weight` fades the globe-only parts as the map flattens.
enum GlobeLighting {
    private static let oceanInner = RGB(hex: 0x1D4E7A)
    private static let oceanMid = RGB(hex: 0x0E2840)
    private static let oceanOuter = RGB(hex: 0x06141F)
    private static let airBlue = RGB(hex: 0x5F8FD0)
    private static let warm = RGB(hex: 0xFFD8A0)
    private static let rim = RGB(hex: 0x7FB2FF)
    private static let nightShade = RGB(hex: 0x020814)
    private static let cityGlow = RGB(hex: 0xFFC872)

    private static func disc(_ c: CGPoint, _ radius: Double) -> Path {
        Path(ellipseIn: CGRect(x: c.x - radius, y: c.y - radius, width: radius * 2, height: radius * 2))
    }

    private static func toward(_ c: CGPoint, _ v: (Double, Double, Double), _ distance: Double) -> CGPoint {
        CGPoint(x: c.x + distance * v.0, y: c.y - distance * v.1)
    }

    /// The glow behind the globe, warmer on the sunlit side when cinematic.
    static func atmosphere(in context: inout GraphicsContext, center c: CGPoint, radius r: Double,
                           sun: MapSun, weight: Double, cinematic: Bool) {
        guard weight > 0.001 else { return }
        let outer = 1.18 * r
        context.fill(disc(c, outer), with: .radialGradient(
            Gradient(stops: [
                .init(color: airBlue.color(0.55 * weight), location: 1 / 1.18),
                .init(color: airBlue.color(0.22 * weight), location: 0.92),
                .init(color: airBlue.color(0), location: 1),
            ]),
            center: c, startRadius: 0, endRadius: outer
        ))
        if cinematic {
            let len = max((sun.view.0 * sun.view.0 + sun.view.1 * sun.view.1).squareRoot(), 1e-6)
            let shifted = toward(c, (sun.view.0 / len, sun.view.1 / len, 0), 0.06 * r)
            context.fill(disc(shifted, outer), with: .radialGradient(
                Gradient(stops: [
                    .init(color: warm.color(0.14 * weight), location: 1 / 1.18),
                    .init(color: warm.color(0), location: 1),
                ]),
                center: shifted, startRadius: 0, endRadius: outer
            ))
        }
    }

    /// The sea on the globe's disc, brightest towards the sun.
    static func ocean(in context: inout GraphicsContext, center c: CGPoint, radius r: Double, sun: MapSun, weight: Double) {
        guard weight > 0.001 else { return }
        let focus = toward(c, sun.view, 0.45 * r)
        context.fill(disc(c, r), with: .radialGradient(
            Gradient(stops: [
                .init(color: oceanInner.color(weight), location: 0),
                .init(color: oceanMid.color(weight), location: 0.6),
                .init(color: oceanOuter.color(weight), location: 1),
            ]),
            center: focus, startRadius: 0, endRadius: 1.35 * r
        ))
    }

    /// Limb darkening, the inner rim and, when cinematic, the sun's highlight and glint.
    static func surface(in context: inout GraphicsContext, center c: CGPoint, radius r: Double,
                        sun: MapSun, weight: Double, cinematic: Bool) {
        guard weight > 0.001 else { return }
        let globe = disc(c, r)
        context.fill(globe, with: .radialGradient(
            Gradient(stops: [
                .init(color: .black.opacity(0), location: 0.70),
                .init(color: .black.opacity(0.35 * weight), location: 0.92),
                .init(color: .black.opacity(0.60 * weight), location: 1),
            ]),
            center: c, startRadius: 0, endRadius: r
        ))
        context.fill(globe, with: .radialGradient(
            Gradient(stops: [
                .init(color: rim.color(0), location: 0.80),
                .init(color: rim.color(0.35 * weight), location: 1),
            ]),
            center: c, startRadius: 0, endRadius: r
        ))
        guard cinematic else { return }
        let focus = toward(c, sun.view, 0.45 * r)
        context.fill(globe, with: .radialGradient(
            Gradient(colors: [.white.opacity(0.10 * weight), .white.opacity(0)]),
            center: focus, startRadius: 0, endRadius: 1.1 * r
        ))
        let hz = sun.view.2 + 1
        let hn = max((sun.view.0 * sun.view.0 + sun.view.1 * sun.view.1 + hz * hz).squareRoot(), 1e-6)
        let glint = toward(c, (sun.view.0 / hn, sun.view.1 / hn, 0), r)
        context.fill(disc(glint, 0.16 * r), with: .radialGradient(
            Gradient(colors: [.white.opacity(0.22 * weight), .white.opacity(0)]),
            center: glint, startRadius: 0, endRadius: 0.16 * r
        ))
    }

    /// The night side as four soft bands along the terminator.
    static func night(in context: inout GraphicsContext, projection p: MapProjection, sun: MapSun, weight: Double) {
        guard weight > 0.001 else { return }
        let lon0 = p.lon0 * 180 / .pi
        for offset in [0.12, 0.05, -0.02, -0.10] {
            let ring = MapProjection.nightRing(sunLon: sun.lonDegrees, sunLat: sun.latDegrees, offset: offset, centerLon: lon0)
            var path = Path()
            p.addPolygon(ring, to: &path)
            context.fill(path, with: .color(nightShade.color(0.13 * weight)))
        }
    }

    /// Warm glows on the night side, added to what is beneath. `height` raises a city onto its
    /// region's tile; `scale` widens the glow on the main menu's small globe.
    static func cityLights(in context: inout GraphicsContext, projection p: MapProjection, sun: MapSun,
                           cities: [WorldMapFile.City], weight: Double,
                           height: (Int) -> Double = { _ in 0 }) {
        guard weight > 0.001 else { return }
        var glow = context
        glow.blendMode = .plusLighter
        var cores = Path()
        for (index, city) in cities.enumerated() {
            let geo = GeoPoint(e4: city.x, city.y)
            let light = sun.nightLight(geo) * weight
            guard light > 0.01 else { continue }
            let (flatPoint, visible) = p.flat(geo)
            guard visible, sun.dot(geo) < 0.2 else { continue }
            if p.t < 0.5 {
                let u = geo.unit
                guard u.0 * p.view.0 + u.1 * p.view.1 + u.2 * p.view.2 > 0 else { continue }
            }
            let point = p.lift(flatPoint, height(index))
            let radius = max(2 + 2.5 * log10(max(Double(city.population), 1) / 1e5), 1.2)
            glow.fill(disc(point, radius * 2), with: .radialGradient(
                Gradient(colors: [cityGlow.color(0.55 * light), cityGlow.color(0)]),
                center: point, startRadius: 0, endRadius: radius * 2
            ))
            if light > 0.3 {
                cores.addEllipse(in: CGRect(x: point.x - 1, y: point.y - 1, width: 2, height: 2))
            }
        }
        glow.fill(cores, with: .color(cityGlow.color(0.9 * weight)))
    }

    /// Darkens the corners of a rectangle.
    static func vignette(in context: inout GraphicsContext, size: CGSize, strength: Double) {
        guard strength > 0.001 else { return }
        let c = CGPoint(x: size.width / 2, y: size.height / 2)
        let reach = (Double(size.width * size.width + size.height * size.height)).squareRoot() / 2
        context.fill(Path(CGRect(origin: .zero, size: size)), with: .radialGradient(
            Gradient(stops: [
                .init(color: .black.opacity(0), location: 0.6),
                .init(color: .black.opacity(0.35 * strength), location: 1),
            ]),
            center: c, startRadius: 0, endRadius: reach
        ))
    }
}

/// Stars behind the globe in three depth layers, from the menu's long-standing seed.
enum Starfield {
    struct Star {
        let x: Double
        let y: Double
        let radius: Double
        let alpha: Double
        let layer: Int
        let rate: Double
        let phase: Double
    }

    static let stars: [Star] = {
        var stream = MapNoise.Stream(seed: 7)
        var list: [Star] = []
        for index in 0..<300 {
            let x = stream.next()
            let y = stream.next()
            let layer = index % 3
            let radius = (0.5 + stream.next() * 0.8) * [0.8, 1.1, 1.5][layer]
            let alpha = (0.10 + stream.next() * 0.35) * [0.7, 0.9, 1.0][layer]
            list.append(Star(x: x, y: y, radius: radius, alpha: alpha, layer: layer,
                             rate: 0.8 + stream.next() * 2.2, phase: stream.next() * 2 * .pi))
        }
        return list
    }()

    /// Stars `from..<to`, each layer shifted by `shift[layer]` points and wrapped across the view.
    /// `time` twinkles them; nil leaves them still.
    static func draw(in context: inout GraphicsContext, size: CGSize, range: Range<Int>,
                     shift: (Double, Double, Double) = (0, 0, 0), time: Double?, weight: Double) {
        guard weight > 0.001, size.width > 0 else { return }
        let width = Double(size.width)
        for index in range where index < stars.count {
            let star = stars[index]
            let offset = star.layer == 0 ? shift.0 : star.layer == 1 ? shift.1 : shift.2
            let x = fract(star.x + offset / width) * width
            let y = star.y * Double(size.height)
            var alpha = star.alpha * weight
            if let time { alpha *= 0.6 + 0.4 * sin(time * star.rate + star.phase) }
            let r = star.radius
            context.fill(Path(ellipseIn: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2)),
                         with: .color(.white.opacity(alpha)))
        }
    }
}

/// Cloud puffs spread over an area of the world, drifting east and wrapping round.
struct CloudField {
    struct Puff {
        let lon: Double
        let lat: Double
        let radius: Double
        /// Four overlapping circles: offset in puff radii and relative size.
        let parts: [(dx: Double, dy: Double, size: Double)]
    }

    let west: Double
    let east: Double
    let puffs: [Puff]

    private static var cache: [String: CloudField] = [:]

    /// The field for a count of puffs over bounds in degrees, built once.
    static func field(count: Int, west: Double, south: Double, east: Double, north: Double) -> CloudField {
        let key = "\(count)|\(west)|\(south)|\(east)|\(north)"
        if let held = cache[key] { return held }
        var stream = MapNoise.Stream(seed: 23)
        var puffs: [Puff] = []
        for _ in 0..<min(count, 90) {
            let lon = west + stream.next() * (east - west)
            let lat = south + stream.next() * (north - south)
            let radius = 0.5 + stream.next()
            var parts: [(dx: Double, dy: Double, size: Double)] = []
            for _ in 0..<4 {
                parts.append((dx: (stream.next() - 0.5) * 1.2, dy: (stream.next() - 0.5) * 0.6, size: 0.55 + stream.next() * 0.45))
            }
            puffs.append(Puff(lon: lon, lat: lat, radius: radius, parts: parts))
        }
        let field = CloudField(west: west, east: east, puffs: puffs)
        cache[key] = field
        return field
    }

    /// A puff's longitude after drifting east at a quarter degree a second.
    func lon(of puff: Puff, time: Double) -> Double {
        let span = max(east - west, 1)
        return west + fract((puff.lon - west + 0.25 * time) / span) * span
    }

    /// One soft circle of a puff or its shadow.
    static func blob(in context: inout GraphicsContext, at c: CGPoint, radius: Double, color: Color) {
        guard radius > 0.3 else { return }
        context.fill(Path(ellipseIn: CGRect(x: c.x - radius, y: c.y - radius, width: radius * 2, height: radius * 2)),
                     with: .radialGradient(Gradient(colors: [color, color.opacity(0)]), center: c, startRadius: 0, endRadius: radius))
    }
}

/// A box standing on the map: a building or a skyline tower. Corners run back left, back right,
/// front right, front left, at its base and its top.
struct MapBox {
    let base: [CGPoint]
    let top: [CGPoint]

    var height: Double { Double(base[3].y - top[3].y) }

    /// Draws the two faces towards the viewer, then the top. Window rects collect into `windows`
    /// so the caller fills them all at once.
    func draw(in context: inout GraphicsContext, side: RGB, front: RGB, roof: RGB, viewCenterX: Double,
              windows: inout Path, outline: Color? = nil) {
        // The left face shows when the box stands right of the view centre, the right face when left.
        let leftFace = Double(base[3].x + base[2].x) / 2 > viewCenterX
        let sideFace = leftFace ? [base[0], base[3], top[3], top[0]] : [base[2], base[1], top[1], top[2]]
        let frontFace = [base[3], base[2], top[2], top[3]]
        context.fill(Self.quad(sideFace), with: .color(side.color()))
        context.fill(Self.quad(frontFace), with: .color(front.color()))
        context.fill(Self.quad(top), with: .color(roof.color()))
        if let outline {
            context.stroke(Self.quad(top), with: .color(outline), lineWidth: 1)
        }
        let rows = Int(height / 5)
        guard rows > 0 else { return }
        for face in [sideFace, frontFace] {
            for row in 0..<rows {
                let v = (Double(row) + 0.5) / Double(rows)
                for u in [0.3, 0.7] {
                    let bottom = Self.mix(face[0], face[1], u)
                    let upper = Self.mix(face[3], face[2], u)
                    let p = Self.mix(bottom, upper, v)
                    windows.addRect(CGRect(x: p.x - 1, y: p.y - 0.75, width: 2, height: 1.5))
                }
            }
        }
    }

    static func quad(_ points: [CGPoint]) -> Path {
        var path = Path()
        path.addLines(points)
        path.closeSubpath()
        return path
    }

    static func mix(_ a: CGPoint, _ b: CGPoint, _ f: Double) -> CGPoint {
        CGPoint(x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f)
    }
}

/// A rolling average of frames a second, for the optional counter.
final class FrameMeter {
    private var stamps: [Double] = []

    func tick(_ time: Double) -> Double {
        if let last = stamps.last, time <= last { return rate }
        stamps.append(time)
        if stamps.count > 40 { stamps.removeFirst(stamps.count - 40) }
        return rate
    }

    var rate: Double {
        guard stamps.count > 1, let first = stamps.first, let last = stamps.last, last > first else { return 0 }
        return Double(stamps.count - 1) / (last - first)
    }

    func reset() { stamps.removeAll() }
}
