import SwiftUI

/// Technology: the level ladder, progress to the next level, what the next level buys and the
/// weekly research and marketing budgets.
struct ResearchView: View {
    @EnvironmentObject private var store: AppStore
    @State private var researchBudget: Double = 0
    @State private var marketingBudget: Double = 0
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Research")
                    .font(.title2.weight(.bold))
                    .foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                if let firm, let research = store.research, research.firm == firm.handle {
                    ladder(research)
                    Eyebrow(text: "What the next level buys")
                    StatGroup(
                        stats: [
                            Stat(label: "Inputs and labour per unit",
                                 value: String(format: "%.0f%% to %.0f%%", Double(research.intensityBps) / 100, Double(research.nextIntensityBps) / 100),
                                 note: "of the baseline", tone: Theme.positive),
                            Stat(label: "Quality",
                                 value: String(format: "%.0f to %.0f", Double(research.qualityBps) / 100, Double(research.nextQualityBps) / 100),
                                 note: "out of 200", tone: Theme.positive),
                        ],
                        columns: 2
                    )
                    researchPanel(firm, research)
                    marketingPanel(firm)
                    if let rejection {
                        Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                    }
                } else {
                    CalloutRow(icon: "research", text: "Found or take over a company to research.", tint: Theme.secondary)
                }
            }
            .padding(16)
            .frame(maxWidth: 900)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.ink)
        .onAppear { syncBudgets() }
        .onChange(of: store.research?.firm) { _, _ in syncBudgets() }
        .onAppear { store.tutorial.visited(.research) }
    }

    private func syncBudgets() {
        guard let research = store.research else { return }
        researchBudget = Double(research.researchBudgetCents) / 100
        marketingBudget = Double(research.marketingBudgetCents) / 100
    }

    private func ladder(_ research: ResearchDto) -> some View {
        let toNext = research.nextThresholdCents
        let fraction = toNext > 0 ? min(max(Double(research.progressCents) / Double(toNext), 0), 1) : 1
        return Panel {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 2) {
                        Eyebrow(text: "Technology level")
                        Text("\(research.level) of \(research.maxLevel)")
                            .font(.largeTitle.weight(.bold).monospacedDigit())
                            .foregroundStyle(Theme.primaryText)
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Eyebrow(text: "Industry frontier")
                        Text("Level \(research.frontier)")
                            .font(.headline)
                            .foregroundStyle(Theme.gold)
                        Text("You can research one level past it")
                            .font(.caption2)
                            .foregroundStyle(Theme.secondary)
                    }
                }
                HStack(alignment: .bottom, spacing: 3) {
                    ForEach(1...max(Int(research.maxLevel), 1), id: \.self) { step in
                        let reached = step <= Int(research.level)
                        let reachable = step <= Int(research.frontier) + 1
                        RoundedRectangle(cornerRadius: 2)
                            .fill(reached ? Theme.gold : (reachable ? Theme.gold.opacity(0.25) : Theme.panelRaised))
                            .frame(height: 10 + CGFloat(step) * 1.6)
                    }
                }
                .frame(height: 44, alignment: .bottom)
                .accessibilityHidden(true)
                Text(research.level >= research.maxLevel
                     ? "Top level reached."
                     : "Next level: \(Format.moneyCompact(research.progressCents)) of \(Format.moneyCompact(toNext)) spent (\(Int(fraction * 100))%). At the threshold a breakthrough lands \(research.successBps / 100) times in 100; a miss keeps half the progress.")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                ProgressView(value: fraction)
                    .tint(Theme.gold)
            }
        }
    }

    private func researchPanel(_ firm: FirmDto, _ research: ResearchDto) -> some View {
        let cap = max(1_000_000.0, min(Double(firm.cashCents) / 100 / 26, 5_000_000.0))
        return Panel {
            VStack(alignment: .leading, spacing: 8) {
                Eyebrow(text: "Research budget")
                ValueSlider(
                    value: $researchBudget,
                    range: 0...cap, step: 5_000, ticks: 10,
                    label: { $0 == 0 ? "None" : Format.money(Int64($0 * 100)) + " a week" },
                    detail: { value in
                        if value <= 0 { return "Research is paused." }
                        let remaining = max(research.nextThresholdCents - research.progressCents, 0)
                        let weeks = Int(Double(remaining) / (value * 100))
                        return "About \(weeks) weeks to the next threshold at this rate."
                    },
                    onCommit: { value in
                        Task { await setBudget("SetResearchBudget", firm: firm.handle, dollars: value) }
                    }
                )
            }
        }
    }

    private func marketingPanel(_ firm: FirmDto) -> some View {
        Panel {
            VStack(alignment: .leading, spacing: 8) {
                Eyebrow(text: "Marketing budget")
                ValueSlider(
                    value: $marketingBudget,
                    range: 0...2_000_000, step: 5_000, ticks: 10,
                    label: { $0 == 0 ? "None" : Format.money(Int64($0 * 100)) + " a week" },
                    detail: { _ in "Brand spend lifts demand for your goods in the regions you sell in." },
                    onCommit: { value in
                        Task { await setBudget("SetMarketingBudget", firm: firm.handle, dollars: value) }
                    }
                )
            }
        }
    }

    private func setBudget(_ command: String, firm: UInt64, dollars: Double) async {
        let json = CommandJSON.command(command, ["firm": firm, "amount_cents": Int64((dollars * 100).rounded())])
        rejection = await store.submit(json)
    }
}
