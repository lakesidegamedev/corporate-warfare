import SwiftUI
import UIKit

/// Hosts the session's Metal view in SwiftUI and keeps the renderer asked for the camera on screen.
struct GpuMapSurface: UIViewRepresentable {
    let session: GpuMapSession
    let camera: MapCamera
    let size: CGSize
    /// The player's tilt setting, in radians.
    let tilt: Double

    func makeUIView(context: Context) -> GpuMetalView { session.view }

    func updateUIView(_ view: GpuMetalView, context: Context) {
        session.request(camera: GpuCamera(camera, size: size, tilt: tilt), size: size)
    }
}

/// Game state as the GPU map's update JSON: the same fields the desktop shell sends.
enum GpuMapUpdate {
    static func json(plants: [MapPlantDto], divisions: [DivisionMarker], layer: MapLayer, shading: [String: Double],
                     night: Bool, clouds: Int, week: Int) -> String {
        var object: [String: Any] = [:]
        object["plants"] = plants.map { p -> [String: Any] in
            ["region": p.region, "industry": p.industry, "firm": p.firm, "blocks": p.blocks,
             "player": p.player, "status": p.status.lowercased()]
        }
        var seen = Set<String>()
        object["holdings"] = divisions.map(\.region).filter { seen.insert($0).inserted }
        var heat: [String: Int] = [:]
        if layer != .presence && layer != .freight {
            for (code, value) in shading { heat[code] = min(max(Int(value * 3.999), 0), 3) }
        }
        object["heat"] = heat
        object["heat_ramp"] = layer.gpuRamp
        object["night"] = night
        object["clouds"] = min(max(clouds, 0), 3)
        object["week"] = week
        let data = (try? JSONSerialization.data(withJSONObject: object)) ?? Data("{}".utf8)
        return String(decoding: data, as: UTF8.self)
    }
}

extension MapLayer {
    /// The palette ramp the GPU map shades a layer with.
    var gpuRamp: String {
        switch self {
        case .competition: return "brick"
        case .room: return "forest"
        case .demand: return "urban"
        case .wages, .skill: return "water"
        case .infrastructure, .land: return "soil"
        case .population: return "signal"
        default: return "gold"
        }
    }
}

/// City names, region names and division badges over the GPU frame, placed by the same label
/// planner as the classic map. Every position comes from the engine's projection of the camera of
/// the frame on screen, so a name sits on the place the picture shows and nothing slides while the
/// view moves. Buildings come first: each footprint on screen is reserved so no name lands on it,
/// and a building too small to read is marked by a badge at its position.
struct GpuLabels: View {
    let session: GpuMapSession
    let map: WorldMap
    let divisions: [DivisionMarker]
    /// 0 none, 1 cities, 2 cities and regions.
    let labels: Int
    let planner: LabelPlanner
    /// Changes with every presented frame, so the canvas redraws.
    let version: Int

    private static let minBuildingPx: Float = 6
    private static let badgePx: CGFloat = 11

