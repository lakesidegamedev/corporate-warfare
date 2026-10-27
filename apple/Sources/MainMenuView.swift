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
/// to face the viewer, and fades as it rotates away.
struct GlobeBackdrop: View {
    var offsetX: CGFloat = 0.22
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @AppStorage(GraphicsKeys.animations) private var animations = true

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

    private var animate: Bool { animations && !reduceMotion }

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: !animate)) { timeline in
            Canvas { context, size in
                let seconds = timeline.date.timeIntervalSinceReferenceDate
                let spin = animate ? seconds * 2.2 : 0
                let lon = (((-82 + spin) + 540).truncatingRemainder(dividingBy: 360)) - 180
                draw(&context, size: size, lon: lon)
            }
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
        var land = Path()
        for country in map.countries {
            for ring in country.rings { p.addPolygon(ring, to: &land) }
        }
        context.fill(land, with: .color(Theme.land.opacity(0.9)), style: FillStyle(eoFill: true))
        context.stroke(land, with: .color(Theme.gold.opacity(0.35)), lineWidth: 0.7)
        context.stroke(Path(ellipseIn: disc), with: .color(Theme.water.opacity(0.9)), lineWidth: 1.5)

        for area in map.scenarioAreas {
            var facing = -1.0
            for ring in area.rings {
                let c = ring.center
                facing = max(facing, c.0 * p.view.0 + c.1 * p.view.1 + c.2 * p.view.2)
            }
            let x = min(max((facing - 0.35) / 0.5, 0), 1)
            let glow = x * x * (3 - 2 * x)
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
                    .foregroundColor(Color.white.opacity(glow))
                context.draw(context.resolve(label), at: CGPoint(x: at.x, y: at.y - 6), anchor: .bottom)
            }
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
                .font(.system(size: 17, weight: .bold))
                .tracking(1.5)
                .foregroundStyle(primary ? Theme.ink : Theme.primaryText)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 15)
                .background(primary ? Theme.gold : Theme.panel.opacity(0.88), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(primary ? Theme.gold : Theme.stroke, lineWidth: 1))
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
