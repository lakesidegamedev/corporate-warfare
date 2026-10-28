import SwiftUI
import UIKit

/// The player's divisions as cel-shaded voxel buildings, one model per industry in three sizes,
/// plus a "city" block, a building "site" and a "crane" in three sizes, and countryside props
/// (trees, fields, barns, houses, rocks) whose three slots are summer, autumn and winter, rendered front on (roof and front face, square to the map)
/// and baked by tools/voxel_sprites.py into atlases at 1, 2 and 3 pixels per half voxel. The map picks
/// the scale for the zoom and screen density and draws the sprite one pixel to one pixel, with no
/// smoothing, so it stays crisp.
enum Voxels {
    struct Frame: Decodable {
        let x: Int
        let y: Int
        let w: Int
        let h: Int
        let ax: Double
        let ay: Double
    }

    private struct Atlas: Decodable {
        let scales: [Int]
        let frames: [String: [String: Frame]]
    }

    private static let atlas: Atlas? = Bundle.main.url(forResource: "voxels", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(Atlas.self, from: $0) }

    private static let sheets: [Int: CGImage] = {
        var out: [Int: CGImage] = [:]
        for scale in 1...3 {
            if let url = Bundle.main.url(forResource: "voxels-\(scale)", withExtension: "png"),
               let image = UIImage(contentsOfFile: url.path)?.cgImage {
                out[scale] = image
            }
        }
        return out
    }()

    /// How a sprite is coloured: as baked, muted for rivals' plants, or dark for mothballed plants
    /// and cities in a recession.
    enum Tone: String {
        case plain, muted, dark

        /// Saturation, then a darkening per channel, as Android's colour matrices.
        fileprivate var matrix: (saturation: Double, r: Double, g: Double, b: Double)? {
            switch self {
            case .plain: return nil
            case .muted: return (0.2, 0.78, 0.8, 0.86)
            case .dark: return (0.15, 0.5, 0.52, 0.58)
            }
        }
    }

    /// Cut sprites, kept per tone: there are 72 frames at each of three scales.
    private static var cut: [String: CGImage] = [:]
    private static let lock = NSLock()

    /// Small up to three plants, medium up to eight, large beyond.
    static func size(_ plants: Int) -> Int {
        plants <= 3 ? 0 : plants <= 8 ? 1 : 2
    }

    /// The pixel scale for a map drawn at `zoomScale` on a screen of `displayScale` pixels a point:
    /// the smallest just past the zoom where models appear, growing as you zoom further in.
    static func scale(_ zoomScale: Double, displayScale: Double) -> Int {
        min(max(Int((zoomScale * displayScale * 0.6).rounded()), 1), 3)
    }

    static func frame(_ sector: String, size: Int, scale: Int) -> Frame? {
        atlas?.frames["\(sector)/\(size)"]?["\(scale)"]
    }

    private static func sprite(_ name: String, variant: Int, scale: Int, tone: Tone, frame f: Frame) -> CGImage? {
        let key = "\(name)/\(variant)/\(scale)/\(tone.rawValue)"
        lock.lock()
        defer { lock.unlock() }
        if let held = cut[key] { return held }
        guard let sheet = sheets[scale], let plain = sheet.cropping(to: CGRect(x: f.x, y: f.y, width: f.w, height: f.h)),
              let image = toned(plain, tone) else {
            return nil
        }
        cut[key] = image
        return image
    }

    /// A sprite recoloured once for its tone, so drawing it costs no filter.
    private static func toned(_ image: CGImage, _ tone: Tone) -> CGImage? {
        guard let m = tone.matrix else { return image }
        let width = image.width
        let height = image.height
        let info = CGImageAlphaInfo.premultipliedLast.rawValue
        guard let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info),
              let data = context.data else { return nil }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let px = data.assumingMemoryBound(to: UInt8.self)
        // Linear in premultiplied colour, so alpha needs no special care.
        let s = m.saturation
        let lr = 0.213 * (1 - s), lg = 0.715 * (1 - s), lb = 0.072 * (1 - s)
        for i in 0..<(width * height) {
            let r = Double(px[4 * i]), g = Double(px[4 * i + 1]), b = Double(px[4 * i + 2])
            let nr = (lr + s) * r + lg * g + lb * b
            let ng = lr * r + (lg + s) * g + lb * b
            let nb = lr * r + lg * g + (lb + s) * b
            px[4 * i] = UInt8(min(max(nr * m.r, 0), 255))
            px[4 * i + 1] = UInt8(min(max(ng * m.g, 0), 255))
            px[4 * i + 2] = UInt8(min(max(nb * m.b, 0), 255))
        }
        return context.makeImage()
    }

    /// The top of the sprite on screen, in points, for a building whose ground anchor is at `ground`.
    static func top(_ sector: String, plants: Int, ground: CGPoint, scale: Int, displayScale: Double) -> CGFloat? {
        guard let f = frame(sector, size: size(plants), scale: scale) else { return nil }
        return CGFloat(((Double(ground.y) * displayScale - f.ay).rounded()) / displayScale)
    }

    /// Draws a building with its ground anchor at `ground` and returns the top of the sprite on
    /// screen, or nil when there is no sprite for it (the caller falls back to a plain box).
    @discardableResult
    static func draw(_ sector: String, plants: Int, ground: CGPoint, scale: Int, displayScale: Double, tone: Tone = .plain,
                     in context: inout GraphicsContext) -> CGFloat? {
        drawVariant(sector, variant: size(plants), ground: ground, scale: scale, displayScale: displayScale, tone: tone, in: &context)
    }

    /// Draws one frame of a model by its variant slot: a size for buildings, a season for props.
    @discardableResult
    static func drawVariant(_ name: String, variant: Int, ground: CGPoint, scale: Int, displayScale: Double,
                            tone: Tone = .plain, in context: inout GraphicsContext) -> CGFloat? {
        let ds = max(displayScale, 1)
        guard let f = frame(name, size: variant, scale: scale),
              let image = sprite(name, variant: variant, scale: scale, tone: tone, frame: f) else { return nil }
        // Snap to whole device pixels so each sprite pixel lands on one screen pixel.
        let left = (Double(ground.x) * ds - f.ax).rounded() / ds
        let top = (Double(ground.y) * ds - f.ay).rounded() / ds
        let rect = CGRect(x: left, y: top, width: Double(f.w) / ds, height: Double(f.h) / ds)
        context.draw(Image(decorative: image, scale: ds).interpolation(.none), in: rect)
        return CGFloat(top)
    }
}

