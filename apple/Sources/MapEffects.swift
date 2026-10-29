import SwiftUI

/// A colour as components, so walls and faces can be shaded by multiplying it.
struct RGB: Equatable {
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

    static let gold = RGB(hex: 0xE2B04A)
    static let positive = RGB(0.247, 0.749, 0.541)
    static let copper = RGB(0.788, 0.502, 0.290)
    static let blue = RGB(0.259, 0.647, 0.961)
    static let white = RGB(1, 1, 1)
    static let black = RGB(0, 0, 0)
}

/// Deterministic noise, the same on every launch and platform (Swift's own hashing is seeded per
/// launch).
enum MapNoise {
    /// The same linear congruential stream the menu has always used for its stars.
    struct Stream {
        var seed: UInt64
        mutating func next() -> Double {
            seed = seed &* 6364136223846793005 &+ 1442695040888963407
            return Double(seed >> 33) / 2147483648.0
        }
    }
}

/// A small stable hash for seeding per-city and per-division detail, the same as the Android app.
func stableHash(_ text: String) -> Int {
    var h: UInt32 = 0x811C9DC5
    for unit in text.utf16 { h = (h ^ UInt32(unit)) &* 0x01000193 }
    return Int(h & 0x7FFFFFFF)
}

/// Seconds since the app first drew a moving scene, shared by every moving thing so they stay in
/// step. The sun starts over the Americas at noon.
enum MapClock {
    private static let start = Date().timeIntervalSinceReferenceDate
    static var now: Double { seconds(Date()) }
    static func seconds(_ date: Date) -> Double { date.timeIntervalSinceReferenceDate - start }
}

func fract(_ x: Double) -> Double { x - floor(x) }

func smoothstep(_ edge0: Double, _ edge1: Double, _ x: Double) -> Double {
    let t = min(max((x - edge0) / (edge1 - edge0), 0), 1)
    return t * t * (3 - 2 * t)
}

/// Where the sun is overhead, in radians.
struct MapSun: Equatable {
    let lon: Double
    let lat: Double
    let x: Double
    let y: Double
    let z: Double

    init(lon: Double, lat: Double) {
        self.lon = lon
        self.lat = lat
        x = cos(lat) * cos(lon)
        y = cos(lat) * sin(lon)
        z = sin(lat)
    }

    /// How much a place faces the sun: 1 noon, 0 at dusk, negative at night.
    func facing(lon: Double, lat: Double) -> Double {
        cos(lat) * cos(lon) * x + cos(lat) * sin(lon) * y + sin(lat) * z
    }

    func facing(_ p: GeoPoint) -> Double {
        p.cosLat * p.cosLon * x + p.cosLat * p.sinLon * y + p.sinLat * z
    }

    /// The light used when day and night is off: from the upper left, in front of the globe.
    static func fixed(_ p: MapProjection) -> MapSun {
        let n = (0.50 * 0.50 + 0.55 * 0.55 + 0.67 * 0.67).squareRoot()
        let g = p.fromView(-0.50 / n, 0.55 / n, 0.67 / n)
        return MapSun(lon: g.lon, lat: g.lat)
    }

    /// A low sun from the left for the menu, so a broad crescent of night shows its lights.
    static func side(_ p: MapProjection) -> MapSun {
        let n = (0.80 * 0.80 + 0.30 * 0.30 + 0.30 * 0.30).squareRoot()
        let g = p.fromView(-0.80 / n, 0.30 / n, 0.30 / n)
        return MapSun(lon: g.lon, lat: g.lat)
    }

    /// The moving sun: a whole day every four minutes, stepped by half a degree.
    static func moving(_ seconds: Double) -> MapSun {
        let degrees = -90 - 360 * seconds.truncatingRemainder(dividingBy: 240) / 240
        let stepped = floor(degrees * 2) / 2
        return MapSun(lon: stepped * .pi / 180, lat: 18 * .pi / 180)
    }
}

/// Soft round sprites, drawn as radial gradients: alpha stops from the centre out.
enum Sprite {
    case glow, cloud

    var stops: [(Double, Double)] {
        switch self {
        case .glow: return [(0, 1), (0.25, 0.55), (1, 0)]
        case .cloud: return [(0, 0.95), (0.6, 0.85), (0.82, 0.35), (1, 0)]
        }
    }
}

