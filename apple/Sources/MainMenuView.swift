import SwiftUI

private func scenarioTint(_ id: String) -> Color {
    switch id {
    case "texas-1981": return Color(red: 0.898, green: 0.376, blue: 0.369)
    case "silicon-valley-1995": return Color(red: 0.259, green: 0.647, blue: 0.961)
    case "ruhr-rhine-1991": return Color(red: 0.247, green: 0.749, blue: 0.541)
    default: return Theme.gold
    }
}

/// The menu's world: a lit globe turning in space with night and city lights on its far side,
/// trade routes arcing between the great cities, satellites and the sun's glare. Each
/// scenario glows and is named as it turns to face the viewer. It draws the same still layer as
/// the map, so the two always look alike.
struct GlobeBackdrop: View {
    var offsetX: CGFloat = 0.22
    var menuEdge: CGFloat = 0
    /// The world file is read in the background at launch; the globe appears when it is ready.
    @State private var map = WorldMap.sharedIfReady

    var body: some View {
        if let map {
            GlobeScene(map: map, offsetX: offsetX, menuEdge: menuEdge)
        } else {
            Color.clear.task { map = await Task.detached(priority: .userInitiated) { WorldMap.shared }.value }
        }
    }
}

private struct GlobeScene: View {
    let map: WorldMap
    var offsetX: CGFloat = 0.22
    /// Where the menu column ends, in points from the left. Scenario names never show under it,
    /// where they would read through the gaps between the buttons.
    var menuEdge: CGFloat = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.displayScale) private var displayScale
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @AppStorage(GraphicsKeys.preset) private var preset = GraphicsPreset.high.rawValue
    @AppStorage(GraphicsKeys.lighting) private var lighting = 2
    @AppStorage(GraphicsKeys.cityLights) private var cityLights = true
    @AppStorage(GraphicsKeys.clouds) private var clouds = 1
    @AppStorage(GraphicsKeys.frameRate) private var frameRate = 60
    @State private var geometry = MapGeometryCache()

    private static let names: [String: String] = {
        var result: [String: String] = [:]
        for scenario in ScenarioCatalogue.all { result[scenario.id] = scenario.name }
        return result
    }()

    /// The world's great trading cities, longitude, latitude and millions of people, for the
    /// trade routes and night lights. The scenario maps only carry their own region's cities.
    private static let hubData: [Double] = [
        -74.0, 40.7, 19.0, -87.6, 41.9, 9.5, -118.2, 34.1, 13.0, -95.4, 29.8, 7.0, -79.4, 43.7, 6.2, -99.1, 19.4, 21.0,
        -46.6, -23.5, 22.0, -43.2, -22.9, 13.0, -58.4, -34.6, 15.0, -77.0, -12.0, 10.0, -74.1, 4.7, 11.0, -122.4, 37.8, 7.0,
        -0.1, 51.5, 14.0, 2.35, 48.9, 11.0, 13.4, 52.5, 6.0, 4.9, 52.4, 2.5, 6.96, 50.9, 3.5, 12.5, 41.9, 4.3, -3.7, 40.4, 6.7,
        37.6, 55.8, 12.5, 28.97, 41.0, 15.0, 31.2, 30.0, 21.0, 3.4, 6.5, 15.0, 36.8, -1.3, 5.0, 28.0, -26.2, 9.0,
        55.3, 25.3, 3.5, 51.4, 35.7, 9.0, 72.9, 19.1, 21.0, 77.2, 28.6, 30.0, 88.4, 22.6, 15.0, 100.5, 13.8, 11.0,
        103.8, 1.35, 6.0, 106.8, -6.2, 11.0, 114.2, 22.3, 7.5, 121.5, 31.2, 27.0, 116.4, 39.9, 21.0, 127.0, 37.6, 10.0,
        139.7, 35.7, 37.0, 135.5, 34.7, 19.0, 151.2, -33.9, 5.3, 144.9, -37.8, 5.0, 174.8, -36.8, 1.7,
    ]
    private static let hubs: [(geo: GeoPoint, people: Double)] = stride(from: 0, to: hubData.count, by: 3).map {
        (geo: GeoPoint(lonDegrees: hubData[$0], latDegrees: hubData[$0 + 1]), people: hubData[$0 + 2])
    }
    private static let gold = RGB(hex: 0xE8B44C)
    private static let arcSlots = 9
    private static let arcLife = 4.5

    private var animate: Bool { animations && !reduceMotion && scenePhase == .active }

    private var look: MapLook {
        MapLook(detail: 0, tilt: 0, extrude: false, lighting: lighting, dayNight: false, cityLights: cityLights,
                clouds: clouds, traffic: 0, buildings: 0, frameRate: frameRate, animations: animations, showFps: false,
                ultra: preset == GraphicsPreset.ultra.rawValue)
    }

    var body: some View {
        // The globe turns two degrees a second, smooth at 30 frames; only Unlimited asks for more.
        TimelineView(.animation(minimumInterval: GraphicsSettings.frameInterval(frameRate == 0 ? 0 : min(frameRate, 30)),
                                paused: !animate)) { timeline in
            let seconds = animate ? MapClock.seconds(timeline.date) : 0
            // The real clouds are a Metal layer between the globe and what glows over it.
            ZStack {
                Canvas { context, size in drawBack(&context, size: size, seconds: seconds) }
                    .drawingGroup()
                if clouds > 0 {
                    GeometryReader { proxy in CloudLayer(sky: sky(size: proxy.size, seconds: seconds)) }
                }
                Canvas { context, size in drawFront(&context, size: size, seconds: seconds) }
                    .drawingGroup()
            }
        }
        .accessibilityHidden(true)
    }

    private static func spin(_ seconds: Double) -> Double {
        (((-82 + seconds * 2.2) + 540).truncatingRemainder(dividingBy: 360)) - 180
    }

    private static func camera(_ seconds: Double) -> MapCamera {
        MapCamera(lon: spin(seconds), lat: 24 + 4 * sin(seconds / 23), zoom: 1.25)
    }

    /// Where the globe sits in the view: right of centre, a little low.
    private func origin(_ size: CGSize) -> CGPoint { CGPoint(x: size.width * offsetX, y: size.height * 0.08) }

    /// The clouds over the turning globe, sped up so they visibly move at this distance.
    private func sky(size: CGSize, seconds: Double) -> CloudSky {
        let camera = Self.camera(seconds)
        return CloudSky(camera: camera, size: size, tilt: 0, origin: origin(size),
                        sun: MapSun.side(MapProjection(camera: camera, size: size)), seconds: seconds * 3, amount: clouds,
                        bounds: map.playBounds, altitude: 0, zoom: 1, shadows: false)
    }

    /// Stars, the sun's glare, the globe and the cities' night lights, under the clouds.
    private func drawBack(_ context: inout GraphicsContext, size: CGSize, seconds: Double) {
        let look = self.look
        let cinematic = look.lighting == 2
        let stars = look.ultra ? 300 : cinematic ? 220 : 120
        StarField.lander.draw(in: &context, size: size, seconds: seconds, drift: animations ? 1 : 0, twinkle: cinematic, count: stars)
        let camera = Self.camera(seconds)
        var scene = context
        let at = origin(size)
        scene.translateBy(x: at.x, y: at.y)
        let g = geometry.of(map: map, camera: camera, size: size, tilt: 0, detail: 0)
        let p = g.projection
        let sun = MapSun.side(p)
        if cinematic { drawGlare(&scene, projection: p, sun: sun) }
        MapScene(map: map, camera: camera, look: look, tint: .gold, shading: [:], heights: [:], selected: nil,
                 divisions: [], sun: sun, space: false, displayScale: displayScale)
            .drawStatic(in: &scene, geometry: g)
        if look.cityLights && look.lighting > 0 { drawHubLights(&scene, projection: p, sun: sun) }
    }

    /// The scenarios, trade routes, satellites and vignette, over the clouds.
    private func drawFront(_ context: inout GraphicsContext, size: CGSize, seconds: Double) {
        let look = self.look
        var scene = context
        let at = origin(size)
        scene.translateBy(x: at.x, y: at.y)
        let p = geometry.of(map: map, camera: Self.camera(seconds), size: size, tilt: 0, detail: 0).projection
        drawScenarios(&scene, projection: p, clearOf: menuEdge - at.x)
        if look.lighting > 0 {
            drawArcs(&scene, projection: p, seconds: seconds)
            drawSatellites(&scene, projection: p, seconds: seconds)
        }
        if look.lighting > 0 {
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .radialGradient(Gradient(stops: [
                .init(color: .clear, location: 0),
                .init(color: .clear, location: 0.7),
                .init(color: .black.opacity(0.35), location: 1),
            ]), center: CGPoint(x: size.width / 2, y: size.height / 2), startRadius: 0,
                endRadius: max(size.width, size.height) * 0.72))
        }
    }

    /// The trading cities glowing where night has fallen on them.
    private func drawHubLights(_ context: inout GraphicsContext, projection p: MapProjection, sun: MapSun) {
        var glow = context
        glow.blendMode = .plusLighter
        for hub in Self.hubs {
            let light = smoothstep(0.05, -0.15, sun.facing(hub.geo))
            guard light > 0, let at = p.at(lon: hub.geo.lon, lat: hub.geo.lat) else { continue }
            let radius = 3 + 2.2 * log(1 + hub.people)
            stamp(&glow, .glow, at: at, radius: radius * 1.1, color: RGB(hex: 0xFFC872), alpha: 0.7 * light)
            context.fill(disc(at, 1.6), with: .color(RGB(hex: 0xFFF1D0).color(light)))
        }
    }

    /// The sun off the globe's upper left, and faint lens ghosts across it. A small hard sun with
    /// a tight glow, not a wash of light across the scene.
    private func drawGlare(_ context: inout GraphicsContext, projection p: MapProjection, sun: MapSun) {
        let sv = p.toView(lon: sun.lon, lat: sun.lat)
        let n = max((sv.0 * sv.0 + sv.1 * sv.1).squareRoot(), 1e-6)
        let r = p.r
        let centre = CGPoint(x: p.cx, y: p.cy)
        let at = CGPoint(x: centre.x + sv.0 / n * r * 1.25, y: centre.y - sv.1 / n * r * 1.25)
        context.fill(disc(at, r * 0.22), with: .radialGradient(Gradient(stops: [
            .init(color: RGB(hex: 0xFFFBF0).color(), location: 0),
            .init(color: RGB(hex: 0xFFF0D0).color(0.9), location: 0.16),
            .init(color: RGB(hex: 0xFFD890).color(0.10), location: 0.3),
            .init(color: .clear, location: 1),
        ]), center: at, startRadius: 0, endRadius: r * 0.22))
        for (along, radius) in [(0.6, 0.05), (1.35, 0.09), (1.8, 0.03)] {
            let ghost = CGPoint(x: at.x + (centre.x - at.x) * along, y: at.y + (centre.y - at.y) * along)
            context.fill(disc(ghost, r * radius), with: .color(RGB(hex: 0xFFE2B0).color(0.035)))
        }
    }

    /// Each scenario's area glows and is named as it turns to face the viewer.
    /// Names fade out as they reach `edge`, the menu column's edge in the globe's coordinates.
    private func drawScenarios(_ context: inout GraphicsContext, projection p: MapProjection, clearOf edge: CGFloat) {
        for area in map.scenarioAreas {
            var facing = -1.0
            for ring in area.rings {
                let c = ring.center
                facing = max(facing, c.0 * p.view.0 + c.1 * p.view.1 + c.2 * p.view.2)
            }
            let x = min(max((facing - 0.35) / 0.5, 0), 1)
            let glow = x * x * (3 - 2 * x)
            if glow <= 0 { continue }
            let tint = scenarioTint(area.id)
            let path = p.path(area.rings)
            context.stroke(path, with: .color(tint.opacity(0.16 * glow)), lineWidth: 6)
            context.fill(path, with: .color(tint.opacity(0.55 * glow)), style: FillStyle(eoFill: true))
            context.stroke(path, with: .color(Color.white.opacity(0.75 * glow)), lineWidth: 1.6)
            guard let anchor = area.rings.max(by: { $0.points.count < $1.points.count }),
                  let name = Self.names[area.id] else { continue }
            let c = anchor.center
            let lonDeg = atan2(c.1, c.0) * 180 / .pi
            let latDeg = asin(min(max(c.2, -1), 1)) * 180 / .pi
            let (at, front) = p.project(GeoPoint(lonDegrees: lonDeg, latDegrees: latDeg))
            if front {
                let label = Text(name).font(.system(size: 13, weight: .bold)).foregroundColor(.white)
                let resolved = context.resolve(label)
                let left = at.x - resolved.measure(in: CGSize(width: 1000, height: 100)).width / 2
                let clear = smoothstep(Double(edge), Double(edge) + 40, Double(left))
                guard clear > 0 else { continue }
                var named = context
                named.opacity = min(glow, 1) * clear
                named.draw(resolved, at: CGPoint(x: at.x, y: at.y - 6), anchor: .bottom)
            }
        }
    }

    /// A point along an arc on screen, or nil when the globe hides it.
    private func arcPoint(_ a: GeoPoint, _ b: GeoPoint, angle: Double, _ s: Double, height: Double, projection p: MapProjection) -> CGPoint? {
        let ua = a.unit
        let ub = b.unit
        let sa = sin((1 - s) * angle) / sin(angle)
        let sb = sin(s * angle) / sin(angle)
        let v = p.toView((sa * ua.0 + sb * ub.0, sa * ua.1 + sb * ub.1, sa * ua.2 + sb * ub.2))
        let lift = 1 + height * sin(.pi * s)
        let vx = v.0 * lift
        let vy = v.1 * lift
        if v.2 < 0 && (vx * vx + vy * vy).squareRoot() < 1 { return nil }
        return CGPoint(x: p.cx + p.r * vx, y: p.cy - p.r * vy)
    }

    /// Trade routes: arcs lifted off the surface between pairs of great cities facing the viewer.
    /// Each grows from its origin, lands with a ring, and its tail follows it home.
    private func drawArcs(_ context: inout GraphicsContext, projection p: MapProjection, seconds: Double) {
        let hubs = Self.hubs
        var glow = context
        glow.blendMode = .plusLighter
        for slot in 0..<Self.arcSlots {
            let local = seconds + Double(slot) * 0.5
            let cycle = Int(floor(local / Self.arcLife))
            let u = local - Double(cycle) * Self.arcLife
            // Routes start from cities facing the viewer when they set off, so they are seen.
            let startLon = Self.spin(Double(cycle) * Self.arcLife - Double(slot) * 0.5)
            let start = MapProjection(camera: MapCamera(lon: startLon, lat: p.lat0 * 180 / .pi, zoom: 1), size: CGSize(width: 2, height: 2))
            let facing = hubs.indices.filter { start.toView(hubs[$0].geo.unit).2 > 0.25 }
            guard facing.count >= 2 else { continue }
            let hash = stableHash("arc\(cycle)/\(slot)")
            let ai = hash % facing.count
            let a = facing[ai]
            let b = facing[(ai + 1 + (hash / facing.count) % (facing.count - 1)) % facing.count]
            let ua = hubs[a].geo.unit
            let ub = hubs[b].geo.unit
            let angle = acos(min(max(ua.0 * ub.0 + ua.1 * ub.1 + ua.2 * ub.2, -1), 1))
            guard angle >= 0.05 else { continue }
            let head = min(max(u / 2.5, 0), 1)
            let tail = min(max((u - 1.0) / 3.5, 0), 1)
            guard head - tail >= 0.01 else { continue }
            let height = 0.16 * min(max(angle / 1.2, 0.35), 1)
            var path = Path()
            var drawing = false
            var last: CGPoint?
            for i in 0...32 {
                let s = tail + (head - tail) * Double(i) / 32
                guard let at = arcPoint(hubs[a].geo, hubs[b].geo, angle: angle, s, height: height, projection: p) else {
                    drawing = false
                    continue
                }
                if drawing { path.addLine(to: at) } else { path.move(to: at) }
                drawing = true
                last = at
            }
            let fade = 1 - min(max((u - 3.6) / 0.9, 0), 1)
            context.stroke(path, with: .color(Self.gold.color(0.22 * fade)), lineWidth: 6)
            context.stroke(path, with: .color(Self.gold.color(0.85 * fade)), lineWidth: 2)
            if u < 2.5, let last {
                stamp(&glow, .glow, at: last, radius: 9, color: Self.gold, alpha: 0.9)
                context.fill(disc(last, 2.2), with: .color(RGB(hex: 0xFFF1CC).color()))
            }
            if u >= 2.5 && u <= 3.5, let end = arcPoint(hubs[a].geo, hubs[b].geo, angle: angle, 1, height: height, projection: p) {
                let f = u - 2.5
                context.stroke(disc(end, 3 + 14 * f), with: .color(Self.gold.color(0.6 * (1 - f))), lineWidth: 1.5)
            }
        }
    }

    /// Three satellites on tilted orbits with fading trails, hidden as they pass behind the globe.
    private func drawSatellites(_ context: inout GraphicsContext, projection p: MapProjection, seconds: Double) {
        let orbits: [(period: Double, tilt: Double, phase: Double)] = [(19, 0.45, 0.2), (27, -0.8, 1.9), (41, 1.2, 4.1)]
        let r = p.r * 1.12
        let color = RGB(hex: 0xDDE8F4)
        for orbit in orbits {
            for k in 0..<24 {
                let angle = 2 * Double.pi * (seconds - Double(k) * 0.05) / orbit.period + orbit.phase
                let x0 = cos(angle)
                let z0 = sin(angle)
                // Tilt the orbit about the x axis, then lean it a little.
                let y = -z0 * sin(orbit.tilt)
                let z = z0 * cos(orbit.tilt)
                let x = x0 * cos(orbit.tilt * 0.3) - y * sin(orbit.tilt * 0.3)
                let yy = x0 * sin(orbit.tilt * 0.3) + y * cos(orbit.tilt * 0.3)
                if z < 0 && (x * x + yy * yy).squareRoot() * 1.12 < 1 { continue }
                let at = CGPoint(x: p.cx + r * x, y: p.cy - r * yy)
                let alpha = k == 0 ? 0.95 : 0.22 * (1 - Double(k) / 24)
                context.fill(disc(at, k == 0 ? 1.8 : 1), with: .color(color.color(alpha)))
            }
        }
    }
}

