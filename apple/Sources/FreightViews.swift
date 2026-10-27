import SwiftUI

/// Green while a lane has room, amber as it fills, red when it is full or spilling to outside carriers.
func laneColor(_ lane: LaneDto) -> Color {
    if lane.outsideLots > 0 || lane.fillBps >= 9_000 { return Theme.negative }
    if lane.fillBps >= 6_000 { return Theme.warning }
    return Theme.positive
}

/// The picture for a freight mode id.
func modeArt(_ id: String) -> String { id == "sea" ? "ship" : id }

/// Corridor freight at a glance, then the lanes: capacity, owners, rate and a contract action.
struct FreightPanelView: View {
    @EnvironmentObject private var store: AppStore
    let focus: String?
    @State private var open: String?

    private var lanes: [LaneDto] {
        store.lanes
            .filter { focus == nil || $0.a == focus || $0.b == focus }
            .sorted { ($0.capacityLots + $0.outsideLots) > ($1.capacityLots + $1.outsideLots) }
            .prefix(14).map { $0 }
    }

    private func key(_ lane: LaneDto) -> String { "\(lane.a)-\(lane.b)-\(lane.mode)" }

    var body: some View {
        Panel(padding: 10) {
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    if let summary = store.freight {
                        HStack(spacing: 6) {
                            PixelIcon("lane", size: 14).foregroundStyle(Theme.gold)
                            Text("\(summary.movedLots.formatted()) lots moved · \(summary.outsideShareBps / 100)% by outside carriers · \(summary.congestedLanes) congested")
                                .font(.caption).foregroundStyle(Theme.primaryText)
                        }
                    }
                    if lanes.isEmpty {
                        Text("No freight capacity here yet. Build trucking, rail, a pipeline or shipping to own a lane.")
                            .font(.caption).foregroundStyle(Theme.secondary)
                    }
                    ForEach(lanes, id: \.id) { lane in
                        Button {
                            open = open == key(lane) ? nil : key(lane)
                        } label: {
                            HStack(spacing: 6) {
                                PixelIcon(modeArt(lane.modeId), size: 14).foregroundStyle(laneColor(lane))
                                Text("\(lane.aName) to \(lane.bName)").font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                                Spacer(minLength: 4)
                                Text("\(lane.fillBps / 100)% · \(Format.price(lane.rateCentsPerLot)) a lot")
                                    .font(.caption.weight(.semibold).monospacedDigit()).foregroundStyle(laneColor(lane))
                            }
                        }
                        .buttonStyle(.plain)
                        if open == key(lane) { LaneDetail(lane: lane) }
                    }
                }
            }
        }
    }
}

extension LaneDto: Identifiable {
    public var id: String { "\(a)-\(b)-\(mode)" }
}

private struct LaneDetail: View {
    @EnvironmentObject private var store: AppStore
    let lane: LaneDto
    @State private var lots: Double = 100
    @State private var weeks: Double = 13
    @State private var rejection: CommandRejection?

    private var mine: [LaneContractDto] {
        store.laneContracts.filter { $0.mine && $0.a == lane.a && $0.b == lane.b && $0.mode == lane.mode }
    }

