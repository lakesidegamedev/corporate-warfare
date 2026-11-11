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
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicTextField
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
import androidx.compose.ui.graphics.SolidColor
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
)

/** Body text for figures: the readable face with every digit the same width, so columns line up. */
val GridFigures = TextStyle(fontFeatureSettings = "tnum", fontSize = 12.sp)

/** Rows of a grid at desktop density. 24 dp rows put 30 and more on a 1080p page under the strips. */
val GridRowHeight: Dp = 24.dp

/**
 * The shared data grid for every table in the 2.0 interface: sortable columns (click a heading,
 * click again to reverse), a filter box, a header that stays put while the rows scroll, right
 * aligned tabular figures, inline bars and sparklines, a hover line with the row's breakdown,
 * click to select and double action (Enter or the row's open button) to open, Up and Down to move,
 * and a column chooser. Below [phoneBelow] wide it becomes two-line rows with the same sort and
 * filter: the first column and the sorted figure on top, the next figures underneath.
 */
@OptIn(ExperimentalLayoutApi::class)
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
    trailing: @Composable RowScope.() -> Unit = {},
) {
    var sortId by remember { mutableStateOf(initialSort) }
    var descending by remember { mutableStateOf(initialDescending) }
    var filter by remember { mutableStateOf("") }
    var choosing by remember { mutableStateOf(false) }
    val hidden = remember { mutableStateListOf<String>().apply { addAll(columns.filter { !it.shown }.map { it.id }) } }
    var hovered by remember { mutableStateOf<Any?>(null) }
    val visible = columns.filter { it.id !in hidden }
    val sortCol = columns.firstOrNull { it.id == sortId }
    val shown = remember(rows, filter, sortId, descending) {
        val needle = filter.trim().lowercase()
        val kept = if (needle.isEmpty()) rows else rows.filter { filterText(it).lowercase().contains(needle) }
        val sorted = when {
            sortCol == null -> kept
            sortCol.sort != null -> kept.sortedBy { sortCol.sort.invoke(it) }
            else -> kept.sortedBy { sortCol.text(it).lowercase() }
        }
        if (sortCol != null && descending) sorted.reversed() else sorted
    }
    fun pickSort(col: GridColumn<T>) {
        if (sortId == col.id) descending = !descending else { sortId = col.id; descending = col.numeric }
    }
    val list = rememberLazyListState()
    LaunchedEffect(selected, shown) {
        val at = shown.indexOfFirst { key(it) == selected }
        if (at >= 0 && (at < list.firstVisibleItemIndex || at > list.firstVisibleItemIndex + list.layoutInfo.visibleItemsInfo.size - 2)) list.scrollToItem((at - 3).coerceAtLeast(0))
    }
    Column(
        modifier.border(1.dp, Palette.stroke).background(Palette.panel)
            .onPreviewKeyEvent { e ->
                if (e.type != KeyEventType.KeyDown || shown.isEmpty()) return@onPreviewKeyEvent false
                val at = shown.indexOfFirst { key(it) == selected }
                when (e.key) {
                    Key.DirectionDown -> { onSelect(shown[(at + 1).coerceAtMost(shown.size - 1)]); true }
                    Key.DirectionUp -> { onSelect(shown[(at - 1).coerceAtLeast(0)]); true }
                    Key.Enter -> { if (at >= 0) onOpen?.invoke(shown[at]); at >= 0 && onOpen != null }
                    else -> false
                }
            }.focusable(),
    ) {
        // Title, count, filter and the column chooser.
        Row(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(title.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 11.sp, maxLines = 1)
            Spacer(Modifier.width(8.dp))
            Text(if (shown.size == rows.size) "${rows.size}" else "${shown.size} of ${rows.size}", color = Palette.secondary, style = GridFigures, fontSize = 11.sp)
            Spacer(Modifier.weight(1f))
            trailing()
            GridFilter(filter) { filter = it }
            Spacer(Modifier.width(6.dp))
            GridButton(if (choosing) "Done" else "Columns", choosing) { choosing = !choosing }
        }
        if (choosing) {
            FlowRow(Modifier.fillMaxWidth().background(Palette.ink).padding(6.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                columns.drop(1).forEach { c ->
                    GridButton(c.title, c.id !in hidden) { if (c.id in hidden) hidden.remove(c.id) else hidden.add(c.id) }
                }
            }
        }
        BoxWithConstraints(Modifier.fillMaxWidth().weight(1f)) {
            if (maxWidth < phoneBelow) {
                TwoLineRows(shown, visible, sortCol, descending, key, selected, onSelect, onOpen, { pickSort(it) })
                return@BoxWithConstraints
            }
            Column(Modifier.fillMaxSize()) {
                // The header stays put while the rows scroll under it.
                Row(Modifier.fillMaxWidth().height(22.dp).background(Palette.ink).padding(horizontal = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Spacer(Modifier.width(2.dp))
                    visible.forEach { c ->
                        val arrow = if (c.id == sortId) (if (descending) " v" else " ^") else ""
                        Explain(c.tip ?: Words.about(c.title), cellModifier(c)) {
                            Text(
                                c.title.uppercase() + arrow, color = if (c.id == sortId) Palette.gold else Palette.secondary,
                                fontSize = 10.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.4.sp, maxLines = 1, overflow = TextOverflow.Clip,
                                textAlign = if (c.numeric) TextAlign.End else TextAlign.Start,
                                modifier = Modifier.fillMaxWidth().pointerHoverIcon(PointerIcon.Hand).clickable { pickSort(c) }.padding(horizontal = 4.dp),
                            )
                        }
                    }
                    if (onOpen != null) Spacer(Modifier.width(18.dp))
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
                LazyColumn(Modifier.fillMaxWidth().weight(1f), state = list) {
                    itemsIndexed(shown, key = { _, r -> key(r) }) { i, r ->
                        GridRow(i, r, visible, key(r) == selected, onHover = { hovered = if (it) key(r) else if (hovered == key(r)) null else hovered }, onClick = { onSelect(r) }, onOpen = onOpen?.let { o -> { o(r) } })
                    }
                }
                // The breakdown of the row under the pointer, or of the selected row.
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

private fun <T> RowScope.cellModifier(c: GridColumn<T>): Modifier = if (c.width == null) Modifier.weight(1f) else Modifier.width(c.width)

@Composable
private fun <T> GridRow(index: Int, row: T, columns: List<GridColumn<T>>, selected: Boolean, onHover: (Boolean) -> Unit, onClick: () -> Unit, onOpen: (() -> Unit)?) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    LaunchedEffect(hovered) { onHover(hovered) }
    val base = if (index % 2 == 1) Palette.panelRaised.copy(alpha = 0.45f) else Color.Transparent
    Row(
        Modifier.fillMaxWidth().height(GridRowHeight)
            .background(if (selected) Palette.gold.copy(alpha = 0.18f) else if (hovered) Palette.panelRaised else base)
            .hoverable(hover).pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick).padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(2.dp).fillMaxHeight().background(if (selected) Palette.gold else Color.Transparent))
        columns.forEach { c -> GridCell(row, c) }
        if (onOpen != null) {
            Text(
                ">", color = if (hovered || selected) Palette.gold else Palette.stroke, fontFamily = Pixel.font, fontSize = 11.sp,
                modifier = Modifier.width(18.dp).clickable(onClick = onOpen).padding(start = 6.dp),
            )
        }
    }
}

@Composable
private fun <T> RowScope.GridCell(row: T, c: GridColumn<T>) {
    val m = (if (c.width == null) Modifier.weight(1f) else Modifier.width(c.width)).fillMaxHeight().padding(horizontal = 4.dp)
    val spark = c.spark
    if (spark != null) {
        Box(m.padding(vertical = 4.dp)) { GridSpark(spark(row), Modifier.fillMaxSize()) }
        return
    }
    Box(m, contentAlignment = if (c.numeric) Alignment.CenterEnd else Alignment.CenterStart) {
        c.bar?.let { bar ->
            val f = bar(row).coerceIn(0f, 1f)
            if (f > 0f) Box(Modifier.align(Alignment.CenterStart).fillMaxWidth(f).height(14.dp).background(Palette.gold.copy(alpha = 0.22f)))
        }
        Text(
            c.text(row), color = c.tone(row), style = GridFigures, fontWeight = if (c.bold) FontWeight.SemiBold else FontWeight.Normal,
            maxLines = 1, overflow = TextOverflow.Ellipsis, textAlign = if (c.numeric) TextAlign.End else TextAlign.Start,
        )
    }
}

/** A stepped column chart of a series: one flat column per value, gold when the series ends up, red when down. */
@Composable
fun GridSpark(values: List<Long>, modifier: Modifier) {
    if (values.size < 2) return
    val lo = values.min()
    val hi = values.max()
    val tone = if (values.last() >= values.first()) Palette.positive else Palette.negative
    Canvas(modifier) {
        val n = values.size
        val step = size.width / n
        val span = (hi - lo).coerceAtLeast(1).toFloat()
        values.forEachIndexed { i, v ->
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
            itemsIndexed(shown, key = { _, r -> key(r) }) { i, r ->
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

@Composable
private fun GridFilter(text: String, onText: (String) -> Unit) {
    BasicTextField(
        text, onText, singleLine = true,
        textStyle = TextStyle(color = Palette.primaryText, fontSize = 12.sp),
        cursorBrush = SolidColor(Palette.gold),
        modifier = Modifier.textEntry().width(150.dp),
        decorationBox = { inner ->
            Row(Modifier.background(Palette.ink).border(1.dp, Palette.stroke).padding(horizontal = 6.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                PixelIcon("search", size = 12.dp, tint = Palette.secondary)
                Spacer(Modifier.width(4.dp))
                Box(Modifier.weight(1f)) {
                    if (text.isEmpty()) Text("Filter", color = Palette.secondary, fontSize = 12.sp)
                    inner()
                }
            }
        },
    )
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