/// Stamps a sprite centred on `c`, `radius` points tall and `aspect` times as wide, tinted and
/// faded. The caller sets any blend mode on the context.
func stamp(_ context: inout GraphicsContext, _ sprite: Sprite, at c: CGPoint, radius: Double, color: RGB, alpha: Double,
           aspect: Double = 1) {
    guard radius >= 0.4, alpha > 0.004 else { return }
    let a = min(max(alpha, 0), 1)
    let gradient = Gradient(stops: sprite.stops.map { Gradient.Stop(color: color.color(a * $0.1), location: $0.0) })
    if aspect == 1 {
        let rect = CGRect(x: c.x - radius, y: c.y - radius, width: radius * 2, height: radius * 2)
        context.fill(Path(ellipseIn: rect), with: .radialGradient(gradient, center: c, startRadius: 0, endRadius: radius))
        return
    }
    var stretched = context
    stretched.translateBy(x: c.x, y: c.y)
    stretched.scaleBy(x: aspect, y: 1)
    let rect = CGRect(x: -radius, y: -radius, width: radius * 2, height: radius * 2)
    stretched.fill(Path(ellipseIn: rect), with: .radialGradient(gradient, center: .zero, startRadius: 0, endRadius: radius))
}

func disc(_ c: CGPoint, _ radius: Double) -> Path {
    Path(ellipseIn: CGRect(x: c.x - radius, y: c.y - radius, width: radius * 2, height: radius * 2))
}

/// Soft light drawn at half the screen's pixel density and stretched over the view: night, the
/// globe's shading, the atmosphere, the vignette. These change smoothly across the screen, so a
/// quarter of the pixels looks the same and costs a fraction. It covers a margin round the view
/// too, so a translated drawing (the menu's globe sits off centre) never shows its edge. Redrawn
/// only when its key or the view's size changes.
final class LightLayer {
    private var key: [AnyHashable]?
    private var frame = CGRect.zero
    private var image: Image?

    /// Draws `build` through the layer; `build` gets the view's full size. Draws it directly when
    /// the layer cannot be rendered off screen.
    func draw(in context: inout GraphicsContext, size: CGSize, displayScale: Double, key: [AnyHashable],
              build: @escaping (inout GraphicsContext, CGSize) -> Void) {
        let pad = (max(size.width, size.height) * 0.25).rounded(.down)
        let frame = CGRect(x: -pad, y: -pad, width: size.width + 2 * pad, height: size.height + 2 * pad)
        if image == nil || key != self.key || frame != self.frame {
            image = Self.render(frame: frame, size: size, scale: max(displayScale, 1) / 2, build: build)
            self.key = key
            self.frame = frame
        }
        if let image {
            context.draw(image, in: frame)
        } else {
            build(&context, size)
        }
    }

    private static func render(frame: CGRect, size: CGSize, scale: Double,
                               build: @escaping (inout GraphicsContext, CGSize) -> Void) -> Image? {
        guard Thread.isMainThread, frame.width >= 1, frame.height >= 1 else { return nil }
        var image: Image?
        MainActor.assumeIsolated {
            let content = Canvas { context, _ in
                context.translateBy(x: -frame.minX, y: -frame.minY)
                build(&context, size)
            }
            .frame(width: frame.width, height: frame.height)
            let renderer = ImageRenderer(content: content)
            renderer.scale = scale
            renderer.isOpaque = false
            if let cgImage = renderer.cgImage { image = Image(decorative: cgImage, scale: scale).interpolation(.medium) }
        }
        return image
    }
}

// MARK: - Land

/// Land colours by climate, so the globe reads as a planet rather than a silhouette: temperate,
/// dry, tropical, polar.
enum Biomes {
    static let colors = [RGB(hex: 0x3B4A3A), RGB(hex: 0x5A5040), RGB(hex: 0x2E4733), RGB(hex: 0x8E9AA4)]
    static let grey = RGB(hex: 0x25282B)

    /// Which climate band a country's centre falls in.
    static func of(_ ring: GeoRing) -> Int {
        let lat = asin(min(max(ring.center.2, -1), 1)) * 180 / .pi
        let lon = atan2(ring.center.1, ring.center.0) * 180 / .pi
        let a = abs(lat)
        if a > 62 { return 3 }
        if a < 14 { return 2 }
        // The great deserts: Sahara and Arabia, central Asia, Australia, the American southwest.
        if (14...35).contains(lat) && (-18...60).contains(lon) { return 1 }
        if (35...50).contains(lat) && (50...110).contains(lon) { return 1 }
        if (-35 ... -18).contains(lat) && (115...145).contains(lon) { return 1 }
        if (22...36).contains(lat) && (-118 ... -100).contains(lon) { return 1 }
        return 0
    }

