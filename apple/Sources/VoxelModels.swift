import SwiftUI

/// The voxel models drawn as true 3D: each model is a handful of boxes (tools/voxel_models.py)
/// projected through the map's camera every frame, so turning or tilting the map shows every side,
/// and vehicles turn as they follow the road. Faces are cel shaded by a fixed sun in the north-west
/// sky, so the lit sides change as the map turns, with dark ink along each face's edges.
enum VoxelModels {
    struct Box {
        let x0: Double
        let y0: Double
        let z0: Double
        let x1: Double
        let y1: Double
        let z1: Double
        let color: RGB
    }

    struct Model {
        let w: Double
        let d: Double
        let h: Double
        let boxes: [Box]
    }

    private struct File: Decodable {
        let models: [String: ModelFile]
    }

    private struct ModelFile: Decodable {
        let w: Double
        let d: Double
        let h: Double
        let boxes: [[BoxValue]]
    }

    /// A box is six numbers and a colour in one array.
    private enum BoxValue: Decodable {
        case number(Double)
        case text(String)

        init(from decoder: Decoder) throws {
            let value = try decoder.singleValueContainer()
            if let number = try? value.decode(Double.self) {
                self = .number(number)
            } else {
                self = .text(try value.decode(String.self))
            }
        }

        var number: Double {
            if case let .number(n) = self { return n }
            return 0
        }

        var text: String {
            if case let .text(t) = self { return t }
            return ""
        }
    }

