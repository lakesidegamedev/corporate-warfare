import SwiftUI

private func scenarioTint(_ id: String) -> Color {
    switch id {
    case "texas-1981": return Color(red: 0.898, green: 0.376, blue: 0.369)
    case "silicon-valley-1995": return Color(red: 0.259, green: 0.647, blue: 0.961)
    case "ruhr-rhine-1991": return Color(red: 0.247, green: 0.749, blue: 0.541)
    default: return Theme.gold
    }
}

/// A slowly turning globe behind the menu. Each scenario's region glows and is named as it turns
/// to face the viewer, and fades as it rotates away. With lighting on it becomes a lit scene:
/// stars in depth, sunlight and atmosphere, night with city lights, clouds, trade arcs between
/// cities and satellites. Lighting off keeps the plain globe.
struct GlobeBackdrop: View {
    var offsetX: CGFloat = 0.22
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage(GraphicsKeys.animations) private var animations = true
    @AppStorage(GraphicsKeys.preset) private var preset = GraphicsPreset.high.rawValue
    @AppStorage(GraphicsKeys.lighting) private var lighting = 2
    @AppStorage(GraphicsKeys.dayNight) private var dayNight = true
    @AppStorage(GraphicsKeys.cityLights) private var cityLights = true
    @AppStorage(GraphicsKeys.clouds) private var clouds = 1
    @AppStorage(GraphicsKeys.frameRate) private var frameRate = 60

    private let map = WorldMap.shared

    private static let names: [String: String] = {
        var result: [String: String] = [:]
        for scenario in ScenarioCatalogue.all { result[scenario.id] = scenario.name }
        return result
    }()

    private static let stars: [(x: Double, y: Double, radius: Double, alpha: Double)] = {
        var seed: UInt64 = 7
        func next() -> Double {
            seed = seed &* 6364136223846793005 &+ 1442695040888963407
            return Double(seed >> 33) / 2147483648.0
        }
        var list: [(x: Double, y: Double, radius: Double, alpha: Double)] = []
        for _ in 0..<160 {
            let x = next()
            let y = next()
            let radius = 0.6 + next() * 1.4
            let alpha = 0.08 + next() * 0.35
            list.append((x: x, y: y, radius: radius, alpha: alpha))
        }
        return list
    }()

    /// The most populous cities in the world file, as unit vectors, for trade arcs.
    private static let hubs: [GeoPoint] = WorldMap.worldCities.prefix(60).map { GeoPoint(e4: $0.x, $0.y) }
    private static let gold = RGB(hex: 0xE8B44C)
    private static let glare = RGB(hex: 0xFFE2B0)

    private var animate: Bool { animations && !reduceMotion && scenePhase == .active }

    private var starCount: Int {
        switch GraphicsPreset(rawValue: preset) {
        case .ultra: return 300
        case .high: return 220
        case .custom: return lighting == 2 ? 220 : 120
        default: return 120
        }
    }

    var body: some View {
        let interval = lighting > 0 ? GraphicsSettings.frameInterval(frameRate) : 1.0 / 30.0
        TimelineView(.animation(minimumInterval: interval, paused: !animate)) { timeline in
            Canvas { context, size in
                let seconds = animate ? timeline.date.timeIntervalSinceReferenceDate : 0
                let spin = seconds * 2.2
                let lon = (((-82 + spin) + 540).truncatingRemainder(dividingBy: 360)) - 180
                if lighting > 0 {
                    drawLit(&context, size: size, lon: lon, time: seconds)
                } else {
                    draw(&context, size: size, lon: lon)
                }
            }
            .drawingGroup()
        }
        .accessibilityHidden(true)
    }

    private func draw(_ context: inout GraphicsContext, size: CGSize, lon: Double) {
        for star in Self.stars {
            let r = star.radius
            let rect = CGRect(x: star.x * size.width - r, y: star.y * size.height - r, width: r * 2, height: r * 2)
            context.fill(Path(ellipseIn: rect), with: .color(Color.white.opacity(star.alpha)))
        }
        let base = 1.25 * 0.45 * min(size.width, size.height)
        let centre = CGPoint(x: size.width * (0.5 + offsetX), y: size.height * 0.58)
        let glowRadius = base * 1.25
        let glowRect = CGRect(x: centre.x - glowRadius, y: centre.y - glowRadius, width: glowRadius * 2, height: glowRadius * 2)
        context.fill(
            Path(ellipseIn: glowRect),
            with: .radialGradient(
                Gradient(stops: [
                    .init(color: Color(red: 0.18, green: 0.435, blue: 0.69).opacity(0.20), location: 0),
                    .init(color: Color(red: 0.18, green: 0.435, blue: 0.69).opacity(0.13), location: 0.85),
                    .init(color: Color.clear, location: 1),
                ]),
                center: centre, startRadius: 0, endRadius: glowRadius
            )
        )
        context.translateBy(x: size.width * offsetX, y: size.height * 0.08)
        let camera = MapCamera(lon: lon, lat: 28, zoom: 1.25)
        let p = MapProjection(camera: camera, size: size)
        let disc = CGRect(x: p.cx - p.r, y: p.cy - p.r, width: p.r * 2, height: p.r * 2)
        context.fill(Path(ellipseIn: disc), with: .color(Color(red: 0.035, green: 0.09, blue: 0.16)))
        drawLand(&context, projection: p)
        context.stroke(Path(ellipseIn: disc), with: .color(Theme.water.opacity(0.9)), lineWidth: 1.5)
        drawScenarios(&context, projection: p, pulse: 1)
    }

