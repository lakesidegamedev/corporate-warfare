import Foundation
import SwiftUI

// The model behind the data grid on iPad and Mac: timeframes, column modes, filters, the sort and
// saved views. Plain values and functions with no view code, so the unit tests cover them on their
// own. It follows the desktop grid (compose GridView.kt) so a view means the same on every platform.

/// How far back a change, a percent change or a trend looks. A total or a change column reads its
/// history over this window; a plain figure ignores it.
enum GridTimeframe: String, Codable, CaseIterable, Identifiable {
    case week, quarter, year, career

    var id: String { rawValue }

    var label: String {
        switch self {
        case .week: return "Week"
        case .quarter: return "Quarter"
        case .year: return "Year"
        case .career: return "Career"
        }
    }

    /// Weeks covered, or nil for the whole career (all the history the game keeps).
    var weeks: Int? {
        switch self {
        case .week: return 1
        case .quarter: return 13
        case .year: return 52
        case .career: return nil
        }
    }

    static let standard = GridTimeframe.quarter
}

/// How a numeric column shows: the figure, its change over the timeframe, the change in percent,
/// or a small trend line.
enum GridColumnMode: String, Codable, CaseIterable {
    case value, change, percent, trend

    var label: String {
        switch self {
        case .value: return "Value"
        case .change: return "Change"
        case .percent: return "Change %"
        case .trend: return "Trend"
        }
    }

    /// The heading suffix a mode adds to the column title.
    func title(_ base: String) -> String {
        switch self {
        case .value: return base
        case .change: return "\(base) chg"
        case .percent: return "\(base) %"
        case .trend: return "\(base) trend"
        }
    }
}

/// One shown column and the way it shows. A column may show twice (the figure and its change).
struct GridColumnSpec: Codable, Hashable {
    var id: String
    var mode: GridColumnMode = .value

    init(_ id: String, _ mode: GridColumnMode = .value) {
        self.id = id
        self.mode = mode
    }

    /// Tells two ways of showing the same column apart, and names the sort.
    var key: String { mode == .value ? id : "\(id).\(mode.rawValue)" }

    /// Reads "price" or "price.percent" back into a spec.
    static func parse(_ key: String) -> GridColumnSpec {
        let parts = key.split(separator: ".", maxSplits: 1).map(String.init)
        if parts.count == 2, let mode = GridColumnMode(rawValue: parts[1]) { return GridColumnSpec(parts[0], mode) }
        return GridColumnSpec(key)
    }
}

/// Which rows by owner: everything, the player's own, or everyone else's.
enum GridOwner: String, Codable, CaseIterable {
    case all, mine, rivals

    var label: String {
        switch self {
        case .all: return "All"
        case .mine: return "You"
        case .rivals: return "Rivals"
        }
    }
}

/// The kinds of filter chip. Figures are in the units a player reads: money in whole units,
/// percent in percent.
enum GridFilterOp: String, Codable {
    case equals, not, above, below
}

/// One filter chip under the control bar.
struct GridFilter: Codable, Hashable {
    var op: GridFilterOp
    var column: String
    var text: String = ""
    var number: Double = 0

    func label(_ title: (String) -> String) -> String {
        switch op {
        case .equals: return "\(title(column)): \(text)"
        case .not: return "\(title(column)): not \(text)"
        case .above: return "\(title(column)) > \(GridFilter.trim(number))"
        case .below: return "\(title(column)) < \(GridFilter.trim(number))"
        }
    }

    static func trim(_ d: Double) -> String {
        d == d.rounded() && abs(d) < 1e15 ? String(Int64(d)) : String(d)
    }
}

/// A grid's whole shape, which a player can keep under a name.
struct GridViewState: Codable, Equatable {
    var name: String
    var columns: [GridColumnSpec]
    var sort: String?
    var descending: Bool = true
    var filters: [GridFilter] = []
    var owner: GridOwner = .all
    /// Scope choices that are on ("sector:chemicals", "region:TX"); empty means everything.
    var scope: [String] = []
    var timeframe: GridTimeframe = .standard

