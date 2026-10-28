package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.roundToInt

/** How a chart draws its series. */
enum class ChartKind(val title: String) { Line("Line"), Bar("Bar"), Area("Area") }

/** How far back a chart looks, in weeks; All is everything the engine kept. */
enum class ChartRange(val title: String, val weeks: Int?) {
    W4("4W", 4), W13("13W", 13), Y1("1Y", 52), Y5("5Y", 260), All("All", null)
}

/** The bucket each point stands for. */
enum class ChartPeriod(val title: String, val weeks: Int) { Weekly("Weekly", 1), Quarterly("Quarterly", 13), Yearly("Yearly", 52) }

/** One value at a game week: the first week of the period it covers. */
data class ChartPoint(val week: Int, val value: Long)

/** A named series drawn in one colour. */
data class ChartSeries(val name: String, val color: Color, val points: List<ChartPoint>)

/** Game dates for axes and read-outs. The game year is exactly 52 weeks, a quarter 13. */
object GameDates {
    fun year(week: Int): Int = ResultBuckets.startYear + (max(week, 1) - 1) / 52
    fun quarter(week: Int): Int = (max(week, 1) - 1) % 52 / 13 + 1

    fun label(week: Int, period: ChartPeriod): String = when (period) {
        ChartPeriod.Yearly -> "${year(week)}"
        ChartPeriod.Quarterly -> "${year(week)} Q${quarter(week)}"
        ChartPeriod.Weekly -> "W%02d %d".format((max(week, 1) - 1) % 52 + 1, year(week))
    }

    /** Fold weekly closes into quarters or years, keeping each bucket's last close. */
    fun lastClose(points: List<ChartPoint>, period: ChartPeriod): List<ChartPoint> {
        if (period == ChartPeriod.Weekly) return points
        return points.groupBy { (max(it.week, 1) - 1) / period.weeks }.toSortedMap().map { (bucket, p) ->
            ChartPoint(bucket * period.weeks + 1, p.last().value)
        }
    }

    /** Keep the points inside a range, counted back from the latest week of any series. */
    fun within(series: List<ChartSeries>, range: ChartRange): List<ChartSeries> {
        val weeks = range.weeks ?: return series
        val latest = series.flatMap { it.points }.maxOfOrNull { it.week } ?: return series
        return series.map { s -> s.copy(points = s.points.filter { it.week > latest - weeks }) }
    }
}

/** Small option chips in the pixel font, the selected one gold. */
@Composable
fun ChartChips(options: List<String>, selected: String, onPick: (String) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
        options.forEach { option ->
            Text(
                option, color = if (option == selected) Palette.ink else Palette.primaryText, fontSize = 10.sp, fontFamily = Pixel.font,
                maxLines = 1, softWrap = false,
                modifier = Modifier.clip(Pixel.small).background(if (option == selected) Palette.gold else Palette.panelRaised)
                    .clickable { onPick(option) }.padding(horizontal = 7.dp, vertical = 3.dp),
            )
        }
    }
}

