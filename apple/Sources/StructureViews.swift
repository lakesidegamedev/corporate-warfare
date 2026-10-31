import SwiftUI

/// Groups, transfer prices, reputation, bonds, joint ventures and activists in one dense column.
/// Set once; the engine alerts only when a decision is needed.
struct StructureSection: View {
    @Environment(AppStore.self) private var store
    @State private var settings: [String: Double] = [:]
    @State private var rejection: CommandRejection?

    private func pct(_ bps: Int64) -> String { String(format: "%.1f%%", Double(bps) / 100) }

    private func reason(_ key: String) -> String {
        switch key {
        case "falling_price": return "the share price has fallen a long way from its peak"
        case "missed_goals": return "the board missed its goals twice running"
        case "missed_guidance": return "management missed the profit range it gave the analysts"
        case "overvalued": return "the share price is far above what the business is worth"
        case "squeeze": return "pricing inside the group is taking value from minority holders"
        case "dividend_cut": return "the dividend stopped while the company can afford it"
        default: return key
        }
    }

    private func demand(_ key: String) -> String {
        switch key {
        case "dividend": return "a special dividend"
        case "divest": return "that the weakest part is floated or spun off"
        case "ceo": return "a new chief executive"
        case "restitution": return "restitution for the minority and prices back at market"
        default: return key
        }
    }

    private func outcome(_ key: String) -> String {
        switch key {
        case "conceded": return "conceded"
        case "compromised": return "settled"
        case "activist_won": return "the activist won"
        case "activist_lost": return "the activist lost"
        default: return key
        }
    }

    var body: some View {
        if let view = store.structure {
            VStack(alignment: .leading, spacing: 12) {
                let open = view.campaigns.filter { $0.mine && $0.outcome == "open" }
                if !open.isEmpty {
                    Eyebrow(text: "Activists")
                    ForEach(open, id: \.id) { campaign in card(campaign) }
                }
                Eyebrow(text: "Group supply and transfer prices")
                links(view)
                Eyebrow(text: "Reputation")
                reputation(view)
                Eyebrow(text: "Bonds")
                bonds(view)
                Eyebrow(text: "Joint ventures")
                ventures(view)
                let settled = view.campaigns.filter { $0.mine && $0.outcome != "open" }.prefix(3)
                if !settled.isEmpty {
                    Eyebrow(text: "Past campaigns")
                    ForEach(Array(settled), id: \.id) { c in
                        Text("\(c.firmName): \(outcome(c.outcome)) (\(demand(c.demand)))")
                            .font(.caption2).foregroundStyle(Theme.secondary)
                    }
                }
                if let rejection {
                    Text(rejection.message).font(.caption).foregroundStyle(Theme.negative)
                }
                Text("Bonds issued \(view.stats.bondsIssued) · ventures \(view.stats.venturesFormed) · listings \(view.stats.ipos) · spin-offs \(view.stats.spinOffs) · carve-outs \(view.stats.carveOuts) · activist campaigns \(view.stats.activistCampaigns) (\(view.stats.activistWins) won) · tax moved by internal pricing \(Format.signedMoney(view.stats.taxShiftCents)). Carve out, list, spin off, form a venture or issue a bond from Actions.")
                    .font(.caption2).foregroundStyle(Theme.secondary)
            }
        } else {
            Text("Nothing to show yet.").font(.caption).foregroundStyle(Theme.secondary)
        }
    }

