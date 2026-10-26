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
        let magnitude = abs(Double(cents) / 100.0)
        // The sign leads the currency symbol: -$21K, not $-21K.
        let sign = cents < 0 ? "-" : ""
        if magnitude >= 1_000_000_000 {
            return sign + String(format: "$%.1fB", magnitude / 1_000_000_000)
        }
        if magnitude >= 1_000_000 {
            return sign + String(format: "$%.1fM", magnitude / 1_000_000)
        }
        if magnitude >= 10_000 {
            return sign + String(format: "$%.0fK", magnitude / 1_000)
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

// MARK: - Screen frame and density system
//
// Three levels on every screen: a headline (the figure or decision that matters), compact
// supporting data grouped in one panel, and explanation only on demand. Root tab screens share
// one compact header instead of a large navigation title, and their content stops at the tab
// bar instead of showing through it.

/// A root tab screen: compact header, scrolling content, hidden navigation bar.
struct TabScreen<Trailing: View, Content: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var trailing: () -> Trailing
    @ViewBuilder var content: () -> Content

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    ScreenHeader(title: title, subtitle: subtitle, trailing: trailing)
                    content()
                }
                .padding(.horizontal, 16)
                .padding(.top, 4)
                .padding(.bottom, 24)
                .frame(maxWidth: 900)
                .frame(maxWidth: .infinity)
            }
            .background(Theme.ink)
            .toolbar(.hidden, for: .navigationBar)
            .modifier(HardBottomEdge())
        }
    }
}

extension TabScreen where Trailing == EmptyView {
    init(title: String, subtitle: String? = nil, @ViewBuilder content: @escaping () -> Content) {
        self.title = title
        self.subtitle = subtitle
        self.trailing = { EmptyView() }
        self.content = content
    }
}

/// Content scrolling under the floating tab bar ends at a hard edge instead of showing through
/// the glass.
struct HardBottomEdge: ViewModifier {
    func body(content: Content) -> some View {
        if #available(iOS 26.0, *) {
            content.scrollEdgeEffectStyle(.hard, for: .bottom)
        } else {
            content
        }
    }
}

/// Title, one line of context and the screen's own action on a single row.
struct ScreenHeader<Trailing: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.title2.weight(.bold))
                    .foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.caption.weight(.medium).monospacedDigit())
                        .foregroundStyle(Theme.secondary)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 8)
            trailing()
        }
        .padding(.vertical, 2)
    }
}

/// One figure in a stat group.
struct Stat: Identifiable {
    let label: String
    let value: String
    var note: String?
    var tone: Color = Theme.primaryText
    var id: String { label }
}

/// Related figures in one panel: an optional headline figure, then compact figures three to a
/// row. Replaces a card per number.
struct StatGroup: View {
    var title: String?
    var headline: Stat?
    let stats: [Stat]
    var columns = 3

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 10) {
                if let title { Eyebrow(text: title) }
                if let headline {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(headline.value)
                            .font(.title.weight(.bold).monospacedDigit())
                            .foregroundStyle(headline.tone)
                            .minimumScaleFactor(0.7)
                            .lineLimit(1)
                        HStack(spacing: 6) {
                            Text(headline.label).font(.caption.weight(.semibold)).foregroundStyle(Theme.secondary)
                            if let note = headline.note {
                                Text(note).font(.caption).foregroundStyle(Theme.secondary)
                            }
                        }
                    }
                    .accessibilityElement(children: .combine)
                }
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10, alignment: .leading), count: columns),
                          alignment: .leading, spacing: 10) {
                    ForEach(stats) { stat in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(stat.label)
                                .font(.caption2.weight(.semibold))
                                .foregroundStyle(Theme.secondary)
                                .lineLimit(1)
                            Text(stat.value)
                                .font(.callout.weight(.semibold).monospacedDigit())
                                .foregroundStyle(stat.tone)
                                .lineLimit(1)
                                .minimumScaleFactor(0.75)
                            if let note = stat.note {
                                Text(note)
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondary)
                                    .lineLimit(1)
                            }
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
            }
        }
    }
}

/// A short status pill: what a row means at a glance.
struct SignalPill: View {
    let text: String
    let tint: Color