    private func drawLand(_ context: inout GraphicsContext, projection p: MapProjection) {
        var land = Path()
        for country in map.countries {
            for ring in country.rings { p.addPolygon(ring, to: &land) }
        }
        context.fill(land, with: .color(Theme.land.opacity(0.9)), style: FillStyle(eoFill: true))
        context.stroke(land, with: .color(Theme.gold.opacity(0.35)), lineWidth: 0.7)
    }

    private func drawScenarios(_ context: inout GraphicsContext, projection p: MapProjection, pulse: Double) {
        for area in map.scenarioAreas {
            var facing = -1.0
            for ring in area.rings {
                let c = ring.center
                facing = max(facing, c.0 * p.view.0 + c.1 * p.view.1 + c.2 * p.view.2)
            }
            let x = min(max((facing - 0.35) / 0.5, 0), 1)
            let glow = x * x * (3 - 2 * x) * pulse
            if glow <= 0.001 { continue }
            let tint = scenarioTint(area.id)
            let path = p.path(area.rings)
            for width in [22.0, 14.0, 8.0] {
                context.stroke(path, with: .color(tint.opacity(0.10 * glow)), lineWidth: width)
            }
            context.fill(path, with: .color(tint.opacity(0.55 * glow)), style: FillStyle(eoFill: true))
            context.stroke(path, with: .color(Color.white.opacity(0.75 * glow)), lineWidth: 1.6)
            guard let anchor = area.rings.max(by: { $0.points.count < $1.points.count }),
                  let name = Self.names[area.id] else { continue }
            let c = anchor.center
            let lonDeg = atan2(c.1, c.0) * 180 / .pi
            let latDeg = asin(min(max(c.2, -1), 1)) * 180 / .pi
            let (at, front) = p.project(GeoPoint(lonDegrees: lonDeg, latDegrees: latDeg))
            if front {
                let label = Text(name)
                    .font(.system(size: 13, weight: .bold))
                    .foregroundColor(Color.white.opacity(min(glow, 1)))
                context.draw(context.resolve(label), at: CGPoint(x: at.x, y: at.y - 6), anchor: .bottom)
            }
        }
    }

    /// The lit scene. `time` is zero for a still frame.
    private func drawLit(_ context: inout GraphicsContext, size: CGSize, lon: Double, time: Double) {
        let cinematic = lighting == 2
        // Three layers of stars drifting left at different speeds.
        Starfield.draw(in: &context, size: size, range: 0..<starCount,
                       shift: (-1 * time, -2.5 * time, -5 * time), time: time == 0 ? nil : time, weight: 1)

        var scene = context
        scene.translateBy(x: size.width * offsetX, y: size.height * 0.08)
        let camera = MapCamera(lon: lon, lat: 24 + 4 * sin(time / 23), zoom: 1.25)
        let p = MapProjection(camera: camera, size: size)
        let c = CGPoint(x: p.cx, y: p.cy)
        let sun = MapSun.at(time: time, dayNight: dayNight, projection: p)

        if cinematic { drawGlare(&scene, center: c, radius: p.r, sun: sun) }
        GlobeLighting.atmosphere(in: &scene, center: c, radius: p.r, sun: sun, weight: 1, cinematic: cinematic)
        let disc = CGRect(x: p.cx - p.r, y: p.cy - p.r, width: p.r * 2, height: p.r * 2)
        scene.fill(Path(ellipseIn: disc), with: .color(Color(red: 0.035, green: 0.09, blue: 0.16)))
        GlobeLighting.ocean(in: &scene, center: c, radius: p.r, sun: sun, weight: 1)
        drawLand(&scene, projection: p)
        GlobeLighting.night(in: &scene, projection: p, sun: sun, weight: 1)
        if cityLights {
            GlobeLighting.cityLights(in: &scene, projection: p, sun: sun, cities: WorldMap.worldCities, weight: 1)
        }
        drawScenarios(&scene, projection: p, pulse: 0.85 + 0.15 * sin(time * 2))
        if clouds > 0 { drawClouds(&scene, projection: p, sun: sun, time: time) }
        GlobeLighting.surface(in: &scene, center: c, radius: p.r, sun: sun, weight: 1, cinematic: cinematic)
        scene.stroke(Path(ellipseIn: disc), with: .color(Theme.water.opacity(0.9)), lineWidth: 1.5)
        drawArcs(&scene, projection: p, time: time)
        drawSatellites(&scene, projection: p, time: time)
        GlobeLighting.vignette(in: &context, size: size, strength: 1)
    }

