import SwiftUI

/// The CEO Office as a room (Resources/office-room.json, drawn by tools/office_room.py and shared
/// with the Compose apps): a still picture, a front layer the people sit behind, a few swappable
/// sprites, and for every tappable object its rectangle, its spoken label and where live figures go.
enum OfficeRoomArt {
    struct Layer: Decodable {
        let palette: [String: String]
        let rows: [String]
    }

    struct Sprite: Decodable {
        let at: [Int]?
        let palette: [String: String]
        let rows: [String]
    }

    struct Object: Decodable {
        let id: String
        let label: String
        let rect: [Int]
        let live: [Int]?
    }

    struct File: Decodable {
        let size: [Int]
        let room: Layer
        let front: Layer
        let sprites: [String: Sprite]
        let trophySlots: [[Int]]
        let objects: [Object]

        enum CodingKeys: String, CodingKey {
            case size, room, front, sprites, objects
            case trophySlots = "trophy_slots"
        }
    }

    static let file: File? = Bundle.main.url(forResource: "office-room", withExtension: "json")
        .flatMap { try? Data(contentsOf: $0) }
        .flatMap { try? JSONDecoder().decode(File.self, from: $0) }

    static func object(_ id: String) -> Object? { file?.objects.first { $0.id == id } }
}

/// A part of the CEO Office's own details, shown from the room.
enum OfficeSection: String, Identifiable {
    case brief, goal, executives, figures
    var id: String { rawValue }
}

/// Where a tapped object goes: a whole screen, or a part of the CEO Office.
enum OfficeRoute: Equatable {
    case screen(GamePage)
    case section(OfficeSection)

    /// Every object in the room and where it leads. Nil for an id that is not an object.
    static func of(_ id: String) -> OfficeRoute? {
        switch id {
        case "window": return .screen(.map)
        case "chart", "cabinet": return .screen(.stats)
        case "phone": return .screen(.inbox)
        case "paper": return .screen(.paper)
        case "board", "trophies": return .section(.goal)
        case "door": return .screen(.research)
        case "chief", "nameplate": return .screen(.actions)
        case "mandate": return .section(.goal)
        case "memo": return .section(.brief)
        case "safe": return .section(.figures)
        case "exec0", "exec1", "exec2": return .section(.executives)
        default: return nil
        }
    }
}

/// The figures the room shows, read from what the store already has.
struct OfficeFacts {
    struct Seat {
        let title: String
        let short: String
        let portrait: String?
        let name: String?
    }

    let firmName: String
    let chief: String?
    var chiefName: String? = nil
    let chiefMood: String
    let profitText: String
    let profitGood: Bool
    let profitSeries: [Int64]
    let unread: Int
    let headline: String
    let mandateTitle: String?
    let mandateProgressBps: Int
    let mandateLost: Bool
    let offerWaiting: Bool
    let boardBusts: [String]
    let boardMood: String
    let seats: [Seat]
    let cash: String
    let credit: String
    let trophiesEarned: Int
    let trophiesTotal: Int
    let briefMoves: Int?
    let region: String

    /// The mood the chief wears: what the quarter's profit says.
    static func chiefMood(_ quarter: Int64?) -> String {
        guard let quarter else { return "neutral" }
        return quarter > 0 ? "pleased" : quarter < 0 ? "displeased" : "neutral"
    }

