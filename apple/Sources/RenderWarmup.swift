import SwiftUI
import UIKit

/// Draws a tiny scene once behind the main menu using the same canvas features the map uses (a
/// shadow filter, resolved text, a rasterised group), so the renderer sets those up while the
/// player is on the menu and not on the map's first frame. Safe to call more than once.
@MainActor
enum RenderWarmup {
    private static var done = false

    static func run() {
        guard !done else { return }
        done = true
        let scene = Canvas { context, _ in
            var labels = context
            labels.addFilter(.shadow(color: .black.opacity(0.85), radius: 1.5, x: 0, y: 1))
            labels.draw(Text("Aa").font(.system(size: 10, weight: .semibold)).foregroundStyle(Theme.primaryText),
                        at: CGPoint(x: 12, y: 12), anchor: .leading)
            let count = context.resolve(Text("1").font(.system(size: 10, weight: .bold)).foregroundStyle(Theme.ink))
            context.fill(Path(ellipseIn: CGRect(x: 24, y: 24, width: 16, height: 16)), with: .color(Theme.gold))
            context.draw(count, at: CGPoint(x: 32, y: 32))
        }
        .drawingGroup()
        .frame(width: 48, height: 48)
        let renderer = ImageRenderer(content: scene)
        renderer.scale = UIScreen.main.scale
        _ = renderer.cgImage
        warmControls()
    }

    /// One hidden instance of the heavy controls the detail sheets use (pixel font, pixel button
    /// style, value slider, a navigation stack with a lazy scroll), laid out offscreen, so the
    /// first sheet does not pay for font loading, glyph caching and first-use setup.
    private static func warmControls() {
        _ = Pixel.uiFont(12, bold: true)
        _ = Pixel.uiFont(14)
        let controls = NavigationStack {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    Eyebrow(text: "Warm")
                    Button("Go") {}
                        .font(.caption.weight(.semibold)).buttonStyle(PixelButtonStyle()).tint(Theme.gold)
                    Button("Go") {}.buttonStyle(PixelButtonStyle(prominent: true)).tint(Theme.gold)
                    ValueSlider(value: .constant(5), range: 0...10, step: 1, ticks: 10,
                                label: { "\(Int($0)) weeks" }, detail: { _ in "Warm" })
                    Text("Warm").font(Pixel.font(12, bold: true))
                    Divider().overlay(Theme.stroke)
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle("Warm")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") {} } }
        }
        .preferredColorScheme(.dark)
        let host = UIHostingController(rootView: controls)
        host.view.frame = CGRect(x: 0, y: 0, width: 393, height: 852)
        host.view.layoutIfNeeded()
    }
}
