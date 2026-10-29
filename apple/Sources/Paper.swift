import SwiftUI

/// What a tapped story or price opens.
private struct PaperTarget: Identifiable {
    let id = UUID()
    let link: PaperLinkDto
    let story: StoryDto?
}

private func paperChange(_ bps: Int64) -> String {
    let sign = bps > 0 ? "+" : (bps < 0 ? "-" : "")
    return String(format: "%@%.1f%%", sign, Double(abs(bps)) / 100)
}

private func paperTint(_ bps: Int64) -> Color {
    bps > 0 ? Theme.positive : (bps < 0 ? Theme.negative : Theme.secondary)
}

/// A small pixel arrow, up in green, down in red, a dash when flat.
private struct MoveArrow: View {
    let bps: Int64

    var body: some View {
        PixelIcon(bps > 0 ? "arrow_up" : (bps < 0 ? "arrow_down" : "minus"), size: 12)
            .foregroundStyle(paperTint(bps))
    }
}

private struct PaperRule: View {
    var thick = false

    var body: some View {
        Rectangle()
            .fill(Theme.primaryText.opacity(thick ? 0.9 : 0.5))
            .frame(height: thick ? 3 : 1)
    }
}

/// The weekly paper as a front page: masthead, date line, a lead story, briefs, a markets column
/// and classifieds. Wide screens put the briefs in columns beside the markets; phones stack.
struct PaperView: View {
    @EnvironmentObject private var store: AppStore
    @State private var picked: UInt32?
    @State private var paper: PaperDto?
    @State private var archive: [UInt32] = []
    @State private var target: PaperTarget?

    private var now: UInt32 { store.overview?.tick ?? 0 }
    private var week: UInt32 { picked ?? now }

