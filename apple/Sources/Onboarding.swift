import Foundation
import SwiftUI

/// Keys shared by the app shell, the Start screen and the walkthrough itself.
enum OnboardingKeys {
    static let completed = "cw.onboarding.completed"
    static let step = "cw.onboarding.step"
}

struct GuidedStep: Identifiable {
    let id: Int
    let title: String
    let explanation: String
}

/// Seven-step guided start. Every number comes from the live session DTOs and every action goes
/// through the same AppStore method a normal screen uses; nothing here is tutorial-only state.
struct OnboardingView: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @AppStorage(OnboardingKeys.completed) private var completed = false
    @AppStorage(OnboardingKeys.step) private var step = 0

    @State private var showingBridge = false
    @State private var showingBuild = false
    @State private var wageTier: UInt32 = 1
    @State private var priceProduct: UInt32 = 0
    @State private var priceText = ""

    private let steps: [GuidedStep] = [
        GuidedStep(
            id: 0,
            title: "You act through a company",
            explanation: "You are the investor: you own shares, and each company is a separate legal wallet. Cash in your pocket and cash inside the company are never the same number. Pick the company you want to run; every order in this walkthrough spends that company's money."
        ),
        GuidedStep(
            id: 1,
            title: "Cash is not profit",
            explanation: "Cash is what the company has; profit is what it earned. Buying a facility moves cash into an asset instead of an expense, while depreciation lowers profit without spending cash. Open the bridge to reconcile this week's opening cash to closing cash."
        ),
        GuidedStep(
            id: 2,
            title: "A facility has one bottleneck",
            explanation: "A facility turns workers and inputs into output, and output stops at the first limit, staffing, inputs or the production target. The company pays payroll, inputs, rent and maintenance from its own cash, so add capacity only when that cash can fund the capex."
        ),
        GuidedStep(
            id: 3,
            title: "Wages decide who turns up",
            explanation: "A wage offer is a weekly promise to every worker in that tier. Workers accept when the offer clears their reservation wage, so a higher offer fills vacancies faster and costs more every week. There is no separate hire button; hiring follows the wage."
        ),
        GuidedStep(
            id: 4,
            title: "A price is a demand decision",
            explanation: "Your asking price is per lot. A higher price earns more per sale but loses buyers to rivals and the gateway, and unsold stock stays on the books. Read last week's sale, the regional stock and the gateway prices before you move it."
        ),
        GuidedStep(
            id: 5,
            title: "One week, settled",
            explanation: "Advancing one week settles payroll, input use, production, sales, freight and tax in a fixed order. The figures afterwards are settled amounts, not projections. A pause means a decision is required before time can pass again."
        ),
        GuidedStep(
            id: 6,
            title: "Your week is saved",
            explanation: "Every advance autosaves the committed week, and the previous copy of that slot is kept as last-good, so a failed write cannot erase your last valid week. A manual save adds a named slot; the Start screen lists saves newest first."
        )
    ]

    private var stepIndex: Int { min(max(step, 0), steps.count - 1) }
    private var stepData: GuidedStep { steps[stepIndex] }
    private var isLastStep: Bool { stepIndex == steps.count - 1 }
    private var leadingTitle: String { stepIndex == 0 ? "Close" : "Back" }
    private var leadingLabel: String { stepIndex == 0 ? "Close walkthrough" : "Previous step" }
    private var leadingHint: String {
        stepIndex == 0
            ? "Closes the walkthrough. It resumes at this step next time."
            : "Returns to the previous step."
    }
    private var nextTitle: String { isLastStep ? "Finish" : "Next step" }
    private var stepAnimation: Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.18)
    }
    private var nextHint: String {
        isLastStep
            ? "Closes the walkthrough and stops it opening automatically."
            : "Shows the next step."
    }
    private var canAct: Bool { store.sessionReady && store.selectedFirm != nil }
    private var firmRegion: RegionDto? {
        guard let name = store.selectedFirmDto?.region else { return nil }
        return store.regions.first { $0.name == name }
    }
    private var marketRow: AppStore.MarketRow? {
        store.marketRows.first { $0.product.index == priceProduct }
    }
    private var askingPrice: Int64? {
        let text = priceText.replacingOccurrences(of: ",", with: ".")
        guard let dollars = Double(text), dollars > 0 else { return nil }
        let cents = Int64((dollars * 100).rounded())
        return cents > 0 ? cents : nil
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    progressHeader
                    if !canAct {
                        previewNotice
                    }
                    SectionCard(title: stepData.title) {
                        Text(stepData.explanation)
                            .font(.body)
                            .foregroundStyle(Theme.text(for: scheme))
                            .fixedSize(horizontal: false, vertical: true)
                        Divider()
                        stepAction
                    }
                    .id(stepIndex)
                    .transition(.opacity)
                }
                .padding(16)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .safeAreaInset(edge: .bottom) {
                nextBar
            }
            .background(Theme.background(for: scheme))
            .animation(stepAnimation, value: stepIndex)
            .navigationTitle("Guided start")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(leadingTitle) {
                        if stepIndex == 0 {
                            close()
                        } else {
                            back()
                        }
                    }
                    .accessibilityLabel(leadingLabel)
                    .accessibilityHint(leadingHint)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Skip") { skip() }
                        .accessibilityLabel("Skip walkthrough")
                        .accessibilityHint("Stops the walkthrough opening automatically. You can replay it from Start, About this build.")
                }
            }
            .onAppear { syncPriceSelection() }
            .onChange(of: store.marketRows.count) { _, _ in syncPriceSelection() }
            .onChange(of: stepIndex) { _, newValue in
                if newValue == 4 { syncPriceSelection() }
            }
            .sheet(isPresented: $showingBridge) {
                CashBridgeView()
                    .environmentObject(store)
            }
            .sheet(isPresented: $showingBuild) {
                BuildSheet()
                    .environmentObject(store)
            }
        }
    }

    // MARK: Chrome

    private var progressHeader: some View {
        VStack(alignment: .leading, spacing: 6) {
            ProgressView(value: Double(stepIndex + 1), total: Double(steps.count))
                .accessibilityHidden(true)
            Text("Step \(stepIndex + 1) of \(steps.count)")
                .font(.caption)
                .foregroundStyle(Theme.secondaryText(for: scheme))
                .accessibilityLabel("Step \(stepIndex + 1) of \(steps.count): \(stepData.title)")
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var previewNotice: some View {
        HStack(alignment: .top, spacing: 8) {
            PixelIcon("info")
                .accessibilityHidden(true)
            Text("This is a preview. Start or resume a guided career to run these steps against your own company and real numbers.")
                .font(.footnote)
                .fixedSize(horizontal: false, vertical: true)
        }
        .foregroundStyle(Theme.secondaryText(for: scheme))
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.surface(for: scheme))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .accessibilityElement(children: .combine)
    }

    private var nextBar: some View {
        VStack(spacing: 0) {
            Divider()
            Button {
                advanceStep()
            } label: {
                PixelLabel(
                    nextTitle,
                    icon: isLastStep ? "check" : "chevron_right"
                )
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .accessibilityHint(nextHint)
        }
        .background(Theme.background(for: scheme))
    }

    // MARK: Steps

    @ViewBuilder
    private var stepAction: some View {
        switch stepIndex {
        case 0: identityAction
        case 1: cashAction
        case 2: facilityAction
        case 3: wageAction
        case 4: priceAction
        case 5: advanceAction
        default: saveAction
        }
    }

    @ViewBuilder
    private var identityAction: some View {
        if let overview = store.overview {
            valueRow("Your personal cash", Format.money(overview.investorCashCents))
            valueRow("\(overview.primaryName) cash", Format.money(overview.firmCashCents, currency: overview.primaryCurrency))
        }
        if store.controlledFirms.isEmpty {
            EmptyNote(text: "You do not control a company yet. Start a guided career to follow the whole walkthrough with a running firm.")
        } else {
            ForEach(store.controlledFirms, id: \.handle) { firm in
                Button {
                    Task { await store.selectFirm(firm.handle) }
                } label: {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(firm.name)
                                .font(.body.weight(.medium))
                                .fixedSize(horizontal: false, vertical: true)
                            Text("\(firm.industry) · \(firm.region)")
                                .font(.caption)
                                .foregroundStyle(Theme.secondaryText(for: scheme))
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer(minLength: 8)
                        if store.selectedFirm == firm.handle {
                            PixelLabel("Running", icon: "check_circle")
                                .font(.caption)
                                .foregroundStyle(Theme.positive)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.bordered)
                .accessibilityLabel("Run \(firm.name) in \(firm.region)")
                .accessibilityHint("Makes this company the context for every order in the walkthrough.")
            }
        }
    }

    @ViewBuilder
    private var cashAction: some View {
        if let overview = store.overview {
            valueRow("Company cash now", Format.money(overview.firmCashCents, currency: overview.primaryCurrency))
            valueRow(
                "Last week's EBIT",
                Format.signedMoney(overview.lastWeekEbitCents),
                tone: overview.lastWeekEbitCents >= 0 ? 1 : -1
            )
        }
        Button {
            showingBridge = true
        } label: {
            PixelLabel("Open the cash bridge", icon: "swap")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
        .disabled(!canAct)
        .accessibilityHint("Shows opening cash, receipts, operating payments, investment, financing and tax reconciled to closing cash.")
    }

    @ViewBuilder
    private var facilityAction: some View {
        if let facility = store.facilities.first {
            valueRow("Facility", facility.recipe.capitalized)
            valueRow("Region", facility.region)
            valueRow("Last output", "\(facility.lastOutputLots.formatted()) of \(facility.capacityLots.formatted()) lots a week")
            valueRow("Workers", "\(facility.workersLow) low · \(facility.workersMid) mid · \(facility.workersHigh) high")
            valueRow("Bottleneck", bottleneckText(facility.bottleneck))
        } else if canAct {
            EmptyNote(text: "No facility yet. The build form creates one from a region and a template.")
        }
        // The FFI publishes the recipe list, so the form shows every template with its capex and
        // build time instead of a raw index that could drift away from the dataset.
        Button {
            showingBuild = true
        } label: {
            PixelLabel("Open the build form", icon: "hammer")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
        .disabled(!canAct)
        .accessibilityHint("Opens the real facility build form. Choose a template and check the capex before you confirm.")
        Text("Building spends the company's cash, not yours. The form opens on this company's region and previews the construction escrow before you confirm.")
            .font(.caption)
            .foregroundStyle(Theme.secondaryText(for: scheme))
            .fixedSize(horizontal: false, vertical: true)
    }

    @ViewBuilder
    private var wageAction: some View {
        if let region = firmRegion {
            Picker("Skill tier", selection: $wageTier) {
                Text("Low · \(Format.money(region.wageLowCents)) a week").tag(UInt32(0))
                Text("Mid · \(Format.money(region.wageMidCents)) a week").tag(UInt32(1))
                Text("High · \(Format.money(region.wageHighCents)) a week").tag(UInt32(2))
            }
            .pickerStyle(.menu)
            .accessibilityLabel("Skill tier")
            .accessibilityValue("\(tierName(wageTier)), \(Format.money(tierWage(wageTier) ?? 0)) a week")

            Button {
                guard let wage = tierWage(wageTier) else { return }
                Task { await store.setWage(tier: wageTier, wage: wage) }
            } label: {
                PixelLabel("Offer \(Format.money(tierWage(wageTier) ?? 0)) a week", icon: "user_add")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(!canAct || tierWage(wageTier) == nil)
            .accessibilityHint("Sends this weekly wage offer for the selected tier to the company.")
        } else {
            EmptyNote(text: "Select a company to read its region's prevailing wages.")
        }
        if !store.facilities.isEmpty {
            valueRow(
                "Employed now",
                "\(workersLow) low · \(workersMid) mid · \(workersHigh) high"
            )
        }
    }

    @ViewBuilder
    private var priceAction: some View {
        if store.marketRows.isEmpty {
            EmptyNote(text: "No markets loaded. Open a career to read regional prices and stock.")
        } else {
            Picker("Product", selection: $priceProduct) {
                ForEach(store.marketRows) { row in
                    Text("\(row.product.name) · ref \(Format.money(row.product.referencePriceCents))")
                        .tag(row.product.index)
                }
            }
            .pickerStyle(.menu)
            .accessibilityLabel("Product")
            .accessibilityValue(marketRow?.product.name ?? "None selected")

            TextField("Price in dollars", text: $priceText)
                .textFieldStyle(.roundedBorder)
                .keyboardType(.decimalPad)
                .accessibilityLabel("Asking price in dollars")
                .accessibilityHint("Cents per lot are calculated from this dollar amount.")

            Button {
                if let cents = askingPrice {
                    Task { await store.setPrice(product: priceProduct, price: cents) }
                }
            } label: {
                PixelLabel("Set asking price", icon: "tag")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(!canAct || askingPrice == nil)
            .accessibilityHint("Submits the price for the selected product through the real command path.")

            if let market = marketRow?.market {
                Divider()
                valueRow("Reference price", Format.money(market.referencePriceCents))
                valueRow("Last sale", Format.money(market.lastPriceCents))
                valueRow("Regional stock", Format.lots(market.regionalStockLots))
                valueRow("Sold last week", Format.lots(market.lastWeekSalesLots))
                valueRow("Gateway buys at", Format.money(market.gatewayBuyCents))
                valueRow("Gateway sells at", Format.money(market.gatewaySellCents))
            } else {
                Text("The chosen product has no market snapshot in the selected region.")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    @ViewBuilder
    private var advanceAction: some View {
        if let overview = store.overview {
            valueRow("Current week", overview.weekLabel)
            valueRow("Company cash", Format.money(overview.firmCashCents, currency: overview.primaryCurrency))
            valueRow("Four-week forecast", Format.money(overview.forecastFourWeekCents, currency: overview.primaryCurrency))
            if let pause = overview.pause {
                PixelLabel("Paused: \(pause)", icon: "hand")
                    .font(.callout)
                    .foregroundStyle(Theme.warning)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        Button {
            Task { await store.advance(weeks: 1) }
        } label: {
            PixelLabel("Advance one week", icon: "next_week")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .disabled(!canAct || store.isBusy)
        .accessibilityHint("Settles one week of the real rules and reports the settled result below.")

        if let week = store.finance?.recentWeeks.last {
            Divider()
            valueRow("Last closed week", "Week \(week.week)")
            valueRow("Revenue", Format.money(week.revenueCents))
            valueRow("EBIT", Format.signedMoney(week.ebitCents), tone: week.ebitCents >= 0 ? 1 : -1)
        }
        statusLine
    }

    @ViewBuilder
    private var saveAction: some View {
        Button {
            Task { await store.manualSave() }
        } label: {
            PixelLabel("Save now", icon: "save")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .disabled(!canAct || store.isBusy)
        .accessibilityHint("Writes a manual save slot you can resume from the Start screen.")

        if store.saves.isEmpty {
            EmptyNote(text: "No saves yet. The career autosaves after each advance.")
        } else {
            ForEach(Array(store.saves.prefix(3))) { save in
                VStack(alignment: .leading, spacing: 2) {
                    Text(save.slot)
                        .font(.callout)
                        .fixedSize(horizontal: false, vertical: true)
                    Text("Week \(save.week) · \(save.savedAt.formatted(date: .abbreviated, time: .shortened))")
                        .font(.caption)
                        .foregroundStyle(Theme.secondaryText(for: scheme))
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityElement(children: .combine)
            }
        }
        statusLine
    }

    @ViewBuilder
    private var statusLine: some View {
        if !store.statusLine.isEmpty {
            Text(store.statusLine)
                .font(.footnote)
                .foregroundStyle(Theme.secondaryText(for: scheme))
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: Helpers

    private var workersLow: Int64 { store.facilities.reduce(Int64(0)) { $0 + $1.workersLow } }
    private var workersMid: Int64 { store.facilities.reduce(Int64(0)) { $0 + $1.workersMid } }
    private var workersHigh: Int64 { store.facilities.reduce(Int64(0)) { $0 + $1.workersHigh } }

    private func tierWage(_ tier: UInt32) -> Int64? {
        guard let region = firmRegion else { return nil }
        switch tier {
        case 0: return region.wageLowCents
        case 1: return region.wageMidCents
        case 2: return region.wageHighCents
        default: return nil
        }
    }

    private func tierName(_ tier: UInt32) -> String {
        switch tier {
        case 0: return "Low"
        case 1: return "Mid"
        case 2: return "High"
        default: return "Tier \(tier)"
        }
    }

    private func referencePrice(_ index: UInt32) -> Int64 {
        store.marketRows.first { $0.product.index == index }?.product.referencePriceCents ?? 0
    }

    private func bottleneckText(_ key: String) -> String {
        key == "none" ? "No binding limit" : key.capitalized
    }

    private func dollarsText(_ cents: Int64) -> String {
        String(format: "%.2f", Double(cents) / 100.0)
    }

    private func syncPriceSelection() {
        guard !store.marketRows.isEmpty else { return }
        if !store.marketRows.contains(where: { $0.product.index == priceProduct }) {
            priceProduct = store.marketRows[0].product.index
        }
        if priceText.isEmpty {
            priceText = dollarsText(referencePrice(priceProduct))
        }
    }

    private func valueRow(_ title: String, _ value: String, tone: Int = 0) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title)
                .font(.callout)
                .foregroundStyle(Theme.secondaryText(for: scheme))
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 8)
            Text(value)
                .font(.callout.monospacedDigit())
                .foregroundStyle(toneColor(tone))
                .multilineTextAlignment(.trailing)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }

    private func toneColor(_ tone: Int) -> Color {
        switch tone {
        case 1: return Theme.positive
        case -1: return Theme.negative
        default: return Theme.text(for: scheme)
        }
    }

    // MARK: Navigation

    private func back() {
        step = max(0, stepIndex - 1)
    }

    private func advanceStep() {
        if stepIndex < steps.count - 1 {
            step = stepIndex + 1
        } else {
            finish()
        }
    }

    private func close() {
        dismiss()
    }

    private func skip() {
        completed = true
        step = 0
        dismiss()
    }

    private func finish() {
        completed = true
        step = 0
        dismiss()
    }
}
