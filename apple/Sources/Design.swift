import Foundation
import SwiftUI

/// Dark cartographic palette: an economic strategy game reads as a war room over a map, not a
/// settings form. Colours are fixed in both schemes; the app runs in dark appearance.
enum Theme {
    static let ink = Color(red: 0.043, green: 0.086, blue: 0.133)        // #0B1622 map sea and canvas
    static let panel = Color(red: 0.075, green: 0.129, blue: 0.184)      // #13212F
    static let panelRaised = Color(red: 0.102, green: 0.169, blue: 0.235) // #1A2B3C
    static let stroke = Color(red: 0.133, green: 0.212, blue: 0.290)     // #22364A
    static let land = Color(red: 0.169, green: 0.231, blue: 0.290)       // play-state land
    static let landDim = Color(red: 0.118, green: 0.165, blue: 0.212)    // neighbouring land
    static let water = Color(red: 0.086, green: 0.208, blue: 0.333)
    static let road = Color(red: 0.776, green: 0.541, blue: 0.239)
    static let primaryText = Color(red: 0.910, green: 0.933, blue: 0.961)
    static let secondary = Color(red: 0.576, green: 0.643, blue: 0.722)
    static let gold = Color(red: 0.886, green: 0.690, blue: 0.290)       // money and primary action
    static let navy = gold
    static let positive = Color(red: 0.247, green: 0.749, blue: 0.541)
    static let negative = Color(red: 0.898, green: 0.376, blue: 0.369)
    static let warning = Color(red: 0.949, green: 0.718, blue: 0.282)
    static let copper = Color(red: 0.788, green: 0.502, blue: 0.290)
    static let background = ink
    static let surface = panel
    static let text = primaryText

    static func background(for scheme: ColorScheme) -> Color { ink }
    static func surface(for scheme: ColorScheme) -> Color { panel }
    static func text(for scheme: ColorScheme) -> Color { primaryText }
    static func secondaryText(for scheme: ColorScheme) -> Color { secondary }
}

/// A House Divided sector types: one colour and symbol each, used on the map, in lists and in
/// the build flow so a sector is recognisable at a glance.
enum Sector {
    static let order = [
        "agriculture", "extraction", "energy", "chemical_industries", "manufacturing",
        "construction", "logistics", "retail", "healthcare", "technology", "media",
        "telecommunications", "real_estate", "defense", "financial",
    ]

    /// The engine's industry indices (`cw_sim::state::INDUSTRY_NAMES`), for commands that take
    /// a sector by number. Display order is `order`.
    static let engineOrder = [
        "agriculture", "extraction", "energy", "chemical_industries", "manufacturing",
        "construction", "logistics", "retail", "healthcare", "financial", "technology", "media",
        "telecommunications", "real_estate", "defense",
    ]

    static func engineIndex(_ id: String) -> Int? { engineOrder.firstIndex(of: id) }

    static func name(_ id: String) -> String {
        switch id {
        case "chemical_industries": return "Chemicals"
        case "telecommunications": return "Telecom"
        case "real_estate": return "Real Estate"
        default: return id.prefix(1).uppercased() + id.dropFirst()
        }
    }

    static func symbol(_ id: String) -> String {
        switch id {
        case "agriculture": return "leaf.fill"
        case "extraction": return "mountain.2.fill"
        case "energy": return "bolt.fill"
        case "chemical_industries": return "flask.fill"
        case "manufacturing": return "gearshape.2.fill"
        case "construction": return "hammer.fill"
        case "logistics": return "truck.box.fill"
        case "retail": return "cart.fill"
        case "healthcare": return "cross.case.fill"
        case "technology": return "cpu.fill"
        case "media": return "tv.fill"
        case "telecommunications": return "antenna.radiowaves.left.and.right"
        case "real_estate": return "building.2.fill"
        case "defense": return "shield.lefthalf.filled"
        case "financial": return "banknote.fill"
        default: return "circle.fill"
        }
    }

    static func color(_ id: String) -> Color {
        switch id {
        case "agriculture": return Color(red: 0.545, green: 0.765, blue: 0.290)
        case "extraction": return Color(red: 0.631, green: 0.533, blue: 0.498)
        case "energy": return Color(red: 1.000, green: 0.757, blue: 0.027)
        case "chemical_industries": return Color(red: 0.149, green: 0.776, blue: 0.855)
        case "manufacturing": return Color(red: 0.565, green: 0.643, blue: 0.682)
        case "construction": return Color(red: 1.000, green: 0.541, blue: 0.396)
        case "logistics": return Color(red: 0.494, green: 0.341, blue: 0.761)
        case "retail": return Color(red: 0.925, green: 0.251, blue: 0.478)
        case "healthcare": return Color(red: 0.937, green: 0.325, blue: 0.314)
        case "technology": return Color(red: 0.259, green: 0.647, blue: 0.961)
        case "media": return Color(red: 0.671, green: 0.278, blue: 0.737)
        case "telecommunications": return Color(red: 0.161, green: 0.714, blue: 0.965)
        case "real_estate": return Color(red: 0.400, green: 0.733, blue: 0.416)
        case "defense": return Color(red: 0.471, green: 0.565, blue: 0.612)
        case "financial": return Color(red: 1.000, green: 0.835, blue: 0.310)
        default: return Theme.secondary
        }
    }
}

