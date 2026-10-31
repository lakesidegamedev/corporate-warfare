import Foundation
import SwiftUI

// MARK: - Finance

struct FinanceView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingPrice = false
    @State private var showingDividend = false
    @State private var showingIssue = false
    @State private var showingRepurchase = false
    @State private var showingRetire = false
    @State private var showingIPO = false
    @State private var showingLoan = false
    @State private var showingBond = false
    @State private var showingTraining = false
    @State private var showingQuality = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
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
                        DetailRow(title: "Assets", value: Format.money(finance.assetsCents))
                        DetailRow(title: "Liabilities", value: Format.money(finance.liabilitiesCents))
                        DetailRow(title: "Inventory", value: Format.money(finance.inventoryCents))
                        DetailRow(title: "Restricted cash", value: Format.money(finance.restrictedCents))
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
                    SectionCard(title: "Payout and equity") {
                        Text("Every action here spends or raises this firm's own cash and changes the cap table. Ownership shows the exact votes after an issuance.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                        ActionButton(title: "Pay dividend", icon: "coin") {
                            showingDividend = true
                        }
                        ActionButton(title: "Issue shares", icon: "plus") {
                            showingIssue = true
                        }
                        ActionButton(title: "Repurchase shares", icon: "arrow_down") {
                            showingRepurchase = true
                        }
                        ActionButton(title: "Retire treasury shares", icon: "trash") {
                            showingRetire = true
                        }
                        ActionButton(title: "Launch IPO", icon: "arrow_up") {
                            showingIPO = true
                        }
                        ActionButton(title: "Set a price", icon: "tag") {
                            showingPrice = true
                        }
                    }
                    SectionCard(title: "Credit") {
                        ActionButton(title: "Request a loan", icon: "card") {
                            showingLoan = true
                        }
                        ActionButton(title: "Issue a bond", icon: "doc") {
                            showingBond = true
                        }
                        ActionButton(title: "Fund skill training", icon: "grad") {
                            showingTraining = true
                        }
                        ActionButton(title: "Fund product quality", icon: "sparkle") {
                            showingQuality = true
                        }
                        if store.loans.isEmpty {
                            EmptyNote(text: "No loans.")
                        } else {
                            ForEach(store.loans, id: \.handle) { loan in
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(loan.lender).font(.callout)
                                    Text("\(Format.money(loan.principalCents)) at \(Format.percentBps(loan.rateBps)) · \(loan.remainingWeeks) weeks\(loan.secured ? " · secured" : "")")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                            }
                        }
                    }
                } else if store.selectedFirmDto?.restricted == true {
                    EmptyNote(text: "This company is private. Management rights or a completed diligence give access to its statements, facilities and contracts.")
                        .accessibilityLabel("Private company statements are restricted")
                } else {
                    EmptyNote(text: "Select a company on the Portfolio tab.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Finance")
        .sheet(isPresented: $showingPrice) { PriceSheet().environment(store) }
        .sheet(isPresented: $showingDividend) { DividendSheet().environment(store) }
        .sheet(isPresented: $showingIssue) { IssueSharesSheet().environment(store) }
        .sheet(isPresented: $showingRepurchase) { RepurchaseSheet().environment(store) }
        .sheet(isPresented: $showingRetire) { RetireTreasurySheet().environment(store) }
        .sheet(isPresented: $showingIPO) { IPOSheet().environment(store) }
        .sheet(isPresented: $showingLoan) { LoanSheet().environment(store) }
        .sheet(isPresented: $showingBond) { BondSheet().environment(store) }
        .sheet(isPresented: $showingTraining) { TrainingBudgetSheet().environment(store) }
        .sheet(isPresented: $showingQuality) { QualityBudgetSheet().environment(store) }
    }
}

// MARK: - Ownership

