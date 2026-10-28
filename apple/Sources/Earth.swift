import Foundation

/// The real Earth under the map: NASA's Blue Marble (public domain), baked by tools/earth_tex.py
/// into a world image for the globe and a sharp crop per scenario for close up. The seasons follow
/// the game's calendar.
enum Earth {
    /// Week of the year 1..52 from a label such as "2000 W10"; midsummer when there is none.
    static func week(of label: String?) -> Int {
        guard let label, let range = label.range(of: #"W(\d+)"#, options: .regularExpression),
              let week = Int(label[range].dropFirst()) else { return 28 }
        return week
    }
}