/// A small uppercase label above a figure, the strategy-game equivalent of a column header.
struct Eyebrow: View {
    let text: String
    var body: some View {
        Text(text.uppercased())
            .font(.caption2.weight(.semibold))
            .tracking(0.8)
            .foregroundStyle(Theme.secondary)
    }
}

/// A raised dark panel with a hairline stroke.
struct Panel<Content: View>: View {
    var padding: CGFloat = 14
    @ViewBuilder var content: Content
    var body: some View {
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.panel, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
    }
}

/// A sector icon in its colour.
struct SectorBadge: View {
    let sector: String
    var size: CGFloat = 28
    var body: some View {
        Image(systemName: Sector.symbol(sector))
            .font(.system(size: size * 0.48, weight: .semibold))
            .foregroundStyle(Theme.ink)
            .frame(width: size, height: size)
            .background(Sector.color(sector), in: RoundedRectangle(cornerRadius: size * 0.28, style: .continuous))
            .accessibilityLabel(Sector.name(sector))
    }
}

/// A 0..10000 basis-point gauge, coloured from bad to good.
struct ConditionBar: View {
    let label: String
    let bps: Int32
    var invert = false
    var body: some View {
        let share = min(max(Double(bps) / 10_000.0, 0), 1)
        let quality = invert ? 1 - min(share, 1) : share
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(label).font(.caption).foregroundStyle(Theme.secondary)
                Spacer()
                Text(String(format: "%.0f", Double(bps) / 100)).font(.caption.monospacedDigit()).foregroundStyle(Theme.primaryText)
            }
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.stroke)
                    Capsule()
                        .fill(quality > 0.6 ? Theme.positive : quality > 0.35 ? Theme.warning : Theme.negative)
                        .frame(width: proxy.size.width * share)
                }
            }
            .frame(height: 5)
        }
        .accessibilityElement(children: .combine)
    }
}

enum Format {
    private static let currency: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = "USD"
        formatter.maximumFractionDigits = 0
        return formatter
    }()

    static func money(_ cents: Int64) -> String {
        let dollars = Double(cents) / 100.0
        return currency.string(from: NSNumber(value: dollars)) ?? "$0"
    }

    static func moneyCompact(_ cents: Int64) -> String {
        let dollars = Double(cents) / 100.0
        let magnitude = abs(dollars)
        if magnitude >= 1_000_000_000 {
            return String(format: "$%.1fB", dollars / 1_000_000_000)
        }
        if magnitude >= 1_000_000 {
            return String(format: "$%.1fM", dollars / 1_000_000)
        }
        if magnitude >= 10_000 {
            return String(format: "$%.0fK", dollars / 1_000)
        }
        return money(cents)
    }

    static func percentBps(_ bps: Int32) -> String {
        String(format: "%.1f%%", Double(bps) / 100.0)
    }

    static func lots(_ lots: Int64) -> String {
        "\(lots.formatted()) lots"
    }

    static func signedMoney(_ cents: Int64) -> String {
        (cents >= 0 ? "+" : "") + money(cents)
    }
}

struct MetricTile: View {
    let title: String
    let value: String
    var note: String? = nil
    var tone: Int = 0 // -1 negative, 0 neutral, 1 positive
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
                .foregroundStyle(Theme.secondaryText(for: scheme))
            Text(value)
                .font(.headline.monospacedDigit())
                .foregroundStyle(toneColor)
                .accessibilityLabel("\(title): \(value)")
            if let note {
                Text(note)
                    .font(.caption2)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(Theme.surface(for: scheme))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .accessibilityElement(children: .combine)
        .accessibilityHint("Activate the related row to open the ledger-backed explanation.")
    }

    private var toneColor: Color {
        switch tone {
        case 1: return Theme.positive
        case -1: return Theme.negative
        default: return Theme.text(for: scheme)
        }
    }
}

struct SectionCard<Content: View>: View {
    let title: String
    @ViewBuilder var content: Content

    var body: some View {
        Panel {
            VStack(alignment: .leading, spacing: 10) {
                Eyebrow(text: title)
                content
            }
            .foregroundStyle(Theme.primaryText)
        }
    }
}

struct EmptyNote: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.callout)
            .foregroundStyle(Theme.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}
