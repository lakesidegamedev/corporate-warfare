import SwiftUI

/// Green while a lane has room, amber as it fills, red when it is full or spilling to outside carriers.
func laneColor(_ lane: LaneDto) -> Color {
    if lane.outsideLots > 0 || lane.fillBps >= 9_000 { return Theme.negative }
    if lane.fillBps >= 6_000 { return Theme.warning }
    return Theme.positive
}

/// The picture for a freight mode id.
func modeArt(_ id: String) -> String { id == "sea" ? "ship" : id }

/// Whether the player's group owns capacity on the lane, ships on it or holds a contract there.
func laneIsMine(_ lane: LaneDto) -> Bool {
    lane.myCapacityLots > 0 || lane.myShippedLots > 0 || lane.myContractLots > 0
}

extension LaneDto: Identifiable {
    public var id: String { "\(a)-\(b)-\(mode)" }
}

private func percent(_ bps: Int32) -> String { "\((Int(bps) + 50) / 100)%" }

private func changeText(_ bps: Int32) -> String {
    if abs(bps) < 100 { return "about the same" }
    return bps < 0 ? "\(-bps / 100)% less" : "\(bps / 100)% more"
}

/// What the Freight mode's colours mean: you, the biggest carriers on the busiest lanes, outside
/// carriers, and how width and dashes read.
struct FreightLegend: View {
    let lanes: [LaneDto]

    private struct Carrier: Identifiable {
        let id: UInt64
        let name: String
        let mine: Bool
    }

    private var carriers: [Carrier] {
        var lots: [UInt64: (name: String, mine: Bool, lots: Int)] = [:]
        for owner in FreightArcs.busiest(lanes).flatMap(\.owners) where owner.lots > 0 {
            let held = lots[owner.firm] ?? (owner.mine ? "You" : owner.name, owner.mine, 0)
            lots[owner.firm] = (held.name, held.mine, held.lots + Int(owner.lots))
        }
        return lots.sorted { $0.value.mine != $1.value.mine ? $0.value.mine : $0.value.lots > $1.value.lots }
            .prefix(5).map { Carrier(id: $0.key, name: $0.value.name, mine: $0.value.mine) }
    }

    private func row(_ color: RGB, _ text: String) -> some View {
        HStack(spacing: 6) {
            Rectangle().fill(color.color()).frame(width: 18, height: 6)
            Text(text).font(.system(size: 11)).foregroundStyle(Theme.primaryText).lineLimit(1)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("FREIGHT").font(Pixel.font(10)).foregroundStyle(Theme.primaryText)
            ForEach(carriers) { carrier in
                row(FreightArcs.companyColor(carrier.id, mine: carrier.mine), carrier.name)
            }
            row(FreightArcs.outsideCarriers, "Outside carriers")
            Text("Wider: more freight. Dashed: by water.")
                .font(.system(size: 10)).foregroundStyle(Theme.secondary)
        }
        .accessibilityElement(children: .combine)
    }
}

/// The Freight mode on the map: each of the busiest lanes as a clean arc between its hubs, one
/// coloured stripe per company that carries goods on it, side by side, each as wide as its share of
/// the lane's freight; your own in gold. Sea lanes are dashed. A dark casing keeps the stack
/// readable on any ground; the picked lane is outlined in white and lanes away from a selected
/// region fade.
enum FreightArcs {
    /// Company colours: the player always gold, rivals from a fixed set by firm.
    static let rivalColors: [RGB] = [
        RGB(hex: 0x42A5F5), RGB(hex: 0xEF5350), RGB(hex: 0x66BB6A), RGB(hex: 0xAB47BC),
        RGB(hex: 0x26C6DA), RGB(hex: 0xFF8A65), RGB(hex: 0xEC407A), RGB(hex: 0x9CCC65),
    ]
    static let outsideCarriers = RGB(hex: 0x8A939C)
    static let gold = RGB(hex: 0xE2B04A)

    static func companyColor(_ firm: UInt64, mine: Bool) -> RGB {
        mine ? gold : rivalColors[Int(firm % UInt64(rivalColors.count))]
    }

