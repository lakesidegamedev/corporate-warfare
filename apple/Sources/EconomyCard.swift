import Charts
import SwiftUI

func economyPhaseTint(_ phase: String) -> Color {
    switch phase {
    case "recession": return Theme.negative
    case "boom": return Theme.gold
    case "slowdown": return Theme.warning
    case "recovery": return Theme.positive
    default: return Color(red: 0.259, green: 0.647, blue: 0.961)
    }
}

private func percent(_ bps: Int32, digits: Int = 1) -> String {
    String(format: "%.\(digits)f%%", Double(bps) / 100.0)
}

private let outputTint = Color(red: 0.259, green: 0.647, blue: 0.961)

/// A short coloured pill naming the cycle phase.
struct PhasePill: View {
    let phase: String

    var body: some View {
        Text(phase.prefix(1).uppercased() + phase.dropFirst())
            .font(.caption2.weight(.bold))
            .foregroundStyle(Theme.ink)
            .padding(.horizontal, 9)
            .padding(.vertical, 2)
            .background(economyPhaseTint(phase), in: Capsule())
            .fixedSize()
    }
}

/// The economy, the central bank and the government. On iPhone it is a one-line header that opens
/// on tap; on iPad and Mac it is the full card.
struct EconomyCard: View {
    @EnvironmentObject private var store: AppStore
    var collapsible: Bool
    @State private var open: Bool

    init(collapsible: Bool) {
        self.collapsible = collapsible
        _open = State(initialValue: !collapsible)
    }

    var body: some View {
        if let e = store.economy {
            Panel {
                VStack(alignment: .leading, spacing: 12) {
                    header(e)
                    if open {
                        figures(e)
                        history(e)
                        credit(e)
                        centralBank(e)
                        government(e)
                        if !e.news.isEmpty { news(e) }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func header(_ e: EconomyDto) -> some View {
        let row = HStack(spacing: 8) {
            Eyebrow(text: "Economy")
            PhasePill(phase: e.phase)
            Spacer(minLength: 4)
            Text("Rate \(percent(e.policyRateBps, digits: 2)) · inflation \(percent(e.inflationBps))")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
            if collapsible {
                Image(systemName: open ? "chevron.up" : "chevron.down")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(Theme.secondary)
            }
        }
        if collapsible {
            Button {
                withAnimation(.easeOut(duration: 0.15)) { open.toggle() }
            } label: {
                row.contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(open ? "Economy, tap to fold" : "Economy, tap to open")
        } else {
            row
        }
    }

    private func figures(_ e: EconomyDto) -> some View {
        let stats: [Stat] = [
            Stat(label: "Growth, last year", value: percent(e.growthBps),
                 tone: e.growthBps >= 0 ? Theme.positive : Theme.negative),
            Stat(label: "Inflation", value: percent(e.inflationBps),
                 note: "target \(percent(e.targetInflationBps))",
                 tone: e.inflationBps > e.targetInflationBps + 100 ? Theme.warning : Theme.primaryText),
            Stat(label: "Output vs trend", value: String(format: "%+.1f%%", Double(e.outputGapBps) / 100),
                 tone: e.outputGapBps >= 0 ? Theme.positive : Theme.negative),
            Stat(label: "Central bank rate", value: percent(e.policyRateBps, digits: 2)),
            Stat(label: "Prime lending rate", value: percent(e.primeRateBps, digits: 2)),
            Stat(label: "10-year bond", value: percent(e.tenYearYieldBps, digits: 2)),
        ]
        return LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10, alignment: .leading), count: 3),
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
                        Text(note).font(.caption2).foregroundStyle(Theme.secondary).lineLimit(1)
                    }
                }
                .accessibilityElement(children: .combine)
            }
        }
    }

    /// Output gap, inflation and the central bank rate over the kept history, on one scale.
    @ViewBuilder
    private func history(_ e: EconomyDto) -> some View {
        if e.history.count >= 2 {
            Chart {
                ForEach(e.history, id: \.week) { point in
                    LineMark(x: .value("Week", Int(point.week)), y: .value("Percent", Double(point.outputGapBps) / 100),
                             series: .value("Series", "Output vs trend"))
                        .foregroundStyle(outputTint)
                }
                ForEach(e.history, id: \.week) { point in
                    LineMark(x: .value("Week", Int(point.week)), y: .value("Percent", Double(point.inflationBps) / 100),
                             series: .value("Series", "Inflation"))
                        .foregroundStyle(Theme.warning)
                }
                ForEach(e.history, id: \.week) { point in
                    LineMark(x: .value("Week", Int(point.week)), y: .value("Percent", Double(point.policyRateBps) / 100),
                             series: .value("Series", "Central bank rate"))
                        .foregroundStyle(Theme.gold)
                }
            }
            .chartXAxis(.hidden)
            .chartYAxis {
                AxisMarks(position: .leading) { _ in
                    AxisGridLine().foregroundStyle(Theme.stroke)
                    AxisValueLabel().foregroundStyle(Theme.secondary)
                }
            }
            .frame(height: 130)
            .accessibilityLabel("History of output against trend, inflation and the central bank rate")
            HStack(spacing: 14) {
                key(outputTint, "Output vs trend")
                key(Theme.warning, "Inflation")
                key(Theme.gold, "Central bank rate")
            }
        }
    }