    init(_ name: String, _ columns: [String], sort: String? = nil, descending: Bool = true,
         filters: [GridFilter] = [], owner: GridOwner = .all, scope: [String] = [], timeframe: GridTimeframe = .standard) {
        self.name = name
        self.columns = columns.map(GridColumnSpec.parse)
        self.sort = sort
        self.descending = descending
        self.filters = filters
        self.owner = owner
        self.scope = scope
        self.timeframe = timeframe
    }

    /// The same shape whatever its name: what "changed since saved" compares.
    func sameShape(_ other: GridViewState) -> Bool {
        var a = self, b = other
        a.name = ""
        b.name = ""
        return a == b
    }

    /// A heading tap: the same column reverses, a new one sorts high first for figures.
    func sorted(by key: String, numeric: Bool) -> GridViewState {
        var next = self
        if sort == key {
            next.descending.toggle()
        } else {
            next.sort = key
            next.descending = numeric
        }
        return next
    }

    /// Switch a column on (at the end) or off (every way it shows). The sort leaves with it.
    func with(column id: String, on: Bool) -> GridViewState {
        var next = self
        if on {
            if !columns.contains(where: { $0.id == id }) { next.columns.append(GridColumnSpec(id)) }
        } else {
            next.columns.removeAll { $0.id == id }
            if let s = sort, GridColumnSpec.parse(s).id == id { next.sort = nil }
        }
        return next
    }

    /// Show a column one more way, or one fewer. The last way it shows stays: switch it off instead.
    func toggling(_ id: String, mode: GridColumnMode) -> GridViewState {
        var next = self
        let spec = GridColumnSpec(id, mode)
        let mine = columns.filter { $0.id == id }
        if let at = columns.firstIndex(of: spec) {
            guard mine.count > 1 else { return self }
            next.columns.remove(at: at)
            if sort == spec.key { next.sort = nil }
        } else if let last = mine.last, let at = columns.firstIndex(of: last) {
            next.columns.insert(spec, at: at + 1)
        } else {
            next.columns.append(spec)
        }
        return next
    }

    /// Add a chip; a figure chip replaces one of the same kind on the same column.
    func with(filter f: GridFilter) -> GridViewState {
        var next = self
        next.filters.removeAll { $0.op == f.op && $0.column == f.column && (f.op == .above || f.op == .below || $0.text == f.text) }
        next.filters.append(f)
        return next
    }

    func without(filter f: GridFilter) -> GridViewState {
        var next = self
        next.filters.removeAll { $0 == f }
        return next
    }
}

// MARK: - Timeframe maths

enum GridSeries {
    /// The points of a series inside a timeframe plus the point just before it, so a change can be
    /// measured; nil when the series does not reach back that far. [step] is weeks per point.
    static func window(_ series: [Int64], _ tf: GridTimeframe, step: Int = 1) -> [Int64]? {
        guard let weeks = tf.weeks else { return series.count >= 2 ? series : nil }
        let points = (weeks + max(step, 1) - 1) / max(step, 1)
        return series.count > points ? Array(series.suffix(points + 1)) : nil
    }

    /// Last minus first of the window.
    static func change(_ series: [Int64], _ tf: GridTimeframe, step: Int = 1) -> Int64? {
        window(series, tf, step: step).map { $0[$0.count - 1] - $0[0] }
    }

    /// The change in basis points of where the window started; nil when it starts at zero.
    static func changeBps(_ series: [Int64], _ tf: GridTimeframe, step: Int = 1) -> Int64? {
        guard let w = window(series, tf, step: step), let first = w.first, first != 0, let last = w.last else { return nil }
        return (last - first) * 10_000 / abs(first)
    }

    /// The sum of the points inside the timeframe (not the one before): a total over the window.
    static func total(_ series: [Int64], _ tf: GridTimeframe, step: Int = 1) -> Int64? {
        guard let weeks = tf.weeks else { return series.isEmpty ? nil : series.reduce(0, +) }
        let points = max((weeks + max(step, 1) - 1) / max(step, 1), 1)
        return series.count >= points ? series.suffix(points).reduce(0, +) : nil
    }
}

// MARK: - Rows: filter, search, sort

