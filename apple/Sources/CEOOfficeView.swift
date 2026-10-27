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
        TabScreen(title: "CEO Office", subtitle: store.overview?.weekLabel) {
            if let firm {
                masthead(firm)
                figures(firm)
                RivalsCard()
                NavigationLink {
                    StatsView().environmentObject(store)
                } label: {
                    HStack {
                        PixelLabel("Stats: results, balance sheet, shares, owners", icon: "stats")
                            .font(.callout.weight(.semibold))
                        Spacer()
                        PixelIcon("chevron_right", size: 14)
                    }
                    .padding(12)
                    .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                .tint(Theme.gold)
                NavigationLink {
                    ResearchView().environmentObject(store)
                } label: {
                    HStack {
                        PixelLabel("Research: technology and budgets", icon: "research")
                            .font(.callout.weight(.semibold))
                        Spacer()
                        PixelIcon("chevron_right", size: 12)
                    }
                    .padding(12)
                    .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                .tint(Theme.gold)
                NavigationLink {
                    CommandCenterView().environmentObject(store)
                } label: {
                    HStack {
                        PixelLabel("Actions: search every action in the game", icon: "command")
                            .font(.callout.weight(.semibold))
                        Spacer()
                        PixelIcon("chevron_right", size: 12)
                    }
                    .padding(12)
                    .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                .tint(Theme.gold)
                if !store.cfoAdvice.isEmpty {
                    CfoPanel(currency: firm.currency, rejection: $rejection)
                }
                if let weeks = store.finance?.recentWeeks, !weeks.isEmpty, store.finance?.firm == firm.handle {
                    ProfitDrivers(weeks: weeks)
                }
                PricingPanel()
                boardPanel
                plantsPanel
                focusPanel
                if let rejection {
                    Text(rejection.message).font(.callout).foregroundStyle(Theme.negative)
                }
                NavigationLink {
                    CompanyView().environmentObject(store)
                } label: {
                    HStack {
                        PixelLabel("Finance, shares, board and deals", icon: "bank")
                            .font(.callout.weight(.semibold))
                        Spacer()
                        PixelIcon("chevron_right", size: 14)
                    }
                    .padding(12)
                    .background(Theme.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                .tint(Theme.gold)
            } else if store.controlledFirms.isEmpty {
                FoundCompanyPanel()
            } else {
                CalloutRow(icon: "office", text: "Choose a company from the portfolio, or found one from the map.")
            }
        }
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

    /// Identity on two lines: the name, then sector, home region and credit grade on one line.
    /// The share price sits with the name it belongs to.
    private func masthead(_ firm: FirmDto) -> some View {
        HStack(spacing: 12) {
            SectorBadge(sector: firm.industry, size: 40)
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(firm.name)
                        .font(.headline)
                        .foregroundStyle(Theme.primaryText)
                        .lineLimit(1)
                        .minimumScaleFactor(0.8)
                    Spacer(minLength: 4)
                    Text("\(Format.price(firm.markCents, currency: Format.numeraire))/share")
                        .font(.caption.weight(.semibold).monospacedDigit())
                        .foregroundStyle(Theme.secondary)
                        .lineLimit(1)
                        .fixedSize()
                }
                Text("\(Sector.name(firm.industry)) · HQ \(firm.region) · Credit \(firm.grade)")
                    .font(.caption)
                    .foregroundStyle(Theme.secondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.85)
            }
        }
        .accessibilityElement(children: .combine)
    }

    /// Cash leads; the quarter's profit says whether it is growing; the rest is compact.
    private func figures(_ firm: FirmDto) -> some View {
        let isPrimary = store.overview?.primaryFirm == firm.handle
        let quarter = isPrimary ? store.overview?.quarterNetIncomeCents : nil
        let weeks = Int(store.overview?.quarterWeeks ?? 0)
        var stats: [Stat] = []
        if let quarter {
            stats.append(Stat(label: weeks >= 13 ? "Profit, quarter" : "Profit, \(weeks) wk",
                              value: Format.moneyCompact(quarter, currency: firm.currency),
                              tone: quarter >= 0 ? Theme.positive : Theme.negative))
        }
        stats.append(Stat(label: "Equity", value: Format.moneyCompact(firm.equityCents, currency: firm.currency),
                          tone: firm.equityCents >= 0 ? Theme.primaryText : Theme.negative))
        stats.append(Stat(label: "Fair value", value: Format.moneyCompact(firm.fairValueCents, currency: Format.numeraire)))
        stats.append(Stat(label: "Plants", value: "\(firm.facilities)"))
        stats.append(Stat(label: "Your votes", value: Format.percentBps(firm.investorVotesBps)))
        if let finance = store.finance, finance.firm == firm.handle, finance.restrictedCents > 0 {
            stats.append(Stat(label: "Restricted", value: Format.moneyCompact(finance.restrictedCents)))
        }
        return StatGroup(
            headline: Stat(label: "Company cash", value: Format.moneyCompact(firm.cashCents, currency: firm.currency)),
            stats: stats
        )
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
                        PixelLabel("Refocusing: \(ops.focusTransitionWeeks) more week(s) at 5% payroll overhead.", icon: "refresh")
                            .font(.caption).foregroundStyle(Theme.warning)
                    } else if ops.focusCooldownWeeks > 0 {
                        Text("Focus locked for \(ops.focusCooldownWeeks) more week(s).").font(.caption).foregroundStyle(Theme.secondary)
                    }
                    Divider().overlay(Theme.stroke)
                    HStack(alignment: .center, spacing: 14) {
                        VStack(alignment: .leading, spacing: 6) {
                            Eyebrow(text: "Logistics strength")
                            ProgressView(value: Double(ops.logisticsStrength), total: 200).tint(Theme.gold)
                            Text(ops.divisions < ops.sprawlThreshold
                                 ? "Overhead grows gradually with divisions and reaches 1% of revenue at \(ops.sprawlThreshold); you run \(ops.divisions). Strength \(ops.logisticsStrength) of 200 and owning logistics divisions both cut it."
                                 : "Sprawl is costing you over 1% of revenue: \(ops.divisions) divisions. Strength \(ops.logisticsStrength) of 200 and owning logistics divisions cut it.")
                                .font(.caption)
                                .foregroundStyle(ops.divisions < ops.sprawlThreshold ? Theme.secondary : Theme.warning)
                                .fixedSize(horizontal: false, vertical: true)
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
                        HStack(spacing: 10) {
                            ProgressView(value: Double(objective.progressBps) / 10_000)
                                .tint(objective.progressBps >= 10_000 ? Theme.positive : Theme.gold)
                            Text(objective.progressBps >= 10_000 ? "Met" : objective.progressBps == 0 ? "Not started" : "\(objective.progressBps / 100)%")
                                .font(.caption.weight(.semibold).monospacedDigit())
                                .foregroundStyle(objective.progressBps >= 10_000 ? Theme.positive : Theme.primaryText)
                        }
                        Text("\(objective.weeksLeft) weeks left (week \(objective.dueWeek)) · pays you a management bonus")
                            .font(.caption2).foregroundStyle(Theme.secondary)
                    }
                    ForEach(Array(store.objectiveHistory.prefix(3).enumerated()), id: \.offset) { _, result in
                        HStack {
                            PixelIcon(result.met ? "check_circle" : "close")
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
                        PixelLabel("Retooling", icon: "settings").font(.caption).foregroundStyle(Theme.warning)
                    }
                    Button("Details") { store.openDivision = division }
                        .font(.caption.weight(.semibold))
                        .buttonStyle(.bordered)
                        .tint(Theme.gold)
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
                DivisionStockTarget(division: division)
                DivisionShipping(division: division)
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