/// A band of light that crosses the logo every six seconds, masked to the logo's pixels.
struct LogoShine: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @AppStorage(GraphicsKeys.lighting) private var lighting = 2

    var body: some View {
        if animations && !reduceMotion && lighting > 0 {
            TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: scenePhase != .active)) { timeline in
                Canvas { context, size in
                    let local = fract(timeline.date.timeIntervalSinceReferenceDate / 6) * 6
                    guard local < 1.4 else { return }
                    let progress = local / 1.4
                    let band = 0.3 * Double(size.width)
                    let x = -band + (Double(size.width) + 2 * band) * progress
                    let middle = CGPoint(x: x, y: size.height / 2)
                    // A diagonal band: the gradient runs across it, tilted.
                    let dx = band / 2 * cos(Double.pi / 6)
                    let dy = band / 2 * sin(Double.pi / 6)
                    context.fill(Path(CGRect(origin: .zero, size: size)), with: .linearGradient(
                        Gradient(stops: [
                            .init(color: .white.opacity(0), location: 0),
                            .init(color: .white.opacity(0.55), location: 0.5),
                            .init(color: .white.opacity(0), location: 1),
                        ]),
                        startPoint: CGPoint(x: middle.x - dx, y: middle.y - dy),
                        endPoint: CGPoint(x: middle.x + dx, y: middle.y + dy)
                    ))
                }
            }
            .mask { PixelLogo() }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
    }
}

