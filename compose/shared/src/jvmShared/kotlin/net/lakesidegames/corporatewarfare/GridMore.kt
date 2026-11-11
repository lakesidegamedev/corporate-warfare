package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Drill-down, side panes and "chart these" for the 2.0 grids. A figure with history opens its
 * breakdown over its own period; Ctrl-click (or a long press) sends it to the side pane, where a
 * pinned pane stays so two things can be compared one above the other.
 */

/** One figure's history to drill into: oldest first, [step] weeks a point, read with [fmt]. */
class DrillItem(
    val title: String,
    val series: List<Long>,
    val step: Int,
    val fmt: (Long) -> String,
    val long: List<Long>? = null,
    val longStep: Int = 13,
    val pinned: Boolean = false,
)

/** The side pane's contents: at most two drill-downs; a new one replaces the one not pinned. */
object SidePanes {
    val items = mutableStateListOf<DrillItem>()

    fun open(item: DrillItem) {
        val free = items.indexOfFirst { !it.pinned }
        if (free >= 0) items[free] = item else if (items.size < 2) items.add(item) else items[items.size - 1] = item
    }

    fun pin(item: DrillItem) {
        val at = items.indexOf(item)
        if (at >= 0) items[at] = DrillItem(item.title, item.series, item.step, item.fmt, item.long, item.longStep, !item.pinned)
    }

    fun close(item: DrillItem) { items.remove(item) }
}

/** Colours for the lines of a multi-series chart, from the game's palette, in a fixed order. */
internal val SeriesColors: List<Color> get() = listOf(Palette.gold, Palette.positive, Palette.negative, Color(0xFF6EC6FF), Color(0xFFC792EA), Palette.warning, Palette.primaryText, Palette.secondary)

/** Calls [action] when the secondary (right) button goes down on this element. */
internal fun Modifier.onSecondaryPress(action: (() -> Unit)?): Modifier = if (action == null) this else composedSecondary(action)

private fun Modifier.composedSecondary(action: () -> Unit): Modifier = composed {
    val current by rememberUpdatedState(action)
    pointerInput(Unit) {
        awaitPointerEventScope {
            while (true) {
                val e = awaitPointerEvent(PointerEventPass.Initial)
                if (e.type == PointerEventType.Press && e.buttons.isSecondaryPressed) current()
            }
        }
    }
}

/** A figure's breakdown: its line over its own period, now, start, change, high and low. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DrillView(item: DrillItem, start: Timeframe, modifier: Modifier = Modifier) {
    var tf by remember(item) { mutableStateOf(start) }
    val w = windowOf(item.series, item.step, item.long, item.longStep, tf)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        // Its own period: changing it here leaves the grid's timeframe alone.
        FlowRow(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            Timeframe.entries.forEach { t -> GridButton(t.label, tf == t, 9.sp) { tf = t } }
        }
        if (w == null || w.size < 2) {
            Text("The game has no record of this that far back. Pick a shorter period.", color = Palette.secondary, fontSize = 11.sp)
            return@Column
        }
        StepChart(listOf(w), listOf(Palette.gold), Modifier.fillMaxWidth().height(120.dp))
        val bps = windowBps(w)
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            DrillFigure("Now", item.fmt(w.last()))
            DrillFigure("Start", item.fmt(w.first()))
            DrillFigure("Change", (if (windowChange(w) > 0) "+" else "") + item.fmt(windowChange(w)), tone = changeTint(windowChange(w)))
            DrillFigure("Change %", bps?.let(::changeText) ?: DASH, tone = bps?.let(::changeTint) ?: Palette.secondary)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            DrillFigure("High", item.fmt(w.max()))
            DrillFigure("Low", item.fmt(w.min()))
            DrillFigure("Average", item.fmt(w.sum() / w.size))
            DrillFigure("Points", "${w.size}, ${if (item.series.size >= w.size || item.long == null) item.step else item.longStep} wk each")
        }
    }
}

@Composable
private fun DrillFigure(label: String, value: String, tone: Color = Palette.primaryText) {
    Column {
        Text(label.uppercase(), color = Palette.secondary, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
        Text(value, color = tone, style = GridFigures, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

/**
 * Stepped lines, one per series, on one scale: each value holds flat until the next, joined by a
 * hard vertical step, two pixels thick. No smoothing and no antialiasing.
 */
@Composable
fun StepChart(series: List<List<Long>>, colors: List<Color>, modifier: Modifier) {
    val all = series.flatten()
    if (all.isEmpty()) return
    val lo = all.min()
    val hi = all.max()
    Canvas(modifier.background(Palette.ink).border(1.dp, Palette.stroke)) {
        val pad = 4f
        val span = (hi - lo).coerceAtLeast(1).toFloat()
        val h = size.height - 2 * pad
        val wTotal = size.width - 2 * pad
        // Guide lines at the top, middle and bottom, one pixel, in the frame colour.
        listOf(0f, 0.5f, 1f).forEach { f -> drawRect(Palette.stroke, Offset(pad, kotlin.math.floor(pad + h * f)), Size(wTotal, 1f)) }
        series.forEachIndexed { si, pts ->
            if (pts.size < 2) return@forEachIndexed
            val color = colors[si % colors.size]
            val step = wTotal / (pts.size - 1)
            fun y(v: Long) = kotlin.math.floor(pad + h - h * ((v - lo) / span))
            for (i in 0 until pts.size - 1) {
                val x0 = kotlin.math.floor(pad + i * step)
                val x1 = kotlin.math.floor(pad + (i + 1) * step)
                val y0 = y(pts[i])
                val y1 = y(pts[i + 1])
                drawRect(color, Offset(x0, y0 - 1f), Size((x1 - x0).coerceAtLeast(1f), 2f))
                drawRect(color, Offset(x1 - 1f, minOf(y0, y1) - 1f), Size(2f, kotlin.math.abs(y1 - y0) + 2f))
            }
        }
    }
}

