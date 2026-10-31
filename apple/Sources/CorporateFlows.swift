import SwiftUI

// MARK: - Deals

struct DealsView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingStart = false
    @State private var remedyDeal: DealDto?
    @State private var confirm: ConfirmRequest?

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                SectionCard(title: "Open bids") {
                    if store.deals.isEmpty {
                        EmptyNote(text: "No bids. A tender escrows the full maximum price plus two percent, then diligence runs two weeks before the four-week offer.")
                    } else {
                        ForEach(store.deals, id: \.handle) { deal in
                            VStack(alignment: .leading, spacing: 4) {
                                Text("\(deal.buyer) → \(deal.target)")
                                    .font(.callout.weight(.medium))
                                    .fixedSize(horizontal: false, vertical: true)
                                Text("\(deal.kind.replacingOccurrences(of: "_", with: " ")) · \(deal.state.replacingOccurrences(of: "_", with: " ")) · \(Format.money(deal.offerPriceCents)) per share")
                                    .font(.caption)
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                                    .fixedSize(horizontal: false, vertical: true)
                                Text("Escrow \(Format.money(deal.escrowCents)) · committed \(deal.committedShares.formatted()) shares")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                                if let deadline = deal.deadline {
                                    Text("Next boundary: week \(deadline)")
                                        .font(.caption2)
                                        .foregroundStyle(Theme.warning)
                                }
                                actionRow(for: deal)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 4)
                        }
                    }
                }
                SectionCard(title: "New bid") {
                    Text("Diligence costs 0.5 percent of the bid value and reveals the target's statements and contracts. The offer then stays open four weeks and needs legal review before settlement.")
                        .font(.caption)
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                        .fixedSize(horizontal: false, vertical: true)
                    ActionButton(title: "Start diligence", icon: "search") {
                        showingStart = true
                    }
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Deals")
        .sheet(isPresented: $showingStart) { StartDiligenceSheet().environment(store) }
        .sheet(item: $remedyDeal) { deal in
            RemedySheet(deal: deal).environment(store)
        }
        .sheet(item: $confirm) { request in
            ConfirmCommandSheet(request: request).environment(store)
        }
    }

    @ViewBuilder
    private func actionRow(for deal: DealDto) -> some View {
        if deal.buyerHandle == store.selectedFirm {
            HStack(spacing: 8) {
                if deal.state == "draft" {
                    Button("Fund tender") {
                        confirm = ConfirmRequest(
                            title: "Fund tender",
                            actorName: store.selectedFirmDto?.name ?? "No company",
                            detail: "Moves the draft into a funded tender. The escrow was reserved when diligence started.",
                            submitTitle: "Fund tender",
                            json: CommandJSON.command("FundTender", ["deal": deal.handle])
                        )
                    }
                    .buttonStyle(PixelButtonStyle())
                    .frame(minHeight: 44)
                }
                if deal.state == "review" {
                    Button("Submit remedy") { remedyDeal = deal }
                        .buttonStyle(PixelButtonStyle())
                        .frame(minHeight: 44)
                }
                if deal.state == "settled" {
                    Button("Propose merger") {
                        confirm = ConfirmRequest(
                            title: "Propose merger",
                            actorName: store.selectedFirmDto?.name ?? "No company",
                            detail: "Requires at least ninety percent of the target plus a completed review. Minority holders are paid at the higher of the offer and fair value.",
                            submitTitle: "Propose merger",
                            json: CommandJSON.command("ProposeMerger", ["deal": deal.handle])
                        )
                    }
                    .buttonStyle(PixelButtonStyle())
                    .frame(minHeight: 44)
                }
                if !dealIsTerminal(deal.state) {
                    Button("Withdraw", role: .destructive) {
                        confirm = ConfirmRequest(
                            title: "Withdraw offer",
                            actorName: store.selectedFirmDto?.name ?? "No company",
                            detail: "Returns unused escrow minus costs already incurred. The bid cannot be reopened.",
                            submitTitle: "Withdraw",
                            json: CommandJSON.command("WithdrawOffer", ["deal": deal.handle])
                        )
                    }
                    .buttonStyle(PixelButtonStyle())
                    .frame(minHeight: 44)
                }
            }
            .font(.caption)
            .frame(minHeight: 44)
        } else {
            Text("Only the bidder, \(deal.buyer), can act on this bid. You can still tender your own shares from Ownership.")
                .font(.caption2)
                .foregroundStyle(Theme.secondaryText(for: scheme))
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: - Banking

struct BankingView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingDeposit = false
    @State private var showingWithdraw = false
    @State private var showingApprove = false
    @State private var showingRate = false

    private var banks: [InstitutionDto] {
        store.institutions.filter { $0.kind.contains("bank") }
    }

    private var controlledBank: InstitutionDto? {
        banks.first { $0.controlled && $0.handle == store.selectedFirm }
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                SectionCard(title: "Acting firm") {
                    Text("Deposits and withdrawals move \(store.selectedFirmDto?.name ?? "the firm")'s own cash into or out of a bank. A deposit is a claim on the bank, not company equity.")
                        .font(.caption)
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                        .fixedSize(horizontal: false, vertical: true)
                    ActionButton(title: "Deposit funds", icon: "arrow_down") {
                        showingDeposit = true
                    }
                    ActionButton(title: "Withdraw funds", icon: "arrow_up") {
                        showingWithdraw = true
                    }
                    ActionButton(title: "Approve a loan from the bank", icon: "check_circle") {
                        showingApprove = true
                    }
                }
                ForEach(banks, id: \.handle) { bank in
                    SectionCard(title: bank.name) {
                        DetailRow(title: "Region", value: bank.region)
                        DetailRow(title: "Cash", value: Format.money(bank.cashCents))
                        DetailRow(title: "Deposits held", value: Format.money(bank.depositsCents))
                        DetailRow(title: "Loans outstanding", value: Format.money(bank.loansCents))
                        DetailRow(title: "Equity", value: Format.money(bank.equityCents))
                        DetailRow(title: "Deposit rate", value: Format.percentBps(bank.depositRateBps))
                        PixelLabel(
                            bank.liquidityOk ? "Liquidity meets 20% of deposits" : "Liquidity below 20% of deposits",
                            icon: bank.liquidityOk ? "check" : "warning"
                        )
                        .font(.caption)
                        .foregroundStyle(bank.liquidityOk ? Theme.positive : Theme.warning)
                        PixelLabel(
                            bank.capitalOk ? "Capital meets 12% of risk-weighted loans" : "Capital below 12% of risk-weighted loans",
                            icon: bank.capitalOk ? "check" : "warning"
                        )
                        .font(.caption)
                        .foregroundStyle(bank.capitalOk ? Theme.positive : Theme.warning)
                        if bank.handle == store.selectedFirm {
                            ActionButton(title: "Set this bank's deposit rate", icon: "percent") {
                                showingRate = true
                            }
                        }
                    }
                }
                if banks.isEmpty {
                    EmptyNote(text: "No funded bank in this world.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Banking")
        .sheet(isPresented: $showingDeposit) {
            DepositSheet(mode: .deposit, bank: controlledBank ?? banks.first).environment(store)
        }
        .sheet(isPresented: $showingWithdraw) {
            DepositSheet(mode: .withdraw, bank: controlledBank ?? banks.first).environment(store)
        }
        .sheet(isPresented: $showingApprove) { ApproveLoanSheet().environment(store) }
        .sheet(isPresented: $showingRate) { DepositRateSheet().environment(store) }
    }
}

// MARK: - Insurance

struct InsuranceView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingQuote = false
    @State private var confirm: ConfirmRequest?

    private var insurers: [InstitutionDto] {
        store.institutions.filter { $0.kind.contains("insurer") }
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                SectionCard(title: "Policies") {
                    if store.insurance.isEmpty {
                        EmptyNote(text: "No policies. Property cover pays a claim net of the deductible, up to the limit.")
                    } else {
                        ForEach(store.insurance, id: \.handle) { policy in
                            VStack(alignment: .leading, spacing: 2) {
                                Text(policy.insurer).font(.callout)
                                Text("Insured \(policy.insured) · limit \(Format.money(policy.limitCents)) · deductible \(Format.money(policy.deductibleCents))")
                                    .font(.caption)
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                                    .fixedSize(horizontal: false, vertical: true)
                                Text("Premium \(Format.money(policy.weeklyPremiumCents)) a week · \(policy.status)")
                                    .font(.caption2.monospacedDigit())
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                                if policy.status == "active" {
                                    Button("Cancel policy", role: .destructive) {
                                        confirm = ConfirmRequest(
                                            title: "Cancel policy",
                                            actorName: policy.insured,
                                            detail: "Ends cover for this facility. Premiums already paid are not refunded.",
                                            submitTitle: "Cancel policy",
                                            json: CommandJSON.command("CancelPolicy", ["policy": policy.handle])
                                        )
                                    }
                                    .buttonStyle(PixelButtonStyle())
                                    .frame(minHeight: 44)
                                    .font(.caption)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 4)
                        }
                    }
                    ActionButton(title: "Quote and bind cover", icon: "umbrella") {
                        showingQuote = true
                    }
                }
                ForEach(insurers, id: \.handle) { insurer in
                    SectionCard(title: insurer.name) {
                        DetailRow(title: "Region", value: insurer.region)
                        DetailRow(title: "Cash", value: Format.money(insurer.cashCents))
                        DetailRow(title: "Equity", value: Format.money(insurer.equityCents))
                        DetailRow(title: "Cover in force", value: Format.money(insurer.coverageCents))
                        DetailRow(title: "Mode", value: insurer.specialist ? "Specialist concentration" : "Standard concentration")
                        PixelLabel(
                            insurer.liquidityOk ? "Cash covers two weeks of overhead plus 5% of cover" : "Cash below the underwriting liquidity test",
                            icon: insurer.liquidityOk ? "check" : "warning"
                        )
                        .font(.caption)
                        .foregroundStyle(insurer.liquidityOk ? Theme.positive : Theme.warning)
                        PixelLabel(
                            insurer.capitalOk ? "Capital covers 10% of cover" : "Capital below the underwriting capital test",
                            icon: insurer.capitalOk ? "check" : "warning"
                        )
                        .font(.caption)
                        .foregroundStyle(insurer.capitalOk ? Theme.positive : Theme.warning)
                        Button(insurer.specialist ? "Return to standard concentration" : "Enable specialist concentration") {
                            confirm = ConfirmRequest(
                                title: "Underwriting limit",
                                actorName: insurer.name,
                                detail: insurer.specialist
                                    ? "Specialist mode doubles the capital requirement and removes the 40 percent regional limit. Turning it off restores both."
                                    : "Specialist mode removes the 40 percent regional concentration limit but doubles the capital needed for total cover.",
                                submitTitle: "Change underwriting mode",
                                json: CommandJSON.command("SetUnderwritingLimit", [
                                    "insurer": insurer.handle,
                                    "specialist": !insurer.specialist
                                ])
                            )
                        }
                        .buttonStyle(PixelButtonStyle())
                        .frame(minHeight: 44)
                        .font(.caption)
                    }
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Insurance")
        .sheet(isPresented: $showingQuote) { QuoteBindSheet().environment(store) }
        .sheet(item: $confirm) { request in
            ConfirmCommandSheet(request: request).environment(store)
        }
    }
}

// MARK: - Labour

struct LabourView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingCounter = false
    @State private var showingSettlement = false
    @State private var confirm: ConfirmRequest?

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                if let labour = store.labour, let firm = store.selectedFirmDto {
                    SectionCard(title: "Bargaining state") {
                        DetailRow(title: "State", value: labour.state.capitalized)
                        DetailRow(title: "Organisation", value: Format.percentBps(labour.organisationBps))
                        DetailRow(title: "Employees", value: labour.employees.formatted())
                        DetailRow(title: "Low-tier wage", value: Format.money(labour.currentWageCents))
                        DetailRow(title: "Union demand", value: Format.money(labour.demandCents))
                        DetailRow(title: "Your last offer", value: Format.money(labour.lastOfferCents))
                        DetailRow(title: "Settlement cost", value: Format.money(labour.settlementCostCents))
                        if let deadline = labour.demandDeadline {
                            DetailRow(title: "Demand deadline", value: "Week \(deadline)")
                        }
                        if let until = labour.strikeUntil {
                            Text("On strike until week \(until). Striking workers are not paid and about 70 percent are unavailable, so output falls.")
                                .font(.callout)
                                .foregroundStyle(Theme.warning)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    SectionCard(title: "Respond") {
                        if labour.state == "demanding" || labour.state == "striking" {
                            ActionButton(title: "Accept the demand", icon: "thumb_up") {
                                confirm = ConfirmRequest(
                                    title: "Accept the demand",
                                    actorName: firm.name,
                                    detail: "Adopts the union's wage demand. Payroll rises from the next settled week.",
                                    submitTitle: "Accept",
                                    json: CommandJSON.command("RespondBargaining", [
                                        "firm": firm.handle,
                                        "choice": "Accept",
                                        "offer_cents": 0
                                    ])
                                )
                            }
                            ActionButton(title: "Counter with an offer", icon: "swap") {
                                showingCounter = true
                            }
                            ActionButton(title: "Reject the demand", icon: "thumb_down", role: .destructive) {
                                confirm = ConfirmRequest(
                                    title: "Reject the demand",
                                    actorName: firm.name,
                                    detail: "Two unresolved weeks start a strike. Rejecting does not lower the union's demand.",
                                    submitTitle: "Reject",
                                    json: CommandJSON.command("RespondBargaining", [
                                        "firm": firm.handle,
                                        "choice": "Reject",
                                        "offer_cents": 0
                                    ])
                                )
                            }
                        } else {
                            EmptyNote(text: "No open bargaining. Organisation rises when pay lags the region and falls when it leads. A demand can form every 52 weeks.")
                        }
                        ActionButton(title: "Propose a settlement", icon: "pen") {
                            showingSettlement = true
                        }
                    }
                } else {
                    EmptyNote(text: "Select a company to read its bargaining state.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Labour")
        .sheet(isPresented: $showingCounter) { BargainCounterSheet().environment(store) }
        .sheet(isPresented: $showingSettlement) { SettlementSheet().environment(store) }
        .sheet(item: $confirm) { request in
            ConfirmCommandSheet(request: request).environment(store)
        }
    }
}

// MARK: - Policy

struct PolicyView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingRequest = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                if let state = store.policyState {
                    SectionCard(title: "Lobbying") {
                        DetailRow(title: "Request fee", value: Format.money(state.feeCents))
                        DetailRow(title: "Reputation", value: String(format: "%.1f", Double(state.reputationBps) / 100.0))
                        Text("One active request per region, a four-week decision and a 26-week cooldown. The fee is spent even when the request fails; success is uncertain and never guaranteed.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                        ActionButton(title: "File a lobby request", icon: "mail") {
                            showingRequest = true
                        }
                    }
                    SectionCard(title: "Region cooldowns") {
                        ForEach(state.cooldowns, id: \.region) { row in
                            HStack(alignment: .firstTextBaseline) {
                                Text(row.name).font(.callout)
                                Spacer(minLength: 8)
                                if row.active {
                                    Text("Decision pending").font(.caption).foregroundStyle(Theme.warning)
                                } else if row.availableWeek > (store.overview?.tick ?? 0) {
                                    Text("Available week \(row.availableWeek)")
                                        .font(.caption.monospacedDigit())
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                } else {
                                    Text("Available").font(.caption).foregroundStyle(Theme.positive)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .frame(minHeight: 32)
                        }
                    }
                    SectionCard(title: "Requests") {
                        if state.requests.isEmpty {
                            EmptyNote(text: "No requests filed in this career.")
                        } else {
                            ForEach(state.requests.indices, id: \.self) { index in
                                let request = state.requests[index]
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("\(request.kind.replacingOccurrences(of: "_", with: " ").capitalized) · \(request.regionName)")
                                        .font(.callout)
                                    Text("Fee \(Format.money(request.feeCents)) · decision week \(request.decisionWeek) · \(request.resolved ? (request.success ? "granted" : "declined") : "pending")")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                } else {
                    EmptyNote(text: "Select a company to read its lobbying state.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Policy")
        .sheet(isPresented: $showingRequest) { LobbySheet().environment(store) }
    }
}

// MARK: - Recovery

struct RecoveryView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingFound = false
    @State private var confirm: ConfirmRequest?

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                if let recovery = store.recovery {
                    SectionCard(title: "Management contract") {
                        DetailRow(title: "Your personal cash", value: Format.money(recovery.cashCents))
                        DetailRow(title: "Signing bonus", value: Format.money(recovery.signingBonusCents))
                        DetailRow(title: "Managed firms", value: "\(recovery.managedFirms)")
                        if let candidate = recovery.candidate {
                            DetailRow(title: "Available firm", value: candidate)
                        }
                        Text(recoveryText(recovery))
                            .font(.callout)
                            .foregroundStyle(recovery.eligible ? Theme.text(for: scheme) : Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                        if recovery.eligible {
                            ActionButton(title: "Accept the management contract", icon: "briefcase") {
                                confirm = ConfirmRequest(
                                    title: "Management contract",
                                    actorName: "You, the investor",
                                    detail: "Adds \(Format.money(recovery.signingBonusCents)) of personal cash and makes \(recovery.candidate ?? "the firm") your managed firm. The assisted score label is kept.",
                                    submitTitle: "Accept contract",
                                    json: CommandJSON.unit("AcceptManagementContract"),
                                    asInvestor: true
                                )
                            }
                        } else {
                            Text("This action stays disabled until the reason above clears.")
                                .font(.caption2)
                                .foregroundStyle(Theme.secondaryText(for: scheme))
                        }
                    }
                    SectionCard(title: "Found a firm") {
                        DetailRow(title: "Formation fee", value: Format.money(recovery.formationFeeCents))
                        DetailRow(title: "Minimum paid-in capital", value: Format.money(recovery.minimumCapitalCents))
                        Text("The paid-in capital becomes the new firm's cash and your equity investment. The formation fee is paid to outside legal services. The chosen template is escrowed from that capital.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                        ActionButton(title: "Found a new firm", icon: "office") {
                            showingFound = true
                        }
                    }
                } else {
                    EmptyNote(text: "Recovery state is unavailable until a career is open.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Recovery")
        .sheet(isPresented: $showingFound) { FoundFirmSheet().environment(store) }
        .sheet(item: $confirm) { request in
            ConfirmCommandSheet(request: request).environment(store)
        }
    }

    private func recoveryText(_ recovery: RecoveryDto) -> String {
        switch recovery.reasonKey {
        case "recovery.available":
            return "You qualify: personal cash is below the threshold, you control no firm and you manage none. One contract is offered and the assisted score label is recorded."
        case "recovery.cooldown":
            return "A contract was used recently. The next one cannot be offered before week \(recovery.nextEligibleWeek)."
        case "recovery.no_target":
            return "No unowned operating firm is available for a management contract right now."
        default:
            return "The contract needs true destitution: personal unpledged cash below $25,000, no saleable holding and no controlled viable firm. You do not qualify while a firm or holding remains."
        }
    }
}
