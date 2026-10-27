import Foundation
import SwiftUI

struct CompanyView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 16) {
                    ContextChip()
                    if let firm = store.selectedFirmDto {
                        SectionCard(title: firm.name) {
                            DetailRow(title: "Industry", value: firm.industry)
                            DetailRow(title: "Region", value: firm.region)
                            DetailRow(title: "Legal cash", value: Format.money(firm.cashCents))
                            DetailRow(title: "Your votes", value: Format.percentBps(firm.investorVotesBps))
                            if let parent = store.actingThroughParent {
                                DetailRow(title: "Operating authority", value: parent.name)
                            }
                            DetailRow(title: "Status", value: firm.status.replacingOccurrences(of: "_", with: " "))
                        }
                    } else {
                        SectionCard(title: "No company selected") {
                            EmptyNote(text: "Open the Portfolio tab and choose a company from your group tree.")
                        }
                    }

                    CompanyLink(
                        title: "Finance",
                        detail: "Statements, dividend, issuance, buyback, IPO, loans and bonds",
                        icon: "cash"
                    ) { FinanceView() }

                    CompanyLink(
                        title: "Ownership",
                        detail: "Cap table, board seats, meetings, votes, share orders and tenders",
                        icon: "people"
                    ) { OwnershipView() }

                    CompanyLink(
                        title: "Facilities",
                        detail: "Capacity, output, staffing and the production target",
                        icon: "hammer"
                    ) { FacilitiesView() }

                    CompanyLink(
                        title: "Supply",
                        detail: "Contracts, deliveries and spot purchases",
                        icon: "box"
                    ) { SupplyView() }

                    CompanyLink(
                        title: "Deals and rivals",
                        detail: "Diligence, tender funding, remedies, merger and withdrawal",
                        icon: "branch"
                    ) { DealsView() }

                    CompanyLink(
                        title: "Banking",
                        detail: "Deposits, withdrawals, loan approvals, liquidity and capital",
                        icon: "bank"
                    ) { BankingView() }

                    CompanyLink(
                        title: "Insurance",
                        detail: "Quote, bind, cancel and specialist underwriting",
                        icon: "umbrella"
                    ) { InsuranceView() }

                    CompanyLink(
                        title: "Labour",
                        detail: "Bargaining, counter-offers, settlement and strike state",
                        icon: "people"
                    ) { LabourView() }

                    CompanyLink(
                        title: "Policy",
                        detail: "Permit, training and tax requests with fee and cooldown",
                        icon: "check"
                    ) { PolicyView() }

                    CompanyLink(
                        title: "Group",
                        detail: "Subsidiaries, control percentages and the primary group",
                        icon: "stack"
                    ) { GroupView() }

                    CompanyLink(
                        title: "Recovery",
                        detail: "Management contract and founding a new firm",
                        icon: "refresh"
                    ) { RecoveryView() }

                    if let error = store.errorMessage {
                        Text(error)
                            .font(.footnote)
                            .foregroundStyle(Theme.negative)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(16)
            }
            .background(Theme.background(for: scheme))
            .navigationTitle("Company")
        }
    }
}

