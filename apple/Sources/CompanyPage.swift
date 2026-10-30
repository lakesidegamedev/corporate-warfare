import SwiftUI

/// Generated 16 by 16 logos, composed by the engine from the firm's name so they look the same on every platform.
enum FirmLogos {
    private static var cache: [String: (palette: [String: String], rows: [String])] = [:]
    private static let lock = NSLock()

    static func art(_ name: String) -> (palette: [String: String], rows: [String]) {
        lock.lock()
        defer { lock.unlock() }
        if let hit = cache[name] { return hit }
        let logo = firmLogo(name: name)
        let art = (palette: ["p": logo.plate, "e": logo.edge, "s": logo.symbol], rows: logo.rows)
        cache[name] = art
        return art
    }
}

/// A firm's logo as exact pixels.
struct FirmLogo: View {
    let name: String
    var size: CGFloat = 32

    var body: some View {
        let art = FirmLogos.art(name)
        Canvas { context, area in
            var context = context
            PixelIcons.draw(
                rows: art.rows, palette: art.palette, gridSize: 16,
                in: &context, rect: CGRect(origin: .zero, size: area), tint: nil
            )
        }
        .frame(width: size, height: size)
        .clipShape(Pixel.small)
        .accessibilityHidden(true)
    }
}

/// A logo that opens its company's page when tapped.
struct FirmLogoButton: View {
    let name: String
    let firm: UInt64
    var size: CGFloat = 28
    @State private var open = false

    var body: some View {
        Button { open = true } label: { FirmLogo(name: name, size: size) }
            .buttonStyle(.plain)
            .accessibilityLabel("\(name) company page")
            .sheet(isPresented: $open) { CompanyPageSheet(firm: firm) }
    }
}