    /// Countries grouped by climate, as four paths.
    static func paths(_ map: WorldMap, _ p: MapProjection) -> [Path] {
        var paths = [Path(), Path(), Path(), Path()]
        for country in map.countries {
            guard let main = country.rings.max(by: { $0.points.count < $1.points.count }) else { continue }
            let band = of(main)
            for ring in country.rings { p.addPolygon(ring, to: &paths[band]) }
        }
        return paths
    }

    static func draw(_ paths: [Path], in context: inout GraphicsContext, grey amount: Double) {
        for (index, path) in paths.enumerated() {
            context.fill(path, with: .color(colors[index].mixed(with: grey, amount).color()), style: FillStyle(eoFill: true))
        }
    }
}

// MARK: - Space and the globe

/// A deterministic field of stars in three depths.
final class StarField {
    private var x: [Double] = []
    private var y: [Double] = []
    private var radius: [Double] = []
    private var alpha: [Double] = []
    private var depth: [Int] = []
    private var phase: [Double] = []

    static let shared = StarField(count: 220, seed: 7)
    static let lander = StarField(count: 300, seed: 7)

    init(count: Int, seed: UInt64) {
        var stream = MapNoise.Stream(seed: seed)
        for _ in 0..<count {
            x.append(stream.next())
            y.append(stream.next())
            let d = min(Int(stream.next() * 3), 2)
            depth.append(d)
            radius.append(0.5 + stream.next() * (0.6 + Double(d) * 0.5))
            alpha.append(0.10 + stream.next() * (0.25 + Double(d) * 0.12))
            phase.append(stream.next() * 6.28)
        }
    }

    /// Stars drift left by depth at `drift` points a second; every fifth twinkles.
    func draw(in context: inout GraphicsContext, size: CGSize, seconds: Double, drift: Double, twinkle: Bool, count: Int? = nil) {
        let w = Double(size.width)
        let h = Double(size.height)
        guard w > 0 else { return }
        for i in 0..<min(count ?? x.count, x.count) {
            let speed = drift * (1 + Double(depth[i]) * 1.5)
            var sx = (x[i] * w - seconds * speed).truncatingRemainder(dividingBy: w)
            if sx < 0 { sx += w }
            var a = alpha[i]
            if twinkle && i % 5 == 0 { a *= 0.55 + 0.45 * sin(seconds * (1.3 + Double(depth[i])) + phase[i]) }
            context.fill(disc(CGPoint(x: sx, y: y[i] * h), radius[i]), with: .color(.white.opacity(a)))
        }
    }
}

/// Sunlight, atmosphere and night drawn around and over the globe. Shared by the map and the
/// main menu so both look the same.
enum GlobeLighting {
    private static let ocean = RGB(hex: 0x0E2840)

    /// The glow of the atmosphere behind the globe. Soft, so it is drawn into a small `LightLayer`.
    static func atmosphere(in context: inout GraphicsContext, projection p: MapProjection, sun: MapSun, lighting: Int) {
        let t = p.t
        let c = CGPoint(x: p.cx, y: p.cy)
        let r = p.r
        let sv = p.toView(lon: sun.lon, lat: sun.lat)
        let fade = 1 - t
        guard lighting > 0 && fade > 0.01 else { return }
        let outer = r * 1.06
        let edge = r / outer
        context.fill(disc(c, outer), with: .radialGradient(Gradient(stops: [
            .init(color: .clear, location: 0),
            .init(color: RGB(hex: 0x7FB0F0).color(0.55 * fade), location: edge * 0.985),
            .init(color: RGB(hex: 0x4F80C0).color(0.14 * fade), location: edge + (1 - edge) * 0.25),
            .init(color: .clear, location: 1),
        ]), center: c, startRadius: 0, endRadius: outer))
        if lighting == 2 {
            let warm = CGPoint(x: c.x + sv.0 * 0.06 * r, y: c.y - sv.1 * 0.06 * r)
            context.fill(disc(warm, outer), with: .radialGradient(Gradient(stops: [
                .init(color: .clear, location: 0),
                .init(color: RGB(hex: 0xFFD8A0).color(0.10 * fade), location: 0.93),
                .init(color: .clear, location: 1),
            ]), center: warm, startRadius: 0, endRadius: outer))
        }
    }