    private func pill(_ title: String, tint: Color = Theme.gold, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.caption.weight(.semibold))
                .padding(.horizontal, 10).padding(.vertical, 5)
                .foregroundStyle(Theme.ink)
                .background(tint, in: Pixel.small)
        }
        .buttonStyle(.plain)
    }

    private func send(_ name: String, _ fields: [String: Any]) {
        Task { rejection = await store.submitAsInvestor(CommandJSON.command(name, fields)) }
    }

    private func card(_ c: CampaignDto) -> some View {
        Panel(padding: 10) {
            VStack(alignment: .leading, spacing: 4) {
                Text("\(c.firmName): \(reason(c.reason)).").font(.caption.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text("They want \(demand(c.demand)). They hold \(pct(Int64(c.stakeBps))); in a fight they win \(pct(Int64(c.chanceBps))) of the time. If you do nothing the board answers in week \(c.deadline).")
                    .font(.caption2).foregroundStyle(Theme.secondary)
                HStack(spacing: 6) {
                    pill("Concede") { send("RespondActivist", ["campaign": c.id, "stance": 0]) }
                    pill("Negotiate") { send("RespondActivist", ["campaign": c.id, "stance": 1]) }
                    pill("Fight", tint: Theme.negative) { send("RespondActivist", ["campaign": c.id, "stance": 2]) }
                }
            }
        }
    }

    @ViewBuilder
    private func links(_ view: StructureViewDto) -> some View {
        let mine = view.links.filter { $0.mine }
        if mine.isEmpty {
            Text("No two of your companies trade with each other yet. Divisions in one group buy each other's output before the open market.")
                .font(.caption2).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(mine.enumerated()), id: \.offset) { _, link in
            let key = "\(link.seller)-\(link.industryIndex)-\(link.regionIndex)"
            VStack(alignment: .leading, spacing: 4) {
                Text("\(link.sellerName) sells \(link.product) to \(link.buyerName): \(link.lotsWeekly) lots a week at \(Format.price(link.priceCents)) (cost \(Format.price(link.costCents)), market \(Format.price(link.marketCents)))")
                    .font(.caption).foregroundStyle(Theme.primaryText)
                ValueSlider(
                    value: Binding(
                        get: { settings[key] ?? Double(link.settingBps) },
                        set: { settings[key] = $0 }
                    ),
                    range: 0...12_000, step: 100, ticks: 12,
                    label: { "Transfer price \(Int($0) / 100)% of market" + ($0 < 100 ? " (cost)" : "") },
                    onCommit: { value in
                        send("SetTransferPrice", [
                            "firm": link.seller, "region": link.regionIndex,
                            "industry": link.industryIndex, "bps": Int(value),
                        ])
                    }
                )
                if link.squeezeCents > 0 && link.supplierMinorityBps > 0 {
                    Text("Minority holders of \(link.sellerName) have lost \(Format.moneyCompact(link.squeezeCents)) over the past year. A challenge becomes likely.")
                        .font(.caption2).foregroundStyle(Theme.warning)
                }
                if link.markdownBps > 0 {
                    Text("The market marks \(link.sellerName) down \(pct(Int64(link.markdownBps))) for persistent losses.")
                        .font(.caption2).foregroundStyle(Theme.warning)
                }
            }
        }
    }

    @ViewBuilder
    private func reputation(_ view: StructureViewDto) -> some View {
        if view.reputations.isEmpty {
            Text("Reputation builds from quality spending and marketing in each division.")
                .font(.caption2).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(view.reputations.prefix(10).enumerated()), id: \.offset) { _, r in
            HStack {
                Text("\(r.firmName) · \(Sector.name(r.industry)) · \(r.region)").font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                Spacer(minLength: 4)
                GeometryReader { proxy in
                    ZStack(alignment: .leading) {
                        Pixel.small.fill(Theme.panelRaised)
                        Pixel.small.fill(Theme.gold).frame(width: proxy.size.width * min(max(Double(r.reputationBps) / 10_000, 0), 1))
                    }
                }
                .frame(width: 70, height: 6)
                Text("\(r.reputationBps / 100)/100  \(r.premiumBps >= 0 ? "+" : "")\(pct(Int64(r.premiumBps))) price")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(r.premiumBps >= 0 ? Theme.positive : Theme.negative)
            }
        }
    }

    @ViewBuilder
    private func bonds(_ view: StructureViewDto) -> some View {
        Text("Priced off the ten-year yield, now \(pct(Int64(view.tenYearBps))), plus a credit spread for each issuer.")
            .font(.caption2).foregroundStyle(Theme.secondary)
        if view.bonds.isEmpty {
            Text("No bonds outstanding.").font(.caption2).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(view.bonds.enumerated()), id: \.offset) { _, b in
            VStack(alignment: .leading, spacing: 4) {
                Text("\(b.issuerName): \(Format.moneyCompact(b.principalCents)) at \(pct(Int64(b.couponBps))), \(b.weeksLeft) weeks, price \(pct(b.markBps)), yield \(pct(b.yieldBps)), spread \(pct(Int64(b.spreadNowBps)))" + (b.held > 0 ? " · you hold \(Format.moneyCompact(b.held))" : ""))
                    .font(.caption).foregroundStyle(b.mine ? Theme.gold : Theme.primaryText)
                HStack(spacing: 6) {
                    if b.available > 0 {
                        let qty = min(b.available, 10_000_000)
                        pill("Buy \(Format.moneyCompact(qty))") { send("PlaceBondOrder", ["bond": b.bond, "qty": qty]) }
                    }
                    if b.held > 0 {
                        pill("Sell", tint: Theme.secondary) { send("SellBond", ["bond": b.bond, "qty": b.held]) }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func ventures(_ view: StructureViewDto) -> some View {
        if view.ventures.isEmpty {
            Text("No joint ventures. Two groups can co-own a division, such as a pipeline or a refinery.")
                .font(.caption2).foregroundStyle(Theme.secondary)
        }
        ForEach(Array(view.ventures.enumerated()), id: \.offset) { _, v in
            VStack(alignment: .leading, spacing: 4) {
                Text("\(v.name): \(v.aName) \(pct(Int64(v.stakeABps))) and \(v.bName) \(pct(Int64(v.stakeBBps))), formed week \(v.formed), worth \(Format.moneyCompact(v.equityCents))")
                    .font(.caption).foregroundStyle(v.mine ? Theme.gold : Theme.primaryText)
                if v.mine && v.openToExit {
                    let partner = store.firms.contains(where: { $0.handle == v.a && $0.controlled }) ? v.a : v.b
                    HStack(spacing: 6) {
                        pill("Buy out") { send("BuyOutVenture", ["venture": v.venture, "firm": partner]) }
                        pill("Leave", tint: Theme.secondary) { send("ExitVenture", ["venture": v.venture, "firm": partner]) }
                    }
                } else if v.mine {
                    Text("locked up").font(.caption2).foregroundStyle(Theme.secondary)
                }
            }
        }
    }
}