    static let models: [String: Model] = {
        guard let url = Bundle.main.url(forResource: "voxel-models", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let file = try? JSONDecoder().decode(File.self, from: data) else { return [:] }
        return file.models.mapValues { m in
            Model(w: m.w, d: m.d, h: m.h, boxes: m.boxes.compactMap { b in
                guard b.count >= 7 else { return nil }
                let hex = UInt32(b[6].text.replacingOccurrences(of: "#", with: ""), radix: 16) ?? 0x808080
                return Box(x0: b[0].number, y0: b[1].number, z0: b[2].number, x1: b[3].number, y1: b[4].number,
                           z1: b[5].number, color: RGB(hex: hex))
            })
        }
    }()

    static func has(_ name: String) -> Bool { models[name] != nil }

    /// Height on screen per voxel against its width on the ground, so cubes read as cubes from the
    /// tilted view.
    static let vertical = 2.2

    /// How far a model's top stands above its floor, in the projection's height units.
    static func lift(_ name: String, unit: Double) -> Double { (models[name]?.h ?? 0) * unit * vertical }

    /// The model name for an industry or city at a plant count: small, medium or large.
    static func sized(_ kind: String, plants: Int) -> String { "\(kind)/\(Voxels.size(plants))" }

    /// Points a voxel for the player's buildings at a zoom scale: a fifth more than a pixel a voxel
    /// per point of zoom, clamped. Cities and rivals are drawn at 0.75 of it, props and vehicles at 0.6.
    static func unit(_ zoomScale: Double, displayScale: Double) -> Double {
        min(max(1.2 * zoomScale * displayScale, 1.4), 7) / max(displayScale, 1)
    }

    /// Whether a unit is big enough on screen for ink edges.
    static func inked(_ unit: Double, displayScale: Double) -> Bool { unit * max(displayScale, 1) >= 1.4 }

    // The sun, in world east, north, up: north-west and high.
    private static let lx = -0.55
    private static let ly = 0.62

    /// How a model is coloured: as made, greyed for rivals, dark for closures.
    enum Tone {
        case plain, muted, dark

        func apply(_ c: RGB) -> RGB {
            let g = (c.r + c.g + c.b) / 3
            switch self {
            case .plain: return c
            case .muted: return RGB((g + (c.r - g) * 0.25) * 0.8, (g + (c.g - g) * 0.25) * 0.82, (g + (c.b - g) * 0.25) * 0.88)
            case .dark: return RGB((g + (c.r - g) * 0.15) * 0.5, (g + (c.g - g) * 0.15) * 0.52, (g + (c.b - g) * 0.15) * 0.58)
            }
        }
    }

    /// Draws model `name` standing on the untilted ground point `ground`, `floor` up, `unit` points
    /// to a voxel, turned `heading` radians anticlockwise from its own east. Returns the top of the
    /// model on screen, or nil when there is no such model or nothing of it faces the camera.
    @discardableResult
    static func draw(_ name: String, projection p: MapProjection, ground: CGPoint, floor: Double, unit: Double,
                     heading: Double = 0, tone: Tone = .plain, ink: Bool = true, alpha: Double = 1,
                     in context: inout GraphicsContext) -> CGFloat? {
        guard let model = models[name] else { return nil }
        let e = p.east
        let n = p.north
        let ch = cos(heading)
        let sh = sin(heading)
        // A local east and north on the ground for this model's own heading.
        let ax = (e.dx * ch + n.dx * sh) * unit
        let ay = (e.dy * ch + n.dy * sh) * unit
        let bx = (-e.dx * sh + n.dx * ch) * unit
        let by = (-e.dy * sh + n.dy * ch) * unit
        let gx = Double(ground.x)
        let gy = Double(ground.y)
        var xs = [CGFloat](repeating: 0, count: 8)
        var ys = [CGFloat](repeating: 0, count: 8)
        func corner(_ i: Int, _ x: Double, _ y: Double, _ z: Double) {
            let s = p.lift(CGPoint(x: gx + ax * x + bx * y, y: gy + ay * x + by * y), floor + z * unit * vertical)
            xs[i] = s.x
            ys[i] = s.y
        }
        // Far boxes first: the one whose centre stands highest on screen, then lower boxes first.
        let order = model.boxes.sorted { a, b in
            let da = -(ay * (a.x0 + a.x1) / 2 + by * (a.y0 + a.y1) / 2)
            let db = -(ay * (b.x0 + b.x1) / 2 + by * (b.y0 + b.y1) / 2)
            return da != db ? da < db : a.z0 < b.z0
        }
        // The sun's direction turned into this model's own frame, for the side faces.
        let sunX = lx * ch + ly * sh
        let sunY = -lx * sh + ly * ch
        let sunLen = max((sunX * sunX + sunY * sunY).squareRoot(), 1e-3)
        func side(_ nx: Double, _ ny: Double) -> Double {
            let d = (nx * sunX + ny * sunY) / sunLen
            return d > 0.45 ? 0.84 : d > -0.2 ? 0.68 : 0.52
        }
        /// Fills one face when it faces the camera; true when drawn.
        func face(_ a: Int, _ b: Int, _ c: Int, _ d: Int, _ light: Double, _ base: RGB) -> Bool {
            // Anticlockwise from outside in the world stays anticlockwise on screen: a negative area faces us.
            let area = (xs[a] * ys[b] - xs[b] * ys[a]) + (xs[b] * ys[c] - xs[c] * ys[b])
                + (xs[c] * ys[d] - xs[d] * ys[c]) + (xs[d] * ys[a] - xs[a] * ys[d])
            if area >= -0.5 { return false }
            var path = Path()
            path.move(to: CGPoint(x: xs[a], y: ys[a]))
            path.addLine(to: CGPoint(x: xs[b], y: ys[b]))
            path.addLine(to: CGPoint(x: xs[c], y: ys[c]))
            path.addLine(to: CGPoint(x: xs[d], y: ys[d]))
            path.closeSubpath()
            let color = base.scaled(light)
            context.fill(path, with: .color(color.color(alpha)))
            if ink { context.stroke(path, with: .color(color.scaled(0.45).color(alpha)), lineWidth: 1) }
            return true
        }
        var top = CGFloat.greatestFiniteMagnitude
        for b in order {
            corner(0, b.x0, b.y0, b.z0)
            corner(1, b.x1, b.y0, b.z0)
            corner(2, b.x1, b.y1, b.z0)
            corner(3, b.x0, b.y1, b.z0)
            corner(4, b.x0, b.y0, b.z1)
            corner(5, b.x1, b.y0, b.z1)
            corner(6, b.x1, b.y1, b.z1)
            corner(7, b.x0, b.y1, b.z1)
            let base = tone.apply(b.color)
            // South, east, north, west sides, then the top: corners anticlockwise seen from outside.
            _ = face(0, 1, 5, 4, side(0, -1), base)
            _ = face(1, 2, 6, 5, side(1, 0), base)
            _ = face(2, 3, 7, 6, side(0, 1), base)
            _ = face(3, 0, 4, 7, side(-1, 0), base)
            if face(4, 5, 6, 7, 1, base) { top = min(top, min(min(ys[4], ys[5]), min(ys[6], ys[7]))) }
        }
        return top == .greatestFiniteMagnitude ? nil : top
    }

    /// The heading, radians anticlockwise from east, of an untilted ground direction on screen.
    static func heading(_ dir: CGVector, projection p: MapProjection) -> Double {
        let e = p.east
        let n = p.north
        return atan2(Double(dir.dx) * n.dx + Double(dir.dy) * n.dy, Double(dir.dx) * e.dx + Double(dir.dy) * e.dy)
    }
}

extension MapProjection {
    /// One unit east and one unit north on the untilted ground, turned by the camera's bearing
    /// (screen y points down).
    var east: (dx: Double, dy: Double) { (cosB, sinB) }
    var north: (dx: Double, dy: Double) { (sinB, -cosB) }
}