struct OwnershipView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingNominate = false
    @State private var showingBoardAction = false
    @State private var showingShareOrder = false
    @State private var castingVote: BoardVoteDto?
    @State private var confirm: ConfirmRequest?

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                if let ownership = store.ownership, let firm = store.selectedFirmDto {
                    SectionCard(title: "Votes and control") {
                        DetailRow(title: "Issued shares", value: ownership.issued.formatted())
                        DetailRow(title: "Treasury", value: ownership.treasury.formatted())
                        DetailRow(title: "Eligible votes", value: ownership.eligibleVotes.formatted())
                        DetailRow(title: "Controller", value: ownership.controller ?? "None")
                        DetailRow(title: "Your votes", value: Format.percentBps(firm.investorVotesBps))
                        DetailRow(title: "CEO", value: ownership.ceo)
                        DetailRow(title: "Next meeting", value: "Week \(ownership.nextMeeting)")
                        Text("Control needs strictly more than half of eligible votes. Treasury shares have no votes.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    SectionCard(title: "Holders") {
                        ForEach(ownership.holders, id: \.holder) { holder in
                            VStack(alignment: .leading, spacing: 2) {
                                HStack(alignment: .firstTextBaseline) {
                                    Text(holder.holder).font(.callout)
                                    Spacer(minLength: 8)
                                    Text(holder.quantity.formatted()).font(.caption.monospacedDigit())
                                    Text(Format.percentBps(holder.votesBps))
                                        .font(.caption.monospacedDigit())
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                }
                                Text("Cost basis \(Format.money(holder.costCents))")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                            }
                        }
                    }
                    SectionCard(title: "Board") {
                        ForEach(ownership.boardSeats.indices, id: \.self) { index in
                            DetailRow(title: "Seat \(index + 1)", value: ownership.boardSeats[index])
                        }
                        Text("A proposal passes with three of five seats. NPC directors support solvent proposals; a tie keeps the status quo.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    SectionCard(title: "Governance actions") {
                        ActionButton(title: "Call an extraordinary meeting", icon: "calendar") {
                            confirm = ConfirmRequest(
                                title: "Call meeting",
                                actorName: firm.name,
                                detail: "Needs more than half the eligible votes. Moves the next meeting to four weeks from now.",
                                submitTitle: "Call meeting",
                                json: CommandJSON.command("CallMeeting", ["firm": firm.handle])
                            )
                        }
                        ActionButton(title: "Nominate a CEO", icon: "user") {
                            showingNominate = true
                        }
                        ActionButton(title: "Propose a board action", icon: "doc") {
                            showingBoardAction = true
                        }
                        ActionButton(title: "Place a share order", icon: "swap") {
                            showingShareOrder = true
                        }
                    }
                    SectionCard(title: "Pending votes") {
                        if store.boardVotes.isEmpty {
                            EmptyNote(text: "No pending board votes. Propose an action to open one.")
                        } else {
                            ForEach(store.boardVotes, id: \.id) { vote in
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(vote.subject.replacingOccurrences(of: "_", with: " ").capitalized)
                                        .font(.callout.weight(.medium))
                                    Text("Deadline week \(vote.deadline) · \(vote.yes) yes · \(vote.no) no\(vote.resolved ? (vote.passed ? " · passed" : " · failed") : "")")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                        .fixedSize(horizontal: false, vertical: true)
                                    if !vote.resolved {
                                        HStack {
                                            Button("Vote yes") { castingVote = vote }
                                                .buttonStyle(PixelButtonStyle())
                                                .frame(minHeight: 44)
                                            Button("Vote no") { castingVote = vote }
                                                .buttonStyle(PixelButtonStyle())
                                                .frame(minHeight: 44)
                                        }
                                        .font(.caption)
                                    }
                                }
                                .frame(minHeight: 44, alignment: .leading)
                            }
                        }
                    }
                    SectionCard(title: "Tender responses") {
                        let open = store.deals.filter { !terminal($0.state) }
                        if open.isEmpty {
                            EmptyNote(text: "No open tender addressed to you. Bids appear in Deals.")
                        } else {
                            ForEach(open, id: \.handle) { deal in
                                VStack(alignment: .leading, spacing: 4) {
                                    Text("\(deal.buyer) → \(deal.target)")
                                        .font(.callout)
                                    Text("\(deal.state.replacingOccurrences(of: "_", with: " ")) · \(Format.money(deal.offerPriceCents)) per share")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                    HStack {
                                        Button("Tender my shares") {
                                            confirm = ConfirmRequest(
                                                title: "Respond to tender",
                                                actorName: "You, the investor",
                                                detail: "Tenders every share you hold in \(deal.target) at \(Format.money(deal.offerPriceCents)) per share and moves them to the bidder.",
                                                submitTitle: "Tender",
                                                json: CommandJSON.command("RespondTender", [
                                                    "deal": deal.handle,
                                                    "accept": true
                                                ]),
                                                asInvestor: true
                                            )
                                        }
                                        .buttonStyle(PixelButtonStyle())
                                        .frame(minHeight: 44)
                                        Button("Decline") {
                                            confirm = ConfirmRequest(
                                                title: "Decline tender",
                                                actorName: "You, the investor",
                                                detail: "Declining changes nothing and keeps your shares in \(deal.target).",
                                                submitTitle: "Decline",
                                                json: CommandJSON.command("RespondTender", [
                                                    "deal": deal.handle,
                                                    "accept": false
                                                ]),
                                                asInvestor: true
                                            )
                                        }
                                        .buttonStyle(PixelButtonStyle())
                                        .frame(minHeight: 44)
                                    }
                                    .font(.caption)
                                }
                                .frame(minHeight: 44, alignment: .leading)
                            }
                        }
                    }
                } else {
                    EmptyNote(text: "No company selected.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Ownership")
        .sheet(isPresented: $showingNominate) { NominateCEOSheet().environment(store) }
        .sheet(isPresented: $showingBoardAction) { BoardActionSheet().environment(store) }
        .sheet(isPresented: $showingShareOrder) { ShareOrderSheet().environment(store) }
        .sheet(item: $castingVote) { vote in
            CastVoteSheet(vote: vote).environment(store)
        }
        .sheet(item: $confirm) { request in
            ConfirmCommandSheet(request: request).environment(store)
        }
    }

    private func terminal(_ state: String) -> Bool {
        ["settled", "withdrawn", "expired", "rejected"].contains(state)
    }
}

