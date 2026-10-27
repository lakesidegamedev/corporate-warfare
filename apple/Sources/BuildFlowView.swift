import SwiftUI

/// Build a plant, A House Divided style: choose the sector type, compare its operating
/// strategies, choose the size, and confirm against the engine's own quote and preview. The
/// first plant of a sector type in a region opens a division; later plants join it and run
/// its strategy.
struct BuildFlowView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let regionCode: String

    @State private var sector: String?
    @State private var strategy: StrategyDto?
    @State private var blocks: UInt32 = 1
    @State private var preview: PreviewDto?
    @State private var quote: BuildQuoteDto?
    @State private var rejection: CommandRejection?

    private var board: RegionBoardDto? { store.boards.first { $0.code == regionCode } }

    /// The selected firm's division strategy for a sector type in this region, if any.
    private func locked(_ sector: String) -> UInt32? {
        store.divisions.first { $0.region == regionCode && $0.industry == sector }?.strategy
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if let strategy {
                        confirm(strategy)
                    } else if let sector {
                        strategies(in: sector)
                    } else {
                        sectors
                    }
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
                if sector != nil {
                    ToolbarItem(placement: .navigation) {
                        Button {
                            if strategy != nil { strategy = nil; preview = nil; quote = nil } else { sector = nil }
                        } label: {
                            PixelLabel("Back", icon: "chevron_left")
                        }
                    }
                }
            }
            .task {
                if store.selectedFirm == nil, let primary = store.overview?.primaryFirm {
                    await store.selectFirm(primary)
                }
            }
        }
    }

    private var title: String {
        let place = board?.name ?? regionCode
        if let strategy { return "\(strategy.name) in \(place)" }
        if let sector { return "\(Sector.name(sector)) in \(place)" }
        return "Build in \(place)"
    }

    // MARK: sector types

    private var sectors: some View {
        let ops = store.firmOps
        let columns = [GridItem(.adaptive(minimum: 150), spacing: 10)]
        return VStack(alignment: .leading, spacing: 12) {
            Eyebrow(text: "Sector type")
            Text("Plants in your primary sector use 5% fewer inputs and your secondary 2.5% fewer. Any other sector pays a 10% payroll coordination overhead.")
                .font(.caption).foregroundStyle(Theme.secondary)
            LazyVGrid(columns: columns, spacing: 10) {
                ForEach(Sector.order.filter { $0 != "financial" }, id: \.self) { id in
                    Button {
                        sector = id
                    } label: {
                        HStack(spacing: 10) {
                            SectorBadge(sector: id, size: 32)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(Sector.name(id)).font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                                Text(focusLabel(id, ops)).font(.caption2).foregroundStyle(focusColor(id, ops))
                                if let division = store.divisions.first(where: { $0.region == regionCode && $0.industry == id }) {
                                    Text("Your division · \(division.blocks) plant\(division.blocks == 1 ? "" : "s")")
                                        .font(.caption2.weight(.semibold)).foregroundStyle(Theme.gold)
                                } else if board?.specialization.contains(id) == true {
                                    Text("\(board?.stateName ?? "State") specialty")
                                        .font(.caption2.weight(.semibold)).foregroundStyle(Theme.positive)
                                }
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(10)
                        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func focusLabel(_ id: String, _ ops: FirmOpsDto?) -> String {
        guard let ops else { return "" }
        if ops.primary == id { return "Primary focus" }
        if ops.secondary == id { return "Secondary focus" }
        return "Off focus"
    }

    private func focusColor(_ id: String, _ ops: FirmOpsDto?) -> Color {
        guard let ops else { return Theme.secondary }
        if ops.primary == id || ops.secondary == id { return Theme.positive }
        return Theme.secondary
    }

    // MARK: strategies

    private func strategies(in sector: String) -> some View {
        let options = store.strategyCatalogue.filter { $0.industry == sector }
        let division = locked(sector)
        return VStack(alignment: .leading, spacing: 12) {
            Eyebrow(text: "Operating strategy")
            if division != nil {
                Text("You run a \(Sector.name(sector)) division here. New plants join it and run its strategy; retool the division from the CEO Office to switch.")
                    .font(.caption).foregroundStyle(Theme.secondary)
            } else {
                Text("Your first \(Sector.name(sector)) plant here opens a division: a $100,000 entry fee and the founding plant at half price.")
                    .font(.caption).foregroundStyle(Theme.secondary)
            }
            ForEach(options, id: \.index) { option in
                let allowed = division == nil || division == option.index
                Button {
                    strategy = option
                    blocks = 1
                    Task { await refresh() }
                } label: {
                    StrategyCard(strategy: option, room: roomFor(option))
                        .opacity(allowed ? 1 : 0.45)
                }
                .buttonStyle(.plain)
                .disabled(!allowed)
            }
        }
    }

    private func roomFor(_ strategy: StrategyDto) -> RoomDto? {
        board?.room.first { $0.product == strategy.output }
    }

    // MARK: confirm

    private func confirm(_ strategy: StrategyDto) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            StrategyCard(strategy: strategy, room: roomFor(strategy))
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    Stepper(value: $blocks, in: 1...20) {
                        HStack {
                            Text("Size").foregroundStyle(Theme.primaryText)
                            Spacer()
                            Text("\(blocks) plant\(blocks == 1 ? "" : "s")").monospacedDigit().foregroundStyle(Theme.primaryText)
                        }
                    }
                    .onChange(of: blocks) { _, _ in Task { await refresh() } }
                    if let quote {
                        if quote.opensDivision {
                            figureRow("Entry fee", Format.money(quote.entryFeeCents))
                        }
                        figureRow(quote.opensDivision ? "Plants (founding plant at half price)" : "Plants", Format.money(quote.capexCents))
                        if quote.dominanceBps > 10_000 {
                            figureRow("Dominance surcharge", String(format: "x%.2f", Double(quote.dominanceBps) / 10_000), tone: Theme.warning)
                        }
                        figureRow("Total", Format.money(quote.totalCents))
                        // Staged payment: entry fee and construction service now, equipment
                        // when it is delivered at completion.
                        figureRow("Pay now", Format.money(quote.entryFeeCents + quote.capexCents * 40 / 100))
                        figureRow("On delivery", Format.money(quote.capexCents - quote.capexCents * 40 / 100), tone: Theme.secondary)
                        figureRow("Your share after", "\(quote.shareAfterBps / 100)%")
                        figureRow("Inputs here", locationLabel(quote.locationInputBps), tone: quote.locationInputBps < 10_000 ? Theme.positive : quote.locationInputBps > 10_000 ? Theme.warning : Theme.primaryText)
                    } else {
                        figureRow("Capex", Format.money(strategy.capexCents * Int64(blocks)))
                    }
                    figureRow("Online in", "\(strategy.buildWeeks) weeks")
                    figureRow("Output", "\((strategy.weeklyOutputLots * Int64(blocks)).formatted()) lots of \(strategy.outputName) a week")
                }
            }
            if let preview {
                Panel {
                    VStack(alignment: .leading, spacing: 6) {
                        Eyebrow(text: "Engine preview")
                        figureRow("Cash change", Format.signedMoney(preview.cashChangeCents))
                        ForEach(preview.warnings, id: \.self) { warning in
                            PixelLabel(warning, icon: "warning").font(.caption).foregroundStyle(Theme.warning)
                        }
                        if let blocked = preview.blockedKey {
                            PixelLabel(CommandRejection(key: blocked, detail: preview.blockedDetail ?? "").message, icon: "blocked")
                                .font(.caption).foregroundStyle(Theme.negative)
                        }
                    }
                }
            }
            if let rejection {
                Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
            }
            Button {
                Task { await build(strategy) }
            } label: {
                PixelLabel("Start construction", icon: "hammer")
                    .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
            .disabled(store.isBusy || preview?.blockedKey != nil)
        }
    }

    private func figureRow(_ label: String, _ value: String, tone: Color = Theme.primaryText) -> some View {
        HStack {
            Text(label).font(.callout).foregroundStyle(Theme.secondary)
            Spacer()
            Text(value).font(.callout.monospacedDigit()).foregroundStyle(tone).multilineTextAlignment(.trailing)
        }
    }

    /// Plain words for the location input factor.
    private func locationLabel(_ bps: Int32) -> String {
        let delta = Double(bps - 10_000) / 100
        if bps == 10_000 { return "Home rate" }
        return String(format: "%+.1f%%", delta)
    }

    private func refresh() async {
        guard let firm = store.selectedFirm ?? store.overview?.primaryFirm, let strategy, let board else { return }
        quote = await store.buildQuote(region: board.index, recipe: strategy.index, blocks: blocks)
        preview = await store.previewBuild(firm: firm, region: board.index, recipe: strategy.index, blocks: blocks)
    }

    private func build(_ strategy: StrategyDto) async {
        guard let board else { return }
        rejection = await store.buildFacility(region: board.index, recipe: strategy.index, blocks: blocks)
        if rejection == nil { dismiss() }
    }
}

/// One operating strategy: what it makes, what it needs and what it costs.
struct StrategyCard: View {
    let strategy: StrategyDto
    let room: RoomDto?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                SectorBadge(sector: strategy.industry, size: 30)
                VStack(alignment: .leading, spacing: 2) {
                    Text(strategy.name).font(.headline).foregroundStyle(Theme.primaryText)
                    Text("Makes \(strategy.weeklyOutputLots.formatted()) lots of \(strategy.outputName) a week per plant")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
                Spacer(minLength: 0)
                VStack(alignment: .trailing, spacing: 2) {
                    Text(String(format: "%.0f%%", Double(strategy.referenceMarginBps) / 100))
                        .font(.headline.monospacedDigit())
                        .foregroundStyle(strategy.referenceMarginBps >= 0 ? Theme.positive : Theme.negative)
                    Eyebrow(text: "margin")
                }
            }
            FlowChips(items: strategy.inputs.map { "\($0.productName) \($0.lots)" }, tint: Theme.copper)
            HStack(spacing: 14) {
                small("Capex", Format.moneyCompact(strategy.capexCents))
                small("Build", "\(strategy.buildWeeks) wk")
                small("Staff", "\(strategy.workersLow + strategy.workersMid + strategy.workersHigh)")
                if strategy.depletes { small("Deposit", "20 yr") }
                if let room { small("Imports here", "\(room.importLots.formatted())/wk", tone: Theme.positive) }
            }
        }
        .padding(12)
        .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(room != nil ? Theme.positive.opacity(0.6) : Theme.stroke, lineWidth: 1))
        .accessibilityElement(children: .combine)
    }

    private func small(_ label: String, _ value: String, tone: Color = Theme.primaryText) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Eyebrow(text: label)
            Text(value).font(.caption.monospacedDigit().weight(.semibold)).foregroundStyle(tone)
        }
    }
}

/// Wrapping row of small chips.
struct FlowChips: View {
    let items: [String]
    var tint: Color = Theme.secondary

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 6) { chips }
            VStack(alignment: .leading, spacing: 6) {
                ForEach(Array(stride(from: 0, to: items.count, by: 3)), id: \.self) { start in
                    HStack(spacing: 6) {
                        ForEach(items[start..<min(start + 3, items.count)], id: \.self) { chip($0) }
                    }
                }
            }
        }
    }

    @ViewBuilder private var chips: some View {
        ForEach(items, id: \.self) { chip($0) }
    }

    private func chip(_ text: String) -> some View {
        Text(text)
            .font(.caption2.weight(.medium))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .foregroundStyle(tint)
            .background(tint.opacity(0.14), in: Capsule())
    }
}
