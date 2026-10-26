import SwiftUI

/// Build a plant, A House Divided style: choose the sector type, compare its operating
/// strategies, choose the size, and confirm against the engine's own preview.
struct BuildFlowView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let stateCode: String

    @State private var sector: String?
    @State private var strategy: StrategyDto?
    @State private var blocks: UInt32 = 1
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var state: StateDto? { store.states.first { $0.code == stateCode } }

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
                            if strategy != nil { strategy = nil; preview = nil } else { sector = nil }
                        } label: {
                            Label("Back", systemImage: "chevron.left")
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
        let place = state?.name ?? stateCode
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
        return VStack(alignment: .leading, spacing: 12) {
            Eyebrow(text: "Operating strategy")
            ForEach(options, id: \.index) { option in
                Button {
                    strategy = option
                    blocks = 1
                    Task { await refreshPreview() }
                } label: {
                    StrategyCard(strategy: option, room: roomFor(option))
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func roomFor(_ strategy: StrategyDto) -> RoomDto? {
        state?.room.first { $0.product == strategy.output }
    }

    // MARK: confirm

    private func confirm(_ strategy: StrategyDto) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            StrategyCard(strategy: strategy, room: roomFor(strategy))
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    Stepper(value: $blocks, in: 1...4) {
                        HStack {
                            Text("Size").foregroundStyle(Theme.primaryText)
                            Spacer()
                            Text("\(blocks) block\(blocks == 1 ? "" : "s")").monospacedDigit().foregroundStyle(Theme.primaryText)
                        }
                    }
                    .onChange(of: blocks) { _, _ in Task { await refreshPreview() } }
                    figureRow("Capex", Format.money(strategy.capexCents * Int64(blocks)))
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
                            Label(warning, systemImage: "exclamationmark.triangle.fill").font(.caption).foregroundStyle(Theme.warning)
                        }
                        if let blocked = preview.blockedKey {
                            Label(CommandRejection(key: blocked, detail: preview.blockedDetail ?? "").message, systemImage: "xmark.octagon.fill")
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
                Label("Start construction", systemImage: "hammer.fill")
                    .font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
            .disabled(store.isBusy || preview?.blockedKey != nil)
        }
    }

    private func figureRow(_ label: String, _ value: String) -> some View {
        HStack {
            Text(label).font(.callout).foregroundStyle(Theme.secondary)
            Spacer()
            Text(value).font(.callout.monospacedDigit()).foregroundStyle(Theme.primaryText).multilineTextAlignment(.trailing)
        }
    }

    private func refreshPreview() async {
        guard let firm = store.selectedFirm ?? store.overview?.primaryFirm, let strategy else { return }
        preview = await store.previewBuild(firm: firm, recipe: strategy.index, blocks: blocks)
    }

    private func build(_ strategy: StrategyDto) async {
        guard let state else { return }
        rejection = await store.buildFacility(region: state.index, recipe: strategy.index, blocks: blocks)
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
                    Text("Makes \(strategy.weeklyOutputLots.formatted()) lots of \(strategy.outputName) a week per block")
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
