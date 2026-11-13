import XCTest
@testable import CorporateWarfare

/// The grid model on iPad and Mac: sort, filters, timeframe maths and saved views. Expected values
/// are worked out by hand from the inputs, not read back from the code.
final class DataGridTests: XCTestCase {
    private struct Item {
        let name: String
        let sector: String
        let price: Int64?
        let mine: Bool
        let history: [Int64]
    }

    private let items = [
        Item(name: "Steel", sector: "Metals", price: 500, mine: true, history: [100, 200, 300, 400, 500]),
        Item(name: "Copper", sector: "Metals", price: nil, mine: false, history: []),
        Item(name: "Wheat", sector: "Farming", price: 120, mine: false, history: [150, 120]),
        Item(name: "Corn", sector: "Farming", price: 900, mine: true, history: [0, 900]),
    ]

    private func reader(tf: GridTimeframe = .quarter) -> GridRowReader<Item> {
        GridRowReader(
            text: { r, id in
                switch GridColumnSpec.parse(id).id {
                case "name": return r.name
                case "sector": return r.sector
                default: return nil
                }
            },
            figure: { r, id in id == "price" ? r.price.map { Double($0) / 100 } : nil },
            sortValue: { r, key in
                let spec = GridColumnSpec.parse(key)
                guard spec.id == "price" else { return nil }
                switch spec.mode {
                case .change: return GridSeries.change(r.history, tf)
                case .percent, .trend: return GridSeries.changeBps(r.history, tf)
                case .value: return r.price
                }
            },
            numeric: { GridColumnSpec.parse($0).id == "price" },
            mine: { $0.mine },
            words: { "\($0.name) \($0.sector)" },
            scope: { r, id in id == "sector:\(r.sector)" }
        )
    }

    private func names(_ rows: [Item]) -> [String] { rows.map(\.name) }

    // MARK: Sorting

    func testNumericSortPutsMissingFiguresLastBothWays() {
        let down = GridRows.sort(items, by: "price", descending: true, read: reader())
        XCTAssertEqual(names(down), ["Corn", "Steel", "Wheat", "Copper"])
        let up = GridRows.sort(items, by: "price", descending: false, read: reader())
        XCTAssertEqual(names(up), ["Wheat", "Steel", "Corn", "Copper"])
    }

    func testTextSortIsCaseBlindAndStable() {
        let rows = GridRows.sort(items, by: "sector", descending: false, read: reader())
        // Farming before Metals; within a sector the original order holds.
        XCTAssertEqual(names(rows), ["Wheat", "Corn", "Steel", "Copper"])
    }

    func testHeadingTapSortsNewColumnHighFirstAndSecondTapReverses() {
        let v = GridViewState("Test", ["name", "price"], sort: "name", descending: false)
        let first = v.sorted(by: "price", numeric: true)
        XCTAssertEqual(first.sort, "price")
        XCTAssertTrue(first.descending)
        let second = first.sorted(by: "price", numeric: true)
        XCTAssertFalse(second.descending)
        let text = v.sorted(by: "sector", numeric: false)
        XCTAssertFalse(text.descending, "a text column starts A to Z")
    }

    func testSortByChangeOverTimeframe() {
        // Over a week: Steel +100, Wheat -30, Corn +900, Copper has no history.
        let rows = GridRows.sort(items, by: "price.change", descending: true, read: reader(tf: .week))
        XCTAssertEqual(names(rows), ["Corn", "Steel", "Wheat", "Copper"])
    }

    // MARK: Filters