    @MainActor
    static func read(_ store: AppStore) -> OfficeFacts? {
        guard let firm = store.selectedFirmDto else { return nil }
        let overview = store.overview
        let quarter: Int64? = overview?.primaryFirm == firm.handle ? overview?.quarterNetIncomeCents : nil
        let mandate = store.activeMandate
        let rivalChiefs = Set(store.rivals.map { $0.tag.ceoId })
        let chiefs = CastPortraits.office.keys.sorted().filter { !rivalChiefs.contains($0) }
        let own = store.playerChief
        let chief = own?.portrait ?? (chiefs.isEmpty ? nil : chiefs[Int(firm.handle % UInt64(chiefs.count))])
        let extras = CastPortraits.all.keys.filter { $0.hasPrefix("x_") && !rivalChiefs.contains($0) }.sorted()
        let start = extras.isEmpty ? 0 : Int(firm.handle &* 5 % UInt64(extras.count))
        let board = extras.isEmpty ? [] : (0..<5).map { extras[(start + $0) % extras.count] }
        let boardMood: String
        if let mandate {
            boardMood = mandate.mandate.lost ? "displeased" : (mandate.mandate.progressBps >= 5_000 ? "pleased" : "neutral")
        } else {
            boardMood = "neutral"
        }
        let titles = [("Chief financial officer", "CFO"), ("Chief operating officer", "COO"), ("Logistics chief", "LOG")]
        let posts = store.executives?.posts ?? []
        let seats = (0..<3).map { i -> Seat in
            let post = i < posts.count ? posts[i] : nil
            return Seat(title: post?.title ?? titles[i].0, short: titles[i].1, portrait: post?.holder?.portraitId, name: post?.holder?.name)
        }
        return OfficeFacts(
            firmName: firm.name, chief: chief, chiefName: own?.name, chiefMood: chiefMood(quarter),
            profitText: quarter.map { Format.moneyCompact($0, currency: firm.currency) } ?? "n/a",
            profitGood: (quarter ?? 0) >= 0,
            profitSeries: store.history.suffix(13).map(\.netIncomeCents),
            unread: store.letters.filter { !$0.read }.count,
            headline: store.headline ?? "A quiet week",
            mandateTitle: mandate?.mandate.title, mandateProgressBps: Int(mandate?.mandate.progressBps ?? 0),
            mandateLost: mandate?.mandate.lost ?? false, offerWaiting: store.boardOffer != nil,
            boardBusts: board, boardMood: boardMood, seats: seats,
            cash: Format.moneyCompact(firm.cashCents, currency: firm.currency), credit: firm.grade,
            trophiesEarned: store.hall?.achievements.filter(\.earned).count ?? 0,
            trophiesTotal: store.hall?.achievements.count ?? 0,
            briefMoves: store.brief?.moves.count, region: firm.region
        )
    }

    /// What VoiceOver says for an object, with its figure.
    func describe(_ id: String, label: String) -> String {
        switch id {
        case "chief":
            let look = chiefMood == "pleased" ? "pleased with the results" : chiefMood == "displeased" ? "unhappy with the results" : "steady"
            return "\(chiefName.map { "\($0), chief executive" } ?? "Chief executive") at the desk of \(firmName), looking \(look). Opens Actions."
        case "nameplate": return "Nameplate, \(firmName). Opens Actions."
        case "window": return "Window onto the map at your headquarters in \(region). Opens the map."
        case "chart": return "Wall chart: profit \(profitText). Opens Financials."
        case "phone":
            return unread > 0 ? "Phone, \(unread) unread \(unread == 1 ? "letter" : "letters"). Opens the Inbox." : "Phone, no unread letters. Opens the Inbox."
        case "paper": return "Newspaper, today's headline: \(headline). Opens the Paper."
        case "board": return "Board table. Shows the chapter goal."
        case "mandate":
            if let mandateTitle { return "Mandate card: \(mandateTitle), \(mandateProgressBps / 100) percent\(mandateLost ? ", lost" : ""). Shows the chapter goal." }
            return offerWaiting ? "Mandate card: the board has an offer waiting. Shows the chapter goal." : "Mandate card: no goal set. Shows the chapter goal."
        case "exec0", "exec1", "exec2":
            guard let index = Int(id.suffix(1)), index < seats.count else { return label }
            let seat = seats[index]
            if seat.portrait != nil { return "\(seat.title)\(seat.name.map { ", \($0)" } ?? ""). Opens Executives." }
            return "Empty chair, \(seat.title). Hire. Opens Executives."
        case "cabinet": return "Filing cabinet, financial statements. Opens Financials."
        case "safe": return "Safe, company cash \(cash), credit \(credit). Shows the company figures."
        case "trophies": return "Trophy shelf, \(trophiesEarned) of \(trophiesTotal) achievements earned. Shows the chapter goal and achievements."
        case "door": return "Door to Research."
        case "memo": return "Memo, this week's brief\(briefMoves.map { ", \($0) \($0 == 1 ? "move" : "moves")" } ?? ""). Shows the brief."
        default: return label
        }
    }

