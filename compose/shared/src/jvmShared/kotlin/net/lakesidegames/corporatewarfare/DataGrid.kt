package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * One column of a [DataGrid]. A column is numeric unless [numeric] is false: numeric columns are
 * right aligned in tabular figures and sort by [sort]; text columns sort by their text. A column
 * with no fixed [width] takes the room that is left. [bar] draws a flat bar behind the figure
 * (0 to 1 of the column) and [spark] a stepped column chart of a series instead of text.
 */
class GridColumn<T>(
    val id: String,
    val title: String,
    val width: Dp? = 80.dp,
    val numeric: Boolean = true,
    val text: (T) -> String = { "" },
    val sort: ((T) -> Long)? = null,
    val tone: (T) -> Color = { Palette.primaryText },
    val bar: ((T) -> Float)? = null,
    val spark: ((T) -> List<Long>)? = null,
    val tip: String? = null,
    val shown: Boolean = true,
    val bold: Boolean = false,
    /** When the grid is too narrow for every column, the highest numbers go first (ties: the rightmost). The first column never goes. */
    val dropOrder: Int = 0,
    /** The least room a pinned first column gets. */
    val minWidth: Dp? = null,
    /** Something small drawn before the text, such as a logo or a rank. */
    val lead: (@Composable (T) -> Unit)? = null,
    /**
     * The history of this figure, oldest first, in the units of [sort]: what its change, change in
     * percent and sparkline are worked out from. Null when the engine keeps no history of it, and
     * then those ways of showing it are not offered.
     */
    val series: ((T) -> List<Long>)? = null,
    /** Weeks per point of [series]: 1 for weekly prices, 4 for four-week results. */
    val step: Int = 1,
    /** How a change or a total of this figure reads. */
    val fmt: ((Long) -> String)? = null,
    /** Units of [sort] per unit a player types in a filter: 100 for money in cents and for percent in basis points. */
    val scale: Double = 1.0,
    /** Add this figure up in group subtotals and in the total of what is shown. */
    val total: Boolean = false,
    /** The heading the column chooser files it under. */
    val topic: String = "Figures",
)

/** Body text for figures: the readable face with every digit the same width, so columns line up. */
val GridFigures = TextStyle(fontFeatureSettings = "tnum", fontSize = 12.sp)

/** Rows of a grid at desktop density. 24 dp rows put 30 and more on a 1080p page under the strips. */
val GridRowHeight: Dp = 24.dp

/** A figure that does not apply, or has no history for the chosen timeframe, reads as a dash, never as a zero. */
internal const val DASH = "-"

/** What a scope choice covers, such as one sector or one region. Several can be on at once. */
class ScopeOption<T>(val id: String, val label: String, val kind: String, val color: Color? = null, val test: (T) -> Boolean)

/**
 * What makes a grid keep views: the screen's name in the settings, its built-in views (the first
 * is the default), what counts as the player's own, and the scope choices.
 */
class GridViews<T>(
    val screen: String,
    val builtIn: List<ViewState>,
    val mine: (T) -> Boolean = { false },
    val scopes: List<ScopeOption<T>> = emptyList(),
)

/** Choices that hold for every grid: the game's timeframe and the row density. Kept in the settings file. */
object GridPrefs {
    var timeframe by mutableStateOf(Timeframe.Default)
    var compact by mutableStateOf(true)
    private var loaded = false

    fun load(prefs: ViewPrefs) {
        if (loaded) return
        loaded = true
        Timeframe.of(prefs.read("grid.timeframe"))?.let { timeframe = it }
        prefs.read("grid.density")?.let { compact = it != "comfortable" }
    }

    fun chooseTimeframe(tf: Timeframe, prefs: ViewPrefs?) { timeframe = tf; prefs?.write("grid.timeframe", tf.label) }
    fun chooseCompact(on: Boolean, prefs: ViewPrefs?) { compact = on; prefs?.write("grid.density", if (on) "compact" else "comfortable") }
}

/** Pictures and tests only: open a menu, type a search or reshape a screen's view when its grid first draws. */
object GridDemo {
    var screen: String? = null
    var menu: String? = null
    var search: String = ""
    var shape: ((ViewState) -> ViewState)? = null
    fun clear() { screen = null; menu = null; search = ""; shape = null }
}

