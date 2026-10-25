import Foundation
import SwiftUI

@MainActor
final class AppStore: ObservableObject {
    @Published var sessionReady = false
    @Published var overview: OverviewDto?
    @Published var firms: [FirmDto] = []
    @Published var regions: [RegionDto] = []
    @Published var facilities: [FacilityDto] = []
    @Published var finance: FinanceDto?
    @Published var ownership: OwnershipDto?
    @Published var contracts: [ContractDto] = []
    @Published var loans: [LoanDto] = []
    @Published var insurance: [InsuranceDto] = []
    @Published var deals: [DealDto] = []
    @Published var events: [EventDto] = []
    @Published var selectedFirm: UInt64?
    @Published var selectedRegionCode: String?
    @Published var errorMessage: String?
    @Published var statusLine: String = ""
    @Published var isBusy = false
    @Published var runSpeed: Double = 0
    @Published var saves: [StoredSave] = []
    @Published var marketRows: [MarketRow] = []

    private let engine = Engine()
    private var runTask: Task<Void, Never>?

    struct MarketRow: Identifiable {
        let product: ProductDto
        let market: MarketDto?
        var id: UInt32 { product.index }
    }

    var selectedFirmDto: FirmDto? {
        guard let selectedFirm else { return nil }
        return firms.first { $0.handle == selectedFirm }
    }

    var controlledFirms: [FirmDto] {
        firms.filter { $0.controlled }
    }

    func restoreLatestIfAvailable() async {
        saves = SaveManager.shared.list()
        guard let latest = saves.first else { return }
        await resume(latest, silent: true)
    }

    func newCareer(guided: Bool, fiveYear: Bool, seed: UInt64) async {
        await perform {
            do {
                let overview = try await self.engine.start(seed: seed, guided: guided, fiveYear: fiveYear)
                self.overview = overview
                self.selectedFirm = overview.primaryFirm
                self.sessionReady = true
                await self.reload()
                await self.autosave(slot: "autosave-\(overview.tick)")
            } catch {
                self.errorMessage = error.localizedDescription
            }
        }
    }

    func resume(_ save: StoredSave, silent: Bool = false) async {
        do {
            let data = try SaveManager.shared.load(save)
            let overview = try await engine.loadSave(data)
            self.overview = overview
            self.selectedFirm = overview.primaryFirm
            self.sessionReady = true
            await reload()
            statusLine = "Resumed \(save.slot) at week \(overview.tick)."
        } catch {
            if !silent {
                errorMessage = error.localizedDescription
            }
        }
    }

    func advance(weeks: UInt32) async {
        guard sessionReady else { return }
        await perform {
            if let result = await self.engine.advance(weeks: weeks) {
                if let pause = result.pause {
                    self.statusLine = "Paused: \(pause)"
                } else {
                    self.statusLine = "Advanced \(result.weeksAdvanced) week(s)."
                }
            }
            await self.reload()
            if let overview = self.overview {
                await self.autosave(slot: "autosave-\(overview.tick / 4)")
            }
        }
    }

    func toggleRun() {
        if runTask != nil {
            runTask?.cancel()
            runTask = nil
            runSpeed = 0
            return
        }
        runSpeed = 1
        runTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                await self.advance(weeks: 1)
                if self.overview?.pause != nil { break }
                try? await Task.sleep(nanoseconds: 1_000_000_000)
            }
            await MainActor.run {
                self.runTask = nil
                self.runSpeed = 0
            }
        }
    }

    func reload() async {
        overview = await engine.overview()
        firms = await engine.firms()
        regions = await engine.regions(selected: selectedRegionCode)
        events = await engine.events()
        deals = await engine.deals()
        if let selectedFirm {
            facilities = await engine.facilities(firm: selectedFirm)
            finance = await engine.finance(firm: selectedFirm)
            ownership = await engine.ownership(firm: selectedFirm)
            contracts = await engine.contracts(firm: selectedFirm)
            loans = await engine.loans(firm: selectedFirm)
            insurance = await engine.insurance(firm: selectedFirm)
        } else {
            facilities = []
            finance = nil
            ownership = nil
            contracts = []
            loans = []
            insurance = []
        }
        saves = SaveManager.shared.list()
        await loadMarkets()
    }

    func loadMarkets() async {
        let regionIndex = regions.first { $0.code == selectedRegionCode }?.index ?? regions.first?.index ?? 0
        var rows: [MarketRow] = []
        for product in products() {
            let market = await engine.market(region: regionIndex, product: product.index)
            rows.append(MarketRow(product: product, market: market))
        }
        marketRows = rows
    }

    func selectFirm(_ handle: UInt64) async {
        selectedFirm = handle
        await reload()
    }

    func selectRegion(_ code: String?) async {
        selectedRegionCode = code
        regions = await engine.regions(selected: code)
    }

    func manualSave() async {
        guard let overview else { return }
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd-HHmm"
        let slot = "manual-\(formatter.string(from: Date()))"
        await autosave(slot: slot)
        statusLine = "Saved \(slot)."
    }

    private func autosave(slot: String) async {
        guard sessionReady else { return }
        do {
            let data = try await engine.exportSave()
            _ = try SaveManager.shared.write(data, slot: slot, week: overview?.tick ?? 0)
        } catch {
            statusLine = "Save failed: \(error.localizedDescription)"
        }
    }

    // MARK: Actions

    func setPrice(product: UInt32, price: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.setPrice(firm: selectedFirm, product: product, price: price)
            self.handle(receipt)
            await self.reload()
        }
    }

    func setWage(tier: UInt32, wage: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.setWage(firm: selectedFirm, tier: tier, wage: wage)
            self.handle(receipt)
            await self.reload()
        }
    }

    func setTarget(_ bps: Int32) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.setTarget(firm: selectedFirm, bps: bps)
            self.handle(receipt)
            await self.reload()
        }
    }

    func build(region: UInt32, recipe: UInt32) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.build(firm: selectedFirm, region: region, recipe: recipe)
            self.handle(receipt)
            await self.reload()
        }
    }

    func payDividend(amount: Int64) async {
        guard let selectedFirm else { return }
        await perform {
            let receipt = await self.engine.dividend(firm: selectedFirm, amount: amount)
            self.handle(receipt)
            await self.reload()
        }
    }

    func respond(event: UInt64, choice: String) async {
        await perform {
            let json = "{\"RespondEvent\":{\"event\":\(event),\"choice\":\"\(choice)\"}}"
            let receipt = await self.engine.submit(json: json)
            self.handle(receipt)
            await self.reload()
        }
    }

    func orderShares(firm: UInt64, buy: Bool, quantity: Int64, limit: Int64) async {
        await perform {
            let json = "{\"PlaceShareOrder\":{\"investor\":true,\"firm\":\(firm),\"buy\":\(buy),\"qty\":\(quantity),\"limit_price_cents\":\(limit)}}"
            let receipt = await self.engine.submit(json: json)
            self.handle(receipt)
            await self.reload()
        }
    }

    private func handle(_ receipt: ReceiptDto?) {
        guard let receipt else { return }
        if receipt.accepted {
            statusLine = "Accepted at revision \(receipt.revision)."
            errorMessage = nil
        } else {
            errorMessage = receipt.errorKey ?? "Rejected"
            statusLine = receipt.errorDetail ?? ""
        }
    }

    private func perform(_ block: @escaping () async -> Void) async {
        isBusy = true
        await block()
        isBusy = false
    }
}