    /// The short words that show on hover and long press.
    func short(_ id: String) -> String {
        switch id {
        case "chief": return "Chief executive"
        case "window": return "Map"
        case "chart": return "Financials"
        case "phone": return unread > 0 ? "Inbox, \(unread) new" : "Inbox"
        case "paper": return "The paper"
        case "board": return "The board"
        case "mandate": return "Chapter goal"
        case "exec0", "exec1", "exec2":
            guard let index = Int(id.suffix(1)), index < seats.count else { return "Executive" }
            return seats[index].portrait != nil ? seats[index].title : "Hire \(seats[index].title.lowercased())"
        case "cabinet": return "Statements"
        case "safe": return "Cash and credit"
        case "trophies": return "Achievements"
        case "door": return "Research"
        case "memo": return "This week's brief"
        default: return id
        }
    }
}

/// The room. Scaled by whole pixels so the art stays crisp, centred when there is spare width. With
/// Reduce Motion on, the phone's light stays steady instead of blinking.
struct OfficeScene: View {
    let facts: OfficeFacts
    var onRoute: (OfficeRoute) -> Void

    @Environment(\.displayScale) private var displayScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var lit = true
    @State private var labelled: String?

    var body: some View {
        if let art = OfficeRoomArt.file {
            GeometryReader { proxy in
                let unit = max(1, (proxy.size.width * displayScale / CGFloat(art.size[0])).rounded(.down))
                let px = unit / displayScale
                let w = px * CGFloat(art.size[0])
                let h = px * CGFloat(art.size[1])
                ZStack(alignment: .top) {
                    room(art, px: px)
                        .frame(width: w, height: h, alignment: .topLeading)
                        .clipped()
                }
                .frame(width: proxy.size.width, height: h, alignment: .top)
                .background(Theme.ink)
            }
            .aspectRatio(CGFloat(art.size[0]) / CGFloat(art.size[1]), contentMode: .fit)
            .task(id: facts.unread > 0 && !reduceMotion) {
                lit = true
                while facts.unread > 0 && !reduceMotion && !Task.isCancelled {
                    try? await Task.sleep(nanoseconds: 650_000_000)
                    lit.toggle()
                }
            }
        }
    }

    private func image(_ key: String, _ layer: OfficeRoomArt.Layer) -> some View {
        Group {
            if let cg = PixelIcons.bitmap(key: "office|\(key)", rows: layer.rows, palette: layer.palette) {
                Image(decorative: cg, scale: 1).interpolation(.none).resizable()
            }
        }
    }

    private func sprite(_ key: String, _ art: OfficeRoomArt.File, px: CGFloat) -> some View {
        Group {
            if let s = art.sprites[key], let at = s.at, at.count == 2,
               let cg = PixelIcons.bitmap(key: "office|\(key)", rows: s.rows, palette: s.palette) {
                Image(decorative: cg, scale: 1).interpolation(.none).resizable()
                    .frame(width: px * CGFloat(cg.width), height: px * CGFloat(cg.height))
                    .position(x: px * (CGFloat(at[0]) + CGFloat(cg.width) / 2), y: px * (CGFloat(at[1]) + CGFloat(cg.height) / 2))
            }
        }
    }

    private struct Place: ViewModifier {
        let r: [Int]
        let px: CGFloat
        func body(content: Content) -> some View {
            content.frame(width: px * CGFloat(r[2]), height: px * CGFloat(r[3])).position(x: px * (CGFloat(r[0]) + CGFloat(r[2]) / 2), y: px * (CGFloat(r[1]) + CGFloat(r[3]) / 2))
        }
    }

    private func text(_ s: String, _ color: Color, _ size: CGFloat, px: CGFloat, bold: Bool = false, lines: Int = 1) -> some View {
        Text(s).font(Pixel.font(size * px, bold: bold)).foregroundStyle(color).lineLimit(lines).minimumScaleFactor(0.5).multilineTextAlignment(.center)
    }

