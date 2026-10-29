import SwiftUI
import UIKit

// The shared pixel chart, the same as Compose's Chart.kt: line (stepped), bar or area, date
// range chips, period chips, overlay series with a legend and a read-out of the value under the
// finger with a hard vertical rule. Hard edges throughout, solid fills mixed toward the panel
// colour rather than translucent, labels in the pixel font.

/// How a chart draws its series.
enum PlotKind: String, CaseIterable {
    case line = "Line", bar = "Bar", area = "Area"
}

/// How far back a chart looks, in weeks; All is everything the engine kept.
enum PlotRange: String, CaseIterable {
    case w4 = "4W", w13 = "13W", y1 = "1Y", y5 = "5Y", all = "All"
    var weeks: Int? {
        switch self {
        case .w4: return 4
        case .w13: return 13
        case .y1: return 52
        case .y5: return 260
        case .all: return nil
        }
    }
}

/// The bucket each point stands for.
enum PlotPeriod: String, CaseIterable {
    case weekly = "Weekly", quarterly = "Quarterly", yearly = "Yearly"
    var weeks: Int {
        switch self {
        case .weekly: return 1
        case .quarterly: return 13
        case .yearly: return 52
        }
    }
    /// The engine's name for this period's statements.
    var engineName: String {
        switch self {
        case .weekly: return "week"
        case .quarterly: return "quarter"
        case .yearly: return "year"
        }
    }
}

/// How a chart fits its value axis. Zero starts at zero, for flows, counts per week and bars.
/// Level fits the data's lowest and highest values with a little room, for levels such as a
/// wage, a price, a stock or a balance. Bars always start at zero.
enum PlotScale {
    case zero, level
}

/// One value at a game week: the first week of the period it covers.
struct PlotPoint: Equatable {
    let week: Int
    let value: Int64
}

/// A named series drawn in one colour.
struct PlotSeries {
    let name: String
    let color: Color
    var points: [PlotPoint]
}

enum PlotColors {
    static let blue = Color(red: 0x6E / 255, green: 0xC6 / 255, blue: 1)
    static let violet = Color(red: 0xC7 / 255, green: 0x92 / 255, blue: 0xEA / 255)

    /// A solid mix of two colours, `amount` of the way from `a` to `b`.
    static func mix(_ a: Color, _ b: Color, _ amount: CGFloat) -> Color {
        var ar: CGFloat = 0, ag: CGFloat = 0, ab: CGFloat = 0, aa: CGFloat = 0
        var br: CGFloat = 0, bg: CGFloat = 0, bb: CGFloat = 0, ba: CGFloat = 0
        UIColor(a).getRed(&ar, green: &ag, blue: &ab, alpha: &aa)
        UIColor(b).getRed(&br, green: &bg, blue: &bb, alpha: &ba)
        return Color(red: ar + (br - ar) * amount, green: ag + (bg - ag) * amount, blue: ab + (bb - ab) * amount)
    }
}

/// Game dates for axes and read-outs. The game year is exactly 52 weeks, a quarter 13.
enum GameDates {
    static func year(_ week: Int) -> Int { ResultBuckets.startYear + (max(week, 1) - 1) / 52 }
    static func quarter(_ week: Int) -> Int { (max(week, 1) - 1) % 52 / 13 + 1 }

    static func label(_ week: Int, _ period: PlotPeriod) -> String {
        switch period {
        case .yearly: return String(year(week))
        case .quarterly: return "\(year(week)) Q\(quarter(week))"
        case .weekly: return String(format: "W%02d %d", (max(week, 1) - 1) % 52 + 1, year(week))
        }
    }

    /// Whether the bucket holding `lastWeek`, the latest closed week, still has weeks to come.
    static func partial(_ lastWeek: Int, _ period: PlotPeriod) -> Bool {
        period != .weekly && max(lastWeek, 1) % period.weeks != 0
    }

    /// Fold weekly closes into quarters or years, keeping each bucket's last close.
    static func lastClose(_ points: [PlotPoint], _ period: PlotPeriod) -> [PlotPoint] {
        if period == .weekly { return points }
        var buckets: [Int: Int64] = [:]
        for p in points { buckets[(max(p.week, 1) - 1) / period.weeks] = p.value }
        return buckets.keys.sorted().map { PlotPoint(week: $0 * period.weeks + 1, value: buckets[$0] ?? 0) }
    }

    /// Keep the points inside a range, counted back from the latest week of any series.
    static func within(_ series: [PlotSeries], _ range: PlotRange) -> [PlotSeries] {
        guard let weeks = range.weeks, let latest = series.flatMap(\.points).map(\.week).max() else { return series }
        return series.map { s in
            var copy = s
            copy.points = s.points.filter { $0.week > latest - weeks }
            return copy
        }
    }
}

