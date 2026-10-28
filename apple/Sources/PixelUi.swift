import SwiftUI
import UIKit

/// The 8-bit look: stepped corners, hard two-point edges lit from the upper left, hard drop
/// shadows, no blur and no translucency, and a pixel font (Silkscreen, SIL Open Font License,
/// registered through UIAppFonts) for headings, labels and buttons. Body text and figures stay
/// in the system font so they read easily. Android and Windows follow the same rules.
enum Pixel {
    /// Panels, buttons and dialogs: corners cut by a pixel step.
    static let cut: CGFloat = 4
    /// Chips, pills and small badges.
    static let smallCut: CGFloat = 2
    static let shape = PixelShape(cut: cut)
    static let small = PixelShape(cut: smallCut)

    /// The pixel font at a size, scaled with Dynamic Type.
    static func font(_ size: CGFloat, bold: Bool = false) -> Font {
        .custom(bold ? "Silkscreen-Bold" : "Silkscreen-Regular", size: size)
    }

    static func uiFont(_ size: CGFloat, bold: Bool = false) -> UIFont {
        UIFont(name: bold ? "Silkscreen-Bold" : "Silkscreen-Regular", size: size) ?? .systemFont(ofSize: size, weight: bold ? .bold : .regular)
    }

    /// Solid system bars, so nothing the system draws is glass or blurred.
    static func configureSystemBars() {
        let bar = UINavigationBarAppearance()
        bar.configureWithOpaqueBackground()
        bar.backgroundColor = UIColor(Theme.panel)
        bar.shadowColor = UIColor(Theme.stroke)
        bar.titleTextAttributes = [.foregroundColor: UIColor(Theme.primaryText), .font: uiFont(14, bold: true)]
        bar.largeTitleTextAttributes = [.foregroundColor: UIColor(Theme.primaryText), .font: uiFont(24, bold: true)]
        UINavigationBar.appearance().standardAppearance = bar
        UINavigationBar.appearance().scrollEdgeAppearance = bar
        UINavigationBar.appearance().compactAppearance = bar
        let tabs = UITabBarAppearance()
        tabs.configureWithOpaqueBackground()
        tabs.backgroundColor = UIColor(Theme.panel)
        tabs.shadowColor = UIColor(Theme.stroke)
        for item in [tabs.stackedLayoutAppearance, tabs.inlineLayoutAppearance, tabs.compactInlineLayoutAppearance] {
            // The system font: the tab bar drops titles set in the pixel font.
            let font = UIFont.systemFont(ofSize: 10, weight: .bold)
            item.normal.titleTextAttributes = [.foregroundColor: UIColor(Theme.secondary), .font: font]
            item.selected.titleTextAttributes = [.foregroundColor: UIColor(Theme.gold), .font: font]
        }
        UITabBar.appearance().standardAppearance = tabs
        UITabBar.appearance().scrollEdgeAppearance = tabs
        let toolbar = UIToolbarAppearance()
        toolbar.configureWithOpaqueBackground()
        toolbar.backgroundColor = UIColor(Theme.panel)
        UIToolbar.appearance().standardAppearance = toolbar
        UIToolbar.appearance().scrollEdgeAppearance = toolbar
        let segments = UISegmentedControl.appearance()
        segments.selectedSegmentTintColor = UIColor(Theme.gold)
        segments.backgroundColor = UIColor(Theme.panelRaised)
        segments.setTitleTextAttributes([.foregroundColor: UIColor(Theme.primaryText), .font: uiFont(11)], for: .normal)
        segments.setTitleTextAttributes([.foregroundColor: UIColor(Theme.ink), .font: uiFont(11)], for: .selected)
    }
}

/// A rectangle with its corners cut off by a pixel step.
struct PixelShape: InsettableShape {
    var cut: CGFloat = Pixel.cut
    var inset: CGFloat = 0

    func path(in rect: CGRect) -> Path {
        let r = rect.insetBy(dx: inset, dy: inset)
        let c = max(min(cut, r.width / 2, r.height / 2), 0)
        var path = Path()
        path.move(to: CGPoint(x: r.minX + c, y: r.minY))
        path.addLine(to: CGPoint(x: r.maxX - c, y: r.minY))
        path.addLine(to: CGPoint(x: r.maxX, y: r.minY + c))
        path.addLine(to: CGPoint(x: r.maxX, y: r.maxY - c))
        path.addLine(to: CGPoint(x: r.maxX - c, y: r.maxY))
        path.addLine(to: CGPoint(x: r.minX + c, y: r.maxY))
        path.addLine(to: CGPoint(x: r.minX, y: r.maxY - c))
        path.addLine(to: CGPoint(x: r.minX, y: r.minY + c))
        path.closeSubpath()
        return path
    }

    func inset(by amount: CGFloat) -> PixelShape {
        var shape = self
        shape.inset += amount
        return shape
    }
}