    private func fit(_ s: String, _ width: Int, _ most: CGFloat) -> CGFloat { min(most, CGFloat(width) / (CGFloat(max(s.count, 1)) * 0.78)) }

    @ViewBuilder
    private func room(_ art: OfficeRoomArt.File, px: CGFloat) -> some View {
        let w = px * CGFloat(art.size[0])
        let h = px * CGFloat(art.size[1])
        ZStack(alignment: .topLeading) {
            image("room", art.room).frame(width: w, height: h)

            // The window onto the headquarters region, then its mullions.
            if let r = OfficeRoomArt.object("window")?.live {
                windowGlass(px: px).modifier(Place(r: r, px: px)).clipped()
            }
            // The chief behind the desk, the board's busts behind their table, the executives in their chairs.
            if let r = OfficeRoomArt.object("chief")?.live, let chief = facts.chief {
                PixelPortrait(person: chief, size: px * 64, mood: facts.chiefMood, office: true).modifier(Place(r: r, px: px))
            }
            if let r = OfficeRoomArt.object("board")?.live {
                ForEach(Array(facts.boardBusts.enumerated()), id: \.offset) { k, id in
                    PixelPortrait(person: id, size: px * 16, mood: facts.boardMood, forceSmall: true)
                        .position(x: px * CGFloat(r[0] + 4 + k * 20 + 8), y: px * CGFloat(r[1] + 4 + 8))
                }
            }
            ForEach(0..<3, id: \.self) { k in
                if let r = OfficeRoomArt.object("exec\(k)")?.live, k < facts.seats.count, let id = facts.seats[k].portrait {
                    PixelPortrait(person: id, size: px * 32).modifier(Place(r: r, px: px))
                }
            }

            image("front", art.front).frame(width: w, height: h)
            sprite("window_bars", art, px: px)
            sprite(facts.unread > 0 && lit ? "phone_on" : "phone_off", art, px: px)
            sprite("memo", art, px: px)
            ForEach(Array(art.trophySlots.enumerated()), id: \.offset) { k, slot in
                let key = k < facts.trophiesEarned ? "trophy_on" : "trophy_off"
                if let s = art.sprites[key], let cg = PixelIcons.bitmap(key: "office|\(key)", rows: s.rows, palette: s.palette) {
                    Image(decorative: cg, scale: 1).interpolation(.none).resizable()
                        .frame(width: px * CGFloat(cg.width), height: px * CGFloat(cg.height))
                        .position(x: px * (CGFloat(slot[0]) + CGFloat(cg.width) / 2), y: px * (CGFloat(slot[1]) + CGFloat(cg.height) / 2))
                }
            }

            figures(px: px)

            // Tap, hover and long press targets, biggest first so small objects stay on top.
            ForEach(art.objects.sorted { $0.rect[2] * $0.rect[3] > $1.rect[2] * $1.rect[3] }.filter { $0.id != "nameplate" }, id: \.id) { o in
                if let route = OfficeRoute.of(o.id) {
                    hotspot(o, route, px: px)
                }
            }
        }
        .frame(width: w, height: h)
    }

    /// The window: the headquarters region's name and size over flat sea and land, drawn in room pixels.
    private func windowGlass(px: CGFloat) -> some View {
        ZStack {
            Rectangle().fill(Color(red: 0.086, green: 0.208, blue: 0.333))
            Rectangle().fill(Color(red: 0.169, green: 0.231, blue: 0.290)).frame(height: px * 18).frame(maxHeight: .infinity, alignment: .bottom)
            VStack(spacing: px) {
                Rectangle().fill(Theme.gold).frame(width: px * 4, height: px * 4)
                text(facts.region.uppercased(), Theme.primaryText, 5, px: px, bold: true)
            }
        }
    }

