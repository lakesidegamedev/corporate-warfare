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

/// The 8-bit sector icons, shared with every platform (Resources/sector-icons.json): a 12 by 12
/// grid per sector, each letter a colour from that icon's palette.
enum SectorIcons {
    struct Icon: Decodable {
        let palette: [String: String]
        let rows: [String]
    }

    private struct File: Decodable {
        let icons: [String: Icon]
    }

    static let all: [String: Icon] = {
        guard let url = Bundle.main.url(forResource: "sector-icons", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let file = try? JSONDecoder().decode(File.self, from: data) else { return [:] }
        return file.icons
    }()

    static func color(_ hex: String) -> Color {
        let value = UInt64(hex.dropFirst(), radix: 16) ?? 0
        return Color(red: Double((value >> 16) & 0xFF) / 255, green: Double((value >> 8) & 0xFF) / 255, blue: Double(value & 0xFF) / 255)
    }
}

/// The Corporate Warfare logo: the CW emblem over the pixel wordmark, drawn from the shared
/// Resources/cw-logo.json so every platform shows the same pixels.
struct PixelLogo: View {
    private struct File: Decodable {
        let width: Int
        let height: Int
        let palette: [String: String]
        let rows: [String]
    }

    private static let file: File? = Bundle.main.url(forResource: "cw-logo", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(File.self, from: $0) }

    var body: some View {
        let file = Self.file
        Canvas { context, canvas in
            guard let file else { return }
            let cell = min(canvas.width / CGFloat(file.width), canvas.height / CGFloat(file.height))
            let origin = CGPoint(x: (canvas.width - cell * CGFloat(file.width)) / 2, y: (canvas.height - cell * CGFloat(file.height)) / 2)
            for (y, row) in file.rows.enumerated() {
                for (x, character) in row.enumerated() where character != "." {
                    guard let hex = file.palette[String(character)] else { continue }
                    let rect = CGRect(x: origin.x + CGFloat(x) * cell, y: origin.y + CGFloat(y) * cell, width: cell + 0.5, height: cell + 0.5)
                    context.fill(Path(rect), with: .color(SectorIcons.color(hex)))
                }
            }
        }
        .aspectRatio(CGFloat(file?.width ?? 64) / CGFloat(file?.height ?? 58), contentMode: .fit)
        .accessibilityLabel("Corporate Warfare")
    }
}

/// A sector's 8-bit icon on a dark tile with a rim in the sector's colour.
struct SectorBadge: View {
    let sector: String
    var size: CGFloat = 28

    var body: some View {
        Canvas { context, canvas in
            guard let icon = SectorIcons.all[sector] else { return }
            let inset = canvas.width * 0.12
            let cell = (canvas.width - inset * 2) / 12
            for (y, row) in icon.rows.enumerated() {
                for (x, character) in row.enumerated() where character != "." {
                    guard let hex = icon.palette[String(character)] else { continue }
                    // Whole-pixel cells, slightly overlapping, so the art stays crisp at any size.
                    let rect = CGRect(x: inset + CGFloat(x) * cell, y: inset + CGFloat(y) * cell, width: cell + 0.5, height: cell + 0.5)
                    context.fill(Path(rect), with: .color(SectorIcons.color(hex)))
                }
            }
        }
        .frame(width: size, height: size)
        .background(Theme.ink, in: RoundedRectangle(cornerRadius: size * 0.22, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: size * 0.22, style: .continuous).stroke(Sector.color(sector), lineWidth: max(1, size / 18)))
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

    private static let wholeNumber: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .decimal
        formatter.maximumFractionDigits = 0
        return formatter
    }()

    private static let centsNumber: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 2
        formatter.maximumFractionDigits = 2
        return formatter
    }()

    /// The reporting symbol and the symbol of every currency in the scenario. With one money the
    /// symbol is "$" and the table is empty, so every figure reads exactly as before.
    private static var reportingSymbol = "$"
    private static var symbols: [String: String] = [:]

    /// Currency id that fair values and share marks are quoted in; empty with one money.
    private(set) static var numeraire = ""

    /// Set the reporting symbol and the id to symbol table. An empty `reporting` restores "$".
    static func configure(reporting: String, symbols: [String: String], numeraire: String) {
        self.reportingSymbol = reporting.isEmpty ? "$" : reporting
        self.symbols = symbols
        self.numeraire = numeraire
    }