    static func busiest(_ lanes: [LaneDto]) -> [LaneDto] {
        Array(lanes.sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }.prefix(90))
    }

    /// Who carries a lane: each owner's stripe, biggest first, then outside carriers.
    static func stripes(_ lane: LaneDto) -> [(color: RGB, lots: Int)] {
        var out = lane.owners.filter { $0.lots > 0 }.sorted { $0.lots > $1.lots }
            .map { (color: companyColor($0.firm, mine: $0.mine), lots: Int($0.lots)) }
        if lane.outsideLots > 0 { out.append((outsideCarriers, Int(lane.outsideLots))) }
        return out.isEmpty ? [(outsideCarriers, max(Int(lane.capacityLots), 1))] : out
    }

    /// A lane's arc: hub to hub through a control point, bowed to one side so arcs between nearby
    /// hubs stand apart, more for rail and most for sea so the modes between the same two regions
    /// never overlap.
    struct Arc {
        let a: CGPoint
        let c: CGPoint
        let b: CGPoint

        func at(_ t: Double) -> CGPoint {
            let u = 1 - t
            return CGPoint(x: u * u * Double(a.x) + 2 * u * t * Double(c.x) + t * t * Double(b.x),
                           y: u * u * Double(a.y) + 2 * u * t * Double(c.y) + t * t * Double(b.y))
        }

        /// The arc moved `d` points to its side, for stacking stripes.
        func shifted(_ d: Double) -> Arc {
            let dx = Double(b.x - a.x)
            let dy = Double(b.y - a.y)
            let len = max(hypot(dx, dy), 1e-3)
            let nx = -dy / len * d
            let ny = dx / len * d
            func move(_ p: CGPoint) -> CGPoint { CGPoint(x: Double(p.x) + nx, y: Double(p.y) + ny) }
            return Arc(a: move(a), c: move(c), b: move(b))
        }

        var path: Path {
            var path = Path()
            path.move(to: a)
            path.addQuadCurve(to: b, control: c)
            return path
        }
    }

    static func arc(_ lane: LaneDto, map: WorldMap, projection p: MapProjection, plinth: Double) -> Arc? {
        // The same two hubs always bow the same way, whichever end is listed first.
        let (from, to) = lane.a < lane.b ? (lane.a, lane.b) : (lane.b, lane.a)
        guard let ga = map.anchor(of: from), let gb = map.anchor(of: to) else { return nil }
        let (a, va) = p.project(ga, height: plinth)
        let (b, vb) = p.project(gb, height: plinth)
        guard va, vb else { return nil }
        let dx = Double(b.x - a.x)
        let dy = Double(b.y - a.y)
        guard hypot(dx, dy) >= 2 else { return nil }
        let bow = lane.modeId == "rail" ? 0.26 : lane.modeId == "sea" ? 0.38 : 0.14
        let c = CGPoint(x: Double(a.x + b.x) / 2 - dy * bow, y: Double(a.y + b.y) / 2 + dx * bow)
        return Arc(a: a, c: c, b: b)
    }

    static func draw(_ lanes: [LaneDto], map: WorldMap, projection p: MapProjection, plinth: Double, focus: String?,
                     picked: String?, in context: inout GraphicsContext) {
        let busiest = busiest(lanes)
        guard let first = busiest.first else { return }
        let top = Double(max(first.capacityLots + first.outsideLots, 1))
        // Thin, quiet lanes first so the busy ones lie on top.
        for lane in busiest.reversed() {
            guard let arc = arc(lane, map: map, projection: p, plinth: plinth) else { continue }
            let parts = stripes(lane)
            let total = Double(max(parts.reduce(0) { $0 + $1.lots }, 1))
            let width = 5 + 13 * Double(lane.capacityLots + lane.outsideLots) / top
            let dim = focus != nil && lane.a != focus && lane.b != focus && picked == nil ? 0.3 : 1
            let dash: [CGFloat] = lane.modeId == "sea" ? [10, 6] : []
            if lane.id == picked {
                context.stroke(arc.path, with: .color(.white), style: StrokeStyle(lineWidth: width + 6, lineCap: .round))
            }
            context.stroke(arc.path, with: .color(Theme.ink.opacity(0.85 * dim)), style: StrokeStyle(lineWidth: width + 2.5, lineCap: .round))
            var offset = -width / 2
            for part in parts {
                let w = max(width * Double(part.lots) / total, 1.2)
                context.stroke(arc.shifted(offset + w / 2).path, with: .color(part.color.color(dim)),
                               style: StrokeStyle(lineWidth: w, lineCap: .butt, dash: dash))
                offset += w
            }
        }
        // The hubs the lanes meet at.
        var seen = Set<String>()
        for code in busiest.flatMap({ [$0.a, $0.b] }) where seen.insert(code).inserted {
            guard let anchor = map.anchor(of: code) else { continue }
            let (at, visible) = p.project(anchor, height: plinth)
            guard visible else { continue }
            context.fill(disc(at, 5), with: .color(Theme.ink))
            context.fill(disc(at, 3), with: .color(.white))
        }
    }

    /// The lane drawn nearest a tap, along its arc, within a finger's width.
    static func at(_ tap: CGPoint, lanes: [LaneDto], map: WorldMap, projection p: MapProjection, plinth: Double) -> LaneDto? {
        var best: LaneDto?
        var bestDistance = 22.0
        for lane in busiest(lanes) {
            guard let arc = arc(lane, map: map, projection: p, plinth: plinth) else { continue }
            for k in 0...24 {
                let point = arc.at(Double(k) / 24)
                let d = Double(hypot(point.x - tap.x, point.y - tap.y))
                if d < bestDistance {
                    bestDistance = d
                    best = lane
                }
            }
        }
        return best
    }
}