    var body: some View {
        GeometryReader { proxy in
            let wide = proxy.size.width >= 760
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    if let paper {
                        masthead(paper, wide: wide)
                        issuePicker
                        if wide {
                            HStack(alignment: .top, spacing: 14) {
                                VStack(alignment: .leading, spacing: 12) {
                                    front(paper)
                                    HStack(alignment: .top, spacing: 12) {
                                        ForEach(paper.briefs, id: \.id) { brief($0) }
                                    }
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                VStack(alignment: .leading, spacing: 12) {
                                    markets(paper)
                                    classifieds(paper)
                                }
                                .frame(width: max(240, proxy.size.width * 0.3))
                            }
                        } else {
                            front(paper)
                            ForEach(paper.briefs, id: \.id) { brief($0) }
                            markets(paper)
                            classifieds(paper)
                        }
                    } else {
                        Text("Printing").foregroundStyle(Theme.secondary)
                    }
                }
                .padding(wide ? 14 : 12)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background(Theme.ink)
        .task(id: "\(week)-\(store.historyRefreshKey)") {
            let wanted = week
            paper = await store.read { $0.paper(week: wanted) }
            archive = await store.read { $0.paperArchive() } ?? []
        }
        .onChange(of: now) { _, latest in
            if picked == latest { picked = nil }
        }
        .sheet(item: $target) { target in
            PaperTargetSheet(target: target).environmentObject(store)
        }
    }

    private func masthead(_ paper: PaperDto, wide: Bool) -> some View {
        VStack(spacing: 5) {
            Text(paper.masthead.uppercased())
                .font(Pixel.font(wide ? 40 : 26, bold: true))
                .foregroundStyle(Theme.primaryText)
                .lineLimit(1)
                .minimumScaleFactor(0.5)
                .frame(maxWidth: .infinity)
            PaperRule(thick: true)
            Text(paper.dateLine.uppercased())
                .font(Pixel.font(11))
                .tracking(1)
                .foregroundStyle(Theme.secondary)
                .frame(maxWidth: .infinity)
            PaperRule()
        }
    }

    private var issuePicker: some View {
        let index = archive.firstIndex(of: week)
        let older = index.flatMap { archive.indices.contains($0 + 1) ? archive[$0 + 1] : nil }
        let newer = index.flatMap { $0 > 0 ? archive[$0 - 1] : nil }
        return HStack(spacing: 6) {
            Text("BACK ISSUES").font(Pixel.font(11)).foregroundStyle(Theme.secondary)
            Spacer()
            issueButton("Older", enabled: older != nil) { picked = older }
            Text(week == now ? "Today" : "Issue \(week)").font(Pixel.font(12)).foregroundStyle(Theme.primaryText)
            issueButton("Newer", enabled: newer != nil) { picked = newer == now ? nil : newer }
            issueButton("Latest", enabled: week != now) { picked = nil }
        }
    }

    private func issueButton(_ title: String, enabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(Pixel.font(11))
                .foregroundStyle(enabled ? Theme.primaryText : Theme.secondary.opacity(0.5))
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(Theme.panelRaised)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
    }

    @ViewBuilder
    private func front(_ paper: PaperDto) -> some View {
        if let lead = paper.lead {
            Button { open(lead) } label: {
                VStack(alignment: .leading, spacing: 6) {
                    Text(lead.headline)
                        .font(Pixel.font(26, bold: true))
                        .foregroundStyle(Theme.primaryText)
                        .multilineTextAlignment(.leading)
                    byline(lead)
                    Text(lead.body)
                        .font(.system(size: 15))
                        .foregroundStyle(Theme.primaryText.opacity(0.9))
                        .multilineTextAlignment(.leading)
                    if lead.link.kind != "none" {
                        Text("READ MORE").font(Pixel.font(11)).foregroundStyle(Theme.gold)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .disabled(lead.link.kind == "none")
        } else {
            VStack(alignment: .leading, spacing: 4) {
                Text("A QUIET WEEK").font(Pixel.font(22)).foregroundStyle(Theme.primaryText)
                Text("Nothing big to report. The markets and notices are below.")
                    .font(.system(size: 13))
                    .foregroundStyle(Theme.secondary)
            }
            .padding(.vertical, 12)
        }
    }

    private func byline(_ story: StoryDto) -> some View {
        Text((story.byline + (story.yours ? " · about you" : "")).uppercased())
            .font(Pixel.font(10))
            .tracking(0.6)
            .foregroundStyle(story.yours ? Theme.gold : Theme.secondary)
    }

    private func brief(_ story: StoryDto) -> some View {
        Button { open(story) } label: {
            VStack(alignment: .leading, spacing: 5) {
                Text(story.headline)
                    .font(Pixel.font(15, bold: true))
                    .foregroundStyle(Theme.primaryText)
                    .multilineTextAlignment(.leading)
                byline(story)
                Text(story.body)
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.secondary)
                    .multilineTextAlignment(.leading)
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.panel)
        }
        .buttonStyle(.plain)
        .disabled(story.link.kind == "none")
    }

    private func markets(_ paper: PaperDto) -> some View {
        let m = paper.markets
        return VStack(alignment: .leading, spacing: 6) {
            Text("MARKETS").font(Pixel.font(12)).tracking(0.8).foregroundStyle(Theme.secondary)
            PaperRule()
            if !m.open {
                Text("The exchange has not closed a week yet.").font(.system(size: 12)).foregroundStyle(Theme.secondary)
            } else {
                HStack(spacing: 6) {
                    MoveArrow(bps: m.indexChangeBps)
                    Text("Index").font(.system(size: 13)).foregroundStyle(Theme.primaryText)
                    Spacer()
                    Text(paperChange(m.indexChangeBps)).font(.system(size: 13, weight: .semibold)).foregroundStyle(paperTint(m.indexChangeBps))
                }
                ForEach(m.shares, id: \.firm) { share in
                    Button {
                        target = PaperTarget(link: PaperLinkDto(kind: "company", id: share.firm, key: ""), story: nil)
                    } label: {
                        HStack(spacing: 6) {
                            MoveArrow(bps: share.changeBps)
                            Text(share.name).font(.system(size: 12)).foregroundStyle(Theme.primaryText).lineLimit(1)
                            Spacer()
                            Text(Format.price(share.priceCents)).font(.system(size: 12)).foregroundStyle(Theme.secondary)
                            Text(paperChange(share.changeBps)).font(.system(size: 12, weight: .semibold)).foregroundStyle(paperTint(share.changeBps))
                        }
                    }
                    .buttonStyle(.plain)
                }
                if !m.goods.isEmpty { PaperRule() }
                ForEach(m.goods, id: \.product) { good in
                    Button {
                        target = PaperTarget(link: PaperLinkDto(kind: "good", id: UInt64(good.product), key: ""), story: nil)
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                MoveArrow(bps: good.changeBps)
                                Text(good.name).font(.system(size: 12)).foregroundStyle(Theme.primaryText).lineLimit(1)
                                Spacer()
                                Text(Format.money(good.priceCents)).font(.system(size: 12)).foregroundStyle(Theme.secondary)
                                Text(paperChange(good.changeBps)).font(.system(size: 12, weight: .semibold)).foregroundStyle(paperTint(good.changeBps))
                            }
                            if !good.reason.isEmpty {
                                Text(good.reason).font(.system(size: 11)).foregroundStyle(Theme.secondary).padding(.leading, 18)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                }
                if m.shares.isEmpty && m.goods.isEmpty {
                    Text("Prices were flat.").font(.system(size: 12)).foregroundStyle(Theme.secondary)
                }
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panel)
    }

    private func classifieds(_ paper: PaperDto) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("CLASSIFIEDS").font(Pixel.font(12)).tracking(0.8).foregroundStyle(Theme.secondary)
            PaperRule()
            if paper.classifieds.isEmpty {
                Text("No notices this week.").font(.system(size: 12)).foregroundStyle(Theme.secondary)
            }
            ForEach(Array(paper.classifieds.enumerated()), id: \.offset) { _, item in
                Button {
                    if item.link.kind != "none" { target = PaperTarget(link: item.link, story: nil) }
                } label: {
                    Text(item.text).font(.system(size: 12)).foregroundStyle(Theme.primaryText).multilineTextAlignment(.leading)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.panel)
    }

    private func open(_ story: StoryDto) {
        guard story.link.kind != "none" else { return }
        target = PaperTarget(link: story.link, story: story)
    }
}

/// The thing a story is about: a company page, a market, a region board, the company a deal is
/// aimed at, or the event's own text.
private struct PaperTargetSheet: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.dismiss) private var dismiss
    let target: PaperTarget
    @State private var dealTarget: UInt64?

    var body: some View {
        switch target.link.kind {
        case "company":
            CompanyPageSheet(firm: target.link.id)
        case "deal":
            if let firm = dealTarget {
                CompanyPageSheet(firm: firm)
            } else {
                Text("Loading").foregroundStyle(Theme.secondary).padding(14)
                    .task {
                        let handle = target.link.id
                        dealTarget = await store.read { $0.deals().first { $0.handle == handle }?.targetHandle } ?? nil
                        if dealTarget == nil { dismiss() }
                    }
            }
        case "good":
            let product = UInt32(target.link.id)
            let good = store.commodities.first { $0.product == product }
            sheet(good?.name ?? "Market") {
                GoodDepthSection(product: product, referenceCents: good?.referenceCents ?? 0)
            }
        case "region":
            let region = UInt32(target.link.id)
            sheet(store.boards.first { $0.index == region }?.name ?? "Region") {
                RegionDossierSection(region: region)
            }
        default:
            sheet(target.story?.headline ?? "News") {
                Text(target.story?.body ?? "").foregroundStyle(Theme.primaryText)
            }
        }
    }

    private func sheet<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        NavigationStack {
            ScrollView {
                content().padding(14).frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(Theme.ink)
            .navigationTitle(title)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .presentationBackground(Theme.ink)
    }
}

/// One line with the top headline, on every screen. Time running cycles the top three every few
/// seconds; paused it holds the first; with animations off or Reduce Motion on it never cycles.
/// Tapping opens the paper. It reads the engine only when paused or at 1x.
struct NewsTicker: View {
    @EnvironmentObject private var store: AppStore
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @AppStorage(GraphicsKeys.animations) private var animations = true
    let onOpen: () -> Void
    @State private var headlines: [String] = []
    @State private var shown = 0

    /// The engine is read only when time is paused or at 1x, once per change, never at speed.
    private var readKey: String {
        guard let overview = store.overview else { return "none" }
        return store.runSpeed > 1 ? "fast" : "rev-\(overview.revision)"
    }

    private var cycling: Bool { store.runSpeed > 0 && animations && !reduceMotion && headlines.count > 1 }

    var body: some View {
        Button(action: onOpen) {
            HStack(spacing: 8) {
                Text("NEWS")
                    .font(Pixel.font(10))
                    .foregroundStyle(Theme.ink)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(Theme.gold)
                Text(headlines.indices.contains(shown) ? headlines[shown] : "A quiet week. Open the paper.")
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.primaryText)
                    .lineLimit(1)
                Spacer(minLength: 4)
                PixelIcon("chevron_right", size: 12).foregroundStyle(Theme.secondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 4)
            .frame(maxWidth: .infinity)
            .background(Theme.ink)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("News ticker")
        .task(id: readKey) {
            // At 2x and 4x the last headlines stay up; one read catches up when time slows.
            guard readKey != "fast" else { return }
            let fresh = await store.read { $0.ticker() } ?? headlines
            if fresh != headlines {
                headlines = fresh
                shown = 0
            }
        }
        .task(id: cycling) {
            shown = 0
            while cycling && !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 5_000_000_000)
                if cycling && !headlines.isEmpty { shown = (shown + 1) % headlines.count }
            }
        }
    }
}