    @ViewBuilder
    private func figures(px: CGFloat) -> some View {
        if let r = OfficeRoomArt.object("chart")?.live {
            VStack(alignment: .leading, spacing: px) {
                text(facts.profitText, facts.profitGood ? Theme.positive : Theme.negative, 7, px: px, bold: true)
                Canvas { context, size in
                    let series = facts.profitSeries
                    let color = facts.profitGood ? Theme.positive : Theme.negative
                    guard series.count > 1, let lo = series.min().map({ min($0, 0) }), let hi = series.max().map({ max($0, 0) }) else { return }
                    let span = Double(max(hi - lo, 1))
                    func y(_ v: Int64) -> CGFloat { size.height - 2 * px - CGFloat(Double(v - lo) / span) * (size.height - 3 * px) }
                    var zero = Path()
                    zero.move(to: CGPoint(x: 0, y: y(0))); zero.addLine(to: CGPoint(x: size.width, y: y(0)))
                    context.stroke(zero, with: .color(Color(red: 0.227, green: 0.314, blue: 0.408)), lineWidth: px)
                    var line = Path()
                    for (i, v) in series.enumerated() {
                        let p = CGPoint(x: CGFloat(i) * size.width / CGFloat(series.count - 1), y: y(v))
                        if i == 0 { line.move(to: p) } else { line.addLine(to: p) }
                    }
                    context.stroke(line, with: .color(color), style: StrokeStyle(lineWidth: px, lineCap: .square, lineJoin: .miter))
                }
            }
            .modifier(Place(r: r, px: px))
        }
        if let r = OfficeRoomArt.object("safe")?.live {
            text(facts.cash, Theme.gold, fit(facts.cash, r[2], 6.5), px: px).modifier(Place(r: r, px: px))
        }
        if let r = OfficeRoomArt.object("nameplate")?.live {
            text(facts.firmName.uppercased(), Theme.gold, fit(facts.firmName, r[2], 5.5), px: px).modifier(Place(r: r, px: px))
        }
        if let r = OfficeRoomArt.object("paper")?.live {
            text(facts.headline, Color(red: 0.043, green: 0.086, blue: 0.133), 4.4, px: px, bold: true, lines: 3)
                .modifier(Place(r: [r[0] + 2, r[1] + 7, r[2] - 4, r[3] - 8], px: px)).clipped()
        }
        if let r = OfficeRoomArt.object("trophies")?.live {
            text("AWARDS \(facts.trophiesEarned)/\(facts.trophiesTotal)", Theme.gold, 5.2, px: px)
                .modifier(Place(r: [r[0], r[1], r[2], 7], px: px))
        }
        if let r = OfficeRoomArt.object("memo")?.live {
            text(facts.briefMoves.map { "BRIEF \($0)" } ?? "BRIEF", Color(red: 0.043, green: 0.086, blue: 0.133), 4.6, px: px, bold: true)
                .modifier(Place(r: [r[0] + 2, r[1] + 7, r[2] - 4, 8], px: px))
        }
        if facts.unread > 0, let r = OfficeRoomArt.object("phone")?.live {
            text(facts.unread > 9 ? "9+" : "\(facts.unread)", .white, 5.5, px: px, bold: true)
                .modifier(Place(r: [r[0] + 14, r[1] - 8, 9, 8], px: px))
                .background(Theme.negative.frame(width: px * 9, height: px * 8).position(x: px * CGFloat(r[0] + 14) + px * 4.5, y: px * CGFloat(r[1] - 8) + px * 4))
        }
        if let r = OfficeRoomArt.object("mandate")?.live {
            let title = facts.mandateTitle ?? (facts.offerWaiting ? "Board offer waiting" : "No goal set")
            ZStack(alignment: .topLeading) {
                Rectangle().fill(Color(red: 0.91, green: 0.878, blue: 0.8))
                Rectangle().stroke(Color(red: 0.043, green: 0.086, blue: 0.133), lineWidth: px)
                text(title.uppercased(), Color(red: 0.043, green: 0.086, blue: 0.133), 4.6, px: px, bold: true, lines: 3)
                    .padding(px * 2).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                if facts.mandateTitle != nil {
                    GeometryReader { g in
                        ZStack(alignment: .leading) {
                            Rectangle().fill(Color(red: 0.741, green: 0.706, blue: 0.612))
                            Rectangle().fill(facts.mandateLost ? Theme.negative : Color(red: 0.184, green: 0.541, blue: 0.369))
                                .frame(width: g.size.width * min(max(CGFloat(facts.mandateProgressBps) / 10_000, 0.03), 1))
                        }
                        .frame(height: px * 3)
                        .position(x: g.size.width / 2, y: g.size.height - px * 1.5)
                    }
                }
            }
            .modifier(Place(r: r, px: px))
        }
        ForEach(0..<3, id: \.self) { k in
            if let r = OfficeRoomArt.object("exec\(k)")?.live, k < facts.seats.count {
                if facts.seats[k].portrait == nil {
                    text("HIRE", Theme.gold, 6.5, px: px, bold: true).modifier(Place(r: [r[0] + 2, r[1] + 14, 28, 9], px: px))
                }
                text(facts.seats[k].short, Color(red: 0.91, green: 0.878, blue: 0.8), 5, px: px, bold: true)
                    .modifier(Place(r: [r[0] + 2, r[1] + 33, 28, 7], px: px))
            }
        }
    }