/// A company's page: who runs it, how big it is and, by what the investor may see, the rest.
struct CompanyPageBody: View {
    let page: CompanyPageDto
    var onOpenFirm: (UInt64) -> Void = { _ in }
    var onTrade: (() -> Void)? = nil
    /// Set for your own company: a division row opens its detail.
    var onOpenDivision: ((DivisionLineDto) -> Void)? = nil
    @State private var guiding = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                FirmLogo(name: page.name, size: 56)
                VStack(alignment: .leading, spacing: 2) {
                    Text(page.name).font(.headline).foregroundStyle(Theme.primaryText)
                    Text("\(Sector.name(page.industry)) · \(page.homeRegion)").font(.caption).foregroundStyle(Theme.secondary)
                    Text(kindLine).font(.caption.weight(.semibold)).foregroundStyle(Theme.gold)
                }
            }
            if let ceo = page.ceo {
                HStack(alignment: .top, spacing: 10) {
                    PixelPortrait(person: ceo.id, size: 72, mood: ceo.mood, office: ceo.officePortrait)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(ceo.name).font(.callout.weight(.bold)).foregroundStyle(Theme.primaryText)
                        Text("\(ceo.age) · \(ceo.home)").font(.caption).foregroundStyle(Theme.secondary)
                        Text("\(temperamentLabel(ceo.temperament)) · \(page.style)")
                            .font(.caption.weight(.semibold)).foregroundStyle(Theme.gold)
                        Text("Chief executive for \(page.ceoWeeks) weeks").font(.caption2).foregroundStyle(Theme.secondary)
                    }
                }
                Text(temperamentNote(ceo.temperament)).font(.caption).foregroundStyle(Theme.primaryText)
                Text(ceo.background).font(.caption).foregroundStyle(Theme.secondary)
            } else if page.ceoIsYou {
                Text("You run this company.").font(.caption).foregroundStyle(Theme.secondary)
            }
            if let s = page.stock {
                Eyebrow(text: "Shares")
                StatGroup(
                    headline: Stat(label: "Share price", value: Format.price(s.priceCents),
                                   note: "\(Change.text(s.changeWeekBps)) this week"),
                    stats: [
                        Stat(label: "Quarter", value: Change.text(s.changeQuarterBps), tone: Change.tint(s.changeQuarterBps)),
                        Stat(label: "Market cap", value: Format.moneyCompact(s.marketCapCents)),
                        Stat(label: "Fair value", value: Format.price(s.fairCents)),
                        Stat(label: "Free float", value: Format.percentBps(Int32(clamping: s.floatBps))),
                        Stat(label: "You hold", value: s.yourShares.formatted(),
                             note: Format.moneyCompact(s.yourShares * s.priceCents)),
                    ]
                )
                StockHistorySection(firm: page.firm)
                EarningsSection(earnings: s.earnings, onGiveGuidance: page.kind == "yours" ? { guiding = true } : nil)
                    .sheet(isPresented: $guiding) { GuidanceSheet(firm: page.firm, earnings: s.earnings) }
                if let onTrade {
                    Button("Buy or sell shares", action: onTrade).buttonStyle(PixelButtonStyle())
                }
            }
            if page.finance != nil || page.kind == "listed" {
                Eyebrow(text: "Financials")
                CompanyFinancialsSection(firm: page.firm, name: page.name, industry: page.industry)
            }
            if let f = page.finance {
                Eyebrow(text: "Financial summary (trailing year)")
                StatGroup(stats: [
                    Stat(label: "Revenue", value: Format.moneyCompact(f.revenueTrailingCents)),
                    Stat(label: "EBITDA", value: Format.moneyCompact(f.ebitdaTrailingCents),
                         tone: f.ebitdaTrailingCents < 0 ? Theme.negative : Theme.primaryText),
                    Stat(label: "Net income", value: Format.moneyCompact(f.netIncomeTrailingCents),
                         tone: f.netIncomeTrailingCents < 0 ? Theme.negative : Theme.primaryText),
                    Stat(label: "Cash", value: Format.moneyCompact(f.cashCents)),
                    Stat(label: "Assets", value: Format.moneyCompact(f.assetsCents)),
                    Stat(label: "Liabilities", value: Format.moneyCompact(f.liabilitiesCents)),
                    Stat(label: "Equity", value: Format.moneyCompact(f.equityCents)),
                ])
            }
            if !page.regions.isEmpty {
                Eyebrow(text: "Where it operates")
                ForEach(Array(page.regions.enumerated()), id: \.offset) { _, r in
                    Text("\(r.regionName): \(r.plants) plants, \(r.blocks) blocks").font(.caption).foregroundStyle(Theme.secondary)
                }
            }
            if !page.divisions.isEmpty {
                Eyebrow(text: "Divisions")
                ForEach(Array(page.divisions.enumerated()), id: \.offset) { _, d in
                    let line = Text("\(d.regionName) · \(Sector.name(d.industry)) · \(d.strategyName): \(d.plants) plants, \(d.blocks) blocks")
                        .font(.caption)
                    if let onOpenDivision {
                        Button { onOpenDivision(d) } label: {
                            HStack {
                                line.foregroundStyle(Theme.primaryText).multilineTextAlignment(.leading)
                                Spacer()
                                PixelIcon("chevron_right", size: 10).foregroundStyle(Theme.secondary)
                            }
                            .padding(.vertical, 4)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    } else {
                        line.foregroundStyle(Theme.secondary)
                    }
                }
            }
            if !page.holders.isEmpty {
                Eyebrow(text: "Holders")
                if let controller = page.controller {
                    Text("Controlled by \(controller)").font(.caption).foregroundStyle(Theme.gold)
                }
                ForEach(Array(page.holders.prefix(8).enumerated()), id: \.offset) { _, h in
                    Text("\(h.holder): \(h.quantity.formatted()) shares, \(Format.percentBps(h.votesBps)) of votes")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
            }
            DisclosedStakes(stakes: page.stakes)
            linked("Parent", page.parent.map { [$0] } ?? [])
            linked("Subsidiaries", page.subsidiaries)
            linked("Joint ventures", page.ventures)
            ForEach(Array(page.grudges.enumerated()), id: \.offset) { _, line in
                HStack(alignment: .top, spacing: 6) {
                    PixelIcon("target", size: 12).foregroundStyle(Theme.negative)
                    Text(line).font(.caption).foregroundStyle(Theme.primaryText)
                }
            }
            ForEach(Array(page.allies.enumerated()), id: \.offset) { _, line in
                HStack(alignment: .top, spacing: 6) {
                    PixelIcon("people", size: 12).foregroundStyle(Theme.positive)
                    Text(line).font(.caption).foregroundStyle(Theme.primaryText)
                }
            }
            if !page.moves.isEmpty {
                Eyebrow(text: "Recent moves")
                ForEach(Array(page.moves.prefix(6).enumerated()), id: \.offset) { _, move in
                    Text("\(ResultBuckets.weekLabel(move.week)): \(move.text)").font(.caption).foregroundStyle(Theme.secondary)
                }
            }
            if !page.news.isEmpty {
                Eyebrow(text: "In the news")
                ForEach(Array(page.news.enumerated()), id: \.offset) { _, item in
                    Text("\(ResultBuckets.weekLabel(item.week)): \(item.text)").font(.caption).foregroundStyle(Theme.secondary)
                }
            }
            if page.kind != "yours" { RaidMarketsList(firm: page.firm) }
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    private var kindLine: String {
        switch page.kind {
        case "yours": return "Your company · \(page.sizeBand)"
        case "listed": return "Listed · \(page.sizeBand)"
        default: return "Private · \(page.sizeBand)"
        }
    }

    @ViewBuilder
    private func linked(_ title: String, _ rows: [LinkedFirmDto]) -> some View {
        if !rows.isEmpty {
            Eyebrow(text: title)
            ForEach(Array(rows.enumerated()), id: \.offset) { _, r in
                Button { onOpenFirm(r.firm) } label: {
                    HStack(spacing: 8) {
                        FirmLogo(name: r.name, size: 20)
                        Text("\(r.name): \(r.note)").font(.caption).foregroundStyle(Theme.primaryText)
                    }
                }
                .buttonStyle(.plain)
            }
        }
    }
}

/// The full company page for any firm, opened from the stock list, a dialog, a dossier, a region board or the news.
struct CompanyPageSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    @State var firm: UInt64
    @State private var page: CompanyPageDto?
    @State private var trading: StockDto?
    @State private var opening: DivisionDto?

    /// Your own company's division line, matched to the live division it describes.
    private func division(for line: DivisionLineDto) -> DivisionDto? {
        guard page?.firm == store.selectedFirm else { return nil }
        return store.divisions.first { $0.regionName == line.regionName && $0.industry == line.industry }
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                if let page {
                    CompanyPageBody(page: page, onOpenFirm: { firm = $0 }, onTrade: page.stock.map { stock -> () -> Void in { trading = stock } },
                                    onOpenDivision: page.kind == "yours" ? { line in opening = division(for: line) } : nil)
                        .padding(14)
                } else {
                    Text("Loading").foregroundStyle(Theme.secondary).padding(14)
                }
            }
            .background(Theme.ink)
            .navigationTitle(page?.name ?? "Company")
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .sheet(item: $trading) { stock in StockSheet(stock: stock, showPage: false) }
            .sheet(item: $opening) { division in DivisionDetailSheet(division: division).environmentObject(store) }
        }
        .environmentObject(store)
        .task(id: firm) { page = await store.companyPage(firm) }
    }
}
