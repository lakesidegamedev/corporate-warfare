import SwiftUI

/// The CEO office of the selected company, after A House Divided: masthead, the figures that
/// matter this week, sector focus and logistics, and every division (a sector type in a region) with
/// its strategy, size and utilisation. Deeper corporate controls stay one tap away.
struct CEOOfficeView: View {
    @EnvironmentObject private var store: AppStore
    @State private var retooling: FacilityDto?
    @State private var editingFocus = false
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if let firm {
                        masthead(firm)
                        figures(firm)
                        boardPanel
                        focusPanel
                        plantsPanel
                        if let rejection {
                            Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                        }
                        NavigationLink {
                            CompanyView().environmentObject(store)
                        } label: {
                            Label("Finance, shares, board and deals", systemImage: "chevron.right.circle")
                                .font(.callout.weight(.semibold))
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .tint(Theme.gold)
                    } else {
                        Panel {
                            Text("Choose a company from the portfolio, or found one from the map.")
                                .foregroundStyle(Theme.secondary)
                        }
                    }
                }
                .padding(16)
                .frame(maxWidth: 900)
                .frame(maxWidth: .infinity)
            }
            .background(Theme.ink)
            .navigationTitle("CEO Office")
            .task {
                if store.selectedFirm == nil, let primary = store.overview?.primaryFirm {
                    await store.selectFirm(primary)
                }
            }
            .sheet(item: $retooling) { plant in
                RetoolSheet(plant: plant) { result in rejection = result }
                    .environmentObject(store)
                    .presentationDetents([.medium, .large])
                    .presentationBackground(Theme.panel)
            }
            .sheet(isPresented: $editingFocus) {
                FocusSheet { result in rejection = result }
                    .environmentObject(store)
                    .presentationDetents([.medium])
                    .presentationBackground(Theme.panel)
            }
        }
    }

    private func masthead(_ firm: FirmDto) -> some View {
        HStack(spacing: 14) {
            Text(monogram(firm.name))
                .font(.title2.weight(.heavy))
                .foregroundStyle(Theme.ink)
                .frame(width: 58, height: 58)
                .background(Sector.color(firm.industry), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            VStack(alignment: .leading, spacing: 4) {
                Text(firm.name).font(.title2.weight(.bold)).foregroundStyle(Theme.primaryText)
                HStack(spacing: 8) {
                    Label(Sector.name(firm.industry), systemImage: Sector.symbol(firm.industry))
                    Text("HQ \(firm.region)")
                    Text("Grade \(firm.grade)")
                }
                .font(.caption.weight(.semibold))
                .foregroundStyle(Theme.secondary)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 2) {
                Text(Format.money(firm.markCents)).font(.title3.weight(.bold).monospacedDigit()).foregroundStyle(Theme.primaryText)
                Eyebrow(text: "share price")
            }
        }
    }

    private func monogram(_ name: String) -> String {
        name.split(separator: " ").prefix(2).compactMap(\.first).map(String.init).joined().uppercased()
    }

    private func figures(_ firm: FirmDto) -> some View {
        let columns = [GridItem(.adaptive(minimum: 150), spacing: 10)]
        let ebit = store.overview?.primaryFirm == firm.handle ? store.overview?.lastWeekEbitCents : nil
        return LazyVGrid(columns: columns, spacing: 10) {
            BigFigure(label: "Cash", value: Format.moneyCompact(firm.cashCents))
            BigFigure(label: "Equity", value: Format.moneyCompact(firm.equityCents), tone: firm.equityCents >= 0 ? Theme.primaryText : Theme.negative)
            BigFigure(label: "Fair value", value: Format.moneyCompact(firm.fairValueCents))
            if let ebit {
                BigFigure(label: "EBIT last week", value: Format.moneyCompact(ebit), tone: ebit >= 0 ? Theme.positive : Theme.negative)
            }
            BigFigure(label: "Plants", value: "\(firm.facilities)")
            BigFigure(label: "Your votes", value: Format.percentBps(firm.investorVotesBps))
        }
    }

    private var focusPanel: some View {
        Panel {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Eyebrow(text: "Sector focus")
                    Spacer()
                    Button("Change") { editingFocus = true }
                        .font(.caption.weight(.semibold))
                        .tint(Theme.gold)
                        .disabled((store.firmOps?.focusCooldownWeeks ?? 0) > 0)
                }
                if let ops = store.firmOps {
                    HStack(spacing: 12) {
                        focusChip("Primary", ops.primary, "5% fewer inputs")
                        if let secondary = ops.secondary {
                            focusChip("Secondary", secondary, "2.5% fewer inputs")
                        }
                    }
                    if ops.focusTransitionWeeks > 0 {
                        Label("Refocusing: \(ops.focusTransitionWeeks) more week(s) at 5% payroll overhead.", systemImage: "arrow.triangle.2.circlepath")
                            .font(.caption).foregroundStyle(Theme.warning)
                    } else if ops.focusCooldownWeeks > 0 {
                        Text("Focus locked for \(ops.focusCooldownWeeks) more week(s).").font(.caption).foregroundStyle(Theme.secondary)
                    }
                    Divider().overlay(Theme.stroke)
                    HStack(alignment: .center, spacing: 14) {
                        VStack(alignment: .leading, spacing: 6) {
                            Eyebrow(text: "Logistics strength")
                            ProgressView(value: Double(ops.logisticsStrength), total: 200).tint(Theme.gold)
                            Text("\(ops.logisticsStrength) of 200 · sprawl starts after \(ops.sprawlThreshold) divisions (you run \(ops.divisions))")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                        Menu {
                            ForEach([0, 5_000, 25_000, 100_000], id: \.self) { dollars in
                                Button(dollars == 0 ? "No programme" : "\(Format.money(Int64(dollars) * 100)) a week") {
                                    Task { rejection = await setLogistics(Int64(dollars) * 100) }
                                }
                            }
                        } label: {
                            VStack(alignment: .trailing, spacing: 2) {
                                Text(Format.moneyCompact(ops.logisticsBudgetCents)).font(.callout.monospacedDigit().weight(.semibold))
                                Eyebrow(text: "per week")
                            }
                        }
                        .tint(Theme.gold)
                    }
                }
            }
        }
    }

    private func focusChip(_ role: String, _ sector: String, _ note: String) -> some View {
        HStack(spacing: 8) {
            SectorBadge(sector: sector, size: 28)
            VStack(alignment: .leading, spacing: 1) {
                Text("\(role): \(Sector.name(sector))").font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                Text(note).font(.caption2).foregroundStyle(Theme.positive)
            }
        }
    }

    /// The board's quarterly goal and how the last ones went.
    @ViewBuilder private var boardPanel: some View {
        if store.objective != nil || !store.objectiveHistory.isEmpty {
            Panel {
                VStack(alignment: .leading, spacing: 10) {
                    Eyebrow(text: "Board objective")
                    if let objective = store.objective {
                        Text(objective.title).font(.headline).foregroundStyle(Theme.primaryText)
                        Text(objective.detail).font(.caption).foregroundStyle(Theme.secondary)
                        ProgressView(value: Double(objective.progressBps) / 10_000).tint(Theme.gold)
                        Text("Due week \(objective.dueWeek) · \(objective.weeksLeft) weeks left · meeting it pays you a management bonus")
                            .font(.caption2).foregroundStyle(Theme.secondary)
                    }
                    ForEach(Array(store.objectiveHistory.prefix(3).enumerated()), id: \.offset) { _, result in
                        HStack {
                            Image(systemName: result.met ? "checkmark.circle.fill" : "xmark.circle")
                                .foregroundStyle(result.met ? Theme.positive : Theme.secondary)
                            Text(result.title).font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                            Spacer()
                            Text(result.met ? Format.moneyCompact(result.bonusCents) : "Missed")
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(result.met ? Theme.gold : Theme.secondary)
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
            }
        }
    }

    private var plantsPanel: some View {
        VStack(alignment: .leading, spacing: 12) {
            Eyebrow(text: "Divisions")
            if store.divisions.isEmpty {
                Panel { Text("No divisions yet. Open a region on the map to enter a sector.").foregroundStyle(Theme.secondary) }
            }
            ForEach(store.divisions) { division in
                divisionPanel(division)
            }
        }
    }

    private func divisionPanel(_ division: DivisionDto) -> some View {
        let plants = store.facilities.filter { division.plants.contains($0.handle) }
        return Panel {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    SectorBadge(sector: division.industry, size: 28)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("\(Sector.name(division.industry)) · \(division.regionName)").font(.headline).foregroundStyle(Theme.primaryText)
                        Text("\(division.strategyName) · \(division.blocks) plant\(division.blocks == 1 ? "" : "s") at \(division.plants.count) site\(division.plants.count == 1 ? "" : "s")")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 1) {
                        Text("\(division.shareBps / 100)%").font(.callout.monospacedDigit().weight(.semibold)).foregroundStyle(Theme.gold)
                        Eyebrow(text: "share")
                    }
                }
                HStack {
                    if division.retooling {
                        Label("Retooling", systemImage: "gearshape.arrow.triangle.2.circlepath").font(.caption).foregroundStyle(Theme.warning)
                    }
                    Spacer()
                    if let first = plants.first {
                        Button("Retool division") { retooling = first }
                            .font(.caption.weight(.semibold))
                            .buttonStyle(.bordered)
                            .tint(Theme.gold)
                            .disabled(division.retooling || plants.contains { $0.status != "operating" && $0.status != "mothballed" })
                    }
                }
                ForEach(plants, id: \.handle) { plant in
                    plantRow(plant)
                }
            }
        }
    }

    private func plantRow(_ plant: FacilityDto) -> some View {
        let utilisation = plant.capacityLots > 0 ? Double(plant.lastOutputLots) / Double(plant.capacityLots) : 0
        return VStack(alignment: .leading, spacing: 6) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(strategy(for: plant)?.name ?? plant.recipe).font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                    Text("\(plant.region) · \(plant.blocks) plant\(plant.blocks == 1 ? "" : "s") · \(plant.status)")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
                Spacer()
                if plant.status == "operating" {
                    Button("Mothball") { Task { rejection = await store.submit(CommandJSON.command("MothballFacility", ["facility": plant.handle])) } }
                        .font(.caption.weight(.semibold))
                        .buttonStyle(.bordered)
                        .tint(Theme.secondary)
                        .accessibilityHint("Stops the plant and releases its staff until you reopen it")
                } else if plant.status == "mothballed" {
                    Button("Reopen \(Format.moneyCompact(plant.reopenCostCents))") { Task { rejection = await store.submit(CommandJSON.command("ReopenFacility", ["facility": plant.handle])) } }
                        .font(.caption.weight(.semibold))
                        .buttonStyle(.borderedProminent)
                        .tint(Theme.gold)
                        .accessibilityHint("Restarts the plant; staff are hired again over the following weeks")
                }
            }
            ProgressView(value: min(max(utilisation, 0), 1)).tint(utilisation > 0.8 ? Theme.positive : Theme.warning)
            HStack {
                Text("\(plant.lastOutputLots.formatted()) of \(plant.capacityLots.formatted()) lots last week")
                Spacer()
                if !plant.bottleneck.isEmpty && plant.bottleneck != "none" {
                    Text("Limited by \(plant.bottleneck)").foregroundStyle(Theme.warning)
                }
            }
            .font(.caption2.monospacedDigit())
            .foregroundStyle(Theme.secondary)
        }
        .accessibilityElement(children: .combine)
    }

    private func strategy(for plant: FacilityDto) -> StrategyDto? {
        store.strategyCatalogue.first { $0.id == plant.recipe }
    }

    private func setLogistics(_ cents: Int64) async -> CommandRejection? {
        guard let firm = store.selectedFirm else { return nil }
        return await store.submit(CommandJSON.command("SetLogisticsBudget", ["firm": firm, "amount_cents": cents]))
    }
}