// MARK: - Facilities

struct FacilitiesView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var showingBuild = false
    @State private var targetPercent: Double = 85
    @State private var showingPrice = false
    @State private var showingWage = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                if store.facilities.isEmpty {
                    SectionCard(title: "Facilities") {
                        EmptyNote(text: "No facilities yet. Build one from a region and a template; the form previews the escrow before you confirm.")
                        ActionButton(title: "Build a facility", icon: "hammer") {
                            showingBuild = true
                        }
                    }
                } else {
                    ForEach(store.facilities, id: \.handle) { facility in
                        SectionCard(title: facility.recipe.capitalized) {
                            HStack {
                                Text(facility.region).font(.callout)
                                Spacer()
                                Text(facility.status).font(.caption).foregroundStyle(Theme.secondaryText(for: scheme))
                            }
                            HStack(spacing: 12) {
                                MetricTile(title: "Output", value: Format.lots(facility.lastOutputLots))
                                MetricTile(title: "Capacity", value: Format.lots(facility.capacityLots))
                            }
                            Text("Workers \(facility.workersLow) low · \(facility.workersMid) mid · \(facility.workersHigh) high")
                                .font(.caption.monospacedDigit())
                                .fixedSize(horizontal: false, vertical: true)
                            Text("Bottleneck: \(facility.bottleneck)")
                                .font(.caption)
                                .foregroundStyle(Theme.warning)
                            HStack {
                                Button("Price") { showingPrice = true }
                                    .buttonStyle(PixelButtonStyle())
                                    .frame(minHeight: 44)
                                Button("Wages") { showingWage = true }
                                    .buttonStyle(PixelButtonStyle())
                                    .frame(minHeight: 44)
                            }
                            .frame(minHeight: 44)
                        }
                    }
                    SectionCard(title: "Production target") {
                        let capacity = store.facilities.reduce(Int64(0)) { $0 + $1.capacityLots }
                        ValueSlider(
                            value: $targetPercent, range: 0...100, step: 1, ticks: 20,
                            label: { String(format: "%.0f%% of capacity", $0) },
                            detail: { "About \(Int64(Double(capacity) * $0 / 100).formatted()) of \(capacity.formatted()) lots a week. Lower targets save inputs and wages when stock is piling up." },
                            onCommit: { value in Task { await store.setTarget(Int32(value * 100)) } }
                        )
                        .onAppear { targetPercent = Double(store.facilities.first?.targetBps ?? 8_500) / 100 }
                        ActionButton(title: "Build another facility", icon: "hammer") {
                            showingBuild = true
                        }
                    }
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Facilities")
        .sheet(isPresented: $showingBuild) { BuildSheet().environment(store) }
        .sheet(isPresented: $showingPrice) { PriceSheet().environment(store) }
        .sheet(isPresented: $showingWage) { WageSheet().environment(store) }
    }
}