/** The side pane beside the screen: up to two drill-downs, each with pin and close. */
@Composable
fun SidePaneColumn(modifier: Modifier) {
    Column(modifier.background(Palette.panel).verticalScroll(rememberScrollState()).padding(6.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SidePanes.items.toList().forEach { item ->
            Column(Modifier.fillMaxWidth().border(1.dp, if (item.pinned) Palette.gold else Palette.stroke).padding(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(item.title.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 10.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    GridButton(if (item.pinned) "Pinned" else "Pin", item.pinned, 9.sp) { SidePanes.pin(item) }
                    Spacer(Modifier.width(3.dp))
                    GridButton("Close", false, 9.sp) { SidePanes.close(item) }
                }
                DrillView(item, GridPrefs.timeframe)
            }
        }
        Text("Pin a pane to keep it while you open another beside it.", color = Palette.secondary, fontSize = 10.sp)
    }
}

/**
 * "Chart these": the picked rows of a grid as lines over the timeframe, for any column the game
 * keeps history of. Its own period buttons start at the grid's.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun <T> ChartThese(rows: List<T>, name: (T) -> String, columns: List<GridColumn<T>>, start: String?, tf0: Timeframe, onClose: () -> Unit, modifier: Modifier) {
    val charted = columns.filter { it.series != null }
    var col by remember { mutableStateOf(charted.firstOrNull { it.id == start }?.id ?: charted.firstOrNull()?.id) }
    var tf by remember { mutableStateOf(tf0) }
    val c = charted.firstOrNull { it.id == col }
    Column(modifier.background(Palette.panel).border(2.dp, Palette.gold).padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("CHART: ${rows.size} ROWS", color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp, modifier = Modifier.weight(1f))
            GridButton("Close", false, 9.sp, onClose)
        }
        if (c == null) {
            Text("None of these columns has a history the game keeps.", color = Palette.secondary, fontSize = 11.sp)
            return@Column
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(3.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            charted.forEach { k -> GridButton(k.title, k.id == col, 9.sp) { col = k.id } }
            Spacer(Modifier.width(10.dp))
            Timeframe.entries.forEach { t -> GridButton(t.label, tf == t, 9.sp) { tf = t } }
        }
        val lines = rows.map { r -> r to windowOf(c.series?.invoke(r), c.step, c.longSeries?.invoke(r), c.longStep, tf) }
        val drawn = lines.filter { (it.second?.size ?: 0) >= 2 }
        if (drawn.isEmpty()) {
            Text("No row has a record of ${c.title.lowercase()} that far back. Pick a shorter period.", color = Palette.secondary, fontSize = 11.sp)
            return@Column
        }
        StepChart(drawn.map { it.second!! }, SeriesColors, Modifier.fillMaxWidth().height(220.dp))
        lines.forEachIndexed { i, (r, w) ->
            val at = drawn.indexOfFirst { it.first == r }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.width(10.dp).height(10.dp).background(if (at >= 0) SeriesColors[at % SeriesColors.size] else Palette.stroke))
                Spacer(Modifier.width(6.dp))
                Text(name(r), color = Palette.primaryText, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                Text(w?.let { c.fmt?.invoke(it.last()) ?: Format.count(it.last()) } ?: DASH, color = Palette.primaryText, style = GridFigures, fontSize = 11.sp)
                Spacer(Modifier.width(8.dp))
                val bps = w?.let(::windowBps)
                Text(bps?.let(::changeText) ?: DASH, color = bps?.let(::changeTint) ?: Palette.secondary, style = GridFigures, fontSize = 11.sp, modifier = Modifier.width(60.dp))
            }
            if (i >= 11) return@forEachIndexed
        }
    }
}

/** A drill-down over the grid: the figure's breakdown with its own period, and a way to send it beside. */
@Composable
internal fun DrillOverlay(item: DrillItem, tf: Timeframe, onClose: () -> Unit, modifier: Modifier) {
    Column(modifier.background(Palette.panel).border(2.dp, Palette.gold).padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(item.title.uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            GridButton("Beside", false, 9.sp) { SidePanes.open(item); onClose() }
            Spacer(Modifier.width(3.dp))
            GridButton("Close", false, 9.sp, onClose)
        }
        DrillView(item, tf)
        Text("Ctrl-click a figure, or hold it, to open it beside the screen.", color = Palette.secondary, fontSize = 10.sp)
    }
}

/** A link-like line for small lists: the whole row is the target. */
@Composable
internal fun FindLine(kind: String, text: String, hint: String, on: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().height(22.dp).background(if (on) Palette.gold.copy(alpha = 0.18f) else Color.Transparent)
            .pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(kind.uppercase(), color = Palette.secondary, fontFamily = Pixel.font, fontSize = 8.sp, modifier = Modifier.width(70.dp), maxLines = 1)
        Text(text, color = if (on) Palette.gold else Palette.primaryText, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        Text(hint, color = Palette.secondary, fontSize = 10.sp, maxLines = 1)
    }
}