    /// Configure from the engine's exchange card; nil restores one money in dollars.
    static func configure(fx: FxDto?) {
        guard let fx else { return reset() }
        configure(
            reporting: fx.reportingSymbol,
            symbols: Dictionary(fx.currencies.map { ($0.id, $0.symbol) }, uniquingKeysWith: { first, _ in first }),
            numeraire: fx.currencies.first?.id ?? ""
        )
    }

    static func reset() {
        configure(reporting: "", symbols: [:], numeraire: "")
    }

    /// The symbol for a currency id; an empty or unknown id gives the reporting symbol.
    static func symbol(of currency: String) -> String {
        currency.isEmpty ? reportingSymbol : (symbols[currency] ?? reportingSymbol)
    }

    /// "$" sits against the figure; longer symbols such as "DM" get a thin gap.
    private static func leadText(_ sym: String) -> String {
        sym.count > 1 ? sym + "\u{2009}" : sym
    }

    private static func body(_ cents: Int64, _ sym: String, fractional: Bool) -> String {
        if sym == "$" {
            if fractional { return String(format: "$%.2f", Double(cents) / 100.0) }
            return currency.string(from: NSNumber(value: Double(cents) / 100.0)) ?? "$0"
        }
        let sign = cents < 0 ? "-" : ""
        let magnitude = abs(Double(cents) / 100.0)
        let formatter = fractional ? centsNumber : wholeNumber
        return sign + leadText(sym) + (formatter.string(from: NSNumber(value: magnitude)) ?? "0")
    }

    static func money(_ cents: Int64, currency id: String = "") -> String {
        body(cents, symbol(of: id), fractional: false)
    }

    /// A per-unit price with cents, for share prices where whole dollars hide the movement.
    static func price(_ cents: Int64, currency id: String = "") -> String {
        body(cents, symbol(of: id), fractional: true)
    }

    static func moneyCompact(_ cents: Int64, currency id: String = "") -> String {
        let magnitude = abs(Double(cents) / 100.0)
        // The sign leads the currency symbol: -$21K, not $-21K.
        let sign = cents < 0 ? "-" : ""
        let lead = leadText(symbol(of: id))
        if magnitude >= 1_000_000_000 {
            return sign + lead + String(format: "%.1fB", magnitude / 1_000_000_000)
        }
        if magnitude >= 1_000_000 {
            return sign + lead + String(format: "%.1fM", magnitude / 1_000_000)
        }
        if magnitude >= 10_000 {
            return sign + lead + String(format: "%.0fK", magnitude / 1_000)
        }
        return money(cents, currency: id)
    }

    /// An exchange rate from millionths, four decimals.
    static func rate(_ ppm: Int64) -> String {
        String(format: "%.4f", Double(ppm) / 1_000_000.0)
    }

    static func percentBps(_ bps: Int32) -> String {
        String(format: "%.1f%%", Double(bps) / 100.0)
    }

    static func lots(_ lots: Int64) -> String {
        "\(lots.formatted()) lots"
    }

    static func signedMoney(_ cents: Int64, currency id: String = "") -> String {
        (cents >= 0 ? "+" : "") + money(cents, currency: id)
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
    let icon: String
    let text: String
    var tint: Color = Theme.gold

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            PixelIcon(icon).foregroundStyle(tint)
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

/// The CFO: up to three reasons profit is down, each with one button. Shown only when the
/// company is losing money; the engine decides what matters.
struct CfoPanel: View {
    @EnvironmentObject private var store: AppStore
    let currency: String
    @Binding var rejection: CommandRejection?

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    PixelIcon("warning", size: 14).foregroundStyle(Theme.warning)
                    Eyebrow(text: "Your CFO: why profit is down")
                }
                ForEach(store.cfoAdvice, id: \.key) { item in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(alignment: .firstTextBaseline) {
                            Text(item.headline)
                                .font(.subheadline.weight(.semibold))
                                .foregroundStyle(Theme.primaryText)
                                .fixedSize(horizontal: false, vertical: true)
                            Spacer(minLength: 8)
                            Text("-\(Format.moneyCompact(item.weeklyCents, currency: currency)) a week")
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(Theme.negative)
                        }
                        Text(item.detail)
                            .font(.caption)
                            .foregroundStyle(Theme.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                        Button(item.actionLabel) {
                            Task { rejection = await store.applyAdvice(item) }
                        }
                        .buttonStyle(.bordered)
                        .tint(Theme.gold)
                        .accessibilityHint(item.detail)
                    }
                }
            }
        }
    }
}

