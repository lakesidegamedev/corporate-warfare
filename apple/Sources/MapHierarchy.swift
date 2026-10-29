import SwiftUI

/// What a city is known for, so close in a port, a steel town and a car town do not all read as the
/// same grey block. The map data has no such field, so this is a short table of the cities the
/// scenarios use; anything not listed stays a plain city block of towers. The Compose app keeps the
/// same table in MapHierarchy.kt; change both together.
enum CityKinds {
    private static let ports: Set<String> = [
        "Long Beach", "San Diego", "Oakland", "Houston", "New Orleans", "Corpus Christi", "Hamburg", "Rotterdam",
        "Antwerpen", "Bremen", "Baltimore", "Norfolk", "Virginia Beach", "Gent", "Szczecin", "Stockton",
    ]
    private static let steel: Set<String> = [
        "Pittsburgh", "Cleveland", "Duisburg", "Essen", "Dortmund", "Saarbrücken", "Liège", "Hamilton", "Buffalo", "Akron", "Wuppertal",
    ]
    private static let autos: Set<String> = ["Detroit", "Stuttgart", "Munich", "Cologne", "Dayton", "Mannheim", "Düsseldorf", "Wolfsburg"]

    /// The model family for a city: city_port, city_steel, city_auto, or plain city.
    static func of(_ name: String) -> String {
        if ports.contains(name) { return "city_port" }
        if steel.contains(name) { return "city_steel" }
        if autos.contains(name) { return "city_auto" }
        return "city"
    }
}

/// Decides which map labels show, so none overlap each other or a building, and keeps the answer
/// steady from frame to frame. Callers reserve every building and badge first, then place labels
/// in priority order (cities, then regions): a label that does not fit is dropped, so the lower
/// priority one always loses. A label that was showing last frame keeps its place unless it truly
/// collides, and one that was hidden needs a few points of clear air to come back, so a label at
/// the edge of fitting does not blink while the map pans or zooms.
final class LabelPlanner {
    private var shown = Set<String>()
    private var next = Set<String>()
    private var slotOf: [String: Int] = [:]
    private var nextSlot: [String: Int] = [:]
    private var taken: [CGRect] = []

    /// Starts a frame.
    func begin() {
        taken.removeAll(keepingCapacity: true)
        next = []
        nextSlot = [:]
    }

    /// Ends the frame; what was placed becomes what was showing.
    func end() {
        shown = next
        slotOf = nextSlot
    }

    /// Marks a screen rectangle as occupied.
    func reserve(_ rect: CGRect) { taken.append(rect) }

    private func free(_ r: CGRect) -> Bool {
        for t in taken where r.minX < t.maxX && r.maxX > t.minX && r.minY < t.maxY && r.maxY > t.minY { return false }
        return true
    }

    /// Tries the label's candidate top-left corners, the one it had last time first, and reserves the
    /// first that is clear. Returns the corner used, or nil when the label is dropped.
    func place(_ key: String, size: CGSize, slots: [CGPoint]) -> CGPoint? {
        guard !slots.isEmpty else { return nil }
        let kept = shown.contains(key)
        // A label already showing may squeeze a point or two; a hidden one needs clear air.
        let pad: CGFloat = kept ? -1.5 : 3
        let first = kept ? (slotOf[key] ?? 0) : 0
        for j in 0..<slots.count {
            let i = (first + j) % slots.count
            let at = slots[i]
            let rect = CGRect(x: at.x, y: at.y, width: size.width, height: size.height)
            if free(rect.insetBy(dx: -pad, dy: -pad)) {
                taken.append(rect)
                next.insert(key)
                nextSlot[key] = i
                return at
            }
        }
        return nil
    }
}

/// How freight on a lane is coloured: the player gold, a rival in its sector's colour, outside carriers grey.
enum FreightLook {
    static let outside = RGB(hex: 0x8A939C)
    private static var sectorColors: [String: RGB] = [:]

    static func sectorColor(_ sector: String) -> RGB {
        if let known = sectorColors[sector] { return known }
        let color = MapScene.sectorRGB(sector)
        sectorColors[sector] = color
        return color
    }

    /// The owner colour of a lane: gold when the player moves goods on it, else the sector of the firm that
    /// holds the most capacity, else grey when outside carriers carry more than any firm.
    static func ownerColor(_ lane: LaneDto, firmSector: [UInt64: String]) -> RGB {
        if laneIsMine(lane) { return RGB.gold }
        guard let top = lane.owners.filter({ !$0.mine }).max(by: { $0.lots < $1.lots }),
              top.lots > 0, Int(top.lots) >= Int(lane.outsideLots),
              let sector = firmSector[top.firm] else { return outside }
        return sectorColor(sector)
    }

    /// Each firm's biggest industry by plants, to colour its freight.
    static func firmSectors(_ plants: [MapPlantDto]) -> [UInt64: String] {
        var blocks: [UInt64: [String: Int]] = [:]
        for plant in plants { blocks[plant.firm, default: [:]][plant.industry, default: 0] += Int(plant.blocks) }
        return blocks.compactMapValues { industries in industries.max { $0.value < $1.value }?.key }
    }

    /// Vehicles that may stand on a lane at once in one direction. Neighbours keep at least two vehicle
    /// lengths apart, so a busy lane reads as busy without becoming one unbroken line of specks.
    static func maxPerLane(laneLength: Double, vehicleLength: Double) -> Int {
        max(Int((laneLength / (2 * max(vehicleLength, 1e-3))).rounded(.down)), 1)
    }
}