/// What the grid needs to know of one row to filter and sort it.
struct GridRowReader<Row> {
    /// A column's text, for equals and not chips.
    var text: (Row, String) -> String?
    /// A column's figure in the units a player reads, for above and below chips.
    var figure: (Row, String) -> Double?
    /// The figure to sort a column by, in the column's own units; nil reads as no figure.
    var sortValue: (Row, String) -> Int64?
    /// Whether a column sorts by its figure (true) or its text.
    var numeric: (String) -> Bool
    /// The player's own row.
    var mine: (Row) -> Bool
    /// The words a typed search looks through.
    var words: (Row) -> String
    /// Scope choices by id.
    var scope: (Row, String) -> Bool
}

enum GridRows {
    /// Owner, scope, chips, the typed search and then the sort. Rows with no figure in the sorted
    /// column go last whichever way it sorts.
    static func shown<Row>(_ rows: [Row], view: GridViewState, search: String, read: GridRowReader<Row>) -> [Row] {
        let kept = rows.filter { r in
            switch view.owner {
            case .all: break
            case .mine: if !read.mine(r) { return false }
            case .rivals: if read.mine(r) { return false }
            }
            if !view.scope.isEmpty && !view.scope.contains(where: { read.scope(r, $0) }) { return false }
            return passes(r, view.filters, read) && matches(r, search, read)
        }
        return sort(kept, by: view.sort, descending: view.descending, read: read)
    }

    static func passes<Row>(_ r: Row, _ filters: [GridFilter], _ read: GridRowReader<Row>) -> Bool {
        filters.allSatisfy { f in
            switch f.op {
            case .equals: return read.text(r, f.column)?.localizedCaseInsensitiveContains(f.text) == true
            case .not: return read.text(r, f.column)?.localizedCaseInsensitiveContains(f.text) != true
            case .above: return read.figure(r, f.column).map { $0 > f.number } ?? false
            case .below: return read.figure(r, f.column).map { $0 < f.number } ?? false
            }
        }
    }

    /// Every typed word must appear in the row's words.
    static func matches<Row>(_ r: Row, _ search: String, _ read: GridRowReader<Row>) -> Bool {
        let words = search.split(separator: " ").map(String.init).filter { !$0.isEmpty }
        if words.isEmpty { return true }
        let hay = read.words(r)
        return words.allSatisfy { hay.localizedCaseInsensitiveContains($0) }
    }

    static func sort<Row>(_ rows: [Row], by key: String?, descending: Bool, read: GridRowReader<Row>) -> [Row] {
        guard let key else { return rows }
        let indexed = Array(rows.enumerated())
        if read.numeric(key) {
            let sorted = indexed.sorted { a, b in
                let x = read.sortValue(a.element, key)
                let y = read.sortValue(b.element, key)
                switch (x, y) {
                case let (x?, y?): return x == y ? a.offset < b.offset : (descending ? x > y : x < y)
                case (.some, .none): return true
                case (.none, .some): return false
                case (.none, .none): return a.offset < b.offset
                }
            }
            return sorted.map(\.element)
        }
        let sorted = indexed.sorted { a, b in
            let x = (read.text(a.element, key) ?? "").lowercased()
            let y = (read.text(b.element, key) ?? "").lowercased()
            return x == y ? a.offset < b.offset : (descending ? x > y : x < y)
        }
        return sorted.map(\.element)
    }

    /// Reads typed figure terms ("price>10", "margin<5", "sector:chemicals") into chips and returns
    /// the plain words left over. [resolve] turns a typed column name into its id.
    static func parse(_ input: String, resolve: (String) -> String?) -> (chips: [GridFilter], words: String) {
        var chips: [GridFilter] = []
        var words: [String] = []
        for token in input.split(separator: " ").map(String.init) {
            if let at = token.firstIndex(where: { $0 == ">" || $0 == "<" }), at != token.startIndex {
                let name = String(token[..<at])
                let rest = token[token.index(after: at)...].trimmingCharacters(in: CharacterSet(charactersIn: "=%"))
                if let column = resolve(name), let n = Double(rest) {
                    chips.append(GridFilter(op: token[at] == ">" ? .above : .below, column: column, number: n))
                    continue
                }
            }
            if let at = token.firstIndex(of: ":"), at != token.startIndex, token.index(after: at) != token.endIndex {
                let negate = token.hasPrefix("-") || token.hasPrefix("!")
                let name = String(token[token.index(token.startIndex, offsetBy: negate ? 1 : 0)..<at])
                if let column = resolve(name) {
                    let text = String(token[token.index(after: at)...]).replacingOccurrences(of: "_", with: " ")
                    chips.append(GridFilter(op: negate ? .not : .equals, column: column, text: text))
                    continue
                }
            }
            words.append(token)
        }
        return (chips, words.joined(separator: " "))
    }
}

