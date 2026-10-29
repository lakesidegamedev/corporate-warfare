import SwiftUI

/// Everything about one division in one place: financials, operating strategy, inputs, outputs,
/// plants and labour. Opened from the map, the CEO Office or the company page. The numbers come
/// from the engine; the controls reuse the commands the rest of the game already sends.
struct DivisionDetailSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let division: DivisionDto
    @State private var detail: DivisionDetailDto?
    @State private var posture: ProductLineDto?
    @State private var transfer: Double = 10_000

    private static let costNames = [
        "materials": "Materials", "wages": "Wages", "maintenance": "Maintenance",
        "land": "Land rent", "depreciation": "Depreciation",
    ]
    private static let tierNames = ["Low skill", "Mid skill", "High skill"]

    /// The live copy of the division: the list is replaced on every reload.
    private var live: DivisionDto {
        store.divisions.first { $0.id == division.id } ?? division
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    if let d = detail {
                        financials(d)
                        DivisionOpsSection(division: live)
                        strategy(d)
                        inputs(d)
                        outputs(d)
                        plants(d)
                        labour(d)
                    } else {
                        Text("Loading the division.").font(.callout).foregroundStyle(Theme.secondary)
                    }
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle("\(Sector.name(division.industry)) · \(division.regionName)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
        .task(id: reloadKey) {
            detail = await store.divisionDetail(division)
            if let d = detail { transfer = Double(d.transferBps) }
        }
        .sheet(item: $posture) { line in
            PostureSheet(line: line)
                .environmentObject(store)
                .presentationDetents([.medium, .large])
                .presentationBackground(Theme.panel)
        }
    }

    /// Reload after every command and every week.
    private var reloadKey: String {
        "\(division.id)/\(store.overview?.tick ?? 0)/\(live.blocks)/\(live.stockWeeks)/\(store.productLines.count)"
    }

    private func lots(_ milli: Int64) -> String {
        if milli > 0 && milli < 10_000 { return String(format: "%.1f", Double(milli) / 1000) }
        return (milli / 1000).formatted()
    }

    private func line(_ label: String, _ value: String, tint: Color = Theme.primaryText) -> some View {
        HStack {
            Text(label).font(.caption).foregroundStyle(Theme.secondary)
            Spacer()
            Text(value).font(.callout.weight(.semibold).monospacedDigit()).foregroundStyle(tint)
        }
    }

    private func section<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: title)
            content()
        }
    }

    private func financials(_ d: DivisionDetailDto) -> some View {
        let tint = d.operatingProfitCents >= 0 ? Theme.positive : Theme.negative
        return section("Financials, per week") {
            line("Revenue", Format.money(d.revenueCents))
            ForEach(d.costs, id: \.key) { cost in
                line(Self.costNames[cost.key] ?? cost.key, Format.money(-cost.cents), tint: Theme.secondary)
            }
            Divider().overlay(Theme.stroke)
            line("Operating profit", Format.money(d.operatingProfitCents), tint: tint)
            line("Margin", d.revenueCents > 0 ? String(format: "%.1f%%", Double(d.marginBps) / 100) : "no sales", tint: tint)
            line("Capital invested", Format.money(d.capexCents))
            line("Book value", Format.money(d.bookCents))
            if d.revenueTrend.count >= 2 {
                Text("Revenue, last \(d.revenueTrend.count) weeks, at today's prices")
                    .font(.caption2).foregroundStyle(Theme.secondary)
                trend(d.revenueTrend)
            }
            Text("Estimated from the plants' last week and your sales at today's asking prices.")
                .font(.caption2).foregroundStyle(Theme.secondary)
        }
    }

    private func trend(_ values: [Int64]) -> some View {
        let peak = Double(max(values.max() ?? 1, 1))
        return HStack(alignment: .bottom, spacing: 2) {
            ForEach(Array(values.enumerated()), id: \.offset) { _, value in
                Pixel.small
                    .fill(Theme.gold)
                    .frame(height: max(36 * Double(value) / peak, 1))
            }
        }
        .frame(height: 36, alignment: .bottom)
    }

    private func postureText(_ d: DivisionDetailDto) -> String {
        switch d.priceMode {
        case "market": return d.priceBps == 0 ? "At market" : String(format: "%+ld%% against market", Int(d.priceBps) / 100)
        case "cost_plus": return "Cost plus \(d.priceBps / 100)%"
        case "manual": return "Set by hand"
        default: return "Automatic"
        }
    }

    private func strategy(_ d: DivisionDetailDto) -> some View {
        section("Operating strategy") {
            Text("\(d.strategyName). \(d.meaning)").font(.callout).foregroundStyle(Theme.primaryText)
            HStack {
                line("Pricing", postureText(d))
                if let product = store.productLines.first(where: { $0.product == d.primaryProduct }) {
                    Button("Change") { posture = product }
                        .font(.caption.weight(.semibold)).buttonStyle(PixelButtonStyle()).tint(Theme.gold)
                }
            }
            if d.hasGroupBuyers {
                ValueSlider(
                    value: $transfer,
                    range: 0...12_000, step: 100, ticks: 12,
                    label: { "Transfer price \(Int($0) / 100)% of market" + ($0 < 10_000 ? " (toward cost)" : "") },
                    detail: { _ in "What sister companies pay this division for its output." },
                    onCommit: { value in
                        Task {
                            _ = await store.submit(CommandJSON.command("SetTransferPrice", [
                                "firm": d.firm, "region": d.regionIndex,
                                "industry": d.industryIndex, "bps": Int(value),
                            ]))
                        }
                    }
                )
            } else {
                line("Transfer price", "No group buyers", tint: Theme.secondary)
            }
            line("Stock target", d.stockWeeks > 0 ? "\(d.stockWeeks) weeks" : "\(d.stockWeeksEffective) weeks (company default)")
            DivisionStockTarget(division: live)
            DivisionShipping(division: live)
        }
    }

    private func inputs(_ d: DivisionDetailDto) -> some View {
        section("Inputs, per week") {
            if d.inputs.isEmpty {
                Text("This strategy buys nothing in.").font(.caption).foregroundStyle(Theme.secondary)
            }
            ForEach(Array(d.inputs.enumerated()), id: \.offset) { index, i in
                if index > 0 { Divider().overlay(Theme.stroke) }
                VStack(alignment: .leading, spacing: 1) {
                    Text(i.productName).font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text("Needs \(lots(i.needMilli)) · used \(lots(i.usedMilli)) · \(Format.price(i.priceCents)) a lot")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    Text("From \(i.source)").font(.caption).foregroundStyle(Theme.secondary)
                    if !i.storeClass.isEmpty {
                        let cover = i.needMilli > 0 ? String(format: "%.1f weeks of cover", Double(i.coverWeeksX100) / 100) : "none needed"
                        let tint = i.needMilli > 0 && i.coverWeeksX100 <= 0 ? Theme.negative : i.needMilli > 0 && i.coverWeeksX100 < 100 ? Theme.warning : Theme.secondary
                        Text("\(lots(i.stockMilli)) in stock · \(cover)").font(.caption).foregroundStyle(tint)
                    }
                }
            }
        }
    }

    private func outputs(_ d: DivisionDetailDto) -> some View {
        section("Outputs, last week") {
            ForEach(Array(d.outputs.enumerated()), id: \.offset) { index, o in
                if index > 0 { Divider().overlay(Theme.stroke) }
                VStack(alignment: .leading, spacing: 1) {
                    Text(o.productName + (o.byProduct ? " (by-product)" : ""))
                        .font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text("Made \(lots(o.madeMilli)) · sold \(lots(o.soldMilli)) · stored \(lots(o.storedMilli))")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    Text("Asking \(Format.price(o.priceCents)) a lot · market \(Format.price(o.marketCents))")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    Text("Buyers: " + (o.buyers.isEmpty ? "the open market" : o.buyers.joined(separator: ", ") + " and the open market"))
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
            }
        }
    }

    private func plants(_ d: DivisionDetailDto) -> some View {
        section("Plants") {
            ForEach(Array(d.plants.enumerated()), id: \.offset) { index, p in
                let warn = p.status != "operating" || p.inputsShort || p.staffBps < 9_000
                HStack {
                    Text("Plant \(index + 1) · \(p.blocks) blocks").font(.callout).foregroundStyle(Theme.primaryText)
                    Spacer()
                    Text(p.status == "operating" ? "\(p.utilisationBps / 100)% used" : p.status.capitalized)
                        .font(.caption).foregroundStyle(warn ? Theme.warning : Theme.primaryText)
                }
                let notes = ["\(p.ageWeeks) weeks old", "book \(Format.money(p.bookCents))"]
                    + (p.status == "operating" ? ["\(p.staffBps / 100)% staffed"] : [])
                    + (p.inputsShort ? ["short of inputs"] : [])
                Text(notes.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondary)
            }
        }
    }

    private func labour(_ d: DivisionDetailDto) -> some View {
        section("Labour") {
            ForEach(Array(d.tiers.enumerated()), id: \.offset) { tier, t in
                if t.needed > 0 || t.employed > 0 {
                    let short = t.employed < t.needed
                    let gap = t.wageCents - t.regionalWageCents
                    let versus = t.regionalWageCents <= 0 ? "" : gap >= 0 ? " · \(Format.money(gap)) over the region" : " · \(Format.money(-gap)) under the region"
                    HStack {
                        VStack(alignment: .leading, spacing: 1) {
                            Text("\(Self.tierNames[tier]): \(t.employed.formatted()) of \(t.needed.formatted())")
                                .font(.callout).foregroundStyle(short ? Theme.warning : Theme.primaryText)
                            Text("Offer \(Format.money(t.wageCents)) a week\(versus) · payroll \(Format.money(t.payrollCents))")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                        Spacer()
                        Button("+5%") {
                            Task {
                                guard let firm = store.selectedFirm else { return }
                                _ = await store.submit(CommandJSON.command("SetWageOffer", [
                                    "firm": firm, "tier": tier, "wage_cents": t.wageCents * 105 / 100 + 1,
                                ]))
                            }
                        }
                        .font(.caption.weight(.semibold)).buttonStyle(PixelButtonStyle()).tint(Theme.gold)
                    }
                }
            }
            if d.shortage {
                Text("Short of workers: output is capped by the thinnest tier. Raise the offer to draw more.")
                    .font(.caption).foregroundStyle(Theme.warning)
            }
            let union: String = {
                switch d.unionState {
                case "balloting": return "Union ballot under way"
                case "demanding": return "Union is demanding a settlement"
                case "striking": return "Union is on strike"
                default: return d.unionOrganisationBps > 0 ? "No union action" : "No union"
                }
            }()
            line("Union", union + (d.unionOrganisationBps > 0 ? " · \(d.unionOrganisationBps / 100)% organised" : ""))
            Text("Wage offers are set company wide, so a raise reaches every plant of the company.")
                .font(.caption2).foregroundStyle(Theme.secondary)
        }
    }
}