// MARK: - Supply

struct SupplyView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
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
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                                Text("\(contract.fromRegion) to \(contract.toRegion) · \(contract.remainingWeeks) weeks left")
                                    .font(.caption2)
                                    .foregroundStyle(Theme.secondaryText(for: scheme))
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
                SectionCard(title: "Spot purchases") {
                    EmptyNote(text: "Spot orders reserve cash, pay freight and arrive after the lane lead time. Purchases are never usable for production in the same week.")
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Supply")
    }
}

// MARK: - Group

struct GroupView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.colorScheme) private var scheme
    @State private var confirm: ConfirmRequest?

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                ContextChip()
                SectionCard(title: "Controlled group") {
                    if store.groupTree.isEmpty {
                        EmptyNote(text: "You control no company. Own more than half the votes to add a firm to this tree.")
                    } else {
                        ForEach(store.groupTree, id: \.handle) { node in
                            Button {
                                Task { await store.selectFirm(node.handle) }
                            } label: {
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                                        if node.depth > 0 {
                                            PixelIcon("indent", size: 12)
                                                .foregroundStyle(Theme.secondaryText(for: scheme))
                                                .accessibilityHidden(true)
                                        }
                                        Text(node.name)
                                            .font(.body.weight(.medium))
                                            .fixedSize(horizontal: false, vertical: true)
                                        if store.selectedFirm == node.handle {
                                            PixelIcon("check_circle", size: 12)
                                                .foregroundStyle(Theme.positive)
                                        }
                                    }
                                    Text("\(node.industry) · \(node.region) · \(node.directlyControlled ? "direct holding" : "subsidiary") · control \(Format.percentBps(node.controlBps))")
                                        .font(.caption)
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                        .fixedSize(horizontal: false, vertical: true)
                                    Text("Look-through \(Format.percentBps(node.lookThroughBps)) · cash \(Format.moneyCompact(node.cashCents))")
                                        .font(.caption2.monospacedDigit())
                                        .foregroundStyle(Theme.secondaryText(for: scheme))
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .frame(minHeight: 44, alignment: .leading)
                                .padding(.leading, CGFloat(min(Int(node.depth), 4)) * 12)
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel("\(node.name), \(node.directlyControlled ? "direct holding" : "subsidiary"), control \(Format.percentBps(node.controlBps))")
                        }
                    }
                }
                if let firm = store.selectedFirmDto {
                    SectionCard(title: "Primary group") {
                        Text("The primary group is the default operating context for Portfolio and overview figures. It does not change who owns what.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                        ActionButton(title: "Make \(firm.name) the primary group", icon: "star") {
                            confirm = ConfirmRequest(
                                title: "Set primary group",
                                actorName: "You, the investor",
                                detail: "Switches the operating context to \(firm.name). Ownership and control do not change.",
                                submitTitle: "Set primary group",
                                json: CommandJSON.command("SetPrimaryGroup", ["firm": firm.handle]),
                                asInvestor: true
                            )
                        }
                    }
                }
            }
            .padding(16)
        }
        .background(Theme.background(for: scheme))
        .navigationTitle("Group")
        .sheet(item: $confirm) { request in
            ConfirmCommandSheet(request: request).environment(store)
        }
    }
}
