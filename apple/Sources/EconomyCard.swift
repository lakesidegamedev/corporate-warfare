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
            .background(economyPhaseTint(phase), in: Pixel.small)
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
                        if let fx = e.fx { FxSection(fx: fx) }
                        if !e.worldPrices.isEmpty { worldPrices(e) }
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
                PixelIcon(open ? "chevron_up" : "chevron_down", size: 12)
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
            Pixel.small.fill(color).frame(width: 9, height: 9)
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

    private func worldPrices(_ e: EconomyDto) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Eyebrow(text: "World prices")
            Text(worldPriceNote(e.pricePath))
                .font(.caption)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ForEach(Array(e.worldPrices.enumerated()), id: \.offset) { _, price in
                let level = Double(price.indexBps - 10_000) / 100
                let trend = Double(price.trendBps) / 100
                HStack {
                    Text(price.label).font(.caption).foregroundStyle(Theme.primaryText)
                    Spacer()
                    Text(String(format: "%+.0f%% since day one", level))
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                    Text(trend > 0.5 ? String(format: "up %.0f%%", trend) : trend < -0.5 ? String(format: "down %.0f%%", -trend) : "steady")
                        .font(.caption)
                        .foregroundStyle(trend > 0.5 ? Theme.warning : trend < -0.5 ? Theme.positive : Theme.secondary)
                        .frame(width: 70, alignment: .trailing)
                }
            }
        }
    }

    private func worldPriceNote(_ path: String) -> String {
        switch path {
        case "historical": return "Outside prices follow the real path of the period. They set what gateways charge to import and pay to export. Change over the last quarter shown at right."
        case "historical_variance": return "Outside prices are shaped like the period, with their own timing and depth. They set what gateways charge to import and pay to export. Change over the last quarter shown at right."
        default: return "Outside prices drift around where they began. They set what gateways charge to import and pay to export. Change over the last quarter shown at right."
        }
    }

    private func centralBank(_ e: EconomyDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: "Central bank")
            HStack(spacing: 8) {
                if !e.chairPerson.isEmpty { PixelPortrait(person: e.chairPerson, size: 36) }
                Text(e.chairName).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text(e.chairStance)
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(Theme.ink)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    .background(stanceTint(e.chairStance), in: Pixel.small)
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

    private func postRole(_ post: PostDto) -> String {
        switch post.post {
        case "country_leader": return "Leads the country, \(post.place)"
        case "party_leader": return "Opposition, \(post.place)"
        default: return "Governor of \(post.place)"
        }
    }

    private func government(_ e: EconomyDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: "Government")
            ForEach(Array(e.posts.filter { $0.post != "chair" }.enumerated()), id: \.offset) { _, post in
                PersonLine(person: post.person, role: postRole(post))
            }
            HStack {
                Text(e.partyName).font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Spacer()
                Text("Approval \(e.approvalBps / 100)%").font(.caption).foregroundStyle(Theme.secondary)
            }
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Pixel.small.fill(Theme.panelRaised)
                    Pixel.small
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
            marketRules(e)
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
            lawRow("Common carrier", laws.commonCarrier ? "Spare lane capacity sold at a regulated rate" : "Not required")
            lawRow("Production quotas", laws.quotaBps > 0 && !laws.quotaStates.isEmpty
                   ? "\(laws.quotaStates.joined(separator: ", ")): up to \(laws.quotaBps / 100)% of capacity when prices are soft"
                   : "None")
            lawRow("Supplier and customer deals", laws.verticalReviewBps > 0 ? "Reviewed from \(laws.verticalReviewBps / 100)% market share" : "Not reviewed")
            lawRow("Strategic reserve", laws.reserve ? "Buys in busts, sells in spikes" : "None")
            lawRow("Failing strategic firms", rescueText(laws))
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(heading != nil ? Theme.gold.opacity(0.1) : Theme.ink,
                    in: Pixel.shape)
    }

    private func rescueText(_ laws: LawsDto) -> String {
        let base: String
        switch laws.rescue {
        case "bailout": base = "State rescue loan"
        case "nationalise": base = "Nationalised"
        default: base = "Left to the courts"
        }
        return laws.privatise && laws.rescue != "none" ? base + ", sold back when sound" : base
    }

    /// The strategic reserve, quotas biting now, state firms and the latest disruptions.
    @ViewBuilder
    private func marketRules(_ e: EconomyDto) -> some View {
        if !(e.reserve.isEmpty && e.quotas.isEmpty && e.incidents.isEmpty && e.stateFirms.isEmpty) {
            VStack(alignment: .leading, spacing: 4) {
                Eyebrow(text: "Markets and rules")
                if !e.reserve.isEmpty {
                    HStack(spacing: 6) {
                        PixelIcon("storage", size: 14)
                        Text("Reserve holds \(Format.price(e.reserveValueCents)) at cost")
                            .font(.caption).foregroundStyle(Theme.primaryText)
                    }
                    ForEach(Array(e.reserve.enumerated()), id: \.offset) { _, r in
                        Text("\(r.label): \(r.lots) lots, \(r.phase)")
                            .font(.caption)
                            .foregroundStyle(r.phase == "buying" ? Theme.positive : r.phase == "selling" ? Theme.warning : Theme.secondary)
                    }
                }
                ForEach(Array(e.quotas.enumerated()), id: \.offset) { _, q in
                    HStack(spacing: 6) {
                        PixelIcon("gavel", size: 14)
                        Text("\(q.label) in \(q.regionName) capped at \(q.capBps / 100)% of capacity")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
                if !e.stateFirms.isEmpty {
                    HStack(spacing: 6) {
                        PixelIcon("flag", size: 14)
                        Text("State owned: \(e.stateFirms.joined(separator: ", "))")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                }
                ForEach(Array(e.incidents.prefix(4).enumerated()), id: \.offset) { _, i in
                    HStack(spacing: 6) {
                        PixelIcon(i.touchesYou ? "alert" : "warning", size: 14)
                        Text(i.text + (i.weeksLeft > 0 ? " \(i.weeksLeft) weeks left." : ""))
                            .font(.caption)
                            .foregroundStyle(i.touchesYou ? Theme.primaryText : Theme.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
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
                HStack(spacing: 8) {
                    if let person = item.person { PixelPortrait(person: person, size: 28) }
                    if let firm = item.firm, let name = item.firmName { FirmLogoButton(name: name, firm: firm) }
                    Text("\(ResultBuckets.weekLabel(item.week)): \(item.text)")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }
}

/// One chip per currency of the scenario; picking one re-quotes the investor's figures.
struct CurrencyPicker: View {
    @EnvironmentObject private var store: AppStore
    let fx: FxDto

    var body: some View {
        ChipFlow(spacing: 6) {
            ForEach(fx.currencies, id: \.id) { c in
                let on = c.id == fx.reporting
                Button {
                    Task { await store.chooseReportingCurrency(c.id) }
                } label: {
                    Text("\(c.symbol) \(c.name)")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(on ? Theme.ink : Theme.primaryText)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(on ? Theme.gold : Theme.panelRaised, in: Pixel.small)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
    }
}

/// Exchange rates against the reporting currency, each central bank and peg, and booked exchange results.
struct FxSection: View {
    let fx: FxDto

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                PixelIcon("swap", size: 14).foregroundStyle(Theme.secondary)
                Eyebrow(text: "Currencies")
            }
            ForEach(fx.currencies, id: \.id) { c in row(c) }
            Text("Rates are what one unit buys in \(fx.reporting).")
                .font(.caption2)
                .foregroundStyle(Theme.secondary)
            if fx.gainsCents != 0 || fx.lossesCents != 0 || fx.openForeignSalesCents != 0 {
                HStack(alignment: .top, spacing: 6) {
                    PixelIcon("coin", size: 14).foregroundStyle(Theme.secondary)
                    Text("Exchange results: gains \(Format.money(fx.gainsCents, currency: fx.reporting)), losses \(Format.money(fx.lossesCents, currency: fx.reporting)), foreign sales not yet collected \(Format.money(fx.openForeignSalesCents, currency: fx.reporting))")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            HStack(spacing: 6) {
                PixelIcon("coin", size: 14).foregroundStyle(Theme.gold)
                Text("Show my figures in").font(.caption).foregroundStyle(Theme.primaryText)
            }
            CurrencyPicker(fx: fx)
        }
    }

    private func row(_ c: CurrencyDto) -> some View {
        let isReporting = c.id == fx.reporting
        let trend = Double(c.trendBps) / 100
        let trendTint = trend > 0.05 ? Theme.positive : trend < -0.05 ? Theme.negative : Theme.secondary
        return HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 8) {
                    Text("\(c.symbol) \(c.name) · \(c.country)")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(Theme.primaryText)
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    if isReporting {
                        Text("Reporting currency").font(.caption).foregroundStyle(Theme.gold)
                    } else {
                        Text(Format.rate(c.valuePpm))
                            .font(.footnote.weight(.semibold).monospacedDigit())
                            .foregroundStyle(Theme.primaryText)
                        Text(String(format: "%+.1f%% over 13 wk", trend))
                            .font(.caption)
                            .foregroundStyle(trendTint)
                    }
                }
                Text("\(c.centralBank): rate \(percent(c.policyRateBps, digits: 2)), inflation \(percent(c.inflationBps))")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                if !c.chairName.isEmpty {
                    Text("Chair \(c.chairName), \(c.chairStance)")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if !isReporting {
                    let peg = pegStatus(c)
                    Text(peg.0).font(.caption).foregroundStyle(peg.1).fixedSize(horizontal: false, vertical: true)
                }
            }
            if !isReporting && c.historyPpm.count >= 2 {
                sparkline(c.historyPpm, tint: trendTint)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func pegStatus(_ c: CurrencyDto) -> (String, Color) {
        if !c.pegged { return ("Floating", Theme.secondary) }
        if let crisis = c.crisisWeek {
            return ("Band widened in \(ResultBuckets.weekLabel(crisis)), now plus or minus \(percent(c.bandBps))", Theme.warning)
        }
        if c.bandBps <= 0 { return ("Pegged", Theme.secondary) }
        let used = min(max(Int(abs(c.strainBps)) * 100 / Int(c.bandBps), 0), 100)
        let tint = used >= 75 ? Theme.negative : used >= 50 ? Theme.warning : Theme.secondary
        return ("Pegged, plus or minus \(percent(c.bandBps)): \(used)% of the way to the edge", tint)
    }

    /// A 52 week line of one rate, scaled to its own range.
    private func sparkline(_ values: [Int64], tint: Color) -> some View {
        let low = Double(values.min() ?? 0)
        let high = max(Double(values.max() ?? 1), low + 1)
        return Chart {
            ForEach(Array(values.enumerated()), id: \.offset) { index, value in
                LineMark(x: .value("Week", index), y: .value("Rate", Double(value)))
                    .foregroundStyle(tint)
            }
        }
        .chartXAxis(.hidden)
        .chartYAxis(.hidden)
        .chartYScale(domain: low...high)
        .frame(width: 96, height: 30)
        .accessibilityHidden(true)
    }
}