private val fallbackPrefs = MapViewPrefs()

/**
 * The shared data grid for every table in the 2.0 interface. With [views] it carries the control
 * bar (View, Scope, Timeframe, Search, Filter, Columns, more), saved views, filter chips, column
 * modes over the timeframe and grouping with subtotals. Without, a plain title bar with a filter.
 * Below [phoneBelow] wide it becomes two-line rows and the bar folds into one View menu.
 */
@Composable
fun <T> DataGrid(
    title: String,
    rows: List<T>,
    columns: List<GridColumn<T>>,
    key: (T) -> Any,
    modifier: Modifier = Modifier,
    filterText: (T) -> String = { r -> columns.firstOrNull()?.text?.invoke(r).orEmpty() },
    selected: Any? = null,
    onSelect: (T) -> Unit = {},
    onOpen: ((T) -> Unit)? = null,
    breakdown: ((T) -> String)? = null,
    initialSort: String? = null,
    initialDescending: Boolean = true,
    phoneBelow: Dp = 640.dp,
    views: GridViews<T>? = null,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    val prefs: ViewPrefs = LocalAppSettings.current ?: fallbackPrefs
    remember(prefs) { GridPrefs.load(prefs); 0 }
    val book = remember(views?.screen) { views?.let { ViewBook(it.screen, it.builtIn, prefs) } }
    val plain = remember { ViewState("", columns.filter { it.shown }.map { ColumnSpec(it.id) }, initialSort, initialDescending) }
    val demo = views != null && GridDemo.screen == views.screen
    var view by remember(views?.screen) {
        mutableStateOf(book?.let { b -> b.named(b.activeName)!!.let { v -> if (demo) GridDemo.shape?.invoke(v) ?: v else v } } ?: plain)
    }
    var search by remember(views?.screen) { mutableStateOf(if (demo) GridDemo.search else "") }
    var menu by remember(views?.screen) { mutableStateOf(if (demo) GridDemo.menu else null) }
    var typing by remember { mutableStateOf(false) }
    // Columns the player switched on stay even when they do not fit (the grid then scrolls sideways).
    val extra = remember { mutableStateListOf<String>() }
    val collapsed = remember { mutableStateListOf<String>() }
    var hovered by remember { mutableStateOf<Any?>(null) }
    val base = book?.named(view.name)
    val modified = book != null && base != null && !view.sameShape(base)
    val tf = view.timeframe ?: GridPrefs.timeframe
    val byId = columns.associateBy { it.id }
    val first = columns.first()
    val specs = view.columns.filter { it.id in byId }.let { s -> if (s.none { it.id == first.id }) listOf(ColumnSpec(first.id)) + s else s.sortedBy { if (it.id == first.id) 0 else 1 } }
    fun change(v: ViewState) { view = v }

    // Rows: scope, then chips and the typed search, then sort.
    val reader = remember(columns) {
        RowReader<T>(
            text = { r, id -> byId[id]?.text?.invoke(r) },
            figure = { r, id -> byId[id]?.let { c -> c.sort?.invoke(r)?.let { it / c.scale } } },
            mine = { r -> views?.mine?.invoke(r) ?: false },
            words = filterText,
        )
    }
    fun resolve(name: String): String? {
        val n = name.lowercase().replace("_", "").replace(" ", "")
        return columns.firstOrNull { it.id.lowercase() == n || it.title.lowercase().replace(" ", "") == n || it.title.lowercase().startsWith(n) && n.length >= 3 }?.id
    }
    val typed = ViewQuery.parse(search, ::resolve)
    val scoped = if (view.scope.isEmpty() || views == null) rows else rows.filter { r ->
        view.scope.any { id -> if (id == "mine") views.mine(r) else views.scopes.firstOrNull { it.id == id }?.test?.invoke(r) ?: false }
    }
    val kept = applyFilters(scoped, view.filters + typed, reader)
    val effective = specs.map { s -> applyMode(byId.getValue(s.id), s, tf, kept) }
    val sortCol = effective.firstOrNull { it.id == view.sort }
    val shown = run {
        val sorted = when {
            sortCol == null -> kept
            sortCol.sort != null -> kept.sortedWith(compareBy<T, Long?>(nullsFirst(naturalOrder())) { r -> sortCol.sort.invoke(r).takeIf { it != Long.MIN_VALUE } })
            else -> kept.sortedBy { sortCol.text(it).lowercase() }
        }
        if (sortCol != null && view.descending) sorted.reversed() else sorted
    }
    fun pickSort(col: GridColumn<T>) {
        change(if (view.sort == col.id) view.copy(descending = !view.descending) else view.copy(sort = col.id, descending = col.numeric))
    }
    val list = rememberLazyListState()
    LaunchedEffect(selected, shown) {
        val at = shown.indexOfFirst { key(it) == selected }
        if (at >= 0 && (at < list.firstVisibleItemIndex || at > list.firstVisibleItemIndex + list.layoutInfo.visibleItemsInfo.size - 2)) list.scrollToItem((at - 3).coerceAtLeast(0))
    }
    val rowHeight = if (GridPrefs.compact) GridRowHeight else 30.dp
    BoxWithConstraints(modifier) {
    val gridWidth = maxWidth
    val phone = gridWidth < phoneBelow
    // A column is never narrower than its heading, so a heading cannot clip. The first column is
    // pinned and takes the room that is left; columns that do not fit are left out (the chooser
    // brings them back) and the rest scroll sideways together only when the player asked for more.
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val headStyle = remember { TextStyle(fontSize = 10.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.4.sp) }
    fun headingWidth(c: GridColumn<T>): Dp = with(density) { measurer.measure(AnnotatedString(c.title.uppercase() + " v"), headStyle, maxLines = 1).size.width.toDp() } + 12.dp
    fun columnWidth(c: GridColumn<T>): Dp = maxOf(c.width ?: 110.dp, headingWidth(c))
    val firstCol = effective.first()
    val chrome = 14.dp + if (onOpen != null) 18.dp else 0.dp
    val firstMin = maxOf(firstCol.minWidth ?: 150.dp, firstCol.width ?: 0.dp, headingWidth(firstCol))
    var rest = effective.drop(1)
    if (!phone) {
        while (rest.isNotEmpty() && firstMin + rest.fold(0.dp) { a, c -> a + columnWidth(c) } > gridWidth - chrome) {
            val out = rest.filter { it.id !in extra }.maxWithOrNull(compareBy<GridColumn<T>>({ it.dropOrder }, { effective.indexOf(it) })) ?: break
            rest = rest - out
        }
    }
    val dropped = effective.drop(1).size - rest.size
    val visible = listOf(firstCol) + rest
    Column(
        Modifier.fillMaxSize().border(1.dp, Palette.stroke).background(Palette.panel)
            .onPreviewKeyEvent { e ->
                if (e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                // 1 to 9 switch views, unless the player is typing.
                val digit = digitOf(e.key)
                if (book != null && digit != null && !typing && !TextEntryFocus.active) {
                    book.all.getOrNull(digit - 1)?.let { v -> change(v); book.activate(v.name) }
                    return@onPreviewKeyEvent true
                }
                if (shown.isEmpty()) return@onPreviewKeyEvent false
                val at = shown.indexOfFirst { key(it) == selected }
                when (e.key) {
                    Key.DirectionDown -> { onSelect(shown[(at + 1).coerceAtMost(shown.size - 1)]); true }
                    Key.DirectionUp -> { onSelect(shown[(at - 1).coerceAtLeast(0)]); true }
                    Key.Enter -> { if (at >= 0 && !typing) onOpen?.invoke(shown[at]); at >= 0 && onOpen != null && !typing }
                    else -> false
                }
            }.focusable(),
    ) {
        if (book != null && views != null) {
            ControlBar(
                title, rows.size, shown.size, view, modified, book, views, columns, tf, search, menu, phone, dropped,
                onView = ::change, onSearch = { search = it }, onTyping = { typing = it }, onMenu = { menu = it },
                onCommitSearch = {
                    val chips = ViewQuery.parse(search, ::resolve)
                    change(chips.filter { it.op != FilterOp.Text }.fold(view) { v, f -> v.withFilter(f) })
                    search = chips.firstOrNull { it.op == FilterOp.Text }?.text.orEmpty()
                },
                csv = { gridCsv(shown, visible) },
                distinct = { id -> rows.mapNotNull { byId[id]?.text?.invoke(it) }.filter { it.isNotBlank() && it != DASH }.distinct().sorted() },
                onExtra = { id -> extra.add(id) },
            )
            ChipRow(view, views, columns, ::change)
        } else {
            // Title, count and filter: the small grids inside a pane.
            Row(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(title.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 11.sp, maxLines = 1)
                Spacer(Modifier.width(8.dp))
                Text(if (shown.size == rows.size) "${rows.size}" else "${shown.size} of ${rows.size}", color = Palette.secondary, style = GridFigures, fontSize = 11.sp)
                Spacer(Modifier.weight(1f))
                trailing()
                GridSearch(search, "Filter", 150.dp, { typing = it }, { search = it }, {})
            }
        }
        Box(Modifier.fillMaxWidth().weight(1f)) {
            if (phone) {
                TwoLineRows(shown, visible, sortCol, view.descending, key, selected, onSelect, onOpen, { pickSort(it) })
                return@Box
            }
            val widths = rest.associate { c -> c.id to columnWidth(c) }
            val restWidth = widths.values.fold(0.dp) { a, w -> a + w }
            val firstWidth = if (firstCol.width != null) maxOf(firstCol.width, headingWidth(firstCol)) else (gridWidth - chrome - restWidth).coerceAtLeast(firstMin)
            val hScroll = rememberScrollState()
            val pad = 6.dp
            val group = view.groupBy?.let { byId[it] }
            Column(Modifier.fillMaxSize()) {
                // The header stays put while the rows scroll under it.
                Row(Modifier.fillMaxWidth().height(22.dp).background(Palette.ink).padding(horizontal = pad), verticalAlignment = Alignment.CenterVertically) {
                    Spacer(Modifier.width(2.dp))
                    HeaderCell(firstCol, firstWidth, firstCol.id == view.sort, view.descending, tf) { pickSort(firstCol) }
                    Box(Modifier.weight(1f).horizontalScroll(hScroll)) {
                        Row(Modifier.width(restWidth)) { rest.forEach { c -> HeaderCell(c, widths.getValue(c.id), c.id == view.sort, view.descending, tf) { pickSort(c) } } }
                    }
                    if (onOpen != null) Spacer(Modifier.width(18.dp))
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
                val cellMenu: ((Filter) -> Unit)? = if (views != null) { f -> change(view.withFilter(f)) } else null
                LazyColumn(Modifier.fillMaxWidth().weight(1f), state = list) {
                    if (group == null) {
                        items(shown.size, key = { i -> key(shown[i]) }) { i ->
                            val r = shown[i]
                            GridRow(i, r, firstCol, firstWidth, rest, widths, restWidth, hScroll, key(r) == selected, rowHeight, cellMenu,
                                onHover = { hovered = if (it) key(r) else if (hovered == key(r)) null else hovered }, onClick = { onSelect(r) }, onOpen = onOpen?.let { o -> { o(r) } })
                        }
                    } else {
                        groupRows(shown) { group.text(it) }.forEach { (name, members) ->
                            item(key = "group:$name") {
                                GroupRow(name, members, firstWidth, rest, widths, restWidth, hScroll, name in collapsed, byId) { if (name in collapsed) collapsed.remove(name) else collapsed.add(name) }
                            }
                            if (name !in collapsed) items(members.size, key = { i -> key(members[i]) }) { i ->
                                val r = members[i]
                                GridRow(i, r, firstCol, firstWidth, rest, widths, restWidth, hScroll, key(r) == selected, rowHeight, cellMenu,
                                    onHover = { hovered = if (it) key(r) else if (hovered == key(r)) null else hovered }, onClick = { onSelect(r) }, onOpen = onOpen?.let { o -> { o(r) } })
                            }
                        }
                    }
                }
                // The total of what is shown when the player has narrowed it, then the breakdown line.
                if (views != null && (view.scope.isNotEmpty() || view.filters.isNotEmpty() || typed.isNotEmpty()) && rest.any { byId[it.id]?.total == true }) {
                    Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
                    GroupRow("Total of ${shown.size} shown", shown, firstWidth, rest, widths, restWidth, hScroll, null, byId) {}
                }
                if (breakdown != null) {
                    val focus = shown.firstOrNull { key(it) == hovered } ?: shown.firstOrNull { key(it) == selected }
                    Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
                    Text(
                        focus?.let(breakdown) ?: "Point at a row for its breakdown. Click a heading to sort.",
                        color = if (focus != null) Palette.primaryText else Palette.secondary, style = GridFigures, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.fillMaxWidth().background(Palette.ink).padding(horizontal = 10.dp, vertical = 4.dp),
                    )
                }
            }
        }
    }
    }
}

private fun digitOf(k: Key): Int? = when (k) {
    Key.One -> 1; Key.Two -> 2; Key.Three -> 3; Key.Four -> 4; Key.Five -> 5
    Key.Six -> 6; Key.Seven -> 7; Key.Eight -> 8; Key.Nine -> 9; else -> null
}

/** The heading a column carries in a way of showing it. */
internal fun modeTitle(title: String, mode: ColumnMode): String = when (mode) {
    ColumnMode.Value, ColumnMode.Bar -> title
    ColumnMode.Change -> "$title chg"
    ColumnMode.Percent -> "$title %"
    ColumnMode.Spark -> "$title trend"
    ColumnMode.Rank -> "$title rank"
}

/** The ways a column can show: change, percent and sparkline only when the engine keeps its history. */
internal fun <T> modesOf(c: GridColumn<T>): List<ColumnMode> = when {
    !c.numeric || c.sort == null -> listOf(ColumnMode.Value)
    c.series != null -> ColumnMode.entries
    else -> listOf(ColumnMode.Value, ColumnMode.Bar, ColumnMode.Rank)
}

/** A column as it shows in [spec]'s way over [tf]: its text, sort, tone, bar and sparkline follow. */
private fun <T> applyMode(c: GridColumn<T>, spec: ColumnSpec, tf: Timeframe, rows: List<T>): GridColumn<T> {
    val series = c.series
    val sort = c.sort
    fun make(text: (T) -> String, sortBy: ((T) -> Long)?, tone: (T) -> Color, bar: ((T) -> Float)? = null, spark: ((T) -> List<Long>)? = null, width: Dp? = c.width, tip: String) =
        GridColumn(spec.key, modeTitle(c.title, spec.mode), width, true, text, sortBy, tone, bar, spark, tip, true, false, c.dropOrder, null, null, series, c.step, c.fmt, c.scale, false, c.topic)
    val span = if (tf == Timeframe.All) "over all the history kept" else "over the last ${tf.label}"
    return when (spec.mode) {
        ColumnMode.Value -> c
        ColumnMode.Change -> if (series == null) c else make(
            text = { r -> seriesChange(series(r), tf, c.step)?.let { v -> (if (v > 0) "+" else "") + (c.fmt?.invoke(v) ?: Format.count(v)) } ?: DASH },
            sortBy = { r -> seriesChange(series(r), tf, c.step) ?: Long.MIN_VALUE },
            tone = { r -> seriesChange(series(r), tf, c.step)?.let { if (it > 0) Palette.positive else if (it < 0) Palette.negative else Palette.secondary } ?: Palette.secondary },
            tip = "How much ${c.title.lowercase()} changed $span. A dash means the game has no record that far back.",
        )
        ColumnMode.Percent -> if (series == null) c else make(
            text = { r -> seriesChangeBps(series(r), tf, c.step)?.let(::changeText) ?: DASH },
            sortBy = { r -> seriesChangeBps(series(r), tf, c.step) ?: Long.MIN_VALUE },
            tone = { r -> seriesChangeBps(series(r), tf, c.step)?.let(::changeTint) ?: Palette.secondary },
            width = 70.dp,
            tip = "${c.title} $span, as a percent of where it started. A dash means the game has no record that far back.",
        )
        ColumnMode.Spark -> if (series == null) c else make(
            text = { DASH }, sortBy = { r -> seriesChangeBps(series(r), tf, c.step) ?: Long.MIN_VALUE }, tone = { Palette.secondary },
            spark = { r -> seriesWindow(series(r), tf, c.step) ?: emptyList() }, width = 90.dp,
            tip = "${c.title} $span. Sorting by it sorts by the change in percent.",
        )
        ColumnMode.Bar -> if (sort == null) c else {
            val top = rows.maxOfOrNull { kotlin.math.abs(sort(it).takeIf { v -> v != Long.MIN_VALUE } ?: 0L) }?.coerceAtLeast(1L) ?: 1L
            make(text = c.text, sortBy = sort, tone = c.tone, bar = { r -> kotlin.math.abs(sort(r)).toFloat() / top }, tip = c.tip ?: Words.about(c.title).orEmpty())
        }
        ColumnMode.Rank -> if (sort == null) c else {
            val order = rows.sortedByDescending { sort(it) }
            val rank = order.withIndex().associate { (i, r) -> (r as Any?) to i + 1 }
            make(text = { r -> "#${rank[r] ?: 0}" }, sortBy = { r -> -(rank[r] ?: 0).toLong() }, tone = { r -> if ((rank[r] ?: 99) <= 3) Palette.gold else Palette.primaryText }, width = 64.dp,
                tip = "Place by ${c.title.lowercase()} among the rows shown, 1 the highest.")
        }
    }
}

/** What one grid needs besides its rows: the title bar, the column heading, the breakdown line and the frame. */
val GridChromeHeight: Dp = 88.dp

/** The height a grid wants for [rows] rows, so a small table does not sit in a tall empty pane. */
fun gridNaturalHeight(rows: Int): Dp = GridChromeHeight + GridRowHeight * rows.coerceAtLeast(2)

@Composable
private fun <T> HeaderCell(c: GridColumn<T>, width: Dp, sorted: Boolean, descending: Boolean, tf: Timeframe, onClick: () -> Unit) {
    val arrow = if (sorted) (if (descending) " v" else " ^") else ""
    Explain(c.tip ?: Words.about(c.title), Modifier.width(width)) {
        Text(
            c.title.uppercase() + arrow, color = if (sorted) Palette.gold else Palette.secondary,
            fontSize = 10.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.4.sp, maxLines = 1, softWrap = false, overflow = TextOverflow.Visible,
            textAlign = if (c.numeric) TextAlign.End else TextAlign.Start,
            modifier = Modifier.fillMaxWidth().pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick).padding(horizontal = 4.dp),
        )
    }
}

@Composable
private fun <T> GridRow(
    index: Int, row: T, first: GridColumn<T>, firstWidth: Dp, rest: List<GridColumn<T>>, widths: Map<String, Dp>, restWidth: Dp,
    hScroll: androidx.compose.foundation.ScrollState, selected: Boolean, height: Dp, cellMenu: ((Filter) -> Unit)?,
    onHover: (Boolean) -> Unit, onClick: () -> Unit, onOpen: (() -> Unit)?,
) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    LaunchedEffect(hovered) { onHover(hovered) }
    val base = if (index % 2 == 1) Palette.panelRaised.copy(alpha = 0.45f) else Color.Transparent
    Row(
        Modifier.fillMaxWidth().height(height)
            .background(if (selected) Palette.gold.copy(alpha = 0.18f) else if (hovered) Palette.panelRaised else base)
            .hoverable(hover).pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick).padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(2.dp).fillMaxHeight().background(if (selected) Palette.gold else Color.Transparent))
        GridCell(row, first, firstWidth, null, onClick)
        Box(Modifier.weight(1f).fillMaxHeight().horizontalScroll(hScroll)) {
            Row(Modifier.width(restWidth).fillMaxHeight()) {
                rest.forEach { c ->
                    // A category cell (a sector, a region) offers "only this" and "not this".
                    GridCell(row, c, widths.getValue(c.id), if (!c.numeric) cellMenu else null, onClick)
                }
            }
        }
        if (onOpen != null) {
            Text(
                ">", color = if (hovered || selected) Palette.gold else Palette.stroke, fontFamily = Pixel.font, fontSize = 11.sp,
                modifier = Modifier.width(18.dp).clickable(onClick = onOpen).padding(start = 6.dp),
            )
        }
    }
}