    /// The ocean, lit from the sun: a crisp disc that grows to fill the view as the map flattens.
    static func back(in context: inout GraphicsContext, projection p: MapProjection, sun: MapSun, lighting: Int, size: CGSize) {
        let t = p.t
        let c = CGPoint(x: p.cx, y: p.cy)
        let r = p.r
        let sv = p.toView(lon: sun.lon, lat: sun.lat)
        let oceanRadius = r / max(1 - t, 0.03)
        if t >= 0.999 {
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(ocean.color()))
        } else if lighting > 0 {
            let hl = CGPoint(x: c.x + 0.45 * r * sv.0, y: c.y - 0.45 * r * sv.1)
            context.fill(disc(c, oceanRadius), with: .radialGradient(Gradient(stops: [
                .init(color: RGB(hex: 0x1D4E7A).mixed(with: ocean, t).color(), location: 0),
                .init(color: ocean.color(), location: 0.6),
                .init(color: RGB(hex: 0x06141F).mixed(with: ocean, t).color(), location: 1),
            ]), center: hl, startRadius: 0, endRadius: max(r * 1.35, oceanRadius)))
        } else {
            context.fill(disc(c, oceanRadius), with: .color(ocean.color()))
        }
    }

    /// Shading over the globe once land is drawn: limb, rim, sun highlight and glint.
    static func front(in context: inout GraphicsContext, projection p: MapProjection, sun: MapSun, lighting: Int) {
        let fade = 1 - p.t
        guard lighting > 0, fade >= 0.01 else { return }
        let c = CGPoint(x: p.cx, y: p.cy)
        let r = p.r
        let globe = disc(c, r)
        context.fill(globe, with: .radialGradient(Gradient(stops: [
            .init(color: .clear, location: 0),
            .init(color: .clear, location: 0.78),
            .init(color: .black.opacity(0.30 * fade), location: 0.95),
            .init(color: .black.opacity(0.5 * fade), location: 1),
        ]), center: c, startRadius: 0, endRadius: r))
        context.fill(globe, with: .radialGradient(Gradient(stops: [
            .init(color: .clear, location: 0),
            .init(color: .clear, location: 0.90),
            .init(color: RGB(hex: 0x7FB2FF).color(0.22 * fade), location: 1),
        ]), center: c, startRadius: 0, endRadius: r))
        guard lighting == 2 else { return }
        let sv = p.toView(lon: sun.lon, lat: sun.lat)
        guard sv.2 > -0.2 else { return }
        let hl = CGPoint(x: c.x + 0.45 * r * sv.0, y: c.y - 0.45 * r * sv.1)
        context.fill(globe, with: .radialGradient(Gradient(colors: [.white.opacity(0.05 * fade), .clear]),
                                                  center: hl, startRadius: 0, endRadius: r * 0.9))
        let hz = sv.2 + 1
        let hn = max((sv.0 * sv.0 + sv.1 * sv.1 + hz * hz).squareRoot(), 1e-6)
        let glint = CGPoint(x: c.x + r * sv.0 / hn, y: c.y - r * sv.1 / hn)
        stamp(&context, .glow, at: glint, radius: r * 0.22, color: RGB(hex: 0xDDEBFF),
              alpha: 0.14 * fade * smoothstep(-0.2, 0.3, sv.2))
    }

    /// The edges of night, from twilight in. Twenty thin steps each darken a little, so dusk
    /// fades smoothly instead of showing bands; the deepest night ends about three quarters dark.
    static let nightEdges: [Double] = (0..<20).map { 0.16 - 0.46 * Double($0) / 19 }

    static func night(_ paths: [Path], in context: inout GraphicsContext) {
        // Dusk first: a warm band along the edge of night, which the darker steps then cover.
        let dusk = RGB(hex: 0xFF9A4A).color(0.018)
        for path in paths.prefix(6) { context.fill(path, with: .color(dusk)) }
        let shade = RGB(hex: 0x02060F).color(0.065)
        for path in paths { context.fill(path, with: .color(shade)) }
    }

    /// The night side as rings on the sphere, from the soft edge in. Each is the set of places
    /// where the sun is below one edge of the horizon, closed round the dark side.
    static func nightRings(_ sun: MapSun, _ p: MapProjection) -> [GeoRing] {
        p.t < 0.5 ? nightCaps(sun, p) : nightBands(sun, p.lon0)
    }

    /// On the globe: each edge of night is a circle on the sphere round the point opposite the
    /// sun. The part of it facing the viewer is closed along the globe's rim on the dark side, so
    /// the shape is exactly the night the viewer can see.
    private static func nightCaps(_ sun: MapSun, _ p: MapProjection) -> [GeoRing] {
        let s = p.toView(lon: sun.lon, lat: sun.lat)
        // Two directions at right angles to the sun.
        var ux = s.1
        var uy = -s.0
        var n = (ux * ux + uy * uy).squareRoot()
        if n < 1e-6 {
            ux = 1
            uy = 0
            n = 1
        }
        ux /= n
        uy /= n
        let vx = -s.2 * uy
        let vy = s.2 * ux
        let vz = s.0 * uy - s.1 * ux
        let samples = 120
        return nightEdges.compactMap { c -> GeoRing? in
            let k = (1 - c * c).squareRoot()
            var xs = [Double](repeating: 0, count: samples)
            var ys = xs
            var zs = xs
            for i in 0..<samples {
                let a = 2 * Double.pi * Double(i) / Double(samples)
                xs[i] = c * s.0 + k * (cos(a) * ux + sin(a) * vx)
                ys[i] = c * s.1 + k * (cos(a) * uy + sin(a) * vy)
                zs[i] = c * s.2 + k * sin(a) * vz
            }
            var flat: [Int] = []
            flat.reserveCapacity(400)
            func add(_ x: Double, _ y: Double, _ z: Double) {
                let g = p.fromView(x, y, z)
                flat.append(Int(g.lon * 180 / .pi * 10_000))
                flat.append(Int(g.lat * 180 / .pi * 10_000))
            }
            let front = zs.filter { $0 >= 0 }.count
            if front == 0 { return nil }
            if front == samples {
                for i in 0..<samples { add(xs[i], ys[i], zs[i]) }
                return GeoRing(flat)
            }
            // Start just after the circle comes round from behind the globe.
            guard let first = (0..<samples).first(where: { zs[$0] >= 0 && zs[($0 - 1 + samples) % samples] < 0 }) else { return nil }
            func crossing(_ a: Int, _ b: Int) -> (Double, Double) {
                let f = zs[a] / (zs[a] - zs[b])
                let x = xs[a] + (xs[b] - xs[a]) * f
                let y = ys[a] + (ys[b] - ys[a]) * f
                let m = max((x * x + y * y).squareRoot(), 1e-9)
                return (x / m, y / m)
            }
            let start = crossing((first - 1 + samples) % samples, first)
            add(start.0, start.1, 1e-4)
            var i = first
            while zs[i] >= 0 {
                add(xs[i], ys[i], zs[i])
                i = (i + 1) % samples
            }
            let end = crossing((i - 1 + samples) % samples, i)
            add(end.0, end.1, 1e-4)
            // Back along the rim through the dark side, away from the sun.
            let a0 = atan2(end.1, end.0)
            let a1 = atan2(start.1, start.0)
            var sweep = a1 - a0
            if sweep <= 0 { sweep += 2 * .pi }
            let mid = a0 + sweep / 2
            if cos(mid) * s.0 + sin(mid) * s.1 > 0 { sweep -= 2 * .pi }
            let steps = max(2, Int(abs(sweep) / (4 * Double.pi / 180)))
            for step in 1..<steps {
                let a = a0 + sweep * Double(step) / Double(steps)
                add(cos(a), sin(a), 1e-4)
            }
            return GeoRing(flat)
        }
    }

    /// On the flat map: night as a band from its edge to the dark pole, across every longitude.
    private static func nightBands(_ sun: MapSun, _ lon0: Double) -> [GeoRing] {
        let ds = abs(sun.lat) < 2 * .pi / 180 ? (sun.lat >= 0 ? 2.0 : -2.0) * .pi / 180 : sun.lat
        let pole = ds > 0 ? -89.9 : 89.9
        let centre = lon0 * 180 / .pi
        return nightEdges.map { c in
            var flat: [Int] = []
            flat.reserveCapacity(250)
            var lon = centre - 180
            while lon <= centre + 180.001 {
                let dl = lon * .pi / 180 - sun.lon
                let a = cos(ds) * cos(dl)
                let b = sin(ds)
                let rr = (a * a + b * b).squareRoot()
                // Rising from the dark pole, night ends where the sun's height first passes c.
                let lat: Double
                if c >= rr {
                    lat = -pole
                } else if c <= -rr {
                    lat = pole
                } else {
                    let alpha = atan2(b, a)
                    let k = acos(c / rr)
                    lat = min(max((ds > 0 ? alpha - k : alpha + k) * 180 / .pi, -89.9), 89.9)
                }
                flat.append(Int(lon * 10_000))
                flat.append(Int(lat * 10_000))
                lon += 3
            }
            lon = centre + 180
            while lon >= centre - 180.001 {
                flat.append(Int(lon * 10_000))
                flat.append(Int(pole * 10_000))
                lon -= 3
            }
            return GeoRing(flat)
        }
    }

    /// Cities glowing on the night side, `lift` points up, glows sized by `scale`.
    static func cityLights(in context: inout GraphicsContext, cities: [WorldMapFile.City], projection p: MapProjection,
                           sun: MapSun, lift: Double, scale: Double) {
        var glow = context
        glow.blendMode = .plusLighter
        let core = RGB(hex: 0xFFE6B0)
        for city in cities {
            let geo = GeoPoint(e4: city.x, city.y)
            let light = smoothstep(0.05, -0.15, sun.facing(geo))
            guard light > 0, let at = p.at(lon: geo.lon, lat: geo.lat, height: lift) else { continue }
            let radius = (2 + 2.5 * log10(max(Double(city.population) / 1e5, 1))) * scale
            stamp(&glow, .glow, at: at, radius: radius * 1.3, color: RGB(hex: 0xFFC872), alpha: 0.6 * light)
            context.fill(disc(at, scale), with: .color(core.color(0.9 * light)))
        }
    }
}