/**
 * The shared chart: line, bar or area, overlay series with a legend, date range chips, period
 * chips where the caller can supply quarters or years, and a read-out of the value under the
 * pointer (hover on desktop, tap on a phone). Hard edges throughout: stepped lines, square bars,
 * solid fills mixed toward the panel colour rather than translucent, labels in the pixel font.
 *
 * [period] and [onPeriod] show the period chips; [periods] limits them. Points must be in week
 * order. [format] writes a value for the axis and the read-out.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun Chart(
    series: List<ChartSeries>,
    modifier: Modifier = Modifier,
    title: String? = null,
    kinds: List<ChartKind> = ChartKind.entries,
    initialKind: ChartKind = kinds.first(),
    initialRange: ChartRange = ChartRange.All,
    ranges: List<ChartRange> = ChartRange.entries,
    period: ChartPeriod = ChartPeriod.Weekly,
    periods: List<ChartPeriod> = emptyList(),
    onPeriod: ((ChartPeriod) -> Unit)? = null,
    height: Dp = 170.dp,
    format: (Long) -> String = { Format.compact(it) },
) {
    var kind by remember(kinds) { mutableStateOf(initialKind) }
    var range by remember { mutableStateOf(initialRange) }
    var pointer by remember { mutableStateOf<Float?>(null) }
    val shown = GameDates.within(series, range).filter { it.points.isNotEmpty() }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(5.dp)) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            title?.let { Eyebrow(it) }
            if (ranges.size > 1) ChartChips(ranges.map { it.title }, range.title) { t -> range = ranges.first { it.title == t } }
            if (onPeriod != null && periods.size > 1) ChartChips(periods.map { it.title }, period.title) { t -> onPeriod(periods.first { it.title == t }) }
            if (kinds.size > 1) ChartChips(kinds.map { it.title }, kind.title) { t -> kind = kinds.first { it.title == t } }
        }
        val weeks = shown.flatMap { s -> s.points.map { it.week } }.distinct().sorted()
        if (weeks.isEmpty()) {
            Text("No history in this range yet.", color = Palette.secondary, fontSize = 11.sp, modifier = Modifier.padding(vertical = 12.dp))
            return@Column
        }
        // The read-out: the week nearest the pointer, else the latest.
        val readWeek = pointer?.let { f -> weeks[(f * (weeks.size - 1)).roundToInt().coerceIn(0, weeks.size - 1)] } ?: weeks.last()
        FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(GameDates.label(readWeek, period), color = Palette.gold, fontSize = 10.sp, fontFamily = Pixel.font)
            shown.forEach { s ->
                val v = s.points.lastOrNull { it.week <= readWeek }?.value
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Legend(s.color, s.name)
                    Text(v?.let(format) ?: "n/a", color = Palette.primaryText, fontSize = 11.sp)
                }
            }
        }
        val measurer = rememberTextMeasurer()
        val labelStyle = TextStyle(color = Palette.secondary, fontSize = 8.sp, fontFamily = Pixel.font)
        Canvas(
            Modifier.fillMaxWidth().height(height).pointerInput(weeks.size) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        val x = event.changes.firstOrNull()?.position?.x ?: continue
                        pointer = if (event.type == PointerEventType.Exit) null else (x / size.width).coerceIn(0f, 1f)
                    }
                }
            },
        ) {
            val px = 2.dp.toPx()
            fun snap(v: Float) = floor(v / px) * px
            val axisRoom = 14.dp.toPx()
            val plotH = size.height - axisRoom
            val values = shown.flatMap { s -> s.points.map { it.value } }
            val top = max(values.max(), 0L)
            val bottom = minOf(values.min(), 0L)
            val span = max(top - bottom, 1L).toFloat()
            fun y(v: Long) = snap(plotH * (top - v) / span)
            val zero = y(0)
            val first = weeks.first()
            val last = weeks.last()
            val slot = size.width / weeks.size
            fun x(week: Int): Float = when {
                kind == ChartKind.Bar -> snap(weeks.indexOf(week).coerceAtLeast(0) * slot)
                last == first -> size.width / 2
                else -> snap((week - first).toFloat() / (last - first) * (size.width - px))
            }
            // Grid: zero, top and the middle, labelled at the left.
            listOf(top, (top + bottom) / 2, bottom).distinct().forEach { v ->
                drawRect(if (v == 0L) Palette.stroke else lerp(Palette.panel, Palette.stroke, 0.5f), Offset(0f, y(v)), Size(size.width, px / 2))
                val text = measurer.measure(format(v), labelStyle)
                drawText(text, topLeft = Offset(0f, (y(v) - text.size.height).coerceAtLeast(0f)))
            }
            if (bottom < 0) drawRect(Palette.stroke, Offset(0f, zero), Size(size.width, px))
            when (kind) {
                ChartKind.Bar -> {
                    val bar = max(snap(slot * 0.8f / shown.size), px)
                    shown.forEachIndexed { i, s ->
                        s.points.forEach { p ->
                            val bx = x(p.week) + snap(slot * 0.1f) + i * bar
                            val by = y(p.value)
                            drawRect(s.color, Offset(bx, minOf(by, zero)), Size(max(bar - px / 2, px), max(abs(zero - by), px)))
                        }
                    }
                }
                ChartKind.Line, ChartKind.Area -> shown.forEach { s ->
                    // A stepped path: across at each value, then straight down or up to the next.
                    val path = Path()
                    s.points.forEachIndexed { i, p ->
                        val px0 = x(p.week)
                        if (i == 0) path.moveTo(px0, y(p.value)) else {
                            path.lineTo(px0, y(s.points[i - 1].value)); path.lineTo(px0, y(p.value))
                        }
                    }
                    val endX = if (s.points.size == 1) x(s.points.last().week) + slot else x(s.points.last().week)
                    path.lineTo(endX, y(s.points.last().value))
                    if (kind == ChartKind.Area) {
                        val fill = Path().apply {
                            addPath(path)
                            lineTo(endX, zero); lineTo(x(s.points.first().week), zero); close()
                        }
                        drawPath(fill, lerp(Palette.panel, s.color, 0.35f))
                    }
                    drawPath(path, s.color, style = Stroke(px))
                }
            }
            // The pointer's week, as a hard vertical rule.
            pointer?.let { drawRect(Palette.gold, Offset(snap(x(readWeek) + if (kind == ChartKind.Bar) slot / 2 else 0f), 0f), Size(px / 2, plotH)) }
            // Axis dates: up to five, spread evenly.
            val ticks = minOf(5, weeks.size)
            val used = mutableListOf<Float>()
            for (t in 0 until ticks) {
                val week = weeks[if (ticks == 1) 0 else t * (weeks.size - 1) / (ticks - 1)]
                val text = measurer.measure(GameDates.label(week, if (period == ChartPeriod.Weekly && last - first > 26) ChartPeriod.Quarterly else period), labelStyle)
                val tx = (x(week) - text.size.width / 2f).coerceIn(0f, size.width - text.size.width)
                if (used.any { abs(it - tx) < text.size.width + 4 }) continue
                used += tx
                drawText(text, topLeft = Offset(tx, size.height - text.size.height))
            }
        }
    }
}

/** A share's weekly closes for up to ten years against the corridor index, both 100 at the first close shown. */
@Composable
fun StockHistoryChart(store: GameStore, firm: ULong, height: Dp = 150.dp) {
    val revision = store.overview?.revision
    val data by androidx.compose.runtime.produceState<Pair<List<ChartPoint>, List<ChartPoint>>?>(null, firm, revision) {
        value = store.read { h ->
            h.sharePriceHistory(firm).map { ChartPoint(it.week.toInt(), it.cents) } to h.corridorIndexHistory().map { ChartPoint(it.week.toInt(), it.cents) }
        }
    }
    val (share, index) = data ?: return
    if (share.size < 2) return
    var period by remember { mutableStateOf(ChartPeriod.Weekly) }
    val rebase = { points: List<ChartPoint> -> points.firstOrNull()?.let { b -> points.map { ChartPoint(it.week, it.value * 10_000 / max(b.value, 1)) } } ?: emptyList() }
    Chart(
        listOf(
            ChartSeries("Share", Palette.gold, GameDates.lastClose(rebase(share), period)),
            ChartSeries("Corridor index", Palette.secondary, GameDates.lastClose(rebase(index.filter { it.week >= share.first().week }), period)),
        ),
        title = "Price history (100 at start)", initialRange = ChartRange.Y1, period = period, periods = ChartPeriod.entries, onPeriod = { period = it },
        height = height, format = { "%.1f".format(it / 100.0) },
    )
}

/** A good's weekly corridor price for up to ten years against its reference price. */
@Composable
fun GoodHistoryChart(store: GameStore, product: UInt, referenceCents: Long, height: Dp = 150.dp) {
    val revision = store.overview?.revision
    val data by androidx.compose.runtime.produceState<List<ChartPoint>?>(null, product, revision) {
        value = store.read { h -> h.goodPriceHistory(product).map { ChartPoint(it.week.toInt(), it.cents) } }
    }
    val price = data ?: return
    if (price.size < 2) return
    var period by remember { mutableStateOf(ChartPeriod.Weekly) }
    Chart(
        listOf(
            ChartSeries("Price", Palette.gold, GameDates.lastClose(price, period)),
            ChartSeries("Reference", Palette.secondary, GameDates.lastClose(price.map { ChartPoint(it.week, referenceCents) }, period)),
        ),
        title = "Price per lot", initialRange = ChartRange.Y1, period = period, periods = ChartPeriod.entries, onPeriod = { period = it },
        height = height, format = { Format.money(it) },
    )
}
