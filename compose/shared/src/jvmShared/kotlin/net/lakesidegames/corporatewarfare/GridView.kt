package net.lakesidegames.corporatewarfare

import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * The state of one grid that a player can shape and keep: which columns and how each shows, the
 * sort, the filters, the grouping, the timeframe and the scope. A screen ships a few built-in views;
 * the player's own views and changes to the built-in ones are kept in the settings file, outside
 * any save, so they carry from one career to the next. Plain data, no Compose, so it is tested on
 * its own.
 */

/** How far back every change, sparkline and bar looks. One control for the whole game; a pane may override it. */
@Serializable
enum class Timeframe(val label: String, val weeks: Int?) {
    W4("4W", 4), W13("13W", 13), Y1("1Y", 52), Y5("5Y", 260), All("All", null);

    companion object {
        val Default = W13
        fun of(label: String?): Timeframe? = entries.firstOrNull { it.label.equals(label, true) || it.name.equals(label, true) }
    }
}

/** What a compare puts beside a change: nothing, the period before, or the same period a year earlier. */
@Serializable
enum class Compare(val label: String) { None("No compare"), Previous("Previous period"), YearAgo("Same period last year") }

/** How a numeric column shows: the figure, its change over the timeframe, the change in percent, a sparkline, a bar or its rank. */
@Serializable
enum class ColumnMode(val label: String) {
    Value("Value"), Change("Change"), Percent("Change %"), Spark("Sparkline"), Bar("Bar"), Rank("Rank");
}

/** One shown column and how it shows. */
@Serializable
data class ColumnSpec(val id: String, val mode: ColumnMode = ColumnMode.Value) {
    /** A column may show twice in different ways (price and its change); the key tells them apart. */
    val key: String get() = if (mode == ColumnMode.Value) id else "$id.${mode.name}"
}

/** The kinds of filter chip. Figures are in the units the player reads (money in whole units, percent in percent). */
@Serializable
enum class FilterOp(val symbol: String) { Is(":"), Not("!:"), Above(">"), Below("<"), Top("top"), Bottom("bottom"), Mine("mine"), Text("") }

/** A filter as a chip under the control bar. */
@Serializable
data class Filter(val op: FilterOp, val column: String = "", val text: String = "", val number: Double = 0.0) {
    /** The chip's words, as a player would type them. */
    fun label(title: (String) -> String): String = when (op) {
        FilterOp.Is -> "${title(column)}: $text"
        FilterOp.Not -> "${title(column)}: not $text"
        FilterOp.Above -> "${title(column)} > ${trim(number)}"
        FilterOp.Below -> "${title(column)} < ${trim(number)}"
        FilterOp.Top -> "Top ${number.toInt()} by ${title(column)}"
        FilterOp.Bottom -> "Bottom ${number.toInt()} by ${title(column)}"
        FilterOp.Mine -> "Mine"
        FilterOp.Text -> "\"$text\""
    }
}

private fun trim(d: Double): String = if (d == Math.floor(d) && kotlin.math.abs(d) < 1e15) d.toLong().toString() else d.toString()

/** A grid's whole shape. [name] is the view it came from (or was saved as). */
@Serializable
data class ViewState(
    val name: String,
    val columns: List<ColumnSpec>,
    val sort: String? = null,
    val descending: Boolean = true,
    val filters: List<Filter> = emptyList(),
    val groupBy: String? = null,
    /** Null follows the game's timeframe in the top strip; a value overrides it for this grid. */
    val timeframe: Timeframe? = null,
    /** Scope choices that are on ("mine", "sector:chemicals"); empty is everything. */
    val scope: List<String> = emptyList(),
    val compare: Compare = Compare.None,
) {
    /** The same shape regardless of name: what "modified" compares. */
    fun sameShape(other: ViewState): Boolean = copy(name = "") == other.copy(name = "")

    /** Switch a column on (as a plain figure, at the end or at [at]) or off (every way it shows). */
    fun withColumn(id: String, on: Boolean, at: Int? = null): ViewState = when {
        on && columns.none { it.id == id } -> copy(columns = columns.toMutableList().apply { add((at ?: size).coerceIn(0, size), ColumnSpec(id)) })
        !on -> copy(columns = columns.filter { it.id != id }, sort = sort.takeIf { s -> columns.none { it.key == s && it.id == id } })
        else -> this
    }

    /**
     * Show a column one more way, or one fewer (a column can show as the figure and its change
     * side by side). The last way it shows cannot go: switch the column off for that.
     */
    fun toggleMode(id: String, mode: ColumnMode): ViewState {
        val spec = ColumnSpec(id, mode)
        val mine = columns.filter { it.id == id }
        return when {
            spec in columns && mine.size > 1 -> copy(columns = columns - spec, sort = sort.takeIf { it != spec.key })
            spec in columns -> this
            mine.isEmpty() -> copy(columns = columns + spec)
            else -> copy(columns = columns.toMutableList().apply { add(indexOf(mine.last()) + 1, spec) })
        }
    }

    /** Show the column with [key] another way; the sort follows it. */
    fun withMode(key: String, mode: ColumnMode): ViewState {
        val at = columns.indexOfFirst { it.key == key }
        if (at < 0) return this
        val next = columns[at].copy(mode = mode)
        if (columns.any { it.key == next.key }) return this
        return copy(columns = columns.toMutableList().apply { set(at, next) }, sort = if (sort == key) next.key else sort)
    }
    fun withFilter(f: Filter): ViewState = copy(filters = filters.filterNot { it.op == f.op && it.column == f.column && (f.op != FilterOp.Is && f.op != FilterOp.Not || it.text == f.text) } + f)
    fun withoutFilter(f: Filter): ViewState = copy(filters = filters - f)
}

