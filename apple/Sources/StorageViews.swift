import SwiftUI

/// The company's space in a region, one dense line per class: used of capacity, and any overflow.
struct StorageLines: View {
    let cells: [StorageCellDto]

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            ForEach(cells, id: \.classKey) { cell in
                let over = cell.outsideLots > 0
                HStack(spacing: 6) {
                    PixelIcon("storage", size: 12)
                        .foregroundStyle(over ? Theme.warning : Theme.secondary)
                    Text(line(cell))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(over ? Theme.warning : Theme.primaryText)
                }
                .accessibilityElement(children: .combine)
            }
        }
    }

    private func line(_ cell: StorageCellDto) -> String {
        var text = "\(cell.classLabel) \(cell.usedLots.formatted()) of \(cell.capacityLots.formatted()) lots"
        if cell.outsideLots > 0 {
            text += " · \(cell.outsideLots.formatted()) outside at \(Format.money(cell.outsideFeeCentsWeekly)) a week"
        }
        return text
    }
}

/// The stock target of one division: weeks of inputs to keep, one slider. Zero keeps the company
/// default. Applies to every plant of the division.
struct DivisionStockTarget: View {
    @Environment(AppStore.self) private var store
    let division: DivisionDto
    @State private var weeks: Double
    @State private var rejection: CommandRejection?

    init(division: DivisionDto) {
        self.division = division
        _weeks = State(initialValue: Double(division.stockWeeks))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: "Stock target")
            ValueSlider(
                value: $weeks,
                range: 0...26,
                step: 1,
                ticks: 26,
                label: { [defaultWeeks = division.stockWeeksDefault] in $0 < 0.5 ? "Company default: \(defaultWeeks) weeks" : "\(Int($0)) weeks of inputs" },
                detail: { _ in "Weeks of inputs this division keeps on hand. Perishable inputs keep at most 2 weeks unless you set a target. Grain is also bought ahead in the harvest quarter." },
                onCommit: { value in
                    Task {
                        for plant in division.plants {
                            let result = await store.submit(CommandJSON.command("SetStockTarget", [
                                "facility": plant, "weeks": Int(value), "seasonal": division.seasonalBuild,
                            ]))
                            if let result {
                                rejection = result
                                break
                            }
                        }
                    }
                }
            )
            if !division.storage.isEmpty {
                StorageLines(cells: division.storage)
            }
            if let rejection {
                Text(rejection.message).font(.caption).foregroundStyle(Theme.negative)
            }
        }
    }
}

/// Buy a good on the exchange into storage, and release what is held, from the good's market card.
struct StoreGoodPanel: View {
    @Environment(AppStore.self) private var store
    let line: CommodityDto
    @State private var regionCode: String?
    @State private var lots: Double = 100
    @State private var limit: Double = 100
    @State private var rejection: CommandRejection?

    private var info: ProductDto? { store.productCatalogue.first { $0.index == line.product } }

    private struct Place: Hashable {
        let code: String
        let name: String
    }

    /// The regions where the company has a division, each once.
    private var places: [Place] {
        var seen = Set<String>()
        return store.divisions.compactMap { division in
            seen.insert(division.region).inserted ? Place(code: division.region, name: division.regionName) : nil
        }
    }

    private var held: HeldStockDto? {
        store.held.first { $0.product == line.product && $0.region == (regionCode ?? places.first?.code ?? "") }
    }

    var body: some View {
        if let info, !info.storageClass.isEmpty, store.selectedFirm != nil, !places.isEmpty {
            Panel(padding: 12) {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 6) {
                        PixelIcon("storage", size: 14).foregroundStyle(Theme.secondary)
                        Eyebrow(text: "Store")
                    }
                    Text(caption(info)).font(.caption).foregroundStyle(Theme.secondary)
                    if places.count > 1 {
                        Picker("Store in", selection: Binding(get: { regionCode ?? places[0].code }, set: { regionCode = $0 })) {
                            ForEach(places, id: \.code) { Text($0.name).tag($0.code) }
                        }
                        .pickerStyle(.menu)
                    }
                    if let held, held.heldLots > 0 || held.orderLotsLeft > 0 {
                        HStack {
                            Text("Stored \(held.heldLots.formatted()) lots" + (held.orderLotsLeft > 0 ? " · \(held.orderLotsLeft.formatted()) still on order" : ""))
                                .font(.callout).foregroundStyle(Theme.gold)
                            Spacer()
                            Button("Release all") { Task { rejection = await send("SellFromStore", ["lots": 0]) } }
                                .font(.caption.weight(.semibold))
                                .buttonStyle(PixelButtonStyle())
                                .tint(Theme.gold)
                        }
                    }
                    ValueSlider(value: $lots, range: 10...2_000, step: 10, ticks: 20, label: { "\(Int($0).formatted()) lots" })
                    ValueSlider(
                        value: $limit, range: 50...200, step: 5, ticks: 15,
                        label: { "Pay up to \(Int($0))% of reference" },
                        detail: { _ in "Fills over the following weeks from the cheapest sellers, never above this price a lot." }
                    )
                    Button {
                        Task { rejection = await send("BuyToStore", ["lots": Int(lots), "limit_bps": Int(limit * 100)]) }
                    } label: {
                        PixelLabel("Buy to store", icon: "buy").font(.callout.weight(.semibold)).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PixelButtonStyle(prominent: true))
                    .tint(Theme.gold)
                    .foregroundStyle(Theme.ink)
                    if let rejection {
                        Text(rejection.message).font(.caption).foregroundStyle(Theme.negative)
                    }
                }
            }
        }
    }

    private func caption(_ info: ProductDto) -> String {
        let space: String
        switch info.storageClass {
        case "tank": space = "tank farm"
        case "silo": space = "silo"
        case "cold": space = "cold store"
        default: space = "warehouse"
        }
        var text = "Needs \(space) space. Holding costs \(String(format: "%.2f", Double(info.holdingBpsWeekly) / 100))% a week beyond a plant's yard"
        if info.spoilageBpsWeekly > 0 {
            text += "; \(String(format: "%.1f", Double(info.spoilageBpsWeekly) / 100))% spoils each week."
        } else {
            text += "."
        }
        return text
    }

    private func send(_ variant: String, _ extra: [String: Any]) async -> CommandRejection? {
        guard let firm = store.selectedFirm,
              let code = regionCode ?? places.first?.code,
              let board = store.boards.first(where: { $0.code == code }) else { return nil }
        var fields: [String: Any] = ["firm": firm, "product": line.product, "region": board.index]
        fields.merge(extra) { _, new in new }
        return await store.submit(CommandJSON.command(variant, fields))
    }
}