struct PriceSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var priceText = ""
    @State private var product: UInt32 = 0
    /// Opens on this product when set, as from a market row.
    let initialProduct: UInt32?

    init(initialProduct: UInt32? = nil) {
        self.initialProduct = initialProduct
    }
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var cents: Int64? { parseDollarsToCents(priceText) }

    private func json() -> String? {
        guard let firm, let cents else { return nil }
        return CommandJSON.command("SetPrice", [
            "firm": firm.handle,
            "product": product,
            "price_cents": cents
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Set price",
            actorName: firm?.name ?? "No company",
            actorDetail: "Changes this firm's asking price only. Cash moves when a sale settles.",
            submitTitle: "Set price",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let json = json() else { return false }
                if let rejection = await store.submit(json) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                Picker("Product", selection: $product) {
                    ForEach(products(), id: \.index) { item in
                        Text("\(item.name) · ref \(Format.money(item.referencePriceCents))")
                            .tag(item.index)
                    }
                }
                TextField("Price in dollars", text: $priceText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Asking price in dollars")
                    .accessibilityHint("Entered in dollars per lot. The command sends integer cents.")
            }
        }
        .task {
            if let initialProduct { product = initialProduct }
            await refresh()
        }
        .onChange(of: priceText) { _, _ in Task { await refresh() } }
        .onChange(of: product) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct WageSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var tier: UInt32 = 0
    @State private var wageText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var cents: Int64? { parseDollarsToCents(wageText) }

    private var region: RegionDto? {
        guard let name = firm?.region else { return nil }
        return store.regions.first { $0.name == name }
    }

    private func json() -> String? {
        guard let firm, let cents else { return nil }
        return CommandJSON.command("SetWageOffer", [
            "firm": firm.handle,
            "tier": tier,
            "wage_cents": cents
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Wage offer",
            actorName: firm?.name ?? "No company",
            actorDetail: "A weekly promise to every worker in the chosen tier, paid from this firm's cash.",
            submitTitle: "Set weekly offer",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let json = json() else { return false }
                if let rejection = await store.submit(json) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                Picker("Skill tier", selection: $tier) {
                    Text("Low\(wageSuffix(region?.wageLowCents))").tag(UInt32(0))
                    Text("Mid\(wageSuffix(region?.wageMidCents))").tag(UInt32(1))
                    Text("High\(wageSuffix(region?.wageHighCents))").tag(UInt32(2))
                }
                TextField("Weekly wage in dollars", text: $wageText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Weekly wage offer in dollars")
            }
        }
        .onAppear {
            if wageText.isEmpty, let wage = prevailingWage(tier) {
                wageText = String(format: "%.2f", Double(wage) / 100.0)
            }
        }
        .task { await refresh() }
        .onChange(of: wageText) { _, _ in Task { await refresh() } }
        .onChange(of: tier) { _, newValue in
            if let wage = prevailingWage(newValue) {
                wageText = String(format: "%.2f", Double(wage) / 100.0)
            }
            Task { await refresh() }
        }
    }

    private func wageSuffix(_ cents: Int64?) -> String {
        guard let cents else { return "" }
        return " · \(Format.money(cents)) a week"
    }

    private func prevailingWage(_ tier: UInt32) -> Int64? {
        guard let region else { return nil }
        switch tier {
        case 0: return region.wageLowCents
        case 1: return region.wageMidCents
        default: return region.wageHighCents
        }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct BuildSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var region: UInt32 = 0
    @State private var recipe: UInt32 = 0
    @State private var blocks: UInt32 = 1
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var template: RecipeDto? {
        store.recipeOptions.first { $0.index == recipe } ?? store.recipeOptions.first
    }

    var body: some View {
        CommandSheet(
            title: "Build facility",
            actorName: firm?.name ?? "No company",
            actorDetail: "Spends this firm's cash on construction escrow and a commissioned service.",
            submitTitle: "Build",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: firm != nil,
            onSubmit: {
                if let rejection = await store.buildFacility(region: region, recipe: template?.index ?? recipe, blocks: blocks) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Template") {
                Picker("Region", selection: $region) {
                    ForEach(store.regions, id: \.index) { region in
                        Text(region.name).tag(region.index)
                    }
                }
                Picker("Template", selection: $recipe) {
                    ForEach(store.recipeOptions, id: \.index) { item in
                        Text("\(item.id) · \(Format.money(item.capexCents)) · \(item.buildWeeks)w")
                            .tag(item.index)
                    }
                }
                Stepper(value: $blocks, in: 1...20) {
                    Text("Blocks: \(blocks)")
                }
                if let template {
                    DetailRow(title: "Capex per plant", value: Format.money(template.capexCents))
                    DetailRow(title: "Weekly output", value: Format.lots(template.weeklyOutputLots))
                }
            }
        }
        .onAppear {
            if recipe == 0, let first = store.recipeOptions.first {
                recipe = first.index
            }
            if let name = firm?.region, let match = store.regions.first(where: { $0.name == name }) {
                region = match.index
            }
        }
        .task { await refresh() }
        .onChange(of: recipe) { _, _ in Task { await refresh() } }
        .onChange(of: blocks) { _, _ in Task { await refresh() } }
        .onChange(of: region) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let firm else {
            preview = nil
            return
        }
        preview = await store.previewBuild(firm: firm.handle, region: region, recipe: recipe, blocks: blocks)
    }
}

struct DividendSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var amountText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var cents: Int64? { parseDollarsToCents(amountText) }

    private func json() -> String? {
        guard let firm, let cents else { return nil }
        return CommandJSON.command("PayDividend", ["firm": firm.handle, "amount_cents": cents])
    }

    var body: some View {
        CommandSheet(
            title: "Pay dividend",
            actorName: firm?.name ?? "No company",
            actorDetail: "Paid from distributable retained earnings after the four-week cash buffer. Treasury shares receive nothing.",
            submitTitle: "Pay dividend",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let json = json() else { return false }
                if let rejection = await store.submit(json) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                TextField("Dividend in dollars", text: $amountText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Dividend total in dollars")
                Text("Allocated across eligible shares by largest remainder after the payout rules pass.")
                    .font(.caption)
            }
        }
        .task { await refresh() }
        .onChange(of: amountText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}
