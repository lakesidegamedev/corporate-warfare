import SwiftUI

/// What each good the company makes earns per lot, how much of it sold, and how its price is
/// set. After A House Divided's pricing posture: automatic, a step against the market, or a
/// markup over the company's own cost.
struct PricingPanel: View {
    @EnvironmentObject private var store: AppStore
    @State private var editing: ProductLineDto?

    var body: some View {
        if !store.productLines.isEmpty {
            Panel(padding: 12) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack {
                        Eyebrow(text: "Pricing")
                        Spacer()
                        Text("profit / lot · sold").font(.caption2).foregroundStyle(Theme.secondary)
                    }
                    .padding(.bottom, 6)
                    ForEach(Array(store.productLines.enumerated()), id: \.element.product) { index, line in
                        if index > 0 { Divider().overlay(Theme.stroke) }
                        Button { editing = line } label: { PricingRow(line: line) }
                            .buttonStyle(.plain)
                    }
                }
            }
            .sheet(item: $editing) { line in
                PostureSheet(line: line)
                    .environmentObject(store)
                    .presentationDetents([.medium, .large])
                    .presentationBackground(Theme.panel)
            }
        }
    }
}

extension ProductLineDto: Identifiable {
    public var id: UInt32 { product }
}

enum PostureText {
    static func label(_ line: ProductLineDto) -> String {
        switch line.mode {
        case "market":
            return line.postureBps == 0 ? "At market" : String(format: "%+ld%% vs market", Int(line.postureBps) / 100)
        case "cost_plus":
            return "Cost +\(line.postureBps / 100)%"
        case "fixed":
            return "Fixed price"
        default:
            return "Auto"
        }
    }

    /// Share of last week's output that found a buyer.
    static func soldShare(_ line: ProductLineDto) -> Int? {
        guard line.madeLastWeekLots > 0 else { return nil }
        return Int(min(Double(line.soldLastWeekLots) / Double(line.madeLastWeekLots), 9.99) * 100)
    }
}

private struct PricingRow: View {
    let line: ProductLineDto

    private var perLot: Int64 { line.priceCents - line.unitCostCents }

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(line.name).font(.callout.weight(.medium)).foregroundStyle(Theme.primaryText).lineLimit(1)
                Text("\(Format.money(line.priceCents)) · \(PostureText.label(line))")
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(Theme.secondary)
            }
            Spacer(minLength: 6)
            if line.unitCostCents > 0 {
                Text(Format.signedMoney(perLot))
                    .font(.callout.weight(.semibold).monospacedDigit())
                    .foregroundStyle(perLot >= 0 ? Theme.positive : Theme.negative)
            }
            if let sold = PostureText.soldShare(line) {
                SignalPill(text: "\(sold)%", tint: sold >= 95 ? Theme.positive : sold >= 70 ? Theme.gold : Theme.negative)
            }
            Image(systemName: "chevron.right").font(.caption2.weight(.semibold)).foregroundStyle(Theme.secondary)
        }
        .padding(.vertical, 8)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityHint("Change how this good is priced")
    }
}

/// Choose how one good is priced. Undercut to sell out ahead of rivals; price above the market
/// for margin and risk holding stock; or mark up your own cost.
struct PostureSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let line: ProductLineDto
    @State private var rejection: CommandRejection?
    @State private var exact = false

    private let marketSteps: [Int32] = [-2_000, -1_000, -500, 0, 500, 1_000, 2_000]
    private let costSteps: [Int32] = [1_000, 2_000, 3_000, 5_000]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    StatGroup(
                        headline: Stat(label: "Your price per lot", value: Format.money(line.priceCents), note: PostureText.label(line)),
                        stats: [
                            Stat(label: "Market", value: Format.money(line.marketCents)),
                            Stat(label: "Your cost", value: line.unitCostCents > 0 ? Format.money(line.unitCostCents) : "n/a"),
                            Stat(label: "Sold last week", value: PostureText.soldShare(line).map { "\($0)%" } ?? "n/a",
                                 note: "\(line.soldLastWeekLots.formatted()) of \(line.madeLastWeekLots.formatted()) lots"),
                        ]
                    )
                    Text("Buyers take the cheapest offer first. Undercut to sell out ahead of rivals; price above the market for margin and risk unsold stock. Prices stay between half and twice the reference.")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    option(title: "Automatic", detail: "Follows sell-through: cuts when stock piles up, raises when it sells out.",
                           selected: line.mode == "auto") { await set(mode: 0, bps: 0) }

                    Eyebrow(text: "Against the market")
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 72), spacing: 8)], spacing: 8) {
                        ForEach(marketSteps, id: \.self) { step in
                            chip(step == 0 ? "Market" : String(format: "%+ld%%", Int(step) / 100),
                                 selected: line.mode == "market" && line.postureBps == step) {
                                await set(mode: 1, bps: step)
                            }
                        }
                    }
                    Eyebrow(text: "Over your cost")
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 72), spacing: 8)], spacing: 8) {
                        ForEach(costSteps, id: \.self) { step in
                            chip("+\(step / 100)%", selected: line.mode == "cost_plus" && line.postureBps == step) {
                                await set(mode: 2, bps: step)
                            }
                        }
                    }
                    .disabled(line.unitCostCents == 0)

                    Button("Set an exact price") { exact = true }
                        .font(.callout.weight(.semibold))
                        .tint(Theme.gold)

                    if let rejection {
                        Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                    }
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle(line.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
            .sheet(isPresented: $exact) {
                PriceSheet(initialProduct: line.product).environmentObject(store)
            }
        }
    }

    private func set(mode: Int, bps: Int32) async {
        guard let firm = store.selectedFirm else { return }
        let result = await store.submit(CommandJSON.command("SetPricePosture", [
            "firm": firm, "product": line.product, "mode": mode, "bps": Int(bps),
        ]))
        if let result {
            rejection = result
        } else {
            dismiss()
        }
    }

    private func option(title: String, detail: String, selected: Bool, action: @escaping () async -> Void) -> some View {
        Button {
            Task { await action() }
        } label: {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(selected ? Theme.gold : Theme.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text(detail).font(.caption).foregroundStyle(Theme.secondary).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
            .background(Theme.panelRaised, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
        .buttonStyle(.plain)
        .disabled(store.isBusy)
    }

    private func chip(_ text: String, selected: Bool, action: @escaping () async -> Void) -> some View {
        Button {
            Task { await action() }
        } label: {
            Text(text)
                .font(.callout.weight(.semibold).monospacedDigit())
                .frame(maxWidth: .infinity)
                .padding(.vertical, 9)
                .foregroundStyle(selected ? Theme.ink : Theme.primaryText)
                .background(selected ? Theme.gold : Theme.panelRaised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(Theme.stroke, lineWidth: selected ? 0 : 1))
        }
        .buttonStyle(.plain)
        .disabled(store.isBusy)
    }
}