/** Reads a typed search into chips and leftover words: `sector:chemicals margin>10 mine steel`. */
object ViewQuery {
    /**
     * [resolve] turns a typed column name into a column id (or null if no column matches). Terms
     * that name no column, and plain words, stay as free text.
     */
    fun parse(input: String, resolve: (String) -> String?): List<Filter> {
        val out = mutableListOf<Filter>()
        val words = mutableListOf<String>()
        for (raw in tokens(input)) {
            val t = raw.trim()
            if (t.isEmpty()) continue
            if (t.equals("mine", true)) { out += Filter(FilterOp.Mine); continue }
            val cmp = Regex("^([A-Za-z_%]+)(>=|<=|>|<)(-?[0-9]+(?:\\.[0-9]+)?)%?$").find(t)
            if (cmp != null) {
                val col = resolve(cmp.groupValues[1])
                val n = cmp.groupValues[3].toDouble()
                if (col != null) {
                    out += Filter(if (cmp.groupValues[2].startsWith(">")) FilterOp.Above else FilterOp.Below, col, number = n)
                    continue
                }
            }
            val top = Regex("^(top|bottom):?([0-9]+):([A-Za-z_%]+)$", RegexOption.IGNORE_CASE).find(t)
            if (top != null) {
                val col = resolve(top.groupValues[3])
                if (col != null) {
                    out += Filter(if (top.groupValues[1].equals("top", true)) FilterOp.Top else FilterOp.Bottom, col, number = top.groupValues[2].toDouble())
                    continue
                }
            }
            val colon = t.indexOf(':')
            if (colon > 0 && colon < t.length - 1) {
                val negate = t.startsWith("-") || t.startsWith("!")
                val name = t.substring(if (negate) 1 else 0, colon)
                val col = resolve(name)
                if (col != null) {
                    out += Filter(if (negate) FilterOp.Not else FilterOp.Is, col, text = t.substring(colon + 1).replace('_', ' '))
                    continue
                }
            }
            words += t
        }
        if (words.isNotEmpty()) out += Filter(FilterOp.Text, text = words.joinToString(" "))
        return out
    }

    /** Words split on spaces; a quoted run stays one word. */
    fun tokens(input: String): List<String> {
        val out = mutableListOf<String>()
        val cur = StringBuilder()
        var quoted = false
        for (c in input) {
            when {
                c == '"' -> quoted = !quoted
                c.isWhitespace() && !quoted -> { if (cur.isNotEmpty()) out += cur.toString(); cur.clear() }
                else -> cur.append(c)
            }
        }
        if (cur.isNotEmpty()) out += cur.toString()
        return out
    }
}

/**
 * What a filter needs to know about a row: a column's text (for Is and Not), its figure in the
 * units the player reads (for Above, Below, Top and Bottom), whether the row is the player's, and
 * the row's searchable words.
 */
class RowReader<T>(
    val text: (T, String) -> String?,
    val figure: (T, String) -> Double?,
    val mine: (T) -> Boolean,
    val words: (T) -> String,
)

/** Applies every chip. Top and Bottom keep the first N by that column among the rows the other chips kept. */
fun <T> applyFilters(rows: List<T>, filters: List<Filter>, read: RowReader<T>): List<T> {
    var kept = rows.filter { r ->
        filters.all { f ->
            when (f.op) {
                FilterOp.Is -> read.text(r, f.column)?.contains(f.text, ignoreCase = true) == true
                FilterOp.Not -> read.text(r, f.column)?.contains(f.text, ignoreCase = true) != true
                FilterOp.Above -> (read.figure(r, f.column) ?: return@all false) > f.number
                FilterOp.Below -> (read.figure(r, f.column) ?: return@all false) < f.number
                FilterOp.Mine -> read.mine(r)
                FilterOp.Text -> f.text.split(' ').filter { it.isNotBlank() }.all { w -> read.words(r).contains(w, ignoreCase = true) }
                FilterOp.Top, FilterOp.Bottom -> true
            }
        }
    }
    for (f in filters.filter { it.op == FilterOp.Top || it.op == FilterOp.Bottom }) {
        val n = f.number.toInt().coerceAtLeast(0)
        val withFigure = kept.filter { read.figure(it, f.column) != null }
        val ordered = withFigure.sortedBy { read.figure(it, f.column)!! }
        val pick = (if (f.op == FilterOp.Top) ordered.reversed() else ordered).take(n).toSet()
        kept = kept.filter { it in pick }
    }
    return kept
}

