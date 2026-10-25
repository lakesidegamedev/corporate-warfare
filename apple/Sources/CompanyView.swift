import SwiftUI

enum CompanyTab: String, CaseIterable, Identifiable {
    case finance = "Finance"
    case ownership = "Ownership"
    case facilities = "Facilities"
    case supply = "Supply"
    case debt = "Debt"
    var id: String { rawValue }
}

struct CompanyView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @State private var tab: CompanyTab = .finance
    @State private var showingPrice = false
    @State private var showingWage = false
    @State private var showingBuild = false
    @State private var showingDividend = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    ContextChip()
                    Picker("Section", selection: $tab) {
                        ForEach(CompanyTab.allCases) { tab in
                            Text(tab.rawValue).tag(tab)
                        }
                    }
                    .pickerStyle(.segmented)

                    switch tab {
                    case .finance: financeSection
                    case .ownership: ownershipSection
                    case .facilities: facilitiesSection
                    case .supply: supplySection
                    case .debt: debtSection
                    }

                    if let error = store.errorMessage {
                        Text(error)
                            .font(.footnote)
                            .foregroundStyle(Theme.negative)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .padding(16)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Company")
            .sheet(isPresented: $showingPrice) { PriceSheet().environmentObject(store) }
            .sheet(isPresented: $showingWage) { WageSheet().environmentObject(store) }
            .sheet(isPresented: $showingBuild) { BuildSheet().environmentObject(store) }
            .sheet(isPresented: $showingDividend) { DividendSheet().environmentObject(store) }
        }
    }

    private var financeSection: some View {
        Group {
            if let finance = store.finance {
                HStack(spacing: 12) {
                    MetricTile(title: "Cash", value: Format.moneyCompact(finance.cashCents))
                    MetricTile(title: "Equity", value: Format.moneyCompact(finance.equityCents))
                }
                HStack(spacing: 12) {
                    MetricTile(
                        title: "Trailing EBITDA",
                        value: Format.moneyCompact(finance.ebitdaTrailingCents),
                        tone: finance.ebitdaTrailingCents >= 0 ? 1 : -1
                    )
                    MetricTile(
                        title: "Trailing net income",
                        value: Format.moneyCompact(finance.netIncomeTrailingCents),
                        tone: finance.netIncomeTrailingCents >= 0 ? 1 : -1
                    )
                }
                SectionCard(title: "Balance sheet") {
                    line("Assets", finance.assetsCents)
                    line("Liabilities", finance.liabilitiesCents)
                    line("Inventory", finance.inventoryCents)
                    line("Restricted cash", finance.restrictedCents)
                }
                SectionCard(title: "Recent weeks") {
                    if finance.recentWeeks.isEmpty {
                        EmptyNote(text: "No closed week yet. Step one week.")
                    } else {
                        ForEach(finance.recentWeeks, id: \.week) { week in
                            HStack {
                                Text("W\(week.week)").font(.caption.monospacedDigit())
                                Spacer()
                                Text(Format.moneyCompact(week.revenueCents)).font(.caption.monospacedDigit())
                                Text(Format.signedMoney(week.ebitCents))
                                    .font(.caption.monospacedDigit())
                                    .foregroundStyle(week.ebitCents >= 0 ? Theme.positive : Theme.negative)
                            }
                        }
                    }
                }
                SectionCard(title: "Actions") {
                    Button("Pay dividend") { showingDividend = true }
                    Button("Set a price") { showingPrice = true }
                }
            } else {
                EmptyNote(text: "Select a company on the Portfolio tab.")
            }
        }
    }

    private var ownershipSection: some View {
        Group {
            if let ownership = store.ownership, let firm = store.selectedFirmDto {
                SectionCard(title: "Votes and control") {
                    line("Issued shares", Int64(ownership.issued))
                    line("Treasury", Int64(ownership.treasury))
                    line("Eligible votes", Int64(ownership.eligibleVotes))
                    Text("Controller: \(ownership.controller ?? "none")")
                        .font(.callout)
                    Text("Your votes: \(Format.percentBps(firm.investorVotesBps))")
                        .font(.callout)
                    Text("CEO: \(ownership.ceo)")
                        .font(.callout)
                }
                SectionCard(title: "Holders") {
                    ForEach(ownership.holders, id: \.holder) { holder in
                        HStack {
                            Text(holder.holder).font(.callout)
                            Spacer()
                            Text("\(holder.quantity.formatted())")
                                .font(.caption.monospacedDigit())
                            Text(Format.percentBps(holder.votesBps))
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                SectionCard(title: "Board") {
                    Text("Next meeting: week \(ownership.nextMeeting)")
                        .font(.callout)
                    ForEach(Array(ownership.boardSeats.enumerated()), id: \.offset) { index, seat in
                        Text("Seat \(index + 1): \(seat)").font(.caption)
                    }
                }
            } else {
                EmptyNote(text: "No company selected.")
            }
        }
    }

    private var facilitiesSection: some View {
        Group {
            if store.facilities.isEmpty {
                SectionCard(title: "Facilities") {
                    EmptyNote(text: "No facilities yet. Build one from the Regions tab or below.")
                    Button("Build a facility") { showingBuild = true }
                }
            } else {
                ForEach(store.facilities, id: \.handle) { facility in
                    SectionCard(title: facility.recipe.capitalized) {
                        HStack {
                            Text(facility.region).font(.callout)
                            Spacer()
                            Text(facility.status).font(.caption).foregroundStyle(.secondary)
                        }
                        HStack(spacing: 12) {
                            MetricTile(title: "Output", value: Format.lots(facility.lastOutputLots))
                            MetricTile(title: "Capacity", value: Format.lots(facility.capacityLots))
                        }
                        Text("Workers \(facility.workersLow) / \(facility.workersMid) / \(facility.workersHigh)")
                            .font(.caption.monospacedDigit())
                        Text("Bottleneck: \(facility.bottleneck)")
                            .font(.caption)
                            .foregroundStyle(Theme.warning)
                        HStack {
                            Button("Price") { showingPrice = true }
                            Button("Wages") { showingWage = true }
                        }
                    }
                }
                SectionCard(title: "Production target") {
                    let current = store.facilities.first?.targetBps ?? 8_500
                    Text("Target: \(Format.percentBps(current)) of full capacity")
                        .font(.callout)
                    Button("Set target to 85%") { Task { await store.setTarget(8_500) } }
                    Button("Set target to 100%") { Task { await store.setTarget(10_000) } }
                    Button("Build another facility") { showingBuild = true }
                }
            }
        }
    }

    private var supplySection: some View {
        Group {
            SectionCard(title: "Contracts") {
                if store.contracts.isEmpty {
                    EmptyNote(text: "No supply contracts. Contracts fix price and volume before spot capacity.")
                } else {
                    ForEach(store.contracts, id: \.handle) { contract in
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(contract.product): \(contract.quantityWeekly) lots/week")
                                .font(.callout)
                            Text("\(contract.seller) → \(contract.buyer) · \(contract.status)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text("\(contract.fromRegion) to \(contract.toRegion) · \(contract.remainingWeeks) weeks left")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            SectionCard(title: "Spot purchases") {
                EmptyNote(text: "Spot orders reserve cash, pay freight and arrive after the lane lead time. Purchases are never usable for production in the same week.")
            }
        }
    }

    private var debtSection: some View {
        Group {
            SectionCard(title: "Loans") {
                if store.loans.isEmpty {
                    EmptyNote(text: "No loans.")
                } else {
                    ForEach(store.loans, id: \.handle) { loan in
                        VStack(alignment: .leading) {
                            Text(loan.lender).font(.callout)
                            Text("\(Format.money(loan.principalCents)) at \(Format.percentBps(loan.rateBps)) · \(loan.remainingWeeks) weeks")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            SectionCard(title: "Insurance") {
                if store.insurance.isEmpty {
                    EmptyNote(text: "No policies. Property cover pays a claim net of the deductible, up to the limit.")
                } else {
                    ForEach(store.insurance, id: \.handle) { policy in
                        VStack(alignment: .leading) {
                            Text(policy.insurer).font(.callout)
                            Text("Limit \(Format.money(policy.limitCents)) · premium \(Format.money(policy.weeklyPremiumCents))/week")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }

    private func line(_ title: String, _ cents: Int64) -> some View {
        HStack {
            Text(title).font(.callout)
            Spacer()
            Text(Format.money(cents)).font(.callout.monospacedDigit())
        }
    }
}

struct PriceSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var priceText = ""
    @State private var product: UInt32 = 0

    var body: some View {
        NavigationStack {
            Form {
                Picker("Product", selection: $product) {
                    ForEach(products(), id: \.index) { item in
                        Text("\(item.name) · ref \(Format.money(item.referencePriceCents))")
                            .tag(item.index)
                    }
                }
                TextField("Price in dollars", text: $priceText)
                    .keyboardType(.decimalPad)
                if let firm = store.selectedFirm {
                    Text("Actor: \(firm) · legal entity cash only")
                        .font(.caption)
                }
            }
            .navigationTitle("Set price")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply") {
                        let dollars = Double(priceText) ?? 0
                        Task {
                            await store.setPrice(product: product, price: Int64(dollars * 100))
                            dismiss()
                        }
                    }
                }
            }
        }
    }
}

struct WageSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var tier: UInt32 = 0
    @State private var wageText = ""

    var body: some View {
        NavigationStack {
            Form {
                Picker("Skill tier", selection: $tier) {
                    Text("Low").tag(UInt32(0))
                    Text("Mid").tag(UInt32(1))
                    Text("High").tag(UInt32(2))
                }
                TextField("Weekly wage in dollars", text: $wageText)
                    .keyboardType(.decimalPad)
            }
            .navigationTitle("Wage offer")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply") {
                        let dollars = Double(wageText) ?? 0
                        Task {
                            await store.setWage(tier: tier, wage: Int64(dollars * 100))
                            dismiss()
                        }
                    }
                }
            }
        }
    }
}

struct BuildSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var region: UInt32 = 0
    @State private var recipe: UInt32 = 0

    var body: some View {
        NavigationStack {
            Form {
                Picker("Region", selection: $region) {
                    ForEach(store.regions, id: \.index) { region in
                        Text(region.name).tag(region.index)
                    }
                }
                TextField("Recipe index", value: $recipe, format: .number)
            }
            .navigationTitle("Build facility")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Build") {
                        Task {
                            await store.build(region: region, recipe: recipe)
                            dismiss()
                        }
                    }
                }
            }
        }
    }
}

struct DividendSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State private var amountText = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Dividend in dollars", text: $amountText)
                    .keyboardType(.decimalPad)
                Text("Paid only from distributable retained earnings and after the four-week cash buffer. Treasury shares receive nothing.")
                    .font(.caption)
            }
            .navigationTitle("Pay dividend")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Pay") {
                        let dollars = Double(amountText) ?? 0
                        Task {
                            await store.payDividend(amount: Int64(dollars * 100))
                            dismiss()
                        }
                    }
                }
            }
        }
    }
}