/// A wrapping row that never grows past the width it is offered: an item wider than a line is
/// given the line's width, so a long legend or chip row cannot push a phone page sideways.
struct PlotFlow: Layout {
    var spacing: CGFloat = 6

    private func sizes(_ subviews: Subviews, _ limit: CGFloat) -> [CGSize] {
        subviews.map { view in
            let natural = view.sizeThatFits(.unspecified)
            return natural.width <= limit ? natural : view.sizeThatFits(ProposedViewSize(width: limit, height: nil))
        }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let limit = proposal.width ?? 10_000
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, widest: CGFloat = 0
        for size in sizes(subviews, limit) {
            if x > 0 && x + size.width > limit {
                x = 0
                y += row + spacing
                row = 0
            }
            x += size.width + spacing
            row = max(row, size.height)
            widest = max(widest, min(x - spacing, limit))
        }
        return CGSize(width: widest, height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for (view, size) in zip(subviews, sizes(subviews, bounds.width)) {
            if x > bounds.minX && x + size.width > bounds.maxX {
                x = bounds.minX
                y += row + spacing
                row = 0
            }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}

/// Small option chips in the pixel font, the selected one gold.
struct PlotChips: View {
    let options: [String]
    let selected: String
    let onPick: (String) -> Void

    var body: some View {
        HStack(spacing: 3) {
            ForEach(options, id: \.self) { option in
                let on = option == selected
                Button {
                    onPick(option)
                } label: {
                    Text(option)
                        .font(Pixel.font(10))
                        .lineLimit(1)
                        .fixedSize()
                        .foregroundStyle(on ? Theme.ink : Theme.primaryText)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(on ? Theme.gold : Theme.panelRaised, in: Pixel.small)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
    }
}

/// A square swatch and a name, for chart legends.
struct PlotLegend: View {
    let color: Color
    let name: String
    var body: some View {
        HStack(spacing: 4) {
            Rectangle().fill(color).frame(width: 8, height: 8)
            Text(name).font(.caption2).foregroundStyle(Theme.secondary).lineLimit(1)
        }
    }
}

/// The shared chart. `period` and `onPeriod` show the period chips; `periods` limits them.
/// Points must be in week order. `format` writes a value for the axis and the read-out.
/// `partialLast` marks the latest period as still open, labelled "to date" in the read-out and on
/// the axis. `scale` picks a zero-based or a fitted value axis.
struct PixelChart: View {
    let series: [PlotSeries]
    var title: String?
    var kinds: [PlotKind] = PlotKind.allCases
    var ranges: [PlotRange] = PlotRange.allCases
    var period: PlotPeriod = .weekly
    var periods: [PlotPeriod] = []
    var onPeriod: ((PlotPeriod) -> Void)?
    var height: CGFloat = 170
    var format: (Int64) -> String = { Format.moneyCompact($0) }
    var partialLast = false
    var scale: PlotScale = .zero

    @State private var kind: PlotKind
    @State private var range: PlotRange
    @State private var pointer: CGFloat?

    init(_ series: [PlotSeries], title: String? = nil, kinds: [PlotKind] = PlotKind.allCases, initialKind: PlotKind? = nil,
         initialRange: PlotRange = .all, ranges: [PlotRange] = PlotRange.allCases, period: PlotPeriod = .weekly,
         periods: [PlotPeriod] = [], onPeriod: ((PlotPeriod) -> Void)? = nil, height: CGFloat = 170,
         format: @escaping (Int64) -> String = { Format.moneyCompact($0) }, partialLast: Bool = false, scale: PlotScale = .zero) {
        self.series = series
        self.title = title
        self.kinds = kinds
        self.ranges = ranges
        self.period = period
        self.periods = periods
        self.onPeriod = onPeriod
        self.height = height
        self.format = format
        self.partialLast = partialLast
        self.scale = scale
        _kind = State(initialValue: initialKind ?? kinds.first ?? .line)
        _range = State(initialValue: initialRange)
    }

    private var shown: [PlotSeries] { GameDates.within(series, range).filter { !$0.points.isEmpty } }

    var body: some View {
        let shown = self.shown
        let weeks = Array(Set(shown.flatMap { $0.points.map(\.week) })).sorted()
        VStack(alignment: .leading, spacing: 5) {
            if let title {
                Eyebrow(text: title).fixedSize(horizontal: false, vertical: true)
            }
            PlotFlow(spacing: 8) {
                if ranges.count > 1 {
                    PlotChips(options: ranges.map(\.rawValue), selected: range.rawValue) { t in
                        if let r = ranges.first(where: { $0.rawValue == t }) { range = r }
                    }
                }
                if let onPeriod, periods.count > 1 {
                    PlotChips(options: periods.map(\.rawValue), selected: period.rawValue) { t in
                        if let p = periods.first(where: { $0.rawValue == t }) { onPeriod(p) }
                    }
                }
                if kinds.count > 1 {
                    PlotChips(options: kinds.map(\.rawValue), selected: kind.rawValue) { t in
                        if let k = kinds.first(where: { $0.rawValue == t }) { kind = k }
                    }
                }
            }
            if weeks.isEmpty {
                Text("No history in this range yet.")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .padding(.vertical, 12)
            } else {
                let readWeek = read(weeks)
                PlotFlow(spacing: 10) {
                    Text(dated(readWeek, GameDates.label(readWeek, period), weeks))
                        .font(Pixel.font(10))
                        .foregroundStyle(Theme.gold)
                    ForEach(Array(shown.enumerated()), id: \.offset) { _, s in
                        HStack(spacing: 4) {
                            PlotLegend(color: s.color, name: s.name)
                            Text(s.points.last(where: { $0.week <= readWeek }).map { format($0.value) } ?? "n/a")
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(Theme.primaryText)
                        }
                    }
                }
                GeometryReader { geo in
                    canvas(shown, weeks: weeks, readWeek: readWeek)
                        .contentShape(Rectangle())
                        .gesture(
                            DragGesture(minimumDistance: 0)
                                .onChanged { value in
                                    pointer = min(max(value.location.x / max(geo.size.width, 1), 0), 1)
                                }
                        )
                }
                .frame(height: height)
                .accessibilityElement()
                .accessibilityLabel(title ?? "Chart")
                .accessibilityValue(shown.map { s in "\(s.name) \(s.points.last.map { format($0.value) } ?? "n/a")" }.joined(separator: ", "))
            }
        }
    }

    /// The period under the finger, else the latest.
    private func read(_ weeks: [Int]) -> Int {
        guard let pointer, !weeks.isEmpty else { return weeks.last ?? 0 }
        return weeks[min(max(Int(pointer * CGFloat(weeks.count)), 0), weeks.count - 1)]
    }

    private func dated(_ week: Int, _ label: String, _ weeks: [Int]) -> String {
        partialLast && week == weeks.last ? "\(label) to date" : label
    }

    private func canvas(_ shown: [PlotSeries], weeks: [Int], readWeek: Int) -> some View {
        let kind = self.kind
        let scale = self.scale
        let period = self.period
        let format = self.format
        let pointer = self.pointer
        let partialLast = self.partialLast
        return Canvas { context, size in
            let px: CGFloat = 2
            func snap(_ v: CGFloat) -> CGFloat { (v / px).rounded(.down) * px }
            let axisRoom: CGFloat = 14
            let plotH = size.height - axisRoom
            let values = shown.flatMap { $0.points.map(\.value) }
            guard let high = values.max(), let low = values.min(), let first = weeks.first, let last = weeks.last else { return }
            let top: Int64
            let bottom: Int64
            if scale == .level && kind != .bar {
                // Fitted to the data with a tenth of its spread above and below, and never below
                // zero for figures that cannot go negative.
                let pad = max((high - low) / 10, max(abs(high), abs(low)) / 50, 1)
                top = high + pad
                bottom = low >= 0 ? max(low - pad, 0) : low - pad
            } else {
                top = max(high, 0)
                bottom = min(low, 0)
            }
            let span = CGFloat(max(top - bottom, 1))
            func y(_ v: Int64) -> CGFloat { snap(plotH * CGFloat(top - v) / span) }
            // Fills and bars rest on zero, or on the plot's floor when zero is below it.
            let zero = y(min(max(0, bottom), top))
            let slot = size.width / CGFloat(weeks.count)
            // Every point holds its value across its own period, so the latest one is drawn across
            // the last stretch of the plot rather than as a bare edge at the right.
            let step = zip(weeks, weeks.dropFirst()).map { $1 - $0 }.min() ?? period.weeks
            let stretch = CGFloat(last + step - first)
            let index = Dictionary(weeks.enumerated().map { ($0.element, $0.offset) }, uniquingKeysWith: { a, _ in a })
            func x(_ week: Int) -> CGFloat {
                kind == .bar ? snap(CGFloat(index[week] ?? 0) * slot) : snap(CGFloat(week - first) / stretch * size.width)
            }
            func centre(_ week: Int) -> CGFloat {
                kind == .bar ? x(week) + slot / 2 : x(week) + CGFloat(step) / stretch * size.width / 2
            }
            func label(_ text: String) -> GraphicsContext.ResolvedText {
                context.resolve(Text(text).font(Pixel.font(8)).foregroundColor(Theme.secondary))
            }
            // Grid: zero, top and the middle, labelled at the left.
            var lines: [Int64] = []
            for v in [top, (top + bottom) / 2, bottom] where !lines.contains(v) { lines.append(v) }
            for v in lines {
                let tone = v == 0 ? Theme.stroke : PlotColors.mix(Theme.panel, Theme.stroke, 0.5)
                context.fill(Path(CGRect(x: 0, y: y(v), width: size.width, height: px / 2)), with: .color(tone))
                let text = label(format(v))
                let h = text.measure(in: size).height
                context.draw(text, at: CGPoint(x: 0, y: max(y(v) - h, 0)), anchor: .topLeading)
            }
            if bottom < 0 && top > 0 {
                context.fill(Path(CGRect(x: 0, y: zero, width: size.width, height: px)), with: .color(Theme.stroke))
            }
            switch kind {
            case .bar:
                let bar = max(snap(slot * 0.8 / CGFloat(shown.count)), px)
                for (i, s) in shown.enumerated() {
                    for p in s.points {
                        let bx = x(p.week) + snap(slot * 0.1) + CGFloat(i) * bar
                        let by = y(p.value)
                        let rect = CGRect(x: bx, y: min(by, zero), width: max(bar - px / 2, px), height: max(abs(zero - by), px))
                        context.fill(Path(rect), with: .color(s.color))
                    }
                }
            case .line, .area:
                // A stepped path: across at each value, then straight down or up to the next.
                let paths: [(PlotSeries, Path, CGFloat)] = shown.map { s in
                    var path = Path()
                    for (i, p) in s.points.enumerated() {
                        let px0 = x(p.week)
                        if i == 0 {
                            path.move(to: CGPoint(x: px0, y: y(p.value)))
                        } else {
                            path.addLine(to: CGPoint(x: px0, y: y(s.points[i - 1].value)))
                            path.addLine(to: CGPoint(x: px0, y: y(p.value)))
                        }
                    }
                    let lastPoint = s.points[s.points.count - 1]
                    let endX = min(x(lastPoint.week + step), size.width - px)
                    path.addLine(to: CGPoint(x: endX, y: y(lastPoint.value)))
                    return (s, path, endX)
                }
                // Fills first, the largest at the back, so no fill hides a smaller series; then
                // every line on top.
                if kind == .area {
                    let ordered = paths.sorted { a, b in
                        a.0.points.reduce(0) { $0 + abs($1.value) } > b.0.points.reduce(0) { $0 + abs($1.value) }
                    }
                    for (s, path, endX) in ordered {
                        var fill = path
                        fill.addLine(to: CGPoint(x: endX, y: zero))
                        fill.addLine(to: CGPoint(x: x(s.points[0].week), y: zero))
                        fill.closeSubpath()
                        context.fill(fill, with: .color(PlotColors.mix(Theme.panel, s.color, 0.35)))
                    }
                }
                for (s, path, _) in paths {
                    context.stroke(path, with: .color(s.color), lineWidth: px)
                }
            }
            // The finger's week, as a hard vertical rule.
            if pointer != nil {
                context.fill(Path(CGRect(x: snap(centre(readWeek)), y: 0, width: px / 2, height: plotH)), with: .color(Theme.gold))
            }
            // Axis dates: up to five, spread evenly.
            let ticks = min(5, weeks.count)
            var used: [CGFloat] = []
            var labels = Set<String>()
            for t in 0..<ticks {
                let week = weeks[ticks == 1 ? 0 : t * (weeks.count - 1) / (ticks - 1)]
                let axisPeriod: PlotPeriod = period == .weekly && last - first > 26 ? .quarterly : period
                var name = GameDates.label(week, axisPeriod)
                if partialLast && week == last { name += " to date" }
                // Weekly points labelled by quarter would repeat a quarter; each is written once.
                if labels.contains(name) { continue }
                labels.insert(name)
                let text = label(name)
                let measured = text.measure(in: size)
                let tx = min(max(centre(week) - measured.width / 2, 0), size.width - measured.width)
                if used.contains(where: { abs($0 - tx) < measured.width + 4 }) { continue }
                used.append(tx)
                context.draw(text, at: CGPoint(x: tx, y: size.height - measured.height), anchor: .topLeading)
            }
        }
    }
}