    var body: some View {
        Canvas { context, size in
            _ = version
            guard labels > 0, let shown = session.shown else { return }
            let camera = shown.camera
            let view = shown.size
            // The picture was drawn for `view`; a view that has since resized scales the names with it.
            let sx = size.width / max(view.width, 1)
            let sy = size.height / max(view.height, 1)
            func at(_ x: Float, _ y: Float) -> CGPoint { CGPoint(x: CGFloat(x) * sx, y: CGFloat(y) * sy) }
            let widthKm = camera.widthKm

            let buildings = session.buildings
            let rects = session.buildingRects(camera: camera, size: view)
            let markers = session.project(camera: camera, size: view,
                                          lonLat: buildings.flatMap { [$0.lon, $0.lat] })
            let cities = map.cities.filter { widthKm < 900 || $0.population >= 500_000 }.prefix(80)
            let regions = labels >= 2 && widthKm < 1400
                ? map.regions.compactMap { r in map.anchor(of: r.code).map { (r, $0) } } : []
            let byRegion = Dictionary(grouping: divisions, by: \.region)
            let badges = byRegion.keys.sorted().compactMap { code -> (String, Int, GeoPoint)? in
                map.anchor(of: code).map { (code, byRegion[code]!.reduce(0) { $0 + $1.plants }, $0) }
            }
            var points: [Double] = []
            for c in cities { points += [Double(c.x) / 10_000, Double(c.y) / 10_000] }
            for (_, a) in regions { points += [a.lon * 180 / .pi, a.lat * 180 / .pi] }
            for (_, _, a) in badges { points += [a.lon * 180 / .pi, a.lat * 180 / .pi] }
            let projected = session.project(camera: camera, size: view, lonLat: points)

            planner.begin()
            defer { planner.end() }
            var small: [Int] = []
            for b in buildings.indices {
                let l = rects[4 * b], t = rects[4 * b + 1], r = rects[4 * b + 2], bt = rects[4 * b + 3]
                if l.isNaN || r < -20 || l > Float(view.width) + 20 || bt < -20 || t > Float(view.height) + 20 { continue }
                if max(r - l, bt - t) >= Self.minBuildingPx {
                    let a = at(l - 1, t - 1), z = at(r + 1, bt + 1)
                    planner.reserve(CGRect(x: a.x, y: a.y, width: z.x - a.x, height: z.y - a.y))
                } else if !buildings[b].isScenery {
                    small.append(b)
                }
            }
            // The rivals' badges first, the player's last so it sits on top.
            for b in small.sorted(by: { !buildings[$0].isPlayer && buildings[$1].isPlayer }) {
                let p = at(markers[2 * b], markers[2 * b + 1])
                if p.x.isNaN || p.x < -20 || p.y < -20 || p.x > size.width + 20 || p.y > size.height + 20 { continue }
                let half = Self.badgePx / 2
                let box = CGRect(x: p.x - half, y: p.y - half, width: Self.badgePx, height: Self.badgePx)
                let fill = buildings[b].isPlayer ? Theme.gold : RGB(hex: buildings[b].colour).color()
                context.fill(Path(box.insetBy(dx: -1, dy: -1)), with: .color(Theme.ink))
                context.fill(Path(box), with: .color(fill))
                if buildings[b].isPlayer { context.fill(Path(box.insetBy(dx: 3, dy: 3)), with: .color(Theme.ink)) }
                planner.reserve(box.insetBy(dx: -1, dy: -1))
            }
            // Division badges before names, so a name that would cross one is the name that goes.
            let base = cities.count * 2 + regions.count * 2
            for (k, badge) in badges.enumerated() {
                let p = at(projected[base + 2 * k], projected[base + 2 * k + 1])
                if p.x.isNaN { continue }
                let text = context.resolve(Text("\(badge.1)").font(Pixel.font(10, bold: true)).foregroundColor(Theme.ink))
                let m = text.measure(in: CGSize(width: 80, height: 40))
                let centre = CGPoint(x: p.x, y: p.y - 26)
                let radius = max(m.width, m.height) / 2 + 3
                context.fill(Path(ellipseIn: CGRect(x: centre.x - radius, y: centre.y - radius, width: radius * 2, height: radius * 2)),
                             with: .color(Theme.gold))
                context.draw(text, at: centre, anchor: .center)
                planner.reserve(CGRect(x: centre.x - radius, y: centre.y - radius, width: radius * 2, height: radius * 2))
            }
            for (i, city) in cities.enumerated().sorted(by: { $0.element.population > $1.element.population }) {
                let p = at(projected[2 * i], projected[2 * i + 1])
                if p.x.isNaN || p.x < -40 || p.y < -20 || p.x > size.width + 40 || p.y > size.height + 20 { continue }
                context.fill(Path(ellipseIn: CGRect(x: p.x - 2.5, y: p.y - 2.5, width: 5, height: 5)),
                             with: .color(Theme.primaryText.opacity(0.85)))
                let text = context.resolve(Text(city.name).font(Pixel.font(10)).foregroundColor(Theme.primaryText))
                let m = text.measure(in: CGSize(width: 300, height: 40))
                let slots = [CGPoint(x: p.x + 6, y: p.y - m.height / 2), CGPoint(x: p.x - 6 - m.width, y: p.y - m.height / 2),
                             CGPoint(x: p.x - m.width / 2, y: p.y - m.height - 5), CGPoint(x: p.x - m.width / 2, y: p.y + 5)]
                if let corner = planner.place("c:\(city.name)", size: m, slots: slots) {
                    context.draw(text, at: corner, anchor: .topLeading)
                }
            }
            for (k, entry) in regions.enumerated() {
                let p = at(projected[cities.count * 2 + 2 * k], projected[cities.count * 2 + 2 * k + 1])
                if p.x.isNaN || p.x < 0 || p.y < 0 || p.x > size.width || p.y > size.height { continue }
                let text = context.resolve(Text(entry.0.name.uppercased()).font(Pixel.font(9, bold: true))
                    .foregroundColor(Theme.primaryText.opacity(0.8)))
                let m = text.measure(in: CGSize(width: 300, height: 40))
                let slots = [CGPoint(x: p.x - m.width / 2, y: p.y + 22), CGPoint(x: p.x - m.width / 2, y: p.y - 22 - m.height)]
                if let corner = planner.place("r:\(entry.0.code)", size: m, slots: slots) {
                    context.draw(text, at: corner, anchor: .topLeading)
                }
            }
        }
        .allowsHitTesting(false)
    }
}