// MARK: - Saved views

/// Where views are kept: UserDefaults in the app, a dictionary in tests. Never the save file.
protocol GridPrefs: AnyObject {
    func read(_ key: String) -> String?
    func write(_ key: String, _ value: String?)
}

final class DefaultsGridPrefs: GridPrefs {
    static let shared = DefaultsGridPrefs()
    private let defaults: UserDefaults

    init(_ defaults: UserDefaults = .standard) { self.defaults = defaults }

    func read(_ key: String) -> String? { defaults.string(forKey: key) }

    func write(_ key: String, _ value: String?) {
        if let value { defaults.set(value, forKey: key) } else { defaults.removeObject(forKey: key) }
    }
}

final class MemoryGridPrefs: GridPrefs {
    var values: [String: String] = [:]
    func read(_ key: String) -> String? { values[key] }
    func write(_ key: String, _ value: String?) { values[key] = value }
}

/// What one screen keeps: the player's views (and their saved changes to built-in ones) and the
/// view that was open last.
struct GridSavedViews: Codable, Equatable {
    var views: [GridViewState] = []
    var active: String?
}

/// One screen's views. Built-in views come first (with any changes the player saved over them),
/// then the player's own, in the order they were saved.
final class GridViewBook {
    let screen: String
    let builtIn: [GridViewState]
    private let prefs: GridPrefs
    private(set) var saved: GridSavedViews

    init(screen: String, builtIn: [GridViewState], prefs: GridPrefs = DefaultsGridPrefs.shared) {
        self.screen = screen
        self.builtIn = builtIn
        self.prefs = prefs
        self.saved = GridViewBook.decode(prefs.read(GridViewBook.key(screen)))
    }

    static func key(_ screen: String) -> String { "cw.grid.views.\(screen)" }

    static func encode(_ v: GridSavedViews) -> String {
        (try? JSONEncoder().encode(v)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    static func decode(_ s: String?) -> GridSavedViews {
        guard let data = s?.data(using: .utf8) else { return GridSavedViews() }
        return (try? JSONDecoder().decode(GridSavedViews.self, from: data)) ?? GridSavedViews()
    }

    var all: [GridViewState] {
        builtIn.map { b in saved.views.first { $0.name == b.name } ?? b }
            + saved.views.filter { s in !builtIn.contains { $0.name == s.name } }
    }

    func isBuiltIn(_ name: String) -> Bool { builtIn.contains { $0.name == name } }
    func named(_ name: String) -> GridViewState? { all.first { $0.name == name } }

    var activeName: String {
        if let a = saved.active, named(a) != nil { return a }
        return builtIn.first?.name ?? ""
    }

    var active: GridViewState? { named(activeName) }

    func activate(_ name: String) {
        saved.active = name
        persist()
    }

    /// Keep a view under its name (replacing one of that name) and make it the open one.
    func save(_ state: GridViewState) {
        saved.views.removeAll { $0.name == state.name }
        saved.views.append(state)
        saved.active = state.name
        persist()
    }

    /// A built-in view goes back to how it shipped.
    func restore(_ name: String) {
        guard isBuiltIn(name) else { return }
        saved.views.removeAll { $0.name == name }
        persist()
    }

    /// Only the player's own views can go.
    func delete(_ name: String) {
        guard !isBuiltIn(name) else { return }
        saved.views.removeAll { $0.name == name }
        if saved.active == name { saved.active = nil }
        persist()
    }

    private func persist() { prefs.write(GridViewBook.key(screen), GridViewBook.encode(saved)) }
}