/// Slides, spins and scales a view along a curve as `progress` runs from 0 to 1.
private struct FlightEffect: ViewModifier, Animatable {
    var progress: Double
    let startX: Double
    let startY: Double
    let spin: Double
    let spread: Double

    var animatableData: Double {
        get { progress }
        set { progress = newValue }
    }

    func body(content: Content) -> some View {
        let inv = 1 - progress
        // A quadratic curve: out over the globe, swinging up and round into place.
        let controlX = startX * 0.3
        let controlY = startY - 260
        let x = inv * inv * startX + 2 * inv * progress * controlX
        let y = inv * inv * startY + 2 * inv * progress * controlY
        let scale = 0.6 + 0.4 * progress + 0.25 * sin(progress * Double.pi)
        return content
            .scaleEffect(scale)
            .rotationEffect(.degrees(spin * inv))
            .offset(x: x * spread, y: y * spread)
            .opacity(min(progress * 3, 1))
    }
}

/// The sector icons fly in from around the globe on curved paths, one after another, and settle
/// into a row under the logo.
struct FlyingBadges: View {
    var spread: Double
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @State private var arrived = false

    private static let starts: [(x: Double, y: Double, spin: Double)] = {
        var seed: UInt64 = 31
        func next() -> Double {
            seed = seed &* 6364136223846793005 &+ 1442695040888963407
            return Double(seed >> 33) / 2147483648.0
        }
        var list: [(x: Double, y: Double, spin: Double)] = []
        for _ in 0..<8 {
            let angle = next() * Double.pi * 2
            let distance = 420 + next() * 520
            list.append((x: 620 + cos(angle) * distance, y: sin(angle) * distance * 0.6, spin: next() * 720 - 360))
        }
        return list
    }()