    var body: some View {
        Text(text)
            .font(.caption2.weight(.bold))
            .foregroundStyle(tint)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(tint.opacity(0.14), in: Capsule())
            .lineLimit(1)
            .fixedSize()
    }
}

/// A one-line callout with an icon: the next action, a warning, a quiet-week note.
struct CalloutRow: View {
    let symbol: String
    let text: String
    var tint: Color = Theme.gold

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: symbol).foregroundStyle(tint).font(.callout)
            Text(text)
                .font(.subheadline)
                .foregroundStyle(Theme.primaryText)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .background(tint.opacity(0.10), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(tint.opacity(0.25), lineWidth: 1))
        .accessibilityElement(children: .combine)
    }
}

/// Where the quarter's money went: revenue against each cost line as bars on one scale, the
/// profit that is left, and one sentence naming the biggest driver.
struct ProfitDrivers: View {
    let weeks: [WeekDto]

    private struct Line: Identifiable {
        let name: String
        let cents: Int64
        var id: String { name }
    }

    private var revenue: Int64 { weeks.reduce(0) { $0 + $1.revenueCents } }
    private var net: Int64 { weeks.reduce(0) { $0 + $1.netIncomeCents } }
    private var costs: [Line] {
        [
            Line(name: "Inputs and goods", cents: weeks.reduce(0) { $0 + $1.costOfGoodsCents }),
            Line(name: "Wages", cents: weeks.reduce(0) { $0 + $1.wagesCents }),
            Line(name: "Overheads", cents: weeks.reduce(0) { $0 + $1.operatingExpenseCents }),
            Line(name: "Depreciation", cents: weeks.reduce(0) { $0 + $1.depreciationCents }),
            Line(name: "Interest", cents: weeks.reduce(0) { $0 + $1.netInterestCents }),
            Line(name: "Tax", cents: weeks.reduce(0) { $0 + $1.taxCents }),
        ].filter { $0.cents != 0 }
    }

    private var diagnosis: String {
        guard revenue > 0 else { return "No sales closed yet this quarter: every cost is a loss until the first sale settles." }
        guard let biggest = costs.max(by: { $0.cents < $1.cents }) else { return "No costs booked." }
        let share = Int((Double(biggest.cents) / Double(revenue) * 100).rounded())
        if net < 0 {
            return "\(biggest.name) alone take \(share)% of revenue. Sell more of what you make, raise prices if stock is selling out, or cut what is idle."
        }
        let margin = Int((Double(net) / Double(revenue) * 100).rounded())
        return "\(margin)% net margin. The biggest cost is \(biggest.name.lowercased()) at \(share)% of revenue."
    }

    var body: some View {
        let scale = Double(max(revenue, costs.map(\.cents).max() ?? 0, 1))
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Eyebrow(text: weeks.count >= 13 ? "Last quarter" : "Last \(weeks.count) weeks")
                    Spacer()
                    Text(Format.signedMoney(net))
                        .font(.headline.monospacedDigit())
                        .foregroundStyle(net >= 0 ? Theme.positive : Theme.negative)
                    Text("profit").font(.caption).foregroundStyle(Theme.secondary)
                }
                bar("Revenue", revenue, scale, Theme.positive)
                ForEach(costs) { line in
                    bar(line.name, line.cents, scale, Theme.negative.opacity(0.75))
                }
                Text(diagnosis)
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func bar(_ name: String, _ cents: Int64, _ scale: Double, _ tint: Color) -> some View {
        HStack(spacing: 8) {
            Text(name)
                .font(.caption)
                .foregroundStyle(Theme.primaryText)
                .frame(width: 108, alignment: .leading)
                .lineLimit(1)
            GeometryReader { proxy in
                Capsule()
                    .fill(tint)
                    .frame(width: max(2, proxy.size.width * min(abs(Double(cents)) / scale, 1)))
                    .frame(maxHeight: .infinity, alignment: .center)
            }
            .frame(height: 8)
            Text(Format.moneyCompact(cents))
                .font(.caption.monospacedDigit())
                .foregroundStyle(Theme.secondary)
                .frame(width: 64, alignment: .trailing)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(name) \(Format.moneyCompact(cents))")
    }
}
