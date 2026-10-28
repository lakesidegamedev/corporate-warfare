import SwiftUI
import UIKit

/// The player's divisions as cel-shaded voxel buildings, one model per industry in three sizes,
/// baked by tools/voxel_sprites.py into atlases at 1, 2 and 3 pixels per half voxel. The map picks
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

    /// Cut sprites, kept: there are 45 frames at each of three scales.
    private static var cut: [String: CGImage] = [:]
    private static let lock = NSLock()

    /// Small up to three plants, medium up to eight, large beyond.
    static func size(_ plants: Int) -> Int {
        plants <= 3 ? 0 : plants <= 8 ? 1 : 2
    }

    /// The pixel scale for a map drawn at `zoomScale` on a screen of `displayScale` pixels a point.
    static func scale(_ zoomScale: Double, displayScale: Double) -> Int {
        min(max(Int((zoomScale * displayScale).rounded()), 1), 3)
    }

    static func frame(_ sector: String, size: Int, scale: Int) -> Frame? {
        atlas?.frames["\(sector)/\(size)"]?["\(scale)"]
    }

    private static func sprite(_ sector: String, size: Int, scale: Int, frame f: Frame) -> CGImage? {
        let key = "\(sector)/\(size)/\(scale)"
        lock.lock()
        defer { lock.unlock() }
        if let held = cut[key] { return held }
        guard let sheet = sheets[scale], let image = sheet.cropping(to: CGRect(x: f.x, y: f.y, width: f.w, height: f.h)) else {
            return nil
        }
        cut[key] = image
        return image
    }

    /// The top of the sprite on screen, in points, for a building whose ground anchor is at `ground`.
    static func top(_ sector: String, plants: Int, ground: CGPoint, scale: Int, displayScale: Double) -> CGFloat? {
        guard let f = frame(sector, size: size(plants), scale: scale) else { return nil }
        return CGFloat(((Double(ground.y) * displayScale - f.ay).rounded()) / displayScale)
    }

    /// Draws a building with its ground anchor at `ground` and returns the top of the sprite on
    /// screen, or nil when there is no sprite for it (the caller falls back to a plain box).
    static func draw(_ sector: String, plants: Int, ground: CGPoint, scale: Int, displayScale: Double,
                     in context: inout GraphicsContext) -> CGFloat? {
        let ds = max(displayScale, 1)
        guard let f = frame(sector, size: size(plants), scale: scale),
              let image = sprite(sector, size: size(plants), scale: scale, frame: f) else { return nil }
        // Snap to whole device pixels so each sprite pixel lands on one screen pixel.
        let left = (Double(ground.x) * ds - f.ax).rounded() / ds
        let top = (Double(ground.y) * ds - f.ay).rounded() / ds
        let rect = CGRect(x: left, y: top, width: Double(f.w) / ds, height: Double(f.h) / ds)
        context.draw(Image(decorative: image, scale: ds).interpolation(.none), in: rect)
        return CGFloat(top)
    }
}
