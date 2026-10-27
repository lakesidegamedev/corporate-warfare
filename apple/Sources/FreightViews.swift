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

/// What the lane colours, thickness and gold edge on the Freight map mean.
struct FreightLegend: View {
    private func dot(_ color: Color, _ text: String) -> some View {
        HStack(spacing: 4) {
            Circle().fill(color).frame(width: 8, height: 8)
            Text(text).font(.caption2).foregroundStyle(Theme.secondary)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 10) {
                dot(Theme.positive, "Room")
                dot(Theme.warning, "Filling up")
                dot(Theme.negative, "Full")
            }
            Text("Thicker line, more freight. Gold edge, you own or ship on it. Tap a line for details.")
                .font(.caption2).foregroundStyle(Theme.secondary)
        }
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
                        FreightLegend()
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
            .buttonStyle(.borderedProminent)
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

/// A division's Shipping panel: its freight bill in plain words, then the route policy picker.
/// Pass nil for the whole company (the CEO Office).
struct DivisionShipping: View {
    @EnvironmentObject private var store: AppStore
    let division: DivisionDto?
    @State private var summary: ShippingSummaryDto?
    @State private var rejection: CommandRejection?

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
                            Text(option.label + modeTail(option)).font(.caption.weight(.semibold))
                                .padding(.horizontal, 10).padding(.vertical, 5)
                                .foregroundStyle(option.selected ? Theme.ink : Theme.primaryText)
                                .background(option.selected ? Theme.gold : Theme.panel, in: Capsule())
                                .overlay(Capsule().stroke(Theme.stroke, lineWidth: option.selected ? 0 : 1))
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
            .background(option.selected ? Theme.gold.opacity(0.14) : Theme.panel.opacity(0.5), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
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
        if s.kind == "reserve", store.selectedFirm != nil {
            Button {
                Task { rejection = await reserve(s) }
            } label: {
                PixelLabel("Reserve \(s.lots) loads for 13 weeks", icon: "lane").font(.callout.weight(.semibold)).frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
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