// MARK: - Buildings

/// A box from `floor` to `top` points up at the untilted point (gx, gy): the two sides facing
/// the viewer, then the top, with rows of lit windows on the front.
func drawBox(_ context: inout GraphicsContext, projection p: MapProjection, gx: Double, gy: Double, half: Double,
             floor: Double, top h: Double, color: RGB, windows: Double, outline: Color? = nil) {
    func at(_ dx: Double, _ dy: Double, _ up: Double) -> CGPoint { p.lift(CGPoint(x: gx + dx, y: gy + dy), up) }
    let b0 = at(-half, half, floor)
    let b1 = at(half, half, floor)
    let t0 = at(-half, half, h)
    let t1 = at(half, half, h)
    let tb0 = at(-half, -half, h)
    let tb1 = at(half, -half, h)
    context.fill(quad([b0, b1, t1, t0]), with: .color(color.scaled(0.58).color()))
    // The side towards the view centre shows.
    let left = gx > p.cx
    let sb = left ? at(-half, -half, floor) : at(half, -half, floor)
    context.fill(quad(left ? [b0, sb, tb0, t0] : [b1, sb, tb1, t1]), with: .color(color.scaled(0.80).color()))
    let roof = quad([t0, t1, tb1, tb0])
    context.fill(roof, with: .color(color.scaled(1.15).color()))
    if let outline { context.stroke(roof, with: .color(outline), lineWidth: 1) }
    guard windows > 0.01 else { return }
    let rows = Int((b0.y - t0.y) / 5)
    guard rows > 1 else { return }
    var lit = Path()
    for row in 1..<rows {
        let f = Double(row) / Double(rows)
        let y0 = b0.y + (t0.y - b0.y) * f
        let xa = b0.x + (t0.x - b0.x) * f
        let xb = b1.x + (t1.x - b1.x) * f
        let w = xb - xa
        lit.addRect(CGRect(x: xa + w * 0.18, y: y0, width: w * 0.22, height: 1.4))
        lit.addRect(CGRect(x: xa + w * 0.60, y: y0, width: w * 0.22, height: 1.4))
    }
    context.fill(lit, with: .color(RGB(hex: 0xFFD27A).color(windows)))
}