// MARK: - Value slider

/// A fine-grained control for one number: a custom track with tick marks and a filled run from
/// the origin, a gold handle, a live readout of what the value means, and nudge buttons for exact
/// steps. Dragging updates the readout live; `onCommit` fires once when the drag ends or a nudge
/// lands, so the engine sees one command per change.
struct ValueSlider: View {
    @Binding var value: Double
    let range: ClosedRange<Double>
    var step: Double = 1
    /// Where the filled run starts: zero for a signed range, the bottom otherwise.
    var origin: Double?
    var ticks: Int = 10
    let label: (Double) -> String
    var detail: ((Double) -> String)?
    var tint: Color = Theme.gold
    var onCommit: (Double) -> Void = { _ in }

    @State private var dragging = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                Text(label(value))
                    .font(.title3.weight(.bold).monospacedDigit())
                    .foregroundStyle(Theme.primaryText)
                    .contentTransition(.numericText())
                Spacer()
                nudge("minus", -step)
                nudge("plus", step)
            }
            GeometryReader { proxy in
                let inset: CGFloat = 14
                let width = proxy.size.width - inset * 2
                let x = inset + position(value, width)
                let start = inset + position(origin ?? range.lowerBound, width)
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.panelRaised).frame(width: width, height: 8).offset(x: inset)
                    ForEach(0...ticks, id: \.self) { index in
                        Rectangle()
                            .fill(Theme.secondary.opacity(index % 5 == 0 ? 0.55 : 0.3))
                            .frame(width: 1, height: index % 5 == 0 ? 16 : 10)
                            .offset(x: inset + width * CGFloat(index) / CGFloat(ticks))
                    }
                    Capsule()
                        .fill(LinearGradient(colors: [tint.opacity(0.55), tint], startPoint: .leading, endPoint: .trailing))
                        .frame(width: abs(x - start), height: 8)
                        .offset(x: min(x, start))
                    Circle()
                        .fill(Theme.primaryText)
                        .overlay(Circle().stroke(tint, lineWidth: 3))
                        .frame(width: dragging ? 28 : 24, height: dragging ? 28 : 24)
                        .shadow(color: .black.opacity(0.35), radius: 3, y: 1)
                        .offset(x: x - (dragging ? 14 : 12))
                }
                .frame(height: 30)
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { drag in
                            dragging = true
                            value = snapped(range.lowerBound + Double((drag.location.x - inset) / max(width, 1)) * (range.upperBound - range.lowerBound))
                        }
                        .onEnded { _ in
                            dragging = false
                            onCommit(value)
                        }
                )
            }
            .frame(height: 30)
            HStack {
                Text(label(range.lowerBound))
                Spacer()
                Text(label(range.upperBound))
            }
            .font(.caption2.monospacedDigit())
            .foregroundStyle(Theme.secondary)
            if let detail {
                Text(detail(value))
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityValue(label(value))
        .accessibilityAdjustableAction { direction in
            value = snapped(value + (direction == .increment ? step : -step))
            onCommit(value)
        }
    }

    private func nudge(_ icon: String, _ delta: Double) -> some View {
        Button {
            value = snapped(value + delta)
            onCommit(value)
        } label: {
            PixelIcon(icon, size: 14)
                .frame(width: 30, height: 30)
                .background(Theme.panelRaised, in: Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(icon == "plus" ? "Increase" : "Decrease")
        .foregroundStyle(Theme.primaryText)
    }

    private func position(_ v: Double, _ width: CGFloat) -> CGFloat {
        CGFloat((v - range.lowerBound) / (range.upperBound - range.lowerBound)) * width
    }

    private func snapped(_ v: Double) -> Double {
        let clamped = min(max(v, range.lowerBound), range.upperBound)
        return (clamped / step).rounded() * step
    }
}