    private func key(_ color: Color, _ text: String) -> some View {
        HStack(spacing: 5) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 9, height: 9)
            Text(text).font(.caption2).foregroundStyle(Theme.secondary)
        }
    }

    private func credit(_ e: EconomyDto) -> some View {
        let line: (String, Color)
        if e.creditBps >= 800 {
            line = ("Credit is badly stretched: a bust could come at any time.", Theme.negative)
        } else if e.creditBps >= 450 {
            line = ("Borrowing is running high: the risk of a bust is rising.", Theme.warning)
        } else {
            line = ("Borrowing is moderate.", Theme.secondary)
        }
        return Text(line.0).font(.caption).foregroundStyle(line.1)
    }

    private func centralBank(_ e: EconomyDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: "Central bank")
            HStack(spacing: 8) {
                Text(e.chairName).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text(e.chairStance)
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(Theme.ink)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    .background(stanceTint(e.chairStance), in: Capsule())
            }
            Text("Next meeting \(ResultBuckets.weekLabel(e.nextMeetingWeek)) · term ends \(ResultBuckets.weekLabel(e.chairTermEnds))")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
            ForEach(Array(e.decisions.prefix(4).enumerated()), id: \.offset) { _, d in
                Text("\(ResultBuckets.weekLabel(d.week)): \(decision(d)), citing \(d.reason).")
                    .font(.caption)
                    .foregroundStyle(Theme.primaryText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func stanceTint(_ stance: String) -> Color {
        if stance.contains("hawk") { return Theme.negative.opacity(0.85) }
        if stance.contains("dov") { return outputTint }
        return Theme.secondary
    }

    private func decision(_ d: RateDecisionDto) -> String {
        if d.changeBps > 0 { return "Raised \(percent(d.changeBps, digits: 2)) to \(percent(d.rateBps, digits: 2))" }
        if d.changeBps < 0 { return "Cut \(percent(-d.changeBps, digits: 2)) to \(percent(d.rateBps, digits: 2))" }
        return "Held at \(percent(d.rateBps, digits: 2))"
    }

    private func government(_ e: EconomyDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: "Government")
            HStack {
                Text(e.partyName).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Spacer()
                Text("Approval \(e.approvalBps / 100)%").font(.caption).foregroundStyle(Theme.secondary)
            }
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.panelRaised)
                    Capsule()
                        .fill(e.approvalBps >= 5_000 ? Theme.positive : Theme.warning)
                        .frame(width: proxy.size.width * CGFloat(min(max(Double(e.approvalBps) / 10_000, 0), 1)))
                }
            }
            .frame(height: 6)
            Text("Next election \(ResultBuckets.weekLabel(e.nextElectionWeek))")
                .font(.caption)
                .foregroundStyle(Theme.secondary)
            lawsTable(e.laws, heading: nil)
            if let pending = e.pendingLaws {
                lawsTable(pending, heading: "From \(ResultBuckets.weekLabel(e.pendingEffectiveWeek))")
            }
        }
    }

    private func lawsTable(_ laws: LawsDto, heading: String?) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(heading.map { "Announced, in force \($0)" } ?? "Laws in force")
                .font(.caption2.weight(.semibold))
                .foregroundStyle(heading != nil ? Theme.gold : Theme.secondary)
            lawRow("Corporate tax", percent(laws.corporateTaxBps, digits: 0))
            lawRow("Low-skill wage floor", Format.price(laws.wageFloorHourlyCents) + " an hour")
            lawRow("Merger review from", "\(laws.mergerReviewBps / 100)% market share")
            lawRow("Union law", laws.unions.prefix(1).uppercased() + laws.unions.dropFirst())
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(heading != nil ? Theme.gold.opacity(0.1) : Theme.ink,
                    in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private func lawRow(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title).font(.caption).foregroundStyle(Theme.secondary)
            Spacer()
            Text(value).font(.caption).foregroundStyle(Theme.primaryText)
        }
    }

    private func news(_ e: EconomyDto) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Eyebrow(text: "News")
            ForEach(Array(e.news.prefix(5).enumerated()), id: \.offset) { _, item in
                Text("\(ResultBuckets.weekLabel(item.week)): \(item.text)")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}