func quad(_ points: [CGPoint]) -> Path {
    var path = Path()
    path.addLines(points)
    path.closeSubpath()
    return path
}

/// Heavy industry smokes.
func smokes(_ sector: String) -> Bool {
    ["extraction", "energy", "chemical_industries", "manufacturing", "construction"].contains(sector)
}

// MARK: - Moving pieces

/// A road on screen. `lengths` is the distance along it on the ground, in degrees, so traffic
/// keeps its pace and places whatever the camera does.
struct MapPolyline {
    let points: [CGPoint]
    let lengths: [Double]
    /// The same points on the untilted ground, for vehicles standing on the road in 3D.
    let grounds: [CGPoint]
    var total: Double { lengths.last ?? 0 }

    init(_ points: [CGPoint], lengths: [Double], grounds: [CGPoint]? = nil) {
        self.points = points
        self.lengths = lengths
        self.grounds = grounds ?? points
    }

    /// The untilted ground point a distance along the line.
    func ground(at distance: Double) -> CGPoint {
        Self.interpolate(grounds, lengths, distance)
    }

    private static func interpolate(_ points: [CGPoint], _ lengths: [Double], _ distance: Double) -> CGPoint {
        var low = 0
        var high = lengths.count - 1
        while high - low > 1 {
            let mid = (low + high) / 2
            if lengths[mid] <= distance { low = mid } else { high = mid }
        }
        let a = points[low]
        let b = points[high]
        let f = min(max((distance - lengths[low]) / max(lengths[high] - lengths[low], 1e-9), 0), 1)
        return CGPoint(x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f)
    }