    /// Clouds over the whole world. The globe is small here, so puffs are drawn four times the
    /// map's size to read at all.
    private func drawClouds(_ context: inout GraphicsContext, projection p: MapProjection, sun: MapSun, time: Double) {
        let field = CloudField.field(count: clouds >= 2 ? 80 : 36, west: -180, south: -60, east: 180, north: 70)
        for puff in field.puffs {
            let geo = GeoPoint(lonDegrees: field.lon(of: puff, time: time), latDegrees: puff.lat)
            let u = geo.unit
            let facing = u.0 * p.view.0 + u.1 * p.view.1 + u.2 * p.view.2
            guard facing > 0.05 else { continue }
            let (at, _) = p.project(geo)
            let radius = puff.radius * 4 * Double.pi / 180 * p.r * (0.4 + 0.6 * facing)
            let alpha = 0.42 * (1 - 0.6 * sun.nightLight(geo)) * min(facing * 3, 1)
            for part in puff.parts {
                CloudField.blob(in: &context, at: CGPoint(x: at.x + part.dx * radius, y: at.y + part.dy * radius),
                                radius: radius * part.size, color: .white.opacity(alpha))
            }
        }
    }

    /// A point lifted off the globe, in view space scaled by the globe's radius, and whether it
    /// shows: anything behind the globe is hidden.
    private func screen(_ v: (Double, Double, Double), lift: Double, projection p: MapProjection) -> (CGPoint, Bool) {
        let x = v.0 * lift
        let y = v.1 * lift
        let hidden = v.2 < 0 && x * x + y * y < 1
        return (CGPoint(x: p.cx + x * p.r, y: p.cy - y * p.r), !hidden)
    }

    /// Nine golden arcs at a time between the largest cities, each living four and a half seconds.
    private func drawArcs(_ context: inout GraphicsContext, projection p: MapProjection, time: Double) {
        let hubs = Self.hubs
        guard hubs.count > 1, time > 0 else { return }
        var strokes = Path()
        var heads: [CGPoint] = []
        var rings: [(CGPoint, Double)] = []
        for slot in 0..<9 {
            let shifted = time + Double(slot) * 0.5
            let cycle = floor(shifted / 4.5)
            let local = shifted - cycle * 4.5
            let first = Int(MapNoise.unit(11, slot, Int(cycle)) * Double(hubs.count)) % hubs.count
            var second = Int(MapNoise.unit(11, slot + 100, Int(cycle)) * Double(hubs.count)) % hubs.count
            if second == first { second = (second + 1) % hubs.count }
            let a = hubs[first].unit
            let b = hubs[second].unit
            let cosine = min(max(a.0 * b.0 + a.1 * b.1 + a.2 * b.2, -1), 1)
            let angle = acos(cosine)
            guard angle > 0.01 else { continue }
            let head = min(local / 2.5, 1)
            let tail = min(max((local - 1.0) / 3.5, 0), 1)
            guard head > tail else { continue }
            let rise = min(max(angle / 1.2, 0.35), 1)
            func at(_ s: Double) -> (CGPoint, Bool) {
                let wa = sin((1 - s) * angle) / sin(angle)
                let wb = sin(s * angle) / sin(angle)
                let g = (a.0 * wa + b.0 * wb, a.1 * wa + b.1 * wb, a.2 * wa + b.2 * wb)
                return screen(p.toView(g), lift: 1 + 0.16 * sin(.pi * s) * rise, projection: p)
            }
            var drawing = false
            for step in 0...24 {
                let s = tail + (head - tail) * Double(step) / 24
                let (point, shows) = at(s)
                if shows {
                    if drawing { strokes.addLine(to: point) } else { strokes.move(to: point) }
                    drawing = true
                } else {
                    drawing = false
                }
            }
            if head < 1 {
                let (point, shows) = at(head)
                if shows { heads.append(point) }
            }
            if local >= 2.5 && local < 3.3 {
                let (point, shows) = at(1)
                if shows { rings.append((point, (local - 2.5) / 0.8)) }
            }
        }
        context.stroke(strokes, with: .color(Self.gold.color(0.15)), style: StrokeStyle(lineWidth: 5, lineCap: .round, lineJoin: .round))
        context.stroke(strokes, with: .color(Self.gold.color(0.7)), style: StrokeStyle(lineWidth: 1.6, lineCap: .round, lineJoin: .round))
        for point in heads {
            CloudField.blob(in: &context, at: point, radius: 9, color: Self.gold.color(0.45))
            context.fill(Path(ellipseIn: CGRect(x: point.x - 3, y: point.y - 3, width: 6, height: 6)), with: .color(Self.gold.color(1)))
        }
        for (point, age) in rings {
            let radius = 2 + 10 * age
            context.stroke(Path(ellipseIn: CGRect(x: point.x - radius, y: point.y - radius, width: radius * 2, height: radius * 2)),
                           with: .color(Self.gold.color(0.6 * (1 - age))), lineWidth: 1.2)
        }
    }

