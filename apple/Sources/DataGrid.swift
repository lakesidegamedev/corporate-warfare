import SwiftUI

// The data grid for the iPad and Mac tables: sortable headings, a column chooser, saved views, a
// search box, filter chips, an owner and scope choice and a timeframe for change and trend
// columns. It matches the desktop grid in what a view holds; the model lives in GridModel.swift.

/// One column a grid can show. A column is numeric unless [numeric] is false: numeric columns are
/// right aligned in even-width digits and sort by [value]. With a [series] (oldest first, [step]
/// weeks per point) it can also show its change, change in percent or trend over the timeframe.
struct GridColumn<Row> {
    let id: String
    let title: String
    /// Nil for the leading column, which takes the room that is left.
    let width: CGFloat?
    let numeric: Bool
    let text: (Row) -> String
    let value: ((Row) -> Int64?)?
    let tone: (Row) -> Color
    let series: ((Row) -> [Int64])?
    let step: Int
    let format: ((Int64) -> String)?
    /// Units of [value] per unit a player types in a filter: 100 for cents and basis points.
    let scale: Double
    /// The heading the column chooser files it under.
    let topic: String

    init(_ id: String, _ title: String, width: CGFloat? = 80, numeric: Bool = true, topic: String = "Figures",
         text: @escaping (Row) -> String,
         value: ((Row) -> Int64?)? = nil,
         tone: @escaping (Row) -> Color = { _ in Theme.primaryText },
         series: ((Row) -> [Int64])? = nil, step: Int = 1,
         format: ((Int64) -> String)? = nil, scale: Double = 1) {
        self.id = id
        self.title = title
        self.width = width
        self.numeric = numeric
        self.text = text
        self.value = value
        self.tone = tone
        self.series = series
        self.step = step
        self.format = format
        self.scale = scale
        self.topic = topic
    }

    /// The ways this column can show.
    var modes: [GridColumnMode] { numeric && series != nil ? GridColumnMode.allCases : [.value] }
}

/// A scope choice: one sector, one region, one company. Several can be on at once.
struct GridScope<Row> {
    let id: String
    let label: String
    let kind: String
    let test: (Row) -> Bool
}

/// A column as it shows in one way over one timeframe.
struct GridCell<Row> {
    let key: String
    let title: String
    let width: CGFloat?
    let numeric: Bool
    let text: (Row) -> String
    let sort: (Row) -> Int64?
    let tone: (Row) -> Color
    let trend: ((Row) -> [Int64])?
}

enum GridCells {
    static let dash = "-"

    static func resolve<Row>(_ c: GridColumn<Row>, _ spec: GridColumnSpec, _ tf: GridTimeframe) -> GridCell<Row> {
        let plainSort: (Row) -> Int64? = c.value ?? { _ in nil }
        guard let series = c.series, spec.mode != .value else {
            return GridCell(key: spec.key, title: c.title, width: c.width, numeric: c.numeric, text: c.text, sort: plainSort, tone: c.tone, trend: nil)
        }
        let step = c.step
        let fmt = c.format ?? { Figures.count($0) }
        let title = spec.mode.title(c.title)
        switch spec.mode {
        case .change:
            return GridCell(key: spec.key, title: title, width: max(c.width ?? 80, 80), numeric: true,
                            text: { r in GridSeries.change(series(r), tf, step: step).map { ($0 > 0 ? "+" : "") + fmt($0) } ?? dash },
                            sort: { r in GridSeries.change(series(r), tf, step: step) },
                            tone: { r in tint(GridSeries.change(series(r), tf, step: step)) }, trend: nil)
        case .percent:
            return GridCell(key: spec.key, title: title, width: 72, numeric: true,
                            text: { r in GridSeries.changeBps(series(r), tf, step: step).map(Change.text) ?? dash },
                            sort: { r in GridSeries.changeBps(series(r), tf, step: step) },
                            tone: { r in tint(GridSeries.changeBps(series(r), tf, step: step)) }, trend: nil)
        case .trend:
            return GridCell(key: spec.key, title: title, width: 84, numeric: true,
                            text: { _ in "" },
                            sort: { r in GridSeries.changeBps(series(r), tf, step: step) },
                            tone: { _ in Theme.gold },
                            trend: { r in GridSeries.window(series(r), tf, step: step) ?? [] })
        case .value:
            return GridCell(key: spec.key, title: c.title, width: c.width, numeric: c.numeric, text: c.text, sort: plainSort, tone: c.tone, trend: nil)
        }
    }