    /// The point a distance along the line.
    func point(at distance: Double) -> CGPoint {
        var low = 0
        var high = lengths.count - 1
        while high - low > 1 {
            let mid = (low + high) / 2
            if lengths[mid] <= distance { low = mid } else { high = mid }
        }
        let a = points[low]
        let b = points[high]
        let f = min(max((distance - lengths[low]) / max(lengths[high] - lengths[low], 1e-9), 0), 1)
        return CGPoint(x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f)
    }

    /// Lines on screen `lift` points up, broken where they leave the globe or jump, kept when any
    /// of them is on screen and they run at least 20 points.
    static func build(_ p: MapProjection, _ rings: [GeoRing], lift: Double, size: CGSize) -> [MapPolyline] {
        var out: [MapPolyline] = []
        let jump = p.r * 0.5
        let bounds = CGRect(origin: .zero, size: size).insetBy(dx: -50, dy: -50)
        for ring in rings where ring.points.count >= 2 && p.mayShow(ring) {
            var points: [CGPoint] = []
            var grounds: [CGPoint] = []
            var lengths: [Double] = []
            var screen = 0.0
            var lastGeo: GeoPoint?
            var onScreen = false
            for point in ring.points {
                guard let flat = p.ground(lon: point.lon, lat: point.lat) else { continue }
                let at = p.lift(flat, lift)
                if let last = points.last, abs(at.x - last.x) + abs(at.y - last.y) > jump { continue }
                if bounds.contains(at) { onScreen = true }
                if let last = points.last, let geo = lastGeo {
                    let dLon = (point.lon - geo.lon) * cos(point.lat)
                    let dLat = point.lat - geo.lat
                    lengths.append(lengths[lengths.count - 1] + (dLon * dLon + dLat * dLat).squareRoot() * 180 / .pi)
                    screen += Double(hypot(at.x - last.x, at.y - last.y))
                } else {
                    lengths.append(0)
                }
                lastGeo = point
                points.append(at)
                grounds.append(flat)
            }
            if points.count >= 2 && onScreen && screen > 20 { out.append(MapPolyline(points, lengths: lengths, grounds: grounds)) }
        }
        return out
    }
}

/// A rolling average of frames a second, for the optional counter.
final class FrameMeter {
    private var stamps: [Double] = []

    func tick(_ time: Double) -> Double {
        if let last = stamps.last, time <= last { return rate }
        stamps.append(time)
        if stamps.count > 30 { stamps.removeFirst(stamps.count - 30) }
        return rate
    }

    var rate: Double {
        guard stamps.count > 1, let first = stamps.first, let last = stamps.last, last > first else { return 0 }
        return Double(stamps.count - 1) / (last - first)
    }

    func reset() { stamps.removeAll() }
}