    private func hotspot(_ o: OfficeRoomArt.Object, _ route: OfficeRoute, px: CGFloat) -> some View {
        ZStack(alignment: .top) {
            Color.clear.contentShape(Rectangle())
            if labelled == o.id {
                Rectangle().stroke(Theme.gold, lineWidth: px)
                Text(facts.short(o.id).uppercased())
                    .font(Pixel.font(px * 5.6, bold: true)).foregroundStyle(Theme.gold).lineLimit(1)
                    .padding(.horizontal, px * 2).padding(.vertical, px)
                    .background(Theme.ink).overlay(Rectangle().stroke(Theme.gold, lineWidth: px))
            }
        }
        .frame(width: px * CGFloat(o.rect[2]), height: px * CGFloat(o.rect[3]))
        .position(x: px * (CGFloat(o.rect[0]) + CGFloat(o.rect[2]) / 2), y: px * (CGFloat(o.rect[1]) + CGFloat(o.rect[3]) / 2))
        .onTapGesture { onRoute(route) }
        .onLongPressGesture(minimumDuration: 0.4) {
            labelled = o.id
            Task {
                try? await Task.sleep(nanoseconds: 1_800_000_000)
                if labelled == o.id { labelled = nil }
            }
        }
        .onHover { labelled = $0 ? o.id : (labelled == o.id ? nil : labelled) }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(facts.describe(o.id, label: o.label))
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { onRoute(route) }
    }
}

/// The room wired to the store, with the screens and sections it opens.
struct OfficeRoomHost: View {
    @EnvironmentObject private var store: AppStore
    @State private var screen: GamePage?
    @State private var section: OfficeSection?

    var body: some View {
        Group {
            if let facts = OfficeFacts.read(store) {
                OfficeScene(facts: facts) { route in
                    switch route {
                    case .screen(let page): screen = page
                    case .section(let part): section = part
                    }
                }
            }
        }
        .task(id: store.overview?.tick) { await store.refreshHall() }
        .navigationDestination(item: $screen) { page in
            Group {
                switch page {
                case .map: MapHomeView(embedded: true)
                case .stats: StatsView()
                case .research: ResearchView()
                case .actions: CommandCenterView()
                case .inbox: InboxView()
                case .paper: PaperView()
                default: EmptyView()
                }
            }
            .environmentObject(store)
        }
        .sheet(item: $section) { part in
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    if let firm = store.selectedFirmDto {
                        switch part {
                        case .brief: BriefPanel(currency: firm.currency, firmName: firm.name)
                        case .goal: ChapterBlock(); HallView()
                        case .executives: ExecutivesPanel(currency: firm.currency)
                        case .figures:
                            Text("\(firm.name): cash \(Format.moneyCompact(firm.cashCents, currency: firm.currency)), credit \(firm.grade)")
                                .font(.callout.weight(.semibold)).foregroundStyle(Theme.primaryText)
                        }
                    }
                }
                .padding(16)
            }
            .environmentObject(store)
            .presentationDetents([.medium, .large])
            .presentationBackground(Theme.panel)
        }
    }
}