    static func tint(_ v: Int64?) -> Color {
        guard let v else { return Theme.secondary }
        return v > 0 ? Theme.positive : v < 0 ? Theme.negative : Theme.secondary
    }
}

/// Rows at about 22 points on a Mac and 28 on an iPad.
enum GridMetrics {
    #if targetEnvironment(macCatalyst)
    static let rowHeight: CGFloat = 22
    #else
    static let rowHeight: CGFloat = 28
    #endif
    static let firstMin: CGFloat = 160
    static let figures = Font.system(size: 12).monospacedDigit()
}

struct DataGrid<Row>: View {
    let title: String
    let rows: [Row]
    let columns: [GridColumn<Row>]
    let key: (Row) -> AnyHashable
    let words: (Row) -> String
    let mine: ((Row) -> Bool)?
    let scopes: [GridScope<Row>]
    let onOpen: ((Row) -> Void)?
    let accessory: AnyView?
    private let prefs: GridPrefs

    @State private var book: GridViewBook
    @State private var view: GridViewState
    @State private var search = ""
    @State private var cursor: AnyHashable?
    @State private var naming = false
    @State private var newName = ""
    @State private var numberFor: (column: String, above: Bool)?
    @State private var numberText = ""
    @FocusState private var searchFocused: Bool
    @FocusState private var gridFocused: Bool

    init(_ title: String, screen: String, views: [GridViewState], rows: [Row], columns: [GridColumn<Row>],
         key: @escaping (Row) -> AnyHashable, words: @escaping (Row) -> String,
         mine: ((Row) -> Bool)? = nil, scopes: [GridScope<Row>] = [],
         onOpen: ((Row) -> Void)? = nil, accessory: AnyView? = nil,
         prefs: GridPrefs = DefaultsGridPrefs.shared) {
        self.title = title
        self.rows = rows
        self.columns = columns
        self.key = key
        self.words = words
        self.mine = mine
        self.scopes = scopes
        self.onOpen = onOpen
        self.accessory = accessory
        self.prefs = prefs
        let book = GridViewBook(screen: screen, builtIn: views, prefs: prefs)
        _book = State(initialValue: book)
        _view = State(initialValue: book.active ?? views.first ?? GridViewState("", columns.map(\.id)))
    }

    // MARK: Derived