/// The map's frame counter lines, bottom left: the frame rate and what draws the map.
struct GpuCounter: View {
    let session: GpuMapSession?
    let line: String
    /// Changes with every presented frame, so the counter redraws.
    let version: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            if let session, session.isReady {
                Text("\(Int(session.framesPerSecond)) fps")
            }
            Text(line)
        }
        .font(.system(size: 11, weight: .bold).monospacedDigit())
        .foregroundStyle(session?.isSoftware == true ? Theme.gold : Theme.positive)
        .padding(.horizontal, 6)
        .padding(.vertical, 4)
        .background(Color.black.opacity(0.55))
        .padding(.leading, 6)
        .padding(.top, 8)
        .allowsHitTesting(false)
    }
}

/// Draws one frame without a window, for tests: the same scene and camera through the engine's
/// pixel readback instead of a layer, so a test can look at the pixels.
enum GpuMapProbe {
    struct Frame {
        let adapter: String
        let width: Int
        let height: Int
        let rgba: [UInt8]
        /// For each anchor asked about: the region code a tap on its screen position picks.
        let picks: [String: String]
    }

    /// `anchors` are (region code, lon, lat) in degrees: each is projected to the screen and picked.
    static func frame(scenario: String, camera: GpuCamera, width: Int, height: Int, updateJSON: String,
                      anchors: [(code: String, lon: Double, lat: Double)] = []) -> Frame? {
        guard let assets = Bundle.main.resourcePath else { return nil }
        let id = scenario.withCString { s in assets.withCString { cw_gpu_open($0, s) } }
        guard id != 0 else { return nil }
        defer { cw_gpu_close(id) }
        guard cw_gpu_update(id, updateJSON) >= 0 else { return nil }
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        var timing = [Double](repeating: 0, count: 6)
        let cam = camera.values
        let code = cw_gpu_render(id, cam, UInt32(width), UInt32(height), 1, &pixels, pixels.count, &timing)
        guard code >= 0 else { return nil }
        var info = [CChar](repeating: 0, count: 256)
        _ = cw_gpu_info(id, &info, info.count)
        var picks: [String: String] = [:]
        for anchor in anchors {
            var xy = [Float](repeating: .nan, count: 2)
            _ = cw_gpu_project(id, cam, Float(width), Float(height), [anchor.lon, anchor.lat], 1, &xy)
            guard !xy[0].isNaN else { continue }
            var text = [CChar](repeating: 0, count: 64)
            if cw_gpu_pick(id, cam, Float(width), Float(height), xy[0], xy[1], &text, text.count) > 0 {
                picks[anchor.code] = String(cString: text)
            }
        }
        return Frame(adapter: String(cString: info), width: width, height: height, rgba: pixels, picks: picks)
    }
}