/** Rows in groups by a text key, in the order the groups first appear in [rows]. */
fun <T> groupRows(rows: List<T>, key: (T) -> String): List<Pair<String, List<T>>> =
    rows.groupBy(key).toList()

/**
 * The points of a series that fall in a timeframe, plus the point just before it so a change can be
 * measured, or null when the series does not reach back that far. [step] is the weeks per point
 * (1 for weekly prices, 4 for four-week results).
 */
fun seriesWindow(series: List<Long>, tf: Timeframe, step: Int = 1): List<Long>? {
    val weeks = tf.weeks ?: return series.takeIf { it.size >= 2 }
    val points = (weeks + step - 1) / step
    return if (series.size > points) series.takeLast(points + 1) else null
}

/** A change over a series: last minus first of the window, or null when the series does not reach back that far. */
fun seriesChange(series: List<Long>, tf: Timeframe, step: Int = 1): Long? = seriesWindow(series, tf, step)?.let { it.last() - it.first() }

/** The change as basis points of the start, or null when there is no start to measure from. */
fun seriesChangeBps(series: List<Long>, tf: Timeframe, step: Int = 1): Long? {
    val w = seriesWindow(series, tf, step) ?: return null
    if (w.first() == 0L) return null
    return (w.last() - w.first()) * 10_000 / kotlin.math.abs(w.first())
}

/** A screen's views: its built-in ones and what the player saved. Persisted as one small JSON value per screen. */
@Serializable
data class SavedViews(val views: List<ViewState> = emptyList(), val active: String? = null)

object ViewJson {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = false }
    fun encode(v: SavedViews): String = json.encodeToString(v)
    fun decode(s: String?): SavedViews = s?.let { runCatching { json.decodeFromString<SavedViews>(it) }.getOrNull() } ?: SavedViews()

    /** One view as a small file a player can pass to a friend. */
    fun exportView(v: ViewState): String = json.encodeToString(v)
    fun importView(s: String): ViewState? = runCatching { json.decodeFromString<ViewState>(s.trim()) }.getOrNull()
}

/** Where views and the game's timeframe are kept: the settings file on a real install, a map in tests. */
interface ViewPrefs {
    fun read(key: String): String?
    fun write(key: String, value: String?)
}

class MapViewPrefs(val map: MutableMap<String, String> = mutableMapOf()) : ViewPrefs {
    override fun read(key: String) = map[key]
    override fun write(key: String, value: String?) { if (value == null) map.remove(key) else map[key] = value }
}

/**
 * One screen's views. The current state may differ from the view it came from ("modified"); Save
 * keeps it under that name, Save as under a new one, Reset goes back to the view as saved (or as
 * built, for a built-in view nobody saved over).
 */
class ViewBook(val screen: String, val builtIn: List<ViewState>, private val prefs: ViewPrefs) {
    private var saved: SavedViews = ViewJson.decode(prefs.read("views.$screen"))

    /** Built-in views (with the player's saved changes over them) and then the player's own, in order. */
    val all: List<ViewState>
        get() = builtIn.map { b -> saved.views.firstOrNull { it.name == b.name } ?: b } +
            saved.views.filter { s -> builtIn.none { it.name == s.name } }

    fun isBuiltIn(name: String) = builtIn.any { it.name == name }
    fun named(name: String?): ViewState? = all.firstOrNull { it.name == name }
    val activeName: String get() = saved.active?.takeIf { named(it) != null } ?: builtIn.first().name

    fun activate(name: String) { saved = saved.copy(active = name); persist() }

    fun save(state: ViewState) {
        saved = saved.copy(views = saved.views.filter { it.name != state.name } + state, active = state.name)
        persist()
    }

    /** Back to the built view: drops what the player saved over a built-in view. A player's own view keeps its saved shape. */
    fun restoreBuiltIn(name: String) {
        if (isBuiltIn(name)) { saved = saved.copy(views = saved.views.filter { it.name != name }); persist() }
    }

    fun delete(name: String) {
        if (isBuiltIn(name)) return
        saved = saved.copy(views = saved.views.filter { it.name != name }, active = saved.active.takeIf { it != name })
        persist()
    }

    private fun persist() = prefs.write("views.$screen", ViewJson.encode(saved))
}