/** A group's heading row: its name, how many rows, and the subtotal of every column that adds up. Click to fold. */
@Composable
private fun <T> GroupRow(
    name: String, members: List<T>, firstWidth: Dp, rest: List<GridColumn<T>>, widths: Map<String, Dp>, restWidth: Dp,
    hScroll: androidx.compose.foundation.ScrollState, folded: Boolean?, byId: Map<String, GridColumn<T>>, onClick: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().height(GridRowHeight).background(Palette.panelRaised).clickable(enabled = folded != null, onClick = onClick).padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(2.dp).fillMaxHeight().background(Palette.gold))
        Text(
            (if (folded == null) "" else if (folded) "+ " else "- ") + "${name.ifBlank { DASH }}  (${members.size})", color = Palette.gold, style = GridFigures, fontWeight = FontWeight.SemiBold,
            maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.width(firstWidth).padding(horizontal = 4.dp),
        )
        Box(Modifier.weight(1f).fillMaxHeight().horizontalScroll(hScroll)) {
            Row(Modifier.width(restWidth).fillMaxHeight(), verticalAlignment = Alignment.CenterVertically) {
                rest.forEach { c ->
                    val src = byId[c.id]
                    val sum = if (src?.total == true && src.sort != null) members.sumOf { src.sort.invoke(it) } else null
                    Text(
                        sum?.let { src?.fmt?.invoke(it) ?: Format.count(it) } ?: "", color = Palette.primaryText, style = GridFigures, fontWeight = FontWeight.SemiBold,
                        textAlign = TextAlign.End, maxLines = 1, modifier = Modifier.width(widths.getValue(c.id)).padding(horizontal = 4.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun <T> GridCell(row: T, c: GridColumn<T>, width: Dp, onPick: ((Filter) -> Unit)?, onClick: () -> Unit) {
    // A category cell (a sector, a region) offers "only this" and "not this" when clicked.
    var open by remember { mutableStateOf(false) }
    val value = c.text(row)
    if (open && onPick != null) Box {
        PixelMenu({ open = false }, 220.dp) {
            MenuItem("Only $value") { onPick(Filter(FilterOp.Is, c.id, value)); open = false }
            MenuItem("Not $value") { onPick(Filter(FilterOp.Not, c.id, value)); open = false }
        }
    }
    val m = Modifier.width(width).fillMaxHeight().let { if (onPick != null && value.isNotBlank() && value != DASH) it.clickable { onClick(); open = true } else it }.padding(horizontal = 4.dp)
    val spark = c.spark
    if (spark != null) {
        val values = spark(row)
        if (values.size >= 2) Box(m.padding(vertical = 4.dp)) { GridSpark(values, Modifier.fillMaxSize()) }
        else Box(m, contentAlignment = Alignment.CenterEnd) { Text(DASH, color = Palette.secondary, style = GridFigures) }
        return
    }
    Box(m, contentAlignment = if (c.numeric) Alignment.CenterEnd else Alignment.CenterStart) {
        c.bar?.let { bar ->
            val f = bar(row).coerceIn(0f, 1f)
            if (f > 0f) Box(Modifier.align(Alignment.CenterStart).fillMaxWidth(f).height(14.dp).background(Palette.gold.copy(alpha = 0.22f)))
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            c.lead?.let { lead -> lead(row); Spacer(Modifier.width(6.dp)) }
            Text(
                c.text(row), color = c.tone(row), style = GridFigures, fontWeight = if (c.bold) FontWeight.SemiBold else FontWeight.Normal,
                maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = if (c.numeric) TextAlign.End else TextAlign.Start,
                modifier = if (c.numeric) Modifier else Modifier.weight(1f, fill = false),
            )
        }
    }
}

/** A stepped column chart of a series: one flat column per value, gold when the series ends up, red when down. */
@Composable
fun GridSpark(values: List<Long>, modifier: Modifier) {
    if (values.size < 2) return
    // Long series are cut to the last points that fit one pixel column each, never smoothed.
    val lo = values.min()
    val hi = values.max()
    val tone = if (values.last() >= values.first()) Palette.positive else Palette.negative
    Canvas(modifier) {
        val pts = if (values.size > size.width.toInt()) values.takeLast(size.width.toInt().coerceAtLeast(2)) else values
        val n = pts.size
        val step = size.width / n
        val span = (hi - lo).coerceAtLeast(1).toFloat()
        pts.forEachIndexed { i, v ->
            val h = (2f + (size.height - 2f) * ((v - lo) / span)).coerceAtLeast(1f)
            val x = kotlin.math.floor(i * step)
            val w = kotlin.math.max(1f, kotlin.math.floor(step) - if (step >= 3f) 1f else 0f)
            drawRect(tone, Offset(x, size.height - kotlin.math.floor(h)), Size(w, kotlin.math.floor(h)))
        }
    }
}

/** Phone rows: the first column and the sorted figure on the first line, the next figures under them. */
@Composable
private fun <T> TwoLineRows(
    shown: List<T>, columns: List<GridColumn<T>>, sortCol: GridColumn<T>?, descending: Boolean, key: (T) -> Any,
    selected: Any?, onSelect: (T) -> Unit, onOpen: ((T) -> Unit)?, onSort: (GridColumn<T>) -> Unit,
) {
    val name = columns.first()
    val figures = columns.drop(1).filter { it.spark == null }
    val lead = sortCol?.takeIf { it != name && it.spark == null } ?: figures.firstOrNull()
    val rest = figures.filter { it != lead }.take(3)
    Column(Modifier.fillMaxSize()) {
        // Sorting on a phone: one chip that steps through the columns, and one that reverses.
        Row(Modifier.fillMaxWidth().background(Palette.ink).padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("SORT", color = Palette.secondary, fontSize = 10.sp, fontFamily = Pixel.font)
            Spacer(Modifier.width(6.dp))
            val sortable = columns.filter { it.spark == null }
            GridButton((sortCol ?: name).title, true) {
                val at = sortable.indexOf(sortCol ?: name)
                onSort(sortable[(at + 1) % sortable.size])
            }
            Spacer(Modifier.width(4.dp))
            GridButton(if (descending) "High first" else "Low first", false) { onSort(sortCol ?: name) }
        }
        LazyColumn(Modifier.fillMaxSize()) {
            items(shown.size, key = { i -> key(shown[i]) }) { i ->
                val r = shown[i]
                val picked = key(r) == selected
                Column(
                    Modifier.fillMaxWidth()
                        .background(if (picked) Palette.gold.copy(alpha = 0.18f) else if (i % 2 == 1) Palette.panelRaised.copy(alpha = 0.45f) else Color.Transparent)
                        .clickable { onSelect(r); onOpen?.invoke(r) }.padding(horizontal = 10.dp, vertical = 5.dp),
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(name.text(r), color = name.tone(r), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                        if (lead != null) Text(lead.text(r), color = lead.tone(r), style = GridFigures, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        rest.forEach { c ->
                            Text("${c.title} ", color = Palette.secondary, fontSize = 11.sp, maxLines = 1)
                            Text(c.text(r), color = c.tone(r), style = GridFigures, fontSize = 11.sp, maxLines = 1, modifier = Modifier.padding(start = 0.dp))
                        }
                    }
                }
            }
        }
    }
}

/** The shown rows as comma separated values, headings first, for a spreadsheet. */
internal fun <T> gridCsv(rows: List<T>, columns: List<GridColumn<T>>): String {
    fun cell(s: String) = if (s.any { it == ',' || it == '"' || it == '\n' }) "\"" + s.replace("\"", "\"\"") + "\"" else s
    val head = columns.filter { it.spark == null }
    return (listOf(head.joinToString(",") { cell(it.title) }) + rows.map { r -> head.joinToString(",") { cell(it.text(r)) } }).joinToString("\n")
}

/** A small square pixel button for grid controls: gold when on. */
@Composable
fun GridButton(text: String, on: Boolean, size: TextUnit = 10.sp, onClick: () -> Unit) {
    Text(
        text.uppercase(), color = if (on) Palette.ink else Palette.primaryText, fontFamily = Pixel.font, fontSize = size, maxLines = 1, softWrap = false,
        modifier = Modifier.background(if (on) Palette.gold else Palette.panel).border(1.dp, if (on) Palette.gold else Palette.stroke)
            .pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick).padding(horizontal = 7.dp, vertical = 3.dp),
    )
}