    private var animate: Bool { animations && !reduceMotion }

    var body: some View {
        let sectors = Array(Sector.order.prefix(8))
        HStack(spacing: 8) {
            ForEach(Array(sectors.enumerated()), id: \.offset) { index, sector in
                let start = Self.starts[index % Self.starts.count]
                SectorBadge(sector: sector, size: 30)
                    .modifier(FlightEffect(
                        progress: (arrived || !animate) ? 1 : 0,
                        startX: start.x, startY: start.y, spin: start.spin, spread: spread
                    ))
                    .animation(
                        animate ? .timingCurve(0.2, 0.0, 0.2, 1.0, duration: 1.3).delay(0.15 + Double(index) * 0.11) : nil,
                        value: arrived
                    )
            }
        }
        .accessibilityHidden(true)
        .onAppear { arrived = true }
    }
}

private struct MenuButton: View {
    let title: String
    var primary = false
    var enabled = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title.uppercased())
                .font(Pixel.font(16, bold: true))
                .tracking(1)
                .foregroundStyle(primary ? Theme.ink : Theme.primaryText)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 15)
                .contentShape(Pixel.shape)
                .pixelFrame(primary ? Theme.gold : Theme.panel,
                            outline: primary ? Color(red: 0.541, green: 0.392, blue: 0.125) : Theme.stroke)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.5)
    }
}