    private var rivals: Int {
        store.laneContracts.filter { !$0.mine && $0.a == lane.a && $0.b == lane.b && $0.mode == lane.mode }.count
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(summary).font(.caption2).foregroundStyle(Theme.secondary)
            if lane.owners.isEmpty {
                Text("No firm owns capacity here; outside carriers haul it.").font(.caption2).foregroundStyle(Theme.secondary)
            }
            ForEach(lane.owners, id: \.firm) { owner in
                Text("\(owner.name): \(owner.lots.formatted()) lots")
                    .font(.caption2).foregroundStyle(owner.mine ? Theme.gold : Theme.primaryText)
            }
            ForEach(mine, id: \.id) { contract in
                Text("Your contract: \(contract.heldLots.formatted()) lots at \(Format.price(contract.feeCentsPerLot)), \(contract.weeksLeft) weeks left")
                    .font(.caption2).foregroundStyle(Theme.gold)
            }
            if rivals > 0 {
                Text("\(rivals) rival contract\(rivals == 1 ? "" : "s") on this lane").font(.caption2).foregroundStyle(Theme.warning)
            }
            if lane.reservableLots >= 10, mine.isEmpty, store.selectedFirm != nil {
                reserve
            }
            if let rejection {
                Text(rejection.message).font(.caption2).foregroundStyle(Theme.negative)
            }
        }
        .padding(.leading, 20)
    }

    private var summary: String {
        var text = "\(lane.modeName) · \(lane.km) km · \(lane.usedLots.formatted()) of \(lane.capacityLots.formatted()) lots used"
        if lane.heldLots > 0 { text += ", \(lane.heldLots.formatted()) reserved" }
        if lane.outsideLots > 0 {
            text += ", \(lane.outsideLots.formatted()) overflow to outside carriers at \(Format.price(lane.outsideRateCentsPerLot))"
        }
        return text
    }

    private var reserve: some View {
        let most = Double(max(10, min(lane.reservableLots, Int64(lane.capacityLots) * 8 / 10)))
        return VStack(alignment: .leading, spacing: 6) {
            ValueSlider(
                value: $lots, range: 10...most, step: 10, ticks: 10,
                label: { "Reserve \(Int($0).formatted()) lots a week" },
                detail: { _ in "Locked at \(Format.price(lane.rateCentsPerLot * 9 / 10)) a lot, paid every week whether you ship or not. Rivals cannot use these lots." }
            )
            ValueSlider(value: $weeks, range: 4...52, step: 1, ticks: 12, label: { "\(Int($0)) weeks" })
            Button {
                Task { rejection = await send() }
            } label: {
                PixelLabel("Reserve lane", icon: "lane").font(.callout.weight(.semibold)).frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(Theme.gold)
            .foregroundStyle(Theme.ink)
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

/// A division's standing shipping policy for the freight it pays for, and what a carrier division
/// carries. Set once; the engine routes every week.
struct DivisionShipping: View {
    @EnvironmentObject private var store: AppStore
    let division: DivisionDto
    @State private var rejection: CommandRejection?

    private static let policies = ["Cheapest", "Fastest", "Own first", "Reliable"]
    private static let modes = ["Any", "Truck", "Rail", "Pipeline", "Ship"]

    var body: some View {
        if let plant = division.plants.first {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    PixelIcon("lane", size: 12).foregroundStyle(Theme.secondary)
                    Eyebrow(text: "Shipping")
                }
                if !division.carrierMode.isEmpty {
                    let where_ = division.serves.isEmpty ? "the busiest lanes in reach" : division.serves.joined(separator: ", ")
                    Text("Carries freight by \(division.carrierMode), on \(where_). Its capacity is sold to every shipper; your group ships on it first, at cost.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                } else {
                    pills(Self.policies, selected: Int(division.shipKind)) { kind in
                        Task { rejection = await set(plant, kind: kind, mode: division.shipMode) }
                    }
                    pills(Self.modes, selected: Int(division.shipMode) + 1) { index in
                        Task { rejection = await set(plant, kind: Int(division.shipKind), mode: Int32(index - 1)) }
                    }
                    Text("Applies to goods arriving here. A preferred mode is used when a route allows it.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
                if let rejection {
                    Text(rejection.message).font(.caption).foregroundStyle(Theme.negative)
                }
            }
        }
    }

    private func pills(_ names: [String], selected: Int, pick: @escaping (Int) -> Void) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(Array(names.enumerated()), id: \.offset) { index, name in
                    Button { pick(index) } label: {
                        Text(name).font(.caption.weight(.semibold))
                            .padding(.horizontal, 10).padding(.vertical, 5)
                            .foregroundStyle(index == selected ? Theme.ink : Theme.primaryText)
                            .background(index == selected ? Theme.gold : Theme.panel, in: Capsule())
                            .overlay(Capsule().stroke(Theme.stroke, lineWidth: index == selected ? 0 : 1))
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(index == selected ? .isSelected : [])
                }
            }
        }
    }

    private func set(_ plant: UInt64, kind: Int, mode: Int32) async -> CommandRejection? {
        var fields: [String: Any] = ["facility": plant, "kind": kind]
        if mode >= 0 { fields["mode"] = Int(mode) }
        return await store.submit(CommandJSON.command("SetShippingPolicy", fields))
    }
}
