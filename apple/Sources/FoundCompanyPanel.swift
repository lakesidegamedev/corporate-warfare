import SwiftUI

private let suggestedNames = [
    "Lakeshore Industries", "Keystone Holdings", "Prairie Works", "Iron Valley Group", "Northgate Enterprises",
    "Riverbend Company", "Summit Manufacturing", "Harbor Point Holdings",
]

/// Found a company: name, headquarters, first strategy, capital. Shown wherever a screen needs a
/// company and the player has none yet.
struct FoundCompanyPanel: View {
    @EnvironmentObject private var store: AppStore
    @State private var name = ""
    @State private var region: UInt32?
    @State private var sector = ""
    @State private var recipe: UInt32?
    @State private var capital: Double = 1_000_000
    @State private var preview: PreviewDto?
    @State private var result: (Bool, String)?
    @State private var started = false

    private var cash: Int64 { store.overview?.investorCashCents ?? 0 }

    private var sectors: [String] {
        var seen: [String] = []
        for strategy in store.strategyCatalogue where !seen.contains(strategy.industry) { seen.append(strategy.industry) }
        return seen
    }

    private func roomValue(_ board: RegionBoardDto) -> Int64 {
        board.room.reduce(Int64(0)) { $0 + $1.importLots * $1.importPriceCents }
    }

    private var rankedBoards: [RegionBoardDto] {
        store.boards.sorted { a, b in
            let ra = roomValue(a)
            let rb = roomValue(b)
            if ra != rb { return ra > rb }
            return a.demandIndexBps > b.demandIndexBps
        }
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }

    private var json: String? {
        guard !trimmedName.isEmpty, let region, let recipe else { return nil }
        return CommandJSON.command("FoundFirm", [
            "name": trimmedName,
            "region": region,
            "capital_cents": Int64((capital * 100).rounded()),
            "recipe": recipe,
        ])
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Found your company")
                .font(.title2.weight(.bold))
                .foregroundStyle(Theme.primaryText)
                .accessibilityAddTraits(.isHeader)
            Text("You have \(Format.moneyCompact(cash)) of your own. Put some of it into a company, pick where it is based and what it makes first.")
                .font(.callout)
                .foregroundStyle(Theme.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Panel {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "Name")
                    TextField("Company name", text: $name)
                        .textFieldStyle(.roundedBorder)
                        .autocorrectionDisabled()
                }
            }
            Panel {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "Headquarters")
                    Text("Strong demand and room to build bring buyers; cheaper wages cut costs.")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 170), spacing: 8)], spacing: 8) {
                        ForEach(rankedBoards, id: \.index) { board in
                            regionCard(board)
                        }
                    }
                }
            }
            Panel {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "First strategy")
                    ChipFlow(spacing: 5) {
                        ForEach(sectors, id: \.self) { item in
                            ChoiceChip(text: Sector.name(item), on: item == sector) {
                                sector = item
                                recipe = store.strategyCatalogue.first { $0.industry == item }?.index
                            }
                        }
                    }
                    ForEach(store.strategyCatalogue.filter { $0.industry == sector }, id: \.index) { strategy in
                        strategyRow(strategy)
                    }
                }
            }
            Panel {
                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow(text: "Capital to put in")
                    ValueSlider(
                        value: $capital,
                        range: 1_000_000...max(Double(cash) / 100, 1_000_001), step: 50_000, ticks: 10,
                        label: { Format.moneyCompact(Int64($0 * 100)) },
                        detail: { "You keep \(Format.moneyCompact(cash - Int64($0 * 100))) of your own to invest elsewhere." }
                    )
                }
            }
            HStack(spacing: 12) {
                Button {
                    found()
                } label: {
                    Text("Found the company").frame(minWidth: 160)
                }
                .buttonStyle(PixelButtonStyle(prominent: true))
                .tint(Theme.gold)
                .foregroundStyle(Theme.ink)
                .controlSize(.large)
                .disabled(json == nil || preview?.blockedKey != nil || store.isBusy)
                status
            }
        }
        .onAppear(perform: setup)
        .task(id: json ?? "") {
            preview = nil
            guard let text = json else { return }
            try? await Task.sleep(nanoseconds: 300_000_000)
            if Task.isCancelled { return }
            preview = await store.preview(text)
        }
    }

    @ViewBuilder
    private var status: some View {
        if let result {
            Text(result.1)
                .font(.footnote)
                .foregroundStyle(result.0 ? Theme.positive : Theme.negative)
        } else if let preview {
            if let key = preview.blockedKey {
                Text(CommandRejection(key: key, detail: preview.blockedDetail ?? "").message)
                    .font(.footnote)
                    .foregroundStyle(Theme.negative)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text("Your cash \(Format.signedMoney(preview.cashChangeCents))")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondary)
            }
        }
    }

    private func setup() {
        guard !started else { return }
        started = true
        if name.isEmpty {
            let pick = Int(max(cash, 0) % Int64(suggestedNames.count))
            name = suggestedNames[pick]
        }
        if region == nil { region = rankedBoards.first?.index }
        if sector.isEmpty {
            sector = store.strategyCatalogue.first?.industry ?? ""
            recipe = store.strategyCatalogue.first { $0.industry == sector }?.index
        }
        capital = min(max(Double(cash) * 0.6 / 100, 1_000_000), max(Double(cash) / 100, 1_000_001))
    }

    private func found() {
        guard let region, let recipe else { return }
        let capitalCents = Int64((capital * 100).rounded())
        let before = Set(store.firms.map { $0.handle })
        result = nil
        Task {
            if let rejection = await store.foundFirm(name: trimmedName, region: region, capitalCents: capitalCents, recipe: recipe) {
                result = (false, rejection.message)
            } else {
                result = (true, "Done.")
                if let created = store.firms.last(where: { $0.controlled && !before.contains($0.handle) }) {
                    await store.selectFirm(created.handle)
                }
            }
        }
    }

    private func regionCard(_ board: RegionBoardDto) -> some View {
        let on = board.index == region
        let room = roomValue(board)
        return Button {
            region = board.index
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text("\(board.name), \(board.stateCode)")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Theme.primaryText)
                    .lineLimit(1)
                Text("Wages \(board.wageIndexBps / 100)% · " + (room > 0 ? "room \(Format.moneyCompact(room)) a week" : "demand \(board.demandIndexBps / 100)%"))
                    .font(.caption2)
                    .foregroundStyle(Theme.secondary)
                    .lineLimit(1)
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(on ? Theme.gold.opacity(0.14) : Theme.ink, in: Pixel.shape)
            .overlay(Pixel.shape.stroke(on ? Theme.gold : Theme.stroke, lineWidth: on ? 2 : 1))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func strategyRow(_ strategy: StrategyDto) -> some View {
        let on = strategy.index == recipe
        return Button {
            recipe = strategy.index
        } label: {
            HStack(spacing: 10) {
                SectorBadge(sector: strategy.industry, size: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text(strategy.name)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(on ? Theme.gold : Theme.primaryText)
                    Text("Makes \(strategy.outputName) · \(strategy.weeklyOutputLots) lots a week per plant\(strategy.byProductsText.isEmpty ? "" : " " + strategy.byProductsText)")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                }
                Spacer(minLength: 4)
                VStack(alignment: .trailing, spacing: 2) {
                    Text(Format.moneyCompact(strategy.capexCents) + " a plant")
                        .font(.caption)
                        .foregroundStyle(Theme.primaryText)
                    Text("\(strategy.buildWeeks) weeks · margin \(strategy.referenceMarginBps / 100)%")
                        .font(.caption2)
                        .foregroundStyle(Theme.secondary)
                }
            }
            .padding(10)
            .background(on ? Theme.gold.opacity(0.14) : Color.clear, in: Pixel.shape)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