    private var byId: [String: GridColumn<Row>] {
        Dictionary(columns.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
    }

    /// The shown columns; the leading column always shows, first.
    private var specs: [GridColumnSpec] {
        guard let first = columns.first else { return [] }
        let known = view.columns.filter { byId[$0.id] != nil && $0.id != first.id }
        return [GridColumnSpec(first.id)] + known
    }

    private var cells: [GridCell<Row>] {
        let ids = byId
        return specs.compactMap { s in ids[s.id].map { GridCells.resolve($0, s, view.timeframe) } }
    }

    private func reader() -> GridRowReader<Row> {
        let ids = byId
        let tf = view.timeframe
        let mine = self.mine
        let scopes = self.scopes
        return GridRowReader(
            text: { r, id in ids[GridColumnSpec.parse(id).id]?.text(r) },
            figure: { r, id in
                guard let c = ids[id], let v = c.value?(r) else { return nil }
                return Double(v) / c.scale
            },
            sortValue: { r, key in
                let s = GridColumnSpec.parse(key)
                guard let c = ids[s.id] else { return nil }
                return GridCells.resolve(c, s, tf).sort(r)
            },
            numeric: { key in ids[GridColumnSpec.parse(key).id]?.numeric ?? false },
            mine: { r in mine?(r) ?? false },
            words: words,
            scope: { r, id in scopes.first { $0.id == id }?.test(r) ?? false }
        )
    }

    private var shown: [Row] { GridRows.shown(rows, view: view, search: search, read: reader()) }

    private var modified: Bool {
        guard let saved = book.named(view.name) else { return true }
        return !view.sameShape(saved)
    }

    private var hasSeries: Bool { columns.contains { $0.series != nil } }

    private func resolveName(_ name: String) -> String? {
        let n = name.lowercased()
        return columns.first { $0.id.lowercased() == n || $0.title.lowercased().replacingOccurrences(of: " ", with: "") == n }?.id
            ?? columns.first { n.count >= 3 && $0.title.lowercased().hasPrefix(n) }?.id
    }

    private func titleOf(_ id: String) -> String { byId[id]?.title ?? id }

    // MARK: Body

    var body: some View {
        let list = shown
        let cells = self.cells
        VStack(spacing: 0) {
            controlBar(count: list.count)
            chipRow
            Rectangle().fill(Theme.stroke).frame(height: 1)
            GeometryReader { geo in
                let fixed = cells.dropFirst().reduce(CGFloat(0)) { $0 + ($1.width ?? 80) }
                let width = max(geo.size.width, fixed + GridMetrics.firstMin + 16)
                ScrollView(.horizontal, showsIndicators: true) {
                    VStack(spacing: 0) {
                        header(cells)
                        Rectangle().fill(Theme.stroke).frame(height: 1)
                        table(list, cells)
                    }
                    .frame(width: width, height: geo.size.height)
                }
            }
        }
        .background(Theme.panel)
        .overlay(Rectangle().stroke(Theme.stroke, lineWidth: 1))
        .background(findShortcut)
        .alert("Save view as", isPresented: $naming) {
            TextField("Name", text: $newName)
            Button("Save") { saveAs() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Keeps these columns, sort, filters and timeframe under a name on this device.")
        }
        .alert(numberFor.map { "\(titleOf($0.column)) \($0.above ? "above" : "below")" } ?? "Filter",
               isPresented: Binding(get: { numberFor != nil }, set: { if !$0 { numberFor = nil } })) {
            TextField("Number", text: $numberText).keyboardType(.decimalPad)
            Button("Add") { addNumberFilter() }
            Button("Cancel", role: .cancel) { numberFor = nil }
        } message: {
            Text("Money in whole units, percent in percent.")
        }
    }

    // MARK: Control bar

    private func controlBar(count: Int) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                Text(title.uppercased())
                    .font(Pixel.font(11))
                    .foregroundStyle(Theme.primaryText)
                    .accessibilityAddTraits(.isHeader)
                Text(count == rows.count ? "\(rows.count)" : "\(count) of \(rows.count)")
                    .font(GridMetrics.figures)
                    .foregroundStyle(Theme.secondary)
                viewsMenu
                if mine != nil { ownerMenu }
                if !scopes.isEmpty { scopeMenu }
                if hasSeries { timeframeChoice }
                filterMenu
                columnsMenu
                searchField
                if let accessory { accessory }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
        }
        .background(Theme.panelRaised)
    }