/// The main menu: a turning globe, the way into a career, and the new-game flow.
struct StartView: View {
    private enum Screen { case menu, newGame }

    @EnvironmentObject private var store: AppStore
    @State private var screen = Screen.menu
    @State private var showingSettings = false
    @State private var showingSaves = false
    @State private var showingEditor = false
    @State private var showingCareers = false
    @State private var export: DiagnosticExport?

    private var version: String {
        let short = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
        return short.isEmpty ? "" : "Version \(short)"
    }

    var body: some View {
        Group {
            switch screen {
            case .newGame:
                NewGameView(onBack: { screen = .menu })
            case .menu:
                menu
            }
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView().environmentObject(store)
        }
        .sheet(isPresented: $showingEditor) {
            ScenarioEditorView().environmentObject(store)
        }
        .sheet(isPresented: $showingCareers) {
            CareerListView()
        }
        .sheet(isPresented: $showingSaves) {
            SavesView().environmentObject(store)
        }
        .sheet(item: $export) { item in
            DiagnosticExportView(export: item)
        }
    }

    private var menu: some View {
        GeometryReader { proxy in
            let wide = proxy.size.width >= 900
            let column: CGFloat = wide ? 460 : proxy.size.width
            ZStack(alignment: .leading) {
                Color(red: 0.020, green: 0.035, blue: 0.055).ignoresSafeArea()
                GlobeBackdrop(offsetX: wide ? 0.22 : 0, menuEdge: column + proxy.safeAreaInsets.leading)
                    .ignoresSafeArea()
                // Shade the left side so the menu reads over the globe.
                LinearGradient(
                    stops: [
                        .init(color: Color(red: 0.020, green: 0.035, blue: 0.055).opacity(0.95), location: 0),
                        .init(color: Color(red: 0.020, green: 0.035, blue: 0.055).opacity(0.70), location: wide ? 0.45 : 0.9),
                        .init(color: Color(red: 0.020, green: 0.035, blue: 0.055).opacity(0.12), location: 1),
                    ],
                    startPoint: .leading, endPoint: .trailing
                )
                .ignoresSafeArea()
                .allowsHitTesting(false)
                ScrollView {
                    VStack(spacing: 14) {
                        PixelLogo()
                            .overlay { LogoShine() }
                            .frame(maxWidth: wide ? 300 : 240)
                            .padding(.top, 8)
                        FlyingBadges(spread: wide ? 1 : 0.45)
                        Spacer().frame(height: 10)
                        if let latest = store.saves.first {
                            MenuButton(title: "Continue", primary: true, enabled: !store.isBusy) {
                                Task { await store.resume(latest) }
                            }
                        }
                        MenuButton(title: "New game", primary: store.saves.isEmpty) { screen = .newGame }
                        if !store.saves.isEmpty {
                            MenuButton(title: "Load game") { showingSaves = true }
                        }
                        if !CareerSummary.load().isEmpty {
                            MenuButton(title: "Hall of Careers") { showingCareers = true }
                        }
                        MenuButton(title: "Scenario editor") { showingEditor = true }
                        MenuButton(title: "Settings") { showingSettings = true }
                        if let issue = store.recoveryIssue {
                            RecoveryCard(issue: issue) { export = store.makeDiagnostics() }
                                .environmentObject(store)
                        }
                        if let error = store.errorMessage {
                            Text(error).font(.footnote).foregroundStyle(Theme.negative)
                        }
                        Text(version)
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary.opacity(0.7))
                        Text("No account, no ads. An anonymous market summary is shared every five weeks unless switched off in Settings.")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary.opacity(0.7))
                            .multilineTextAlignment(.center)
                    }
                    .padding(.horizontal, wide ? 48 : 24)
                    .padding(.vertical, 40)
                    .frame(width: column)
                    .frame(minHeight: proxy.size.height)
                }
            }
        }
    }
}