/// Trucks, cars, trains, cargo ships and cargo planes as tiny cel-shaded voxel sprites, baked by
/// tools/voxel_vehicles.py for eight headings and two liveries (neutral, and the player's gold) at
/// 1, 2 and 3 pixels per half voxel. Drawn one to one, facing the way they move on screen, so they
/// stay right when the map is turned or tilted.
enum Vehicles {
    private struct Atlas: Decodable {
        let scales: [Int]
        let headings: [String]
        let frames: [String: [String: Voxels.Frame]]
    }

    private static let atlas: Atlas? = Bundle.main.url(forResource: "vehicles", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(Atlas.self, from: $0) }

    private static let sheets: [Int: CGImage] = {
        var out: [Int: CGImage] = [:]
        for scale in 1...3 {
            if let url = Bundle.main.url(forResource: "vehicles-\(scale)", withExtension: "png"),
               let image = UIImage(contentsOfFile: url.path)?.cgImage {
                out[scale] = image
            }
        }
        return out
    }()

    private static var cut: [String: CGImage] = [:]
    private static let lock = NSLock()
    private static let headings = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"]

    /// The nearest of the eight headings for a screen direction (y down). The sprites see the
    /// ground at about two to one, so the vertical part is doubled back before the angle.
    static func heading(_ dx: Double, _ dy: Double) -> String {
        let angle = atan2(dx, -2 * dy)
        let index = ((Int((angle / (.pi / 4)).rounded()) % 8) + 8) % 8
        return headings[index]
    }

    private static func sprite(_ key: String, scale: Int, frame f: Voxels.Frame, shadow: Bool) -> CGImage? {
        let held = "\(key)/\(scale)/\(shadow)"
        lock.lock()
        defer { lock.unlock() }
        if let image = cut[held] { return image }
        guard let sheet = sheets[scale], var image = sheet.cropping(to: CGRect(x: f.x, y: f.y, width: f.w, height: f.h)) else {
            return nil
        }
        if shadow {
            // The plane's shadow: its outline in black.
            let rect = CGRect(x: 0, y: 0, width: f.w, height: f.h)
            guard let context = CGContext(data: nil, width: f.w, height: f.h, bitsPerComponent: 8, bytesPerRow: f.w * 4,
                                          space: CGColorSpaceCreateDeviceRGB(),
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
            context.draw(image, in: rect)
            context.setBlendMode(.sourceIn)
            context.setFillColor(UIColor.black.cgColor)
            context.fill(rect)
            guard let black = context.makeImage() else { return nil }
            image = black
        }
        cut[held] = image
        return image
    }

    /// Draws a vehicle with its ground anchor at `ground`, facing screen direction `dir`; false
    /// when there is no sprite.
    @discardableResult
    static func draw(_ type: String, mine: Bool, ground: CGPoint, dir: CGVector, scale: Int, displayScale: Double,
                     alpha: Double = 1, shadow: Bool = false, in context: inout GraphicsContext) -> Bool {
        let key = "\(type)/\(mine ? "player" : "neutral")/\(heading(Double(dir.dx), Double(dir.dy)))"
        let ds = max(displayScale, 1)
        guard let f = atlas?.frames[key]?["\(scale)"], let image = sprite(key, scale: scale, frame: f, shadow: shadow) else {
            return false
        }
        let left = (Double(ground.x) * ds - f.ax).rounded() / ds
        let top = (Double(ground.y) * ds - f.ay).rounded() / ds
        let rect = CGRect(x: left, y: top, width: Double(f.w) / ds, height: Double(f.h) / ds)
        if alpha < 1 {
            var faded = context
            faded.opacity = alpha
            faded.draw(Image(decorative: image, scale: ds).interpolation(.none), in: rect)
        } else {
            context.draw(Image(decorative: image, scale: ds).interpolation(.none), in: rect)
        }
        return true
    }
}

/// One vehicle to draw this frame; gathered first so they can be drawn back to front. `ground` is
/// where it stands on the untilted ground and `dir` its direction of travel there.
struct VehicleSpot {
    let type: String
    let mine: Bool
    let ground: CGPoint
    let dir: CGVector
    var height = 0.0
}
