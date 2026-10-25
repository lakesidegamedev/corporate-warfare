import Foundation
import SwiftUI

// MARK: - Equity

struct IssueSharesSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var quantityText = ""
    @State private var priceText = ""
    @State private var buyer: UInt32 = 0
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var quantity: Int64? { parseCount(quantityText) }
    private var priceCents: Int64? { parseDollarsToCents(priceText) }
    private var buyerDto: BuyerDto? { store.fundingBuyers.first { $0.index == buyer } }

    private func json() -> String? {
        guard let firm, let quantity, let priceCents else { return nil }
        return CommandJSON.command("IssueShares", [
            "firm": firm.handle,
            "qty": quantity,
            "price_cents": priceCents,
            "buyer_outside": buyer
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Issue shares",
            actorName: firm?.name ?? "No company",
            actorDetail: "Primary issuance. The funding buyer pays cash to this firm; your vote share falls by the exact amount shown.",
            submitTitle: "Issue shares",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let quantity, let priceCents else { return false }
                if let rejection = await store.issueShares(firm: firm.handle, quantity: quantity, priceCents: priceCents, buyerOutside: buyer) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                TextField("New shares", text: $quantityText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Number of new shares")
                TextField("Price per share in dollars", text: $priceText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Price per share in dollars")
                Picker("Funding buyer", selection: $buyer) {
                    ForEach(store.fundingBuyers, id: \.index) { item in
                        Text("\(item.name) · \(Format.moneyCompact(item.capitalCents)) available")
                            .tag(item.index)
                    }
                }
                if let buyerDto {
                    DetailRow(title: "Buyer capital", value: Format.money(buyerDto.capitalCents))
                }
                Text("Issuance needs a price at or above 90 percent of fair value, a funding buyer with cash, and control of the firm.")
                    .font(.caption)
            }
        }
        .task { await refresh() }
        .onChange(of: quantityText) { _, _ in Task { await refresh() } }
        .onChange(of: priceText) { _, _ in Task { await refresh() } }
        .onChange(of: buyer) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let firm, let quantity, let priceCents else {
            preview = nil
            return
        }
        preview = await store.previewIssue(firm: firm.handle, quantity: quantity, priceCents: priceCents)
    }
}

struct RepurchaseSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var quantityText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var quantity: Int64? { parseCount(quantityText) }

    private func json() -> String? {
        guard let firm, let quantity else { return nil }
        return CommandJSON.command("RepurchaseShares", ["firm": firm.handle, "qty": quantity])
    }

    var body: some View {
        CommandSheet(
            title: "Repurchase shares",
            actorName: firm?.name ?? "No company",
            actorDetail: "Spends this firm's cash and moves the bought shares to treasury. Must leave four weeks of forecast obligations.",
            submitTitle: "Repurchase",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let quantity else { return false }
                if let rejection = await store.repurchaseShares(firm: firm.handle, quantity: quantity) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                TextField("Shares to repurchase", text: $quantityText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Shares to repurchase")
            }
        }
        .task { await refresh() }
        .onChange(of: quantityText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct RetireTreasurySheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var quantityText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var quantity: Int64? { parseCount(quantityText) }

    private func json() -> String? {
        guard let firm, let quantity else { return nil }
        return CommandJSON.command("RetireTreasury", ["firm": firm.handle, "qty": quantity])
    }

    var body: some View {
        CommandSheet(
            title: "Retire treasury",
            actorName: firm?.name ?? "No company",
            actorDetail: "Cancels treasury shares. Issued and treasury shares fall together and no cash moves.",
            submitTitle: "Retire shares",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let quantity else { return false }
                if let rejection = await store.retireTreasury(firm: firm.handle, quantity: quantity) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                if let ownership = store.ownership {
                    DetailRow(title: "Treasury now", value: ownership.treasury.formatted())
                }
                TextField("Treasury shares to cancel", text: $quantityText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Treasury shares to cancel")
            }
        }
        .task { await refresh() }
        .onChange(of: quantityText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct IPOSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var sharesText = ""
    @State private var priceText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var newShares: Int64? { parseCount(sharesText) }
    private var priceCents: Int64? { parseDollarsToCents(priceText) }

    private func json() -> String? {
        guard let firm, let newShares, let priceCents else { return nil }
        return CommandJSON.command("LaunchIPO", [
            "firm": firm.handle,
            "new_shares": newShares,
            "price_cents": priceCents
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Launch IPO",
            actorName: firm?.name ?? "No company",
            actorDetail: "Sells new shares to an outside investor. Costs 2 percent of proceeds, minimum $100,000, and needs 13 weeks of statements.",
            submitTitle: "Launch IPO",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let newShares, let priceCents else { return false }
                if let rejection = await store.launchIPO(firm: firm.handle, newShares: newShares, priceCents: priceCents) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                TextField("New shares to sell", text: $sharesText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("New shares to sell")
                TextField("Price per share in dollars", text: $priceText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Price per share in dollars")
                Text("At least ten percent of post-issue shares must be sold, and the firm must have positive equity.")
                    .font(.caption)
            }
        }
        .task { await refresh() }
        .onChange(of: sharesText) { _, _ in Task { await refresh() } }
        .onChange(of: priceText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

// MARK: - Credit

struct LoanSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var amountText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var amount: Int64? { parseDollarsToCents(amountText) }

    private func json() -> String? {
        guard let firm, let amount else { return nil }
        return CommandJSON.command("RequestLoan", ["firm": firm.handle, "amount_cents": amount])
    }

    var body: some View {
        CommandSheet(
            title: "Request a loan",
            actorName: firm?.name ?? "No company",
            actorDetail: "Borrows from an outside lender. The rate is the policy rate plus this firm's grade spread; principal amortises weekly.",
            submitTitle: "Request loan",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let amount else { return false }
                if let rejection = await store.requestLoan(firm: firm.handle, amountCents: amount) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                TextField("Loan amount in dollars", text: $amountText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Loan amount in dollars")
                Text("Borrowing capacity is capped by secured and unsecured coverage. Debt above 60 percent of tangible assets needs a board vote.")
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

struct BondSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var principalText = ""
    @State private var couponText = "700"
    @State private var weeksText = "260"
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var principal: Int64? { parseDollarsToCents(principalText) }
    private var coupon: Int32? { Int32(couponText) }
    private var weeks: UInt32? { UInt32(weeksText) }

    private func json() -> String? {
        guard let firm, let principal, let coupon, let weeks else { return nil }
        return CommandJSON.command("IssueBond", [
            "firm": firm.handle,
            "principal_cents": principal,
            "coupon_bps": coupon,
            "weeks": weeks
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Issue a bond",
            actorName: firm?.name ?? "No company",
            actorDetail: "A public bond pays a coupon and a 2 percent issue fee. Needs grade A or B, at least $500,000, and debt within 60 percent of tangible assets.",
            submitTitle: "Issue bond",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let principal, let coupon, let weeks else { return false }
                if let rejection = await store.issueBond(firm: firm.handle, principalCents: principal, couponBps: coupon, weeks: weeks) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                TextField("Principal in dollars", text: $principalText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Bond principal in dollars")
                TextField("Coupon in basis points", text: $couponText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Coupon in basis points")
                TextField("Weeks to maturity", text: $weeksText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Weeks to maturity")
                if let firm {
                    DetailRow(title: "Grade", value: firm.grade)
                    DetailRow(title: "Equity", value: Format.money(firm.equityCents))
                }
            }
        }
        .task { await refresh() }
        .onChange(of: principalText) { _, _ in Task { await refresh() } }
        .onChange(of: couponText) { _, _ in Task { await refresh() } }
        .onChange(of: weeksText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

// MARK: - Governance

struct NominateCEOSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var appointSelf = true
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }

    private func json() -> String? {
        guard let firm else { return nil }
        return CommandJSON.command("NominateCEO", ["firm": firm.handle, "investor": appointSelf])
    }

    var body: some View {
        CommandSheet(
            title: "Nominate a CEO",
            actorName: firm?.name ?? "No company",
            actorDetail: "Appointing yourself needs three of five board seats. Appointing a manager hands operating authority to an appointed manager.",
            submitTitle: "Nominate",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm else { return false }
                if let rejection = await store.nominateCEO(firm: firm.handle, investor: appointSelf) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Choice") {
                Toggle("Appoint yourself as CEO", isOn: $appointSelf)
                if let ownership = store.ownership {
                    DetailRow(title: "Your board seats", value: "\(ownership.boardSeats.filter { $0 == "You" }.count) of 5")
                    DetailRow(title: "Current CEO", value: ownership.ceo)
                }
            }
        }
        .task { await refresh() }
        .onChange(of: appointSelf) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct BoardActionSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var subject = "merger_authority"
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private struct BoardSubject: Identifiable {
        let id: String
        let label: String
        let detail: String
    }

    private var firm: FirmDto? { store.selectedFirmDto }
    private let subjects: [BoardSubject] = [
        BoardSubject(id: "merger_authority", label: "Authorise a merger", detail: "Asks the board to approve merging an acquired target."),
        BoardSubject(id: "large_issuance", label: "Authorise a large issuance", detail: "Asks the board to approve issuance above the standing limit."),
        BoardSubject(id: "debt_limit", label: "Raise the debt limit", detail: "Asks the board to allow debt above 60 percent of tangible assets."),
        BoardSubject(id: "conflict_transfer", label: "Approve a conflict transfer", detail: "Minority directors vote against related-party transfers unless the terms are fair.")
    ]

    private func json() -> String? {
        guard let firm else { return nil }
        return CommandJSON.command("ProposeBoardAction", ["firm": firm.handle, "subject": subject])
    }

    var body: some View {
        CommandSheet(
            title: "Propose board action",
            actorName: firm?.name ?? "No company",
            actorDetail: "Opens a four-week board vote. Three of five seats must approve; a tie keeps the status quo.",
            submitTitle: "Propose",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm else { return false }
                if let rejection = await store.proposeBoardAction(firm: firm.handle, subject: subject) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Subject") {
                Picker("Action", selection: $subject) {
                    ForEach(subjects, id: \.id) { item in
                        Text(item.label).tag(item.id)
                    }
                }
                if let chosen = subjects.first(where: { $0.id == subject }) {
                    Text(chosen.detail)
                        .font(.caption)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .task { await refresh() }
        .onChange(of: subject) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct ShareOrderSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var buy = true
    @State private var quantityText = ""
    @State private var limitText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var quantity: Int64? { parseCount(quantityText) }
    private var limit: Int64? { parseDollarsToCents(limitText) }

    private func json() -> String? {
        guard let firm, let quantity, let limit else { return nil }
        return CommandJSON.command("PlaceShareOrder", [
            "investor": true,
            "firm": firm.handle,
            "buy": buy,
            "qty": quantity,
            "limit_price_cents": limit
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Share order",
            actorName: "You, the investor",
            actorDetail: "Trades your personal holding in \(firm?.name ?? "the selected firm"). Cash or shares are reserved until the next week boundary.",
            submitTitle: buy ? "Place buy order" : "Place sell order",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let json = json() else { return false }
                if let rejection = await store.submitAsInvestor(json) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Order") {
                Picker("Side", selection: $buy) {
                    Text("Buy").tag(true)
                    Text("Sell").tag(false)
                }
                .pickerStyle(.segmented)
                TextField("Shares", text: $quantityText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Number of shares")
                TextField("Limit price per share in dollars", text: $limitText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Limit price per share in dollars")
                if let firm {
                    DetailRow(title: "Traded mark", value: Format.money(firm.markCents))
                    DetailRow(title: "Your votes", value: Format.percentBps(firm.investorVotesBps))
                }
            }
        }
        .onAppear {
            if limitText.isEmpty, let firm {
                limitText = String(format: "%.2f", Double(firm.markCents) / 100.0)
            }
        }
        .task { await refresh() }
        .onChange(of: quantityText) { _, _ in Task { await refresh() } }
        .onChange(of: limitText) { _, _ in Task { await refresh() } }
        .onChange(of: buy) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct CastVoteSheet: View {
    let vote: BoardVoteDto
    @EnvironmentObject private var store: AppStore
    @State private var approve = true
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }

    private func json() -> String? {
        guard let firm else { return nil }
        return CommandJSON.command("CastVote", [
            "firm": firm.handle,
            "vote_id": vote.id,
            "approve": approve
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Cast a vote",
            actorName: firm?.name ?? "No company",
            actorDetail: "Records your seat's vote on this proposal. Votes resolve at the deadline by simple majority of five seats.",
            submitTitle: approve ? "Vote yes" : "Vote no",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm else { return false }
                if let rejection = await store.castVote(firm: firm.handle, voteId: vote.id, approve: approve) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Proposal") {
                Text(vote.subject.replacingOccurrences(of: "_", with: " ").capitalized)
                    .font(.body.weight(.medium))
                DetailRow(title: "Deadline", value: "Week \(vote.deadline)")
                DetailRow(title: "Recorded yes", value: "\(vote.yes)")
                DetailRow(title: "Recorded no", value: "\(vote.no)")
                Picker("Your vote", selection: $approve) {
                    Text("Approve").tag(true)
                    Text("Reject").tag(false)
                }
                .pickerStyle(.segmented)
            }
        }
        .task { await refresh() }
        .onChange(of: approve) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

// MARK: - Deals

struct StartDiligenceSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var target: UInt64 = 0
    @State private var kind = "SharePurchase"
    @State private var priceText = ""
    @State private var sharesText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var priceCents: Int64? { parseDollarsToCents(priceText) }
    private var maxShares: Int64? { parseCount(sharesText) }

    private var targets: [FirmDto] {
        store.firms.filter { $0.status == "operating" && $0.handle != firm?.handle }
    }

    private var targetFirm: FirmDto? { targets.first { $0.handle == target } }

    private func json() -> String? {
        guard let firm, target != 0, let priceCents, let maxShares else { return nil }
        return CommandJSON.command("StartDiligence", [
            "buyer": firm.handle,
            "target": target,
            "kind": kind,
            "price_cents": priceCents,
            "max_shares": maxShares
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Start diligence",
            actorName: firm?.name ?? "No company",
            actorDetail: "Escrows the full maximum price plus 2 percent, pays 0.5 percent diligence, then reviews the target's statements and contracts for two weeks.",
            submitTitle: "Start diligence",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, target != 0, let priceCents, let maxShares else { return false }
                if let rejection = await store.startDiligence(buyer: firm.handle, target: target, kind: kind, priceCents: priceCents, maxShares: maxShares) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Target") {
                Picker("Target", selection: $target) {
                    Text("Choose a firm").tag(UInt64(0))
                    ForEach(targets, id: \.handle) { item in
                        Text(item.name).tag(item.handle)
                    }
                }
                if let targetFirm {
                    DetailRow(title: "Region", value: targetFirm.region)
                    DetailRow(title: "Traded mark", value: Format.money(targetFirm.markCents))
                    DetailRow(title: "Status", value: targetFirm.status.replacingOccurrences(of: "_", with: " "))
                }
                Picker("Deal kind", selection: $kind) {
                    Text("Share purchase").tag("SharePurchase")
                    Text("Asset purchase").tag("AssetPurchase")
                    Text("Merger").tag("Merger")
                }
            }
            Section("Bid") {
                TextField("Price per share in dollars", text: $priceText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Offer price per share in dollars")
                TextField("Maximum shares", text: $sharesText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Maximum shares to buy")
            }
        }
        .onAppear {
            if target == 0, let first = targets.first {
                target = first.handle
            }
        }
        .task { await refresh() }
        .onChange(of: target) { _, _ in Task { await refresh() } }
        .onChange(of: kind) { _, _ in Task { await refresh() } }
        .onChange(of: priceText) { _, _ in Task { await refresh() } }
        .onChange(of: sharesText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct RemedySheet: View {
    let deal: DealDto
    @EnvironmentObject private var store: AppStore
    @State private var divestBps: Int32 = 500
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private func json() -> String {
        CommandJSON.command("SubmitRemedy", [
            "deal": deal.handle,
            "divest_share_bps": divestBps
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Submit a remedy",
            actorName: store.selectedFirmDto?.name ?? "No company",
            actorDetail: "Offers to divest enough share to put the combined regional share at or below 35 percent and the HHI delta at or below 200.",
            submitTitle: "Submit remedy",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: divestBps > 0,
            onSubmit: {
                if let rejection = await store.submitRemedy(deal: deal.handle, divestShareBps: divestBps) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Divestiture") {
                Stepper(value: $divestBps, in: 100...5_000, step: 100) {
                    Text("Divest \(Format.percentBps(divestBps)) of the combined share")
                }
                Text("A feasible bundle clears the review. If no bundle reaches the thresholds the merger is rejected rather than inventing regulator discretion.")
                    .font(.caption)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .task { await refresh() }
        .onChange(of: divestBps) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        preview = await store.preview(json())
    }
}

// MARK: - Banking

enum BankTransferMode: Equatable {
    case deposit
    case withdraw
}

struct DepositSheet: View {
    let mode: BankTransferMode
    let bank: InstitutionDto?
    @EnvironmentObject private var store: AppStore
    @State private var bankHandle: UInt64 = 0
    @State private var amountText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var amount: Int64? { parseDollarsToCents(amountText) }
    private var banks: [InstitutionDto] {
        store.institutions.filter { $0.kind.contains("bank") }
    }

    private func json() -> String? {
        guard let firm, bankHandle != 0, let amount else { return nil }
        let variant = mode == .deposit ? "DepositFunds" : "WithdrawFunds"
        return CommandJSON.command(variant, [
            "bank": bankHandle,
            "firm": firm.handle,
            "amount_cents": amount
        ])
    }

    var body: some View {
        CommandSheet(
            title: mode == .deposit ? "Deposit funds" : "Withdraw funds",
            actorName: firm?.name ?? "No company",
            actorDetail: mode == .deposit
                ? "Moves this firm's cash into a bank deposit claim."
                : "Pays out of actual bank cash, up to this firm's deposit claim.",
            submitTitle: mode == .deposit ? "Deposit" : "Withdraw",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, bankHandle != 0, let amount else { return false }
                let rejection: CommandRejection?
                if mode == .deposit {
                    rejection = await store.depositFunds(bank: bankHandle, firm: firm.handle, amountCents: amount)
                } else {
                    rejection = await store.withdrawFunds(bank: bankHandle, firm: firm.handle, amountCents: amount)
                }
                if let rejection {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Transfer") {
                Picker("Bank", selection: $bankHandle) {
                    Text("Choose a bank").tag(UInt64(0))
                    ForEach(banks, id: \.handle) { item in
                        Text("\(item.name) · cash \(Format.moneyCompact(item.cashCents))").tag(item.handle)
                    }
                }
                TextField("Amount in dollars", text: $amountText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Amount in dollars")
                if let finance = store.finance {
                    DetailRow(title: "Firm cash", value: Format.money(finance.cashCents))
                }
            }
        }
        .onAppear {
            if bankHandle == 0 {
                bankHandle = bank?.handle ?? banks.first?.handle ?? 0
            }
        }
        .task { await refresh() }
        .onChange(of: amountText) { _, _ in Task { await refresh() } }
        .onChange(of: bankHandle) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct ApproveLoanSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var bankHandle: UInt64 = 0
    @State private var borrower: UInt64 = 0
    @State private var amountText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var banks: [InstitutionDto] {
        // Approving a loan is an action of the bank itself, so the bank must be the selected
        // firm: its board (or the investor behind it) is the legal actor, never a customer.
        store.institutions.filter { $0.kind.contains("bank") && $0.handle == store.selectedFirm }
    }

    private var borrowers: [FirmDto] {
        store.firms.filter { $0.status == "operating" }
    }

    private var amount: Int64? { parseDollarsToCents(amountText) }

    private func json() -> String? {
        guard bankHandle != 0, borrower != 0, let amount else { return nil }
        return CommandJSON.command("ApproveLoan", [
            "bank": bankHandle,
            "borrower": borrower,
            "amount_cents": amount
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Approve a loan",
            actorName: banks.first { $0.handle == bankHandle }?.name ?? "Choose a bank",
            actorDetail: "Lends the bank's own cash. The bank must keep 20 percent of deposits liquid and capital at 12 percent of risk-weighted loans.",
            submitTitle: "Approve loan",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let amount else { return false }
                if let rejection = await store.approveLoan(bank: bankHandle, borrower: borrower, amountCents: amount) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Loan") {
                if banks.isEmpty {
                    Text("Select the bank itself in Portfolio to approve its loans. Lending is the bank's action, not the customer's.")
                        .font(.caption)
                } else {
                    Picker("Bank", selection: $bankHandle) {
                        Text("Choose a bank").tag(UInt64(0))
                        ForEach(banks, id: \.handle) { item in
                            Text(item.name).tag(item.handle)
                        }
                    }
                    Picker("Borrower", selection: $borrower) {
                        Text("Choose a borrower").tag(UInt64(0))
                        ForEach(borrowers, id: \.handle) { item in
                            Text(item.name).tag(item.handle)
                        }
                    }
                    TextField("Amount in dollars", text: $amountText)
                        .keyboardType(.decimalPad)
                        .accessibilityLabel("Loan amount in dollars")
                }
            }
        }
        .onAppear {
            if bankHandle == 0 { bankHandle = banks.first?.handle ?? 0 }
        }
        .task { await refresh() }
        .onChange(of: amountText) { _, _ in Task { await refresh() } }
        .onChange(of: bankHandle) { _, _ in Task { await refresh() } }
        .onChange(of: borrower) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct DepositRateSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var rateText = "200"
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var rate: Int32? { Int32(rateText) }

    private func json() -> String? {
        guard let firm, let rate else { return nil }
        return CommandJSON.command("SetDepositRate", ["bank": firm.handle, "rate_bps": rate])
    }

    var body: some View {
        CommandSheet(
            title: "Deposit rate",
            actorName: firm?.name ?? "No company",
            actorDetail: "Sets the rate paid on deposits at this bank. Accepts 0 to 1,500 basis points.",
            submitTitle: "Set rate",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let rate else { return false }
                if let rejection = await store.setDepositRate(bank: firm.handle, rateBps: rate) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Rate") {
                TextField("Rate in basis points", text: $rateText)
                    .keyboardType(.numberPad)
                    .accessibilityLabel("Deposit rate in basis points")
                if let rate {
                    Text("Pays \(Format.percentBps(rate)) a year on deposits.")
                        .font(.caption)
                }
            }
        }
        .task { await refresh() }
        .onChange(of: rateText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

// MARK: - Insurance

struct QuoteBindSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.colorScheme) private var scheme
    @State private var insurer: UInt64 = 0
    @State private var facility: UInt64 = 0
    @State private var limitText = ""
    @State private var deductibleText = ""
    @State private var preview: PreviewDto?
    @State private var quote: QuoteDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var limit: Int64? { parseDollarsToCents(limitText) }
    private var deductible: Int64? { parseDollarsToCents(deductibleText) }
    private var insurers: [InstitutionDto] {
        store.institutions.filter { $0.kind.contains("insurer") }
    }

    private func json() -> String? {
        guard let firm, insurer != 0, facility != 0, let limit, let deductible else { return nil }
        return CommandJSON.command("BindPolicy", [
            "insurer": insurer,
            "insured": firm.handle,
            "facility": facility,
            "limit_cents": limit,
            "deductible_cents": deductible
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Quote and bind",
            actorName: firm?.name ?? "No company",
            actorDetail: "Binds property cover for one facility. Premiums are paid weekly; a claim pays net of the deductible up to the limit.",
            submitTitle: "Bind policy",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let limit, let deductible else { return false }
                if let rejection = await store.bindPolicy(insurer: insurer, insured: firm.handle, facility: facility, limitCents: limit, deductibleCents: deductible) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Cover") {
                Picker("Insurer", selection: $insurer) {
                    Text("Choose an insurer").tag(UInt64(0))
                    ForEach(insurers, id: \.handle) { item in
                        Text(item.name).tag(item.handle)
                    }
                }
                Picker("Facility", selection: $facility) {
                    Text("Choose a facility").tag(UInt64(0))
                    ForEach(store.facilities, id: \.handle) { item in
                        Text("\(item.recipe.capitalized) · \(item.region)").tag(item.handle)
                    }
                }
                TextField("Cover limit in dollars", text: $limitText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Cover limit in dollars")
                TextField("Deductible in dollars", text: $deductibleText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Deductible in dollars")
                if let quote {
                    if let blocked = quote.blockedKey {
                        Text(Remedy.title(for: blocked))
                            .font(.caption)
                            .foregroundStyle(Theme.negative)
                        Text(Remedy.text(for: blocked))
                            .font(.caption2)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                    } else {
                        DetailRow(title: "Quoted weekly premium", value: Format.money(quote.weeklyPremiumCents))
                    }
                }
                Button("Send a quote request") {
                    Task { await requestQuote() }
                }
                .buttonStyle(.bordered)
                .frame(minHeight: 44)
            }
        }
        .onAppear {
            if insurer == 0 { insurer = insurers.first?.handle ?? 0 }
            if facility == 0 { facility = store.facilities.first?.handle ?? 0 }
        }
        .task { await refresh() }
        .onChange(of: insurer) { _, _ in Task { await refresh() } }
        .onChange(of: facility) { _, _ in Task { await refresh() } }
        .onChange(of: limitText) { _, _ in Task { await refresh() } }
        .onChange(of: deductibleText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            quote = nil
            return
        }
        preview = await store.preview(json)
        if insurer != 0, facility != 0, let limit {
            quote = await store.insuranceQuote(insurer: insurer, facility: facility, limitCents: limit)
        }
    }

    private func requestQuote() async {
        guard insurer != 0, facility != 0, let limit, let deductible else { return }
        if let rejection = await store.quotePolicy(insurer: insurer, facility: facility, limitCents: limit, deductibleCents: deductible) {
            self.rejection = rejection
        } else {
            self.rejection = nil
            quote = await store.insuranceQuote(insurer: insurer, facility: facility, limitCents: limit)
        }
    }
}

// MARK: - Labour and policy

struct BargainCounterSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var offerText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var offer: Int64? { parseDollarsToCents(offerText) }

    private func json() -> String? {
        guard let firm, let offer else { return nil }
        return CommandJSON.command("RespondBargaining", [
            "firm": firm.handle,
            "choice": "Counter",
            "offer_cents": offer
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Counter-offer",
            actorName: firm?.name ?? "No company",
            actorDetail: "Sends a weekly wage offer to the bargaining unit. A second unresolved week can start a strike.",
            submitTitle: "Send counter",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let offer else { return false }
                if let rejection = await store.respondBargaining(firm: firm.handle, choice: "Counter", offerCents: offer) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Offer") {
                if let labour = store.labour {
                    DetailRow(title: "Union demand", value: Format.money(labour.demandCents))
                    DetailRow(title: "Your current wage", value: Format.money(labour.currentWageCents))
                }
                TextField("Weekly offer in dollars", text: $offerText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Weekly counter-offer in dollars")
            }
        }
        .onAppear {
            if offerText.isEmpty, let labour = store.labour {
                offerText = String(format: "%.2f", Double(labour.currentWageCents) / 100.0)
            }
        }
        .task { await refresh() }
        .onChange(of: offerText) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

struct SettlementSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var amountText = ""
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var amount: Int64? { parseDollarsToCents(amountText) }

    private func json() -> String? {
        guard let firm, let amount else { return nil }
        return CommandJSON.command("ProposeSettlement", ["firm": firm.handle, "amount_cents": amount])
    }

    var body: some View {
        CommandSheet(
            title: "Propose a settlement",
            actorName: firm?.name ?? "No company",
            actorDetail: "Pays a one-off settlement and returns bargaining to idle. The offer must be at least 95 percent of the current demand.",
            submitTitle: "Propose settlement",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm, let amount else { return false }
                if let rejection = await store.proposeSettlement(firm: firm.handle, amountCents: amount) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Settlement") {
                if let labour = store.labour {
                    DetailRow(title: "Union demand", value: Format.money(labour.demandCents))
                    DetailRow(title: "Settlement cost", value: Format.money(labour.settlementCostCents))
                }
                TextField("Settlement in dollars", text: $amountText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Settlement amount in dollars")
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

struct LobbySheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var region: UInt32 = 0
    @State private var kind = "PermitAcceleration"
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var firm: FirmDto? { store.selectedFirmDto }
    private var cooldown: RegionCooldownDto? {
        store.policyState?.cooldowns.first { $0.region == region }
    }
    private var blockedByCooldown: Bool {
        guard let cooldown else { return false }
        return cooldown.active || cooldown.availableWeek > (store.overview?.tick ?? 0)
    }

    private func json() -> String? {
        guard let firm, !blockedByCooldown else { return nil }
        return CommandJSON.command("FileLobbyRequest", [
            "firm": firm.handle,
            "region": region,
            "request": kind
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Lobby request",
            actorName: firm?.name ?? "No company",
            actorDetail: "Pays the request fee to the outside advocacy sector. One active request per region and a 26-week cooldown.",
            submitTitle: "File request",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let firm else { return false }
                if let rejection = await store.fileLobbyRequest(firm: firm.handle, region: region, kind: kind) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Request") {
                Picker("Region", selection: $region) {
                    ForEach(store.regions, id: \.index) { item in
                        Text(item.name).tag(item.index)
                    }
                }
                Picker("Type", selection: $kind) {
                    Text("Permit acceleration").tag("PermitAcceleration")
                    Text("Training grant").tag("TrainingGrant")
                    Text("Tax adjustment").tag("TaxAdjustment")
                }
                if let policy = store.policyState {
                    DetailRow(title: "Fee", value: Format.money(policy.feeCents))
                }
                if let cooldown {
                    if blockedByCooldown {
                        Text(cooldown.active ? "A request in this region is still awaiting a decision." : "This region is in cooldown until week \(cooldown.availableWeek).")
                            .font(.caption)
                            .foregroundStyle(Theme.warning)
                            .fixedSize(horizontal: false, vertical: true)
                    } else {
                        Text("Available now in this region.")
                            .font(.caption)
                            .foregroundStyle(Theme.positive)
                    }
                }
            }
        }
        .onAppear {
            if let name = firm?.region, let match = store.regions.first(where: { $0.name == name }) {
                region = match.index
            }
        }
        .task { await refresh() }
        .onChange(of: region) { _, _ in Task { await refresh() } }
        .onChange(of: kind) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}

// MARK: - Founding

struct FoundFirmSheet: View {
    @EnvironmentObject private var store: AppStore
    @State private var name = ""
    @State private var region: UInt32 = 0
    @State private var capitalText = ""
    @State private var recipe: UInt32 = 0
    @State private var preview: PreviewDto?
    @State private var rejection: CommandRejection?

    private var capital: Int64? { parseDollarsToCents(capitalText) }
    private var trimmedName: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func json() -> String? {
        guard !trimmedName.isEmpty, let capital else { return nil }
        return CommandJSON.command("FoundFirm", [
            "name": trimmedName,
            "region": region,
            "capital_cents": capital,
            "recipe": recipe
        ])
    }

    var body: some View {
        CommandSheet(
            title: "Found a firm",
            actorName: "You, the investor",
            actorDetail: "Pays the formation fee and paid-in capital from your personal cash. The capital becomes the firm's cash and your equity investment.",
            submitTitle: "Found firm",
            preview: preview,
            rejection: rejection,
            isBusy: store.isBusy,
            canSubmit: json() != nil,
            onSubmit: {
                guard let capital else { return false }
                if let rejection = await store.foundFirm(name: trimmedName, region: region, capitalCents: capital, recipe: recipe) {
                    self.rejection = rejection
                    return false
                }
                return true
            }
        ) {
            Section("Firm") {
                TextField("Firm name", text: $name)
                    .accessibilityLabel("New firm name")
                Picker("Region", selection: $region) {
                    ForEach(store.regions, id: \.index) { item in
                        Text(item.name).tag(item.index)
                    }
                }
                TextField("Paid-in capital in dollars", text: $capitalText)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Paid-in capital in dollars")
                Picker("Template", selection: $recipe) {
                    ForEach(store.recipeOptions, id: \.index) { item in
                        Text("\(item.id) · \(Format.money(item.capexCents)) · \(item.buildWeeks)w")
                            .tag(item.index)
                    }
                }
                if let recovery = store.recovery {
                    DetailRow(title: "Formation fee", value: Format.money(recovery.formationFeeCents))
                    DetailRow(title: "Minimum capital", value: Format.money(recovery.minimumCapitalCents))
                }
                if let template = store.recipeOptions.first(where: { $0.index == recipe }) {
                    DetailRow(title: "Template capex", value: Format.money(template.capexCents))
                }
            }
        }
        .onAppear {
            if recipe == 0, let first = store.recipeOptions.first {
                recipe = first.index
            }
        }
        .task { await refresh() }
        .onChange(of: name) { _, _ in Task { await refresh() } }
        .onChange(of: capitalText) { _, _ in Task { await refresh() } }
        .onChange(of: region) { _, _ in Task { await refresh() } }
        .onChange(of: recipe) { _, _ in Task { await refresh() } }
    }

    private func refresh() async {
        guard let json = json() else {
            preview = nil
            return
        }
        preview = await store.preview(json)
    }
}
