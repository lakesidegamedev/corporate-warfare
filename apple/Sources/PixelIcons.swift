import SwiftUI
import CoreGraphics

/// The 8-bit UI icon set, shared with every platform (Resources/pixel-icons.json): a 16 by 16
/// grid per icon, each letter a colour from that icon's own palette, '.' transparent.
enum PixelIcons {
    struct Icon: Decodable {
        let palette: [String: String]
        let rows: [String]
    }

    private struct File: Decodable {
        let size: Int
        let icons: [String: Icon]
    }

    private static let file: File? = Bundle.main.url(forResource: "pixel-icons", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(File.self, from: $0) }

    static let size: Int = file?.size ?? 16
    static let all: [String: Icon] = file?.icons ?? [:]

    private static let cache = NSCache<NSString, CGImage>()

    static func rgb(_ hex: String) -> (UInt8, UInt8, UInt8) {
        let value = UInt32(hex.dropFirst(), radix: 16) ?? 0
        return (UInt8((value >> 16) & 0xFF), UInt8((value >> 8) & 0xFF), UInt8(value & 0xFF))
    }

    private static func luminance(_ hex: String) -> Double {
        let (r, g, b) = rgb(hex)
        return 0.299 * Double(r) / 255 + 0.587 * Double(g) / 255 + 0.114 * Double(b) / 255
    }

    /// One-colour use keeps the icon's shading: lighter palette entries stay solid, darker ones recede.
    static func monoAlpha(_ icon: Icon) -> [Character: Double] {
        let lum = icon.palette.mapValues(luminance)
        let top = max(lum.values.max() ?? 1, 0.0001)
        var out: [Character: Double] = [:]
        for (key, value) in lum {
            if let ch = key.first { out[ch] = min(max(value / top, 0.5), 1) }
        }
        return out
    }

    /// The exact pixels as a bitmap. `mono` keeps only coverage so the view tints it like a symbol.
    static func cgImage(_ name: String, mono: Bool) -> CGImage? {
        let key = "\(name)|\(mono)" as NSString
        if let hit = cache.object(forKey: key) { return hit }
        guard let icon = all[name] else { return nil }
        let n = size
        var bytes = [UInt8](repeating: 0, count: n * n * 4)
        let alpha = monoAlpha(icon)
        for (y, row) in icon.rows.enumerated() where y < n {
            for (x, ch) in row.enumerated() where x < n && ch != "." {
                guard let hex = icon.palette[String(ch)] else { continue }
                let i = (y * n + x) * 4
                if mono {
                    let a = UInt8((alpha[ch] ?? 1) * 255)
                    bytes[i] = a; bytes[i + 1] = a; bytes[i + 2] = a; bytes[i + 3] = a
                } else {
                    let (r, g, b) = rgb(hex)
                    bytes[i] = r; bytes[i + 1] = g; bytes[i + 2] = b; bytes[i + 3] = 255
                }
            }
        }
        guard let provider = CGDataProvider(data: Data(bytes) as CFData),
              let image = CGImage(
                width: n, height: n, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: n * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent
              ) else { return nil }
        cache.setObject(image, forKey: key)
        return image
    }

    /// Canvas renderer: fills each pixel of a grid as an exact rectangle, in one tint or the grid's own colours.
    static func draw(
        rows: [String], palette: [String: String], gridSize: Int,
        in context: inout GraphicsContext, rect: CGRect, tint: Color? = nil
    ) {
        let cell = min(rect.width, rect.height) / CGFloat(gridSize)
        for (y, row) in rows.enumerated() {
            for (x, ch) in row.enumerated() where ch != "." {
                guard let hex = palette[String(ch)] else { continue }
                let (r, g, b) = rgb(hex)
                let color = tint ?? Color(red: Double(r) / 255, green: Double(g) / 255, blue: Double(b) / 255)
                let left = (rect.minX + CGFloat(x) * cell).rounded()
                let top = (rect.minY + CGFloat(y) * cell).rounded()
                let right = (rect.minX + CGFloat(x + 1) * cell).rounded()
                let bottom = (rect.minY + CGFloat(y + 1) * cell).rounded()
                context.fill(Path(CGRect(x: left, y: top, width: right - left, height: bottom - top)), with: .color(color))
            }
        }
    }
}

/// A pixel icon by name. It takes the surrounding foreground colour like an SF Symbol does;
/// pass `colored: true` for the icon's own palette. Scales with Dynamic Type.
struct PixelIcon: View {
    let name: String
    var colored = false
    private let fixedSize: CGFloat?
    @ScaledMetric(relativeTo: .body) private var scaledSize: CGFloat = 16

    init(_ name: String, size: CGFloat? = nil, colored: Bool = false) {
        self.name = name
        self.fixedSize = size
        self.colored = colored
    }

    var body: some View {
        let side = fixedSize ?? scaledSize
        Group {
            if let image = PixelIcons.cgImage(name, mono: !colored) {
                Image(decorative: image, scale: 1)
                    .renderingMode(colored ? .original : .template)
                    .interpolation(.none)
                    .resizable()
            } else {
                Color.clear
            }
        }
        .frame(width: side, height: side)
        .accessibilityHidden(true)
    }
}

/// An icon beside a title. Behaves like a Label, so button styles and tab items treat it the same.
struct PixelLabel: View {
    let title: String
    let icon: String

    init(_ title: String, icon: String) {
        self.title = title
        self.icon = icon
    }

    var body: some View {
        Label {
            Text(title)
        } icon: {
            PixelIcon(icon)
        }
    }
}

/// The same icon drawn on a Canvas with exact pixels and an optional tint.
struct PixelIconCanvas: View {
    let name: String
    var tint: Color? = nil

    var body: some View {
        Canvas { context, size in
            guard let icon = PixelIcons.all[name] else { return }
            var context = context
            PixelIcons.draw(
                rows: icon.rows, palette: icon.palette, gridSize: PixelIcons.size,
                in: &context, rect: CGRect(origin: .zero, size: size), tint: tint
            )
        }
        .accessibilityHidden(true)
    }
}

/// Command Center category to its icon id.
func categoryIcon(_ category: String) -> String {
    "cat_" + category.lowercased().replacingOccurrences(of: " ", with: "_")
}