struct BigFigure: View {
    let label: String
    let value: String
    var tone: Color = Theme.primaryText

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Eyebrow(text: label)
            Text(value).font(.title3.weight(.bold).monospacedDigit()).foregroundStyle(tone)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Theme.stroke, lineWidth: 1))
        .accessibilityElement(children: .combine)
    }
}

/// Switch a division to another strategy of its sector: every plant moves together, for a
/// quarter of the new capex per block and a third of its build time, at least four weeks.
struct RetoolSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let plant: FacilityDto
    let done: (CommandRejection?) -> Void

    private var current: StrategyDto? { store.strategyCatalogue.first { $0.id == plant.recipe } }

    /// Blocks across the division this plant belongs to.
    private var divisionBlocks: Int64 {
        Int64(store.divisions.first { $0.plants.contains(plant.handle) }?.blocks ?? plant.blocks)
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text("A division runs one strategy, so every plant in it retools together: a quarter of the new strategy's capex per plant and a third of its build time, at least four weeks. The division produces nothing meanwhile.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                    ForEach(store.strategyCatalogue.filter { $0.industry == current?.industry && $0.id != plant.recipe }, id: \.index) { option in
                        Button {
                            Task {
                                let result = await store.submit(CommandJSON.command("RetoolFacility", ["facility": plant.handle, "recipe": option.index]))
                                done(result)
                                dismiss()
                            }
                        } label: {
                            VStack(alignment: .leading, spacing: 6) {
                                StrategyCard(strategy: option, room: nil)
                                Text("Retool cost \(Format.money(option.capexCents * divisionBlocks / 4)) · \(max(option.buildWeeks / 3, 4)) weeks")
                                    .font(.caption.monospacedDigit()).foregroundStyle(Theme.gold)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(16)
            }
            .background(Theme.panel)
            .navigationTitle("Retool \(current?.name ?? plant.recipe)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } } }
        }
    }
}

/// Choose a primary and optional secondary sector type.
struct FocusSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let done: (CommandRejection?) -> Void
    @State private var primary = "agriculture"
    @State private var secondary = "none"

    private let sectors = Sector.order.filter { $0 != "financial" }

    var body: some View {
        NavigationStack {
            Form {
                Picker("Primary", selection: $primary) {
                    ForEach(sectors, id: \.self) { Text(Sector.name($0)).tag($0) }
                }
                Picker("Secondary", selection: $secondary) {
                    Text("None").tag("none")
                    ForEach(sectors.filter { $0 != primary }, id: \.self) { Text(Sector.name($0)).tag($0) }
                }
                Section {
                    Text("A change costs 5% of every plant's payroll for four weeks and locks focus for eight. A secondary type doubles the sprawl penalty beyond the division threshold.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
            }
            .scrollContentBackground(.hidden)
            .background(Theme.panel)
            .navigationTitle("Sector focus")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply") {
                        Task {
                            guard let firm = store.selectedFirm,
                                  let p = Sector.engineIndex(primary) else { return }
                            var fields: [String: Any] = ["firm": firm, "primary": p]
                            if let s = Sector.engineIndex(secondary) { fields["secondary"] = s }
                            done(await store.submit(CommandJSON.command("SetSectorFocus", fields)))
                            dismiss()
                        }
                    }
                }
            }
            .onAppear {
                if let ops = store.firmOps {
                    primary = ops.primary
                    secondary = ops.secondary ?? "none"
                }
            }
        }
    }
}