    func testOwnerScopeAndChips() {
        var v = GridViewState("Test", ["name", "price"])
        v.owner = .mine
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Steel", "Corn"])
        v.owner = .rivals
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Copper", "Wheat"])
        v.owner = .all
        v.scope = ["sector:Farming"]
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Wheat", "Corn"])
        v.scope = []
        // Price above 2 (whole units) keeps 500 and 900 cents; a row with no price never passes.
        v = v.with(filter: GridFilter(op: .above, column: "price", number: 2))
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Steel", "Corn"])
        // A second above chip on the same column replaces the first.
        v = v.with(filter: GridFilter(op: .above, column: "price", number: 6))
        XCTAssertEqual(v.filters.count, 1)
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Corn"])
        v.filters = [GridFilter(op: .not, column: "sector", text: "metal")]
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Wheat", "Corn"])
        v.filters = [GridFilter(op: .below, column: "price", number: 5)]
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "", read: reader())), ["Wheat"])
    }

    func testSearchNeedsEveryWord() {
        let v = GridViewState("Test", ["name"])
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "farm", read: reader())), ["Wheat", "Corn"])
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "farming CORN", read: reader())), ["Corn"])
        XCTAssertEqual(names(GridRows.shown(items, view: v, search: "  ", read: reader())).count, 4)
    }

    func testTypedTermsBecomeChips() {
        let resolve: (String) -> String? = { ["price": "price", "sector": "sector"][$0.lowercased()] }
        let parsed = GridRows.parse("price>10 -sector:metals steel sector:big_farm odd>x", resolve: resolve)
        XCTAssertEqual(parsed.chips, [
            GridFilter(op: .above, column: "price", number: 10),
            GridFilter(op: .not, column: "sector", text: "metals"),
            GridFilter(op: .equals, column: "sector", text: "big farm"),
        ])
        XCTAssertEqual(parsed.words, "steel odd>x")
    }

    // MARK: Timeframe maths

    func testWindowChangeAndPercent() {
        let s: [Int64] = Array(1...60).map { Int64($0) * 10 }
        // A week: the last point and the one before it.
        XCTAssertEqual(GridSeries.window(s, .week), [590, 600])
        XCTAssertEqual(GridSeries.change(s, .week), 10)
        // A quarter: 13 weeks back from 600 is 470.
        XCTAssertEqual(GridSeries.change(s, .quarter), 130)
        XCTAssertEqual(GridSeries.changeBps(s, .quarter), 130 * 10_000 / 470)
        // A year needs 53 points; 60 is enough. Career uses everything: 600 - 10.
        XCTAssertEqual(GridSeries.change(s, .year), 520)
        XCTAssertEqual(GridSeries.change(s, .career), 590)
        // Too short for the window reads as no figure, never zero.
        XCTAssertNil(GridSeries.change(Array(s.prefix(13)), .quarter))
        XCTAssertNil(GridSeries.change([5], .career))
    }

    func testFourWeekStepAndZeroStart() {
        // Four-week results: a quarter is four points back (13 weeks rounds up to 4 periods).
        let r: [Int64] = [100, 110, 120, 130, 140, 150]
        XCTAssertEqual(GridSeries.window(r, .quarter, step: 4), [110, 120, 130, 140, 150])
        XCTAssertEqual(GridSeries.change(r, .week, step: 4), 10)
        XCTAssertNil(GridSeries.changeBps([0, 50], .week), "no percent from a zero start")
        XCTAssertEqual(GridSeries.changeBps([-200, -100], .week), 5_000, "measured against the size of the start")
    }

    func testTotalsOverTimeframe() {
        let s: [Int64] = Array(repeating: 2, count: 20)
        XCTAssertEqual(GridSeries.total(s, .week), 2)
        XCTAssertEqual(GridSeries.total(s, .quarter), 26)
        XCTAssertNil(GridSeries.total(s, .year))
        XCTAssertEqual(GridSeries.total(s, .career), 40)
        XCTAssertEqual(GridSeries.total([5, 7, 9], .quarter, step: 13), 9)
    }

    // MARK: Columns

    func testColumnModesToggleAndHide() {
        var v = GridViewState("Test", ["name", "price"], sort: "price.percent")
        v = v.toggling("price", mode: .percent)
        XCTAssertEqual(v.columns.map(\.key), ["name", "price", "price.percent"])
        // The last way a column shows stays.
        let only = GridViewState("Test", ["name", "price"]).toggling("price", mode: .value)
        XCTAssertEqual(only.columns.map(\.key), ["name", "price"])
        v = v.toggling("price", mode: .percent)
        XCTAssertEqual(v.columns.map(\.key), ["name", "price"])
        XCTAssertNil(v.sort, "the sort leaves with its column")
        v = v.with(column: "sector", on: true).with(column: "price", on: false)
        XCTAssertEqual(v.columns.map(\.key), ["name", "sector"])
    }

    // MARK: Saved views

    func testSavedViewRoundTrip() {
        let prefs = MemoryGridPrefs()
        let builtIn = [GridViewState("Overview", ["name", "price"], sort: "price"), GridViewState("Supply", ["name", "made"])]
        let book = GridViewBook(screen: "goods", builtIn: builtIn, prefs: prefs)
        XCTAssertEqual(book.activeName, "Overview")

        var mine = GridViewState("Cheap metals", ["name", "price", "price.trend"], sort: "price", descending: false,
                                 filters: [GridFilter(op: .below, column: "price", number: 10)], owner: .rivals,
                                 scope: ["sector:Metals"], timeframe: .year)
        book.save(mine)
        var changed = builtIn[0]
        changed.timeframe = .career
        book.save(changed)

        // A fresh book reads the same views back from the stored text.
        let again = GridViewBook(screen: "goods", builtIn: builtIn, prefs: prefs)
        XCTAssertEqual(again.all.map(\.name), ["Overview", "Supply", "Cheap metals"])
        XCTAssertEqual(again.named("Cheap metals"), mine)
        XCTAssertEqual(again.named("Overview")?.timeframe, .career)
        XCTAssertEqual(again.activeName, "Overview", "the last saved view is the open one")

        again.restore("Overview")
        XCTAssertEqual(again.named("Overview"), builtIn[0])
        again.delete("Supply")
        XCTAssertNotNil(again.named("Supply"), "built-in views cannot be deleted")
        again.activate("Cheap metals")
        again.delete("Cheap metals")
        XCTAssertNil(again.named("Cheap metals"))
        XCTAssertEqual(again.activeName, "Overview")

        // Another screen keeps its own views; broken stored text falls back to the built-in views.
        XCTAssertEqual(GridViewBook(screen: "league", builtIn: builtIn, prefs: prefs).all, builtIn)
        prefs.write(GridViewBook.key("broken"), "{not json")
        XCTAssertEqual(GridViewBook(screen: "broken", builtIn: builtIn, prefs: prefs).all, builtIn)

        mine.name = "Other"
        XCTAssertTrue(mine.sameShape(again.builtIn[0]) == false)
        XCTAssertTrue(mine.sameShape({ var m = mine; m.name = "x"; return m }()))
    }

    func testBuiltInViewsKeepEveryScreenCalm() {
        // The first view on each screen shows the few columns the desktop shows by default, and
        // every built-in view only names columns the screen has.
        let screens: [(String, [GridViewState], Set<String>)] = [
            ("plants", PlantGrid.views, Set(PlantGrid.columns.map(\.id))),
            ("goods", GoodsGrid.views, Set(GoodsGrid.columns(GoodsGridData()).map(\.id))),
            ("regions", RegionGrid.views, Set(RegionGrid.columns.map(\.id))),
            ("finance", FinanceGrid.views, Set(["name", "group"] + FinanceGrid.periods)),
        ]
        for (screen, views, ids) in screens {
            XCTAssertFalse(views.isEmpty, screen)
            XCTAssertLessThanOrEqual(views[0].columns.count, 11, screen)
            for v in views {
                for c in v.columns { XCTAssertTrue(ids.contains(c.id), "\(screen) \(v.name) names \(c.id)") }
                for f in v.filters { XCTAssertTrue(ids.contains(f.column), "\(screen) \(v.name) filters \(f.column)") }
            }
        }
        XCTAssertEqual(LeagueGrid.views.map(\.name), ["Overview", "Finance", "Shares", "Exchange", "Against me"])
    }
}