/// Corridor freight at a glance, then the lanes, then a simple panel for the lane tapped.
struct FreightPanelView: View {
    @EnvironmentObject private var store: AppStore
    let focus: String?
    @Binding var picked: String?

    private var lanes: [LaneDto] {
        store.lanes
            .filter { focus == nil || $0.a == focus || $0.b == focus }
            .sorted {
                let (x, y) = (laneIsMine($0), laneIsMine($1))
                if x != y { return x }
                return ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots)
            }
            .prefix(14).map { $0 }
    }

    var body: some View {
        Panel(padding: 10) {
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    if let id = picked, let lane = store.lanes.first(where: { $0.id == id }) {
                        Button("Back to all lanes") { picked = nil }
                            .font(.caption.weight(.semibold)).buttonStyle(.plain).foregroundStyle(Theme.gold)
                        LaneDetail(lane: lane)
                    } else {
                        FreightLegend(lanes: store.lanes)
                        if let summary = store.freight {
                            HStack(alignment: .top, spacing: 6) {
                                PixelIcon("lane", size: 14).foregroundStyle(Theme.gold)
                                Text("Last week \(summary.movedLots.formatted()) loads moved on \(summary.lanesWithCapacity) lanes. \(summary.outsideShareBps / 100)% went to outside carriers at a premium. \(summary.congestedLanes) lanes are congested.")
                                    .font(.caption).foregroundStyle(Theme.primaryText)
                            }
                        }
                        if lanes.isEmpty {
                            Text("No freight lanes here yet. Build trucking, rail, a pipeline or shipping to own one.")
                                .font(.caption).foregroundStyle(Theme.secondary)
                        }
                        ForEach(lanes, id: \.id) { lane in
                            Button { picked = lane.id } label: {
                                HStack(spacing: 6) {
                                    PixelIcon(modeArt(lane.modeId), size: 14).foregroundStyle(laneColor(lane))
                                    VStack(alignment: .leading, spacing: 0) {
                                        Text("\(lane.aName) to \(lane.bName)").font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                                        if laneIsMine(lane) {
                                            Text("You ship \(lane.myShippedLots.formatted()) loads here").font(.caption2).foregroundStyle(Theme.gold)
                                        }
                                    }
                                    Spacer(minLength: 4)
                                    Text("\(lane.fillBps / 100)% full · \(store.freightPrice(lane.rateCentsPerLot)) a load")
                                        .font(.caption.weight(.semibold).monospacedDigit()).foregroundStyle(laneColor(lane))
                                }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
    }
}

extension AppStore {
    /// A freight amount in the selected company's money.
    func freightMoney(_ cents: Int64) -> String { Format.money(cents, currency: selectedFirmDto?.currency ?? "") }
    func freightPrice(_ cents: Int64) -> String { Format.price(cents, currency: selectedFirmDto?.currency ?? "") }
    func freightCompact(_ cents: Int64) -> String { Format.moneyCompact(cents, currency: selectedFirmDto?.currency ?? "") }
}

private struct LaneDetail: View {
    @EnvironmentObject private var store: AppStore
    let lane: LaneDto
    @State private var lots: Double = 100
    @State private var weeks: Double = 13
    @State private var quote: ReserveQuoteDto?
    @State private var rejection: CommandRejection?

    private var mine: [LaneContractDto] {
        store.laneContracts.filter { $0.mine && $0.a == lane.a && $0.b == lane.b && $0.mode == lane.mode }
    }

    private var rivals: Int {
        store.laneContracts.filter { !$0.mine && $0.a == lane.a && $0.b == lane.b && $0.mode == lane.mode }.count
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                PixelIcon(modeArt(lane.modeId), size: 16).foregroundStyle(laneColor(lane))
                Text("\(lane.aName) to \(lane.bName) by \(lane.modeName.lowercased())")
                    .font(.subheadline.weight(.semibold)).foregroundStyle(Theme.primaryText)
            }
            Text(summary).font(.caption).foregroundStyle(Theme.secondary)
            Text("Price \(store.freightPrice(lane.rateCentsPerLot)) a load today.").font(.caption).foregroundStyle(Theme.primaryText)
            if lane.outsideLots > 0 {
                Text("\(lane.outsideLots.formatted()) loads spilled to outside carriers at \(store.freightPrice(lane.outsideRateCentsPerLot)) each.")
                    .font(.caption).foregroundStyle(Theme.negative)
            }
            if lane.owners.isEmpty {
                Text("No firm owns trucks, rail or ships here, so outside carriers haul it.").font(.caption).foregroundStyle(Theme.secondary)
            } else {
                Text("Owned by").font(.caption2).foregroundStyle(Theme.secondary)
                ForEach(lane.owners, id: \.firm) { owner in
                    Text("\(owner.name)\(owner.mine ? " (you)" : ""): room for \(owner.lots.formatted()) loads")
                        .font(.caption).foregroundStyle(owner.mine ? Theme.gold : Theme.primaryText)
                }
            }
            if laneIsMine(lane) {
                Text("Your share: you own room for \(lane.myCapacityLots.formatted()) loads and shipped \(lane.myShippedLots.formatted()) here last week.")
                    .font(.caption).foregroundStyle(Theme.gold)
            } else {
                Text("You do not ship on this lane.").font(.caption).foregroundStyle(Theme.secondary)
            }
            ForEach(mine, id: \.id) { contract in
                Text("Your reservation: \(contract.heldLots.formatted()) loads at \(store.freightPrice(contract.feeCentsPerLot)), \(contract.weeksLeft) weeks left")
                    .font(.caption).foregroundStyle(Theme.gold)
            }
            if rivals > 0 {
                Text("\(rivals) rival reservation\(rivals == 1 ? "" : "s") on this lane").font(.caption).foregroundStyle(Theme.warning)
            }
            if lane.reservableLots >= 10, mine.isEmpty, store.selectedFirm != nil {
                reserve
            }
            if let rejection {
                Text(rejection.message).font(.caption).foregroundStyle(Theme.negative)
            }
        }
    }

    private var summary: String {
        var text = "\(lane.fillBps / 100)% full. Room for \(lane.capacityLots.formatted()) loads a week over \(lane.km) km; \(lane.usedLots.formatted()) used"
        if lane.heldLots > 0 { text += ", \(lane.heldLots.formatted()) reserved" }
        return text + "."
    }

    private var most: Double {
        Double(max(10, min(lane.reservableLots, Int64(lane.capacityLots) * 8 / 10)))
    }

    private var reserve: some View {
        VStack(alignment: .leading, spacing: 6) {
            Divider().overlay(Theme.stroke)
            Text("Reserve loads: a locked price that rivals cannot take from you").font(.caption2).foregroundStyle(Theme.secondary)
            ValueSlider(
                value: $lots, range: 10...most, step: 10, ticks: 10,
                label: { "\(Int($0).formatted()) loads a week" },
                detail: { _ in "Rivals cannot use these loads. You pay for them every week, shipped or not." }
            )
            ValueSlider(value: $weeks, range: 4...52, step: 1, ticks: 12, label: { "\(Int($0)) weeks" })
            if let quote, quote.ok {
                Text("\(store.freightPrice(quote.feeCentsPerLot)) a load against \(store.freightPrice(quote.spotCentsPerLot)) today. \(store.freightMoney(quote.weeklyFeeCents)) a week, \(store.freightMoney(quote.totalCents)) over \(Int(weeks)) weeks.")
                    .font(.caption).foregroundStyle(Theme.primaryText)
                if quote.weeklyNetCents >= 0 {
                    Text("At last week's volume (\(quote.expectedUseLots.formatted()) loads) you save about \(store.freightMoney(quote.weeklyNetCents)) a week.")
                        .font(.caption).foregroundStyle(Theme.positive)
                } else {
                    Text("At last week's volume (\(quote.expectedUseLots.formatted()) loads) it costs about \(store.freightMoney(-quote.weeklyNetCents)) a week more than shipping at today's price. It pays if prices rise or you ship more.")
                        .font(.caption).foregroundStyle(Theme.warning)
                }
            }
            Button {
                Task { rejection = await send() }
            } label: {
                PixelLabel("Reserve \(Int(lots).formatted()) loads", icon: "lane").font(.callout.weight(.semibold)).frame(maxWidth: .infinity)
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
        }
        .task(id: "\(lane.id)-\(Int(lots))-\(Int(weeks))-\(store.overview?.revision ?? 0)") {
            quote = await store.reserveQuote(lane: lane, lots: Int32(lots), weeks: UInt32(weeks))
        }
    }

    private func send() async -> CommandRejection? {
        guard let firm = store.selectedFirm,
              let from = store.boards.first(where: { $0.code == lane.a }),
              let to = store.boards.first(where: { $0.code == lane.b }) else { return nil }
        return await store.submit(CommandJSON.command("ReserveLane", [
            "firm": firm, "from": from.index, "to": to.index, "mode": lane.mode,
            "lots": Int(min(lots, Double(lane.reservableLots))), "weeks": Int(weeks),
        ]))
    }
}

/// A build suggestion handed to the build sheet.
private struct BuildHandoff: Identifiable {
    let suggestion: ShippingSuggestionDto
    var id: String { "\(suggestion.a)-\(suggestion.b)-\(suggestion.mode)-\(suggestion.recipe)" }
    init(_ suggestion: ShippingSuggestionDto) { self.suggestion = suggestion }
}

extension BuildHandoff: Equatable {
    static func == (l: BuildHandoff, r: BuildHandoff) -> Bool { l.id == r.id }
}

/// A division's Shipping panel: its freight bill in plain words, then the route policy picker.
/// Pass nil for the whole company (the CEO Office).
struct DivisionShipping: View {
    @EnvironmentObject private var store: AppStore
    let division: DivisionDto?
    @State private var summary: ShippingSummaryDto?
    @State private var rejection: CommandRejection?
    @State private var building: ShippingSuggestionDto?

    init(division: DivisionDto? = nil) { self.division = division }

    private var plant: UInt64? { division?.plants.first }

    private var isCarrier: Bool { !(division?.carrierMode.isEmpty ?? true) }

    var body: some View {
        if division == nil || plant != nil {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    PixelIcon("lane", size: 12).foregroundStyle(Theme.secondary)
                    Eyebrow(text: division.map { "Shipping into \($0.regionName)" } ?? "Shipping")
                }
                if let division, isCarrier {
                    let where_ = division.serves.isEmpty ? "the busiest lanes in reach" : division.serves.joined(separator: ", ")
                    Text("Carries freight by \(division.carrierMode.lowercased()) on \(where_). Every shipper can use its capacity; your group goes first and pays cost.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                } else if let summary {
                    content(summary)
                } else {
                    Text("Reading last week's freight.").font(.caption).foregroundStyle(Theme.secondary)
                }
                if let rejection {
                    Text(rejection.message).font(.caption).foregroundStyle(Theme.negative)
                }
            }
            .task(id: "\(plant ?? 0)-\(store.selectedFirm ?? 0)-\(store.overview?.revision ?? 0)") {
                guard !isCarrier else { return }
                summary = await store.shipping(facility: plant ?? 0)
            }
            .sheet(item: Binding(get: { building.map(BuildHandoff.init) }, set: { if $0 == nil { building = nil } })) { handoff in
                BuildSheet(
                    region: store.regions.first { $0.code == handoff.suggestion.a }?.index,
                    recipe: handoff.suggestion.recipe,
                    blocks: UInt32(clamping: handoff.suggestion.lots)
                )
                .environmentObject(store)
            }
        }
    }

    @ViewBuilder
    private func content(_ s: ShippingSummaryDto) -> some View {
        Text(s.policyText).font(.caption).foregroundStyle(Theme.primaryText)
        if !s.hasData {
            Text(division != nil
                 ? "Nothing was shipped in last week, so there is no bill yet. A load is one lot of goods."
                 : "No freight bill yet. It fills in after the first week with deliveries. A load is one lot of goods.")
                .font(.caption).foregroundStyle(Theme.secondary)
        } else {
            Text("Freight bill \(store.freightMoney(s.weeklyCents)) a week, \(store.freightPrice(s.centsPerLot)) a load across \(s.lots.formatted()) loads."
                 + (s.feeCents > 0 ? " Includes \(store.freightMoney(s.feeCents)) in lane reservations." : ""))
                .font(.caption).foregroundStyle(Theme.primaryText)
            Text("Paid to your carriers \(percent(s.ownShareBps)), other firms' carriers \(percent(s.otherShareBps)), outside carriers \(percent(s.outsideShareBps)).")
                .font(.caption).foregroundStyle(Theme.secondary)
            if s.outsideExtraCents > 0 {
                Text("Outside carriers charge a premium: \(store.freightMoney(s.outsideExtraCents)) a week more than your own carriers would.")
                    .font(.caption).foregroundStyle(Theme.warning)
            }
            ForEach(Array(s.lanes.enumerated()), id: \.offset) { _, lane in
                HStack(spacing: 6) {
                    PixelIcon(modeArt(lane.modeId), size: 13).foregroundStyle(lane.congested ? Theme.negative : Theme.positive)
                    Text("\(lane.aName) to \(lane.bName)").font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                    Spacer(minLength: 4)
                    Text("\(lane.lots.formatted()) loads, \(store.freightCompact(lane.cents))" + (lane.congested ? ", congested" : ""))
                        .font(.caption2).foregroundStyle(lane.congested ? Theme.negative : Theme.secondary)
                }
            }
        }
        if let suggestion = s.suggestion {
            suggestionView(suggestion)
        }
        if let division, let plant, !s.kinds.isEmpty {
            Text("How should goods get here? Pick once; the engine routes every week.")
                .font(.caption).foregroundStyle(Theme.secondary)
            ForEach(s.kinds, id: \.id) { option in
                policyRow(option) { Task { rejection = await set(plant, kind: Int(option.id), mode: division.shipMode) } }
            }
            Text("Preferred vehicle").font(.caption).foregroundStyle(Theme.secondary)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(s.modes, id: \.id) { option in
                        Button {
                            Task { rejection = await set(plant, kind: Int(division.shipKind), mode: option.id) }
                        } label: {
                            Text((option.label + modeTail(option)).uppercased()).font(Pixel.font(11))
                                .padding(.horizontal, 10).padding(.vertical, 5)
                                .foregroundStyle(option.selected ? Theme.ink : Theme.primaryText)
                                .background(option.selected ? Theme.gold : Theme.panel, in: Pixel.small)
                                .overlay(Pixel.small.stroke(Theme.stroke, lineWidth: option.selected ? 0 : 1))
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(option.selected ? .isSelected : [])
                    }
                }
            }
            if let chosen = s.modes.first(where: { $0.selected }) {
                Text(chosen.blurb + " Percentages show the change in this division's freight bill at today's prices.")
                    .font(.caption).foregroundStyle(Theme.secondary)
            }
        }
    }

    private func modeTail(_ option: PolicyOptionDto) -> String {
        guard !option.selected, option.hasEstimate, abs(option.changeBps) >= 100 else { return "" }
        return " \(option.changeBps < 0 ? "-" : "+")\(abs(option.changeBps) / 100)%"
    }

    private func policyRow(_ option: PolicyOptionDto, pick: @escaping () -> Void) -> some View {
        Button(action: pick) {
            HStack(alignment: .top, spacing: 8) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(option.label).font(.caption.weight(.semibold)).foregroundStyle(option.selected ? Theme.gold : Theme.primaryText)
                    Text(option.blurb).font(.caption2).foregroundStyle(Theme.secondary).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 4)
                VStack(alignment: .trailing, spacing: 1) {
                    if option.hasEstimate {
                        Text("\(store.freightCompact(option.weeklyCents)) a week").font(.caption2).foregroundStyle(Theme.primaryText)
                        Text(option.selected ? "your choice" : changeText(option.changeBps))
                            .font(.caption2)
                            .foregroundStyle(option.selected ? Theme.gold : option.changeBps <= -100 ? Theme.positive : option.changeBps >= 100 ? Theme.negative : Theme.secondary)
                    } else if option.selected {
                        Text("your choice").font(.caption2).foregroundStyle(Theme.gold)
                    }
                }
            }
            .padding(.horizontal, 8).padding(.vertical, 5)
            .background(option.selected ? Theme.gold.opacity(0.14) : Theme.ink, in: Pixel.shape)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(option.selected ? .isSelected : [])
    }

    @ViewBuilder
    private func suggestionView(_ s: ShippingSuggestionDto) -> some View {
        let line = s.kind == "build"
            ? "\(s.headline). It costs \(store.freightMoney(s.upfrontCents)) and saves about \(store.freightMoney(s.weeklySavingCents)) a week, so it pays back in \(s.paybackWeeks) weeks."
            : "\(s.headline). At last week's volume it saves about \(store.freightMoney(s.weeklySavingCents)) a week with no upfront cost."
        CalloutRow(icon: "lane", text: line)
        if s.kind == "build", store.selectedFirm != nil {
            Button {
                building = s
            } label: {
                PixelLabel("Review this build", icon: "lane").font(.callout.weight(.semibold)).frame(maxWidth: .infinity)
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
        }
        if s.kind == "reserve", store.selectedFirm != nil {
            Button {
                Task { rejection = await reserve(s) }
            } label: {
                PixelLabel("Reserve \(s.lots) loads for 13 weeks", icon: "lane").font(.callout.weight(.semibold)).frame(maxWidth: .infinity)
            }
            .buttonStyle(PixelButtonStyle(prominent: true))
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
        }
    }

    private func reserve(_ s: ShippingSuggestionDto) async -> CommandRejection? {
        guard let firm = store.selectedFirm,
              let from = store.boards.first(where: { $0.code == s.a }),
              let to = store.boards.first(where: { $0.code == s.b }) else { return nil }
        return await store.submit(CommandJSON.command("ReserveLane", [
            "firm": firm, "from": from.index, "to": to.index, "mode": s.mode, "lots": Int(s.lots), "weeks": 13,
        ]))
    }

    private func set(_ plant: UInt64, kind: Int, mode: Int32) async -> CommandRejection? {
        var fields: [String: Any] = ["facility": plant, "kind": kind]
        if mode >= 0 { fields["mode"] = Int(mode) }
        return await store.submit(CommandJSON.command("SetShippingPolicy", fields))
    }
}