/// The two-point bevel inside a frame: the top and left edges, or the bottom and right.
private struct BevelEdges: Shape {
    var cut: CGFloat
    var lit: Bool

    func path(in rect: CGRect) -> Path {
        let px: CGFloat = 2
        let w = rect.width
        let h = rect.height
        var path = Path()
        guard w > 2 * cut, h > 2 * cut else { return path }
        if lit {
            path.addRect(CGRect(x: cut, y: px, width: w - 2 * cut, height: px))
            path.addRect(CGRect(x: px, y: cut, width: px, height: h - 2 * cut))
        } else {
            path.addRect(CGRect(x: cut, y: h - 2 * px, width: w - 2 * cut, height: px))
            path.addRect(CGRect(x: w - 2 * px, y: cut, width: px, height: h - 2 * cut))
        }
        return path
    }
}

/// A solid frame for a panel or button: a hard shadow down and right, the fill, then a bevel,
/// light along the top and left edges and dark along the bottom and right, inside a hard outline.
struct PixelFrame: View {
    var fill: Color
    var outline: Color = Theme.stroke
    var shadow: CGFloat = 3
    var cut: CGFloat = Pixel.cut
    var raised = true

    var body: some View {
        ZStack {
            if shadow > 0 {
                PixelShape(cut: cut).fill(Color.black.opacity(0.55)).offset(x: shadow, y: shadow)
            }
            PixelShape(cut: cut).fill(fill)
            // Top and left catch the light, bottom and right fall into shade.
            BevelEdges(cut: cut, lit: true).fill(Color.white.opacity(raised ? 0.16 : 0.04))
            BevelEdges(cut: cut, lit: false).fill(Color.black.opacity(raised ? 0.40 : 0.15))
            PixelShape(cut: cut).stroke(outline, lineWidth: 2)
        }
    }
}

extension View {
    /// Draws a pixel frame behind the view and leaves room for its shadow below and to the right.
    func pixelFrame(_ fill: Color, outline: Color = Theme.stroke, shadow: CGFloat = 3, cut: CGFloat = Pixel.cut,
                    raised: Bool = true) -> some View {
        background(PixelFrame(fill: fill, outline: outline, shadow: shadow, cut: cut, raised: raised))
            .padding(.trailing, shadow)
            .padding(.bottom, shadow)
    }
}

/// Buttons in the pixel look: a solid framed block with an uppercase pixel-font label. Prominent
/// buttons fill with the tint; the rest sit on a raised panel.
struct PixelButtonStyle: ButtonStyle {
    var prominent = false
    var size: CGFloat = 12
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        let pressed = configuration.isPressed
        return configuration.label
            .font(Pixel.font(size, bold: true))
            .textCase(.uppercase)
            .tracking(0.5)
            .lineLimit(2)
            .multilineTextAlignment(.center)
            .foregroundStyle(prominent ? Theme.ink : Theme.primaryText)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background {
                ZStack {
                    PixelShape().fill(Color.black.opacity(0.55)).offset(x: pressed ? 1 : 3, y: pressed ? 1 : 3)
                    if prominent {
                        PixelShape().fill(.tint)
                    } else {
                        PixelShape().fill(Theme.panelRaised)
                    }
                    BevelEdges(cut: Pixel.cut, lit: true).fill(Color.white.opacity(pressed ? 0.04 : 0.16))
                    BevelEdges(cut: Pixel.cut, lit: false).fill(Color.black.opacity(pressed ? 0.15 : 0.40))
                    PixelShape().stroke(prominent ? Color.black.opacity(0.35) : Theme.stroke, lineWidth: 2)
                }
            }
            .offset(x: pressed ? 2 : 0, y: pressed ? 2 : 0)
            .padding(.trailing, 3)
            .padding(.bottom, 3)
            .opacity(isEnabled ? 1 : 0.5)
            .contentShape(PixelShape())
    }
}

/// A row of pixel chips standing in for a segmented control: one choice lit in gold.
struct PixelChoices<Value: Hashable>: View {
    /// Each choice's value and label.
    let options: [(Value, String)]
    @Binding var selection: Value
    var label: String = ""

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(options.enumerated()), id: \.offset) { _, option in
                let on = option.0 == selection
                Button {
                    selection = option.0
                } label: {
                    Text(option.1.uppercased())
                        .font(Pixel.font(11))
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                        .foregroundStyle(on ? Theme.ink : Theme.primaryText)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 7)
                        .frame(maxWidth: .infinity)
                        .background(on ? Theme.gold : Theme.panelRaised, in: Pixel.small)
                        .overlay(Pixel.small.stroke(on ? Color.clear : Theme.stroke, lineWidth: 1))
                        .contentShape(Pixel.small)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(label.isEmpty ? option.1 : "\(label): \(option.1)")
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
    }
}