    private func barLabel(_ text: String, icon: String? = nil, on: Bool = false) -> some View {
        HStack(spacing: 4) {
            if let icon { PixelIcon(icon, size: 12) }
            Text(text).font(.caption.weight(.semibold)).lineLimit(1)
        }
        .foregroundStyle(on ? Theme.gold : Theme.primaryText)
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Theme.ink, in: Pixel.small)
        .overlay(Pixel.small.stroke(on ? Theme.gold : Theme.stroke, lineWidth: 1))
    }

    private var viewsMenu: some View {
        Menu {
            ForEach(Array(book.all.enumerated()), id: \.offset) { _, v in
                Button {
                    view = v
                    book.activate(v.name)
                } label: {
                    if v.name == view.name { Label(v.name, systemImage: "checkmark") } else { Text(v.name) }
                }
            }
            Divider()
            if modified && !view.name.isEmpty {
                Button("Save \"\(view.name)\"") { book.save(view); refreshBook() }
            }
            Button("Save as new view") {
                newName = ""
                naming = true
            }
            if modified, let saved = book.named(view.name) {
                Button("Undo changes") { view = saved }
            }
            if book.isBuiltIn(view.name) && book.saved.views.contains(where: { $0.name == view.name }) {
                Button("Restore built-in view") {
                    book.restore(view.name)
                    refreshBook()
                    if let v = book.named(view.name) { view = v }
                }
            }
            if !view.name.isEmpty && !book.isBuiltIn(view.name) {
                Button("Delete \"\(view.name)\"", role: .destructive) {
                    book.delete(view.name)
                    refreshBook()
                    if let v = book.active { view = v }
                }
            }
        } label: {
            barLabel("View: \(view.name.isEmpty ? "Custom" : view.name)\(modified ? " *" : "")", icon: "sort", on: modified)
        }
        .accessibilityLabel("Views")
    }

    private var ownerMenu: some View {
        Menu {
            ForEach(GridOwner.allCases, id: \.self) { o in
                Button {
                    view.owner = o
                } label: {
                    if view.owner == o { Label(o.label, systemImage: "checkmark") } else { Text(o.label) }
                }
            }
        } label: {
            barLabel("Owner: \(view.owner.label)", on: view.owner != .all)
        }
    }

    private var scopeMenu: some View {
        let kinds = scopes.reduce(into: [String]()) { if !$0.contains($1.kind) { $0.append($1.kind) } }
        return Menu {
            if !view.scope.isEmpty {
                Button("Show everything") { view.scope = [] }
            }
            ForEach(kinds, id: \.self) { kind in
                Menu(kind) {
                    ForEach(scopes.filter { $0.kind == kind }, id: \.id) { s in
                        Button {
                            if view.scope.contains(s.id) { view.scope.removeAll { $0 == s.id } } else { view.scope.append(s.id) }
                        } label: {
                            if view.scope.contains(s.id) { Label(s.label, systemImage: "checkmark") } else { Text(s.label) }
                        }
                    }
                }
            }
        } label: {
            barLabel(view.scope.isEmpty ? (kinds.first.map { "\($0): all" } ?? "Scope") : "Scope: \(view.scope.count)", on: !view.scope.isEmpty)
        }
    }

    private var timeframeChoice: some View {
        HStack(spacing: 2) {
            ForEach(GridTimeframe.allCases) { tf in
                Button {
                    view.timeframe = tf
                } label: {
                    Text(tf.label)
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(view.timeframe == tf ? Theme.ink : Theme.secondary)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 4)
                        .background(view.timeframe == tf ? Theme.gold : Theme.ink, in: Pixel.small)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(view.timeframe == tf ? .isSelected : [])
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Timeframe")
    }

    private var filterMenu: some View {
        Menu {
            ForEach(columns.filter { $0.numeric && $0.value != nil }, id: \.id) { c in
                Menu(c.title) {
                    Button("Above...") { numberText = ""; numberFor = (c.id, true) }
                    Button("Below...") { numberText = ""; numberFor = (c.id, false) }
                }
            }
            Divider()
            ForEach(columns.dropFirst().filter { !$0.numeric }, id: \.id) { c in
                let values = Set(rows.map(c.text)).filter { !$0.isEmpty && $0 != GridCells.dash }.sorted()
                if !values.isEmpty && values.count <= 40 {
                    Menu(c.title) {
                        ForEach(values, id: \.self) { v in
                            Button(v) { view = view.with(filter: GridFilter(op: .equals, column: c.id, text: v)) }
                        }
                    }
                }
            }
        } label: {
            barLabel("Filter", icon: "filter", on: !view.filters.isEmpty)
        }
    }

    private var columnsMenu: some View {
        let topics = columns.dropFirst().reduce(into: [String]()) { if !$0.contains($1.topic) { $0.append($1.topic) } }
        return Menu {
            ForEach(topics, id: \.self) { topic in
                Section(topic) {
                    ForEach(columns.dropFirst().filter { $0.topic == topic }, id: \.id) { c in
                        let on = view.columns.contains { $0.id == c.id }
                        if c.modes.count > 1 {
                            Menu(on ? "\(c.title) (on)" : c.title) {
                                Button(on ? "Hide" : "Show") { view = view.with(column: c.id, on: !on) }
                                if on {
                                    ForEach(c.modes, id: \.self) { m in
                                        Button {
                                            view = view.toggling(c.id, mode: m)
                                        } label: {
                                            if view.columns.contains(GridColumnSpec(c.id, m)) { Label(m.label, systemImage: "checkmark") } else { Text(m.label) }
                                        }
                                    }
                                }
                            }
                        } else {
                            Button {
                                view = view.with(column: c.id, on: !on)
                            } label: {
                                if on { Label(c.title, systemImage: "checkmark") } else { Text(c.title) }
                            }
                        }
                    }
                }
            }
        } label: {
            barLabel("Columns")
        }
    }

    private var searchField: some View {
        HStack(spacing: 4) {
            PixelIcon("search", size: 12).foregroundStyle(Theme.secondary)
            TextField("Search", text: $search)
                .textFieldStyle(.plain)
                .font(.caption)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .focused($searchFocused)
                .onSubmit(commitSearch)
                .frame(width: 150)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Theme.ink, in: Pixel.small)
        .overlay(Pixel.small.stroke(searchFocused ? Theme.gold : Theme.stroke, lineWidth: 1))
    }

    /// Return in the search box turns typed figure terms ("price>10") into chips.
    private func commitSearch() {
        let parsed = GridRows.parse(search, resolve: resolveName)
        guard !parsed.chips.isEmpty else { return }
        view = parsed.chips.reduce(view) { $0.with(filter: $1) }
        search = parsed.words
    }

    private func saveAs() {
        let name = newName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        var next = view
        next.name = name
        book.save(next)
        refreshBook()
        view = next
    }

    private func addNumberFilter() {
        guard let target = numberFor, let n = Double(numberText.replacingOccurrences(of: ",", with: "")) else { numberFor = nil; return }
        view = view.with(filter: GridFilter(op: target.above ? .above : .below, column: target.column, number: n))
        numberFor = nil
    }

    /// The book is a class; a fresh copy makes SwiftUI redraw the menus after a save.
    private func refreshBook() {
        book = GridViewBook(screen: book.screen, builtIn: book.builtIn, prefs: prefs)
    }

    // MARK: Chips

    @ViewBuilder
    private var chipRow: some View {
        if !view.filters.isEmpty || !view.scope.isEmpty || view.owner != .all {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    if view.owner != .all {
                        chip("Owner: \(view.owner.label)") { view.owner = .all }
                    }
                    ForEach(view.scope, id: \.self) { id in
                        chip(scopes.first { $0.id == id }?.label ?? id) { view.scope.removeAll { $0 == id } }
                    }
                    ForEach(Array(view.filters.enumerated()), id: \.offset) { _, f in
                        chip(f.label(titleOf)) { view = view.without(filter: f) }
                    }
                    Button("Clear") {
                        view.filters = []
                        view.scope = []
                        view.owner = .all
                    }
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Theme.secondary)
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 4)
            }
            .background(Theme.panel)
        }
    }

    private func chip(_ text: String, remove: @escaping () -> Void) -> some View {
        Button(action: remove) {
            HStack(spacing: 4) {
                Text(text).font(.caption2.weight(.semibold)).lineLimit(1)
                Text("x").font(.caption2.weight(.bold)).foregroundStyle(Theme.secondary)
            }
            .foregroundStyle(Theme.gold)
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .background(Theme.ink, in: Pixel.small)
            .overlay(Pixel.small.stroke(Theme.gold, lineWidth: 1))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Remove filter \(text)")
    }

    // MARK: Table

    private func header(_ cells: [GridCell<Row>]) -> some View {
        HStack(spacing: 0) {
            ForEach(Array(cells.enumerated()), id: \.offset) { i, c in
                Button {
                    view = view.sorted(by: c.key, numeric: c.numeric)
                } label: {
                    HStack(spacing: 2) {
                        if c.numeric { Spacer(minLength: 0) }
                        Text(c.title.uppercased())
                            .font(.system(size: 10, weight: .semibold))
                            .tracking(0.4)
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                        if view.sort == c.key {
                            Text(view.descending ? "v" : "^").font(.system(size: 10, weight: .bold))
                        }
                        if !c.numeric { Spacer(minLength: 0) }
                    }
                    .foregroundStyle(view.sort == c.key ? Theme.gold : Theme.secondary)
                    .padding(.horizontal, 6)
                    .frame(maxWidth: i == 0 ? .infinity : nil)
                    .frame(width: i == 0 ? nil : (c.width ?? 80))
                    .frame(minWidth: i == 0 ? GridMetrics.firstMin : nil)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Sort by \(c.title)")
                .accessibilityAddTraits(view.sort == c.key ? .isSelected : [])
            }
        }
        .frame(height: 22)
        .background(Theme.ink)
    }

    private func table(_ list: [Row], _ cells: [GridCell<Row>]) -> some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical) {
                LazyVStack(spacing: 0) {
                    ForEach(Array(list.enumerated()), id: \.offset) { i, r in
                        row(i, r, cells)
                            .id(key(r))
                    }
                    if list.isEmpty {
                        Text(rows.isEmpty ? "Nothing here yet." : "No rows match. Clear a filter or the search.")
                            .font(.caption)
                            .foregroundStyle(Theme.secondary)
                            .padding(16)
                    }
                }
            }
            .focusable()
            .focused($gridFocused)
            .focusEffectDisabled()
            .onKeyPress(.downArrow) { move(1, list, proxy) }
            .onKeyPress(.upArrow) { move(-1, list, proxy) }
            .onKeyPress(.return) {
                guard let onOpen, let r = list.first(where: { key($0) == cursor }) else { return .ignored }
                onOpen(r)
                return .handled
            }
        }
    }

    private func move(_ by: Int, _ list: [Row], _ proxy: ScrollViewProxy) -> KeyPress.Result {
        guard !list.isEmpty else { return .ignored }
        let at = list.firstIndex { key($0) == cursor } ?? (by > 0 ? -1 : list.count)
        let next = min(max(at + by, 0), list.count - 1)
        let k = key(list[next])
        cursor = k
        proxy.scrollTo(k)
        return .handled
    }

    private func row(_ i: Int, _ r: Row, _ cells: [GridCell<Row>]) -> some View {
        let k = key(r)
        let picked = k == cursor
        return HStack(spacing: 0) {
            ForEach(Array(cells.enumerated()), id: \.offset) { ci, c in
                cell(c, r, first: ci == 0)
            }
        }
        .frame(height: GridMetrics.rowHeight)
        .background(picked ? Theme.panelRaised : (i % 2 == 0 ? Theme.panel : Theme.ink))
        .overlay(alignment: .leading) {
            if picked { Rectangle().fill(Theme.gold).frame(width: 2) }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            cursor = k
            gridFocused = true
            onOpen?(r)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }

    @ViewBuilder
    private func cell(_ c: GridCell<Row>, _ r: Row, first: Bool) -> some View {
        Group {
            if let trend = c.trend {
                GridTrend(values: trend(r))
                    .padding(.vertical, 5)
            } else {
                Text(c.text(r))
                    .font(first ? .system(size: 12, weight: .semibold) : GridMetrics.figures)
                    .foregroundStyle(c.tone(r))
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .frame(maxWidth: .infinity, alignment: c.numeric ? .trailing : .leading)
            }
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: first ? .infinity : nil)
        .frame(width: first ? nil : (c.width ?? 80))
        .frame(minWidth: first ? GridMetrics.firstMin : nil)
    }

    /// Command F jumps to the search box.
    private var findShortcut: some View {
        Button("Find") { searchFocused = true }
            .keyboardShortcut("f", modifiers: .command)
            .opacity(0)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

/// A trend as stepped columns, one per point, scaled to the window's own range: hard pixels, no
/// line smoothing, rising in gold and falling in red against where the window started.
struct GridTrend: View {
    let values: [Int64]

    var body: some View {
        Canvas { context, size in
            guard values.count > 1, let low = values.min(), let high = values.max() else { return }
            let span = Double(max(high - low, 1))
            let w = max(floor(size.width / CGFloat(values.count)), 1)
            let rising = values[values.count - 1] >= values[0]
            let color = rising ? Theme.gold : Theme.negative
            for (i, v) in values.enumerated() {
                let h = max(floor(size.height * CGFloat(Double(v - low) / span)), 1)
                let rect = CGRect(x: CGFloat(i) * w, y: size.height - h, width: max(w - 1, 1), height: h)
                context.fill(Path(rect), with: .color(color))
            }
        }
        .accessibilityHidden(true)
    }
}