    /// Three satellites on tilted orbits with fading trails, hidden behind the globe.
    private func drawSatellites(_ context: inout GraphicsContext, projection p: MapProjection, time: Double) {
        let orbits: [(period: Double, tilt: Double, node: Double, phase: Double)] = [
            (19, 1.15, 0.4, 0.0), (27, 0.55, 2.1, 2.0), (41, 1.45, 4.0, 4.1),
        ]
        for orbit in orbits {
            let angle = 2 * Double.pi * time / orbit.period + orbit.phase
            for step in 0..<40 {
                let theta = angle - Double(step) * 0.012
                let x0 = cos(theta)
                let y0 = sin(theta) * cos(orbit.tilt)
                let z = sin(theta) * sin(orbit.tilt)
                let x = x0 * cos(orbit.node) - y0 * sin(orbit.node)
                let y = x0 * sin(orbit.node) + y0 * cos(orbit.node)
                let (point, shows) = screen((x, y, z), lift: 1.12, projection: p)
                guard shows else { continue }
                let fade = 1 - Double(step) / 40
                let radius = step == 0 ? 1.8 : 0.9
                context.fill(Path(ellipseIn: CGRect(x: point.x - radius, y: point.y - radius, width: radius * 2, height: radius * 2)),
                             with: .color(.white.opacity((step == 0 ? 0.95 : 0.35) * fade)))
            }
        }
    }

    /// A warm glow where the sun sits off the globe, and faint lens ghosts through the centre.
    private func drawGlare(_ context: inout GraphicsContext, center c: CGPoint, radius r: Double, sun: MapSun) {
        guard sun.view.2 > -0.3 else { return }
        let length = max((sun.view.0 * sun.view.0 + sun.view.1 * sun.view.1).squareRoot(), 1e-6)
        let strength = min(max((sun.view.2 + 0.3) / 0.6, 0), 1)
        let at = CGPoint(x: c.x + sun.view.0 / length * 1.5 * r, y: c.y - sun.view.1 / length * 1.5 * r)
        CloudField.blob(in: &context, at: at, radius: 0.9 * r, color: Self.glare.color(0.22 * strength))
        for (fraction, size) in [(1.5, 0.05), (1.9, 0.09), (2.4, 0.03)] {
            let ghost = CGPoint(x: at.x + (c.x - at.x) * fraction, y: at.y + (c.y - at.y) * fraction)
            context.fill(Path(ellipseIn: CGRect(x: ghost.x - size * r, y: ghost.y - size * r, width: size * r * 2, height: size * r * 2)),
                         with: .color(Self.glare.color(0.06 * strength)))
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
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @AppStorage(GraphicsKeys.animations) private var animations = true

    var body: some View {
        Button(action: action) {
            Text(title.uppercased())
                .font(.system(size: 17, weight: .bold))
                .tracking(1.5)
                .foregroundStyle(primary ? Theme.ink : Theme.primaryText)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 15)
                .background(primary ? Theme.gold : Theme.panel.opacity(0.72), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                // Glass: a soft highlight across the top half.
                .overlay(
                    LinearGradient(colors: [.white.opacity(0.10), .white.opacity(0)], startPoint: .top, endPoint: .center)
                        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .allowsHitTesting(false)
                )
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(primary ? Theme.gold : Theme.stroke, lineWidth: 1))
                .background { if primary { glow } }
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.5)
    }

    /// The primary button's gold glow, breathing over three seconds.
    private var glow: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: !(animations && !reduceMotion && scenePhase == .active))) { timeline in
            let phase = sin(2 * Double.pi * timeline.date.timeIntervalSinceReferenceDate / 3)
            let alpha = animations && !reduceMotion ? 0.35 + 0.10 * phase : 0.35
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(Theme.gold.opacity(alpha))
                .blur(radius: 10)
        }
        .allowsHitTesting(false)
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
            ZStack(alignment: .leading) {
                Color(red: 0.020, green: 0.035, blue: 0.055).ignoresSafeArea()
                GlobeBackdrop(offsetX: wide ? 0.22 : 0)
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
                    .frame(width: wide ? 460 : proxy.size.width)
                    .frame(minHeight: proxy.size.height)
                }
            }
        }
    }
}
