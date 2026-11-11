package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties

/**
 * The one control bar every grid carries, always in this order: View, Scope, Timeframe, Search,
 * Filter, Columns and more. Calm by default: each control is one small button, and what it holds
 * opens in a menu only when asked. On a phone it folds to the View button and the search.
 */
@Composable
internal fun <T> ControlBar(
    title: String, total: Int, shownCount: Int, view: ViewState, modified: Boolean, book: ViewBook, views: GridViews<T>,
    columns: List<GridColumn<T>>, tf: Timeframe, search: String, menu: String?, phone: Boolean, dropped: Int,
    onView: (ViewState) -> Unit, onSearch: (String) -> Unit, onTyping: (Boolean) -> Unit, onMenu: (String?) -> Unit,
    onCommitSearch: () -> Unit, csv: () -> String, distinct: (String) -> List<String>, onExtra: (String) -> Unit,
) {
    val prefs = LocalAppSettings.current
    fun pick(v: ViewState) { onView(v); book.activate(v.name); onMenu(null) }
    Row(
        Modifier.fillMaxWidth().height(30.dp).background(Palette.panelRaised).padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (!phone) {
            Text(title.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 11.sp, maxLines = 1)
            Text(if (shownCount == total) "$total" else "$shownCount of $total", color = Palette.secondary, style = GridFigures, fontSize = 11.sp, maxLines = 1, modifier = Modifier.padding(end = 4.dp))
        }
        // View
        Anchor {
            BarButton("View: " + view.name + if (modified) " *" else "", menu == "view", wide = true) { onMenu(if (menu == "view") null else "view") }
            if (menu == "view") PixelMenu({ onMenu(null) }, 280.dp) {
                ViewMenu(view, modified, book, ::pick, onView, onMenu, phone)
                if (phone) {
                    MenuRule()
                    TimeframeMenu(view, tf, onView)
                    MenuRule()
                    ScopeMenu(view, views, onView)
                }
            }
        }
        if (!phone) {
            // Scope
            Anchor {
                val label = when {
                    view.scope.isEmpty() -> "Everything"
                    view.scope.size == 1 -> scopeLabel(view.scope.first(), views)
                    else -> "${view.scope.size} picked"
                }
                BarButton("Scope: $label", menu == "scope" || view.scope.isNotEmpty(), wide = true) { onMenu(if (menu == "scope") null else "scope") }
                if (menu == "scope") PixelMenu({ onMenu(null) }, 260.dp) { ScopeMenu(view, views, onView) }
            }
            // Timeframe: follows the game's unless this grid overrides it.
            Anchor {
                BarButton(tf.label, menu == "time" || view.timeframe != null) { onMenu(if (menu == "time") null else "time") }
                if (menu == "time") PixelMenu({ onMenu(null) }, 220.dp) { TimeframeMenu(view, tf, onView) }
            }
        }
        GridSearch(search, if (phone) "Search" else "Search, or type sector:energy", 240.dp, onTyping, onSearch, onCommitSearch, Modifier.weight(1f, fill = false))
        if (!phone) {
            Anchor {
                BarButton("+ Filter", menu?.startsWith("filter") == true) { onMenu(if (menu?.startsWith("filter") == true) null else "filter") }
                if (menu?.startsWith("filter") == true) PixelMenu({ onMenu(null) }, 260.dp) { FilterMenu(view, columns, menu, onView, onMenu, distinct) }
            }
            Anchor {
                BarButton("Columns" + if (dropped > 0) " +$dropped" else "", menu == "columns") { onMenu(if (menu == "columns") null else "columns") }
                if (menu == "columns") PixelMenu({ onMenu(null) }, 330.dp) { ColumnsMenu(view, columns, onView, onExtra) }
            }
            Anchor {
                BarButton("...", menu == "more") { onMenu(if (menu == "more") null else "more") }
                if (menu == "more") PixelMenu({ onMenu(null) }, 240.dp, alignEnd = true) {
                    val clip = LocalClipboardManager.current
                    MenuHeading("Group rows by")
                    MenuItem("No groups", on = view.groupBy == null) { onView(view.copy(groupBy = null)) }
                    columns.drop(1).filter { !it.numeric }.forEach { c -> MenuItem(c.title, on = view.groupBy == c.id) { onView(view.copy(groupBy = c.id)) } }
                    MenuRule()
                    MenuHeading("Rows")
                    MenuItem("Compact", on = GridPrefs.compact) { GridPrefs.chooseCompact(true, prefs) }
                    MenuItem("Comfortable", on = !GridPrefs.compact) { GridPrefs.chooseCompact(false, prefs) }
                    MenuRule()
                    MenuItem("Copy the grid for a spreadsheet", hint = "CSV") { clip.setText(AnnotatedString(csv())); onMenu(null) }
                    if (modified) MenuItem("Reset view") { book.named(view.name)?.let(onView); onMenu(null) }
                }
            }
        }
    }
}

/** A box a menu opens under. */
@Composable
private fun Anchor(content: @Composable () -> Unit) { Box { content() } }

@Composable
private fun BarButton(text: String, on: Boolean, wide: Boolean = false, onClick: () -> Unit) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Text(
        text.uppercase() + " v".takeIf { wide }.orEmpty(), color = if (on) Palette.gold else Palette.primaryText, fontFamily = Pixel.font, fontSize = 10.sp,
        maxLines = 1, overflow = TextOverflow.Ellipsis, softWrap = false,
        modifier = Modifier.widthIn(max = if (wide) 190.dp else 140.dp).background(if (hovered) Palette.panel else Palette.ink)
            .border(1.dp, if (on) Palette.gold else Palette.stroke).hoverable(hover).pointerHoverIcon(PointerIcon.Hand)
            .clickable(onClick = onClick).padding(horizontal = 7.dp, vertical = 4.dp),
    )
}

private fun <T> scopeLabel(id: String, views: GridViews<T>): String = if (id == "mine") "Mine" else views.scopes.firstOrNull { it.id == id }?.label ?: id

/** The search box. Enter turns typed terms such as sector:chemicals or margin>10 into chips; plain words stay and filter as you type. */
@Composable
internal fun GridSearch(text: String, hint: String, width: Dp, onTyping: (Boolean) -> Unit, onText: (String) -> Unit, onCommit: () -> Unit, modifier: Modifier = Modifier) {
    BasicTextField(
        text, onText, singleLine = true,
        textStyle = TextStyle(color = Palette.primaryText, fontSize = 12.sp),
        cursorBrush = SolidColor(Palette.gold),
        modifier = modifier.widthIn(min = 90.dp, max = width).textEntry()
            .onPreviewKeyEvent { e -> if (e.type == KeyEventType.KeyDown && e.key == Key.Enter) { onCommit(); true } else false },
        decorationBox = { inner ->
            Row(Modifier.background(Palette.ink).border(1.dp, Palette.stroke).padding(horizontal = 6.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
                PixelIcon("search", size = 12.dp, tint = Palette.secondary)
                Spacer(Modifier.width(4.dp))
                Box(Modifier.weight(1f)) {
                    if (text.isEmpty()) Text(hint, color = Palette.secondary, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    inner()
                }
            }
        },
    )
}

/** A menu in a hard-edged pixel frame under its button. */
@Composable
internal fun PixelMenu(onDismiss: () -> Unit, width: Dp, alignEnd: Boolean = false, content: @Composable ColumnScope.() -> Unit) {
    val y = with(LocalDensity.current) { 26.dp.roundToPx() }
    Popup(alignment = if (alignEnd) Alignment.TopEnd else Alignment.TopStart, offset = IntOffset(0, y), onDismissRequest = onDismiss, properties = PopupProperties(focusable = true)) {
        // Room for the hard shadow, which falls down and to the right of the frame.
        Box(Modifier.padding(end = 4.dp, bottom = 4.dp)) {
            Column(
                Modifier.width(width).pixelFrame(Palette.panel, Palette.gold.copy(alpha = 0.7f)).padding(horizontal = 3.dp, vertical = 6.dp)
                    .heightIn(max = 560.dp).verticalScroll(rememberScrollState()),
                content = content,
            )
        }
    }
}

@Composable
internal fun MenuHeading(text: String, hint: String = "") {
    Row(Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, top = 4.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text.uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 9.sp, maxLines = 1, modifier = Modifier.weight(1f))
        if (hint.isNotEmpty()) Text(hint, color = Palette.secondary, fontSize = 10.sp, maxLines = 1)
    }
}

@Composable
internal fun MenuRule() { Box(Modifier.fillMaxWidth().padding(vertical = 4.dp).height(1.dp).background(Palette.stroke)) }

/**
 * One line of a menu: an optional tick box (which combines instead of replacing), an optional
 * colour square, the words, and a hint or a second action on the right.
 */
@Composable
internal fun MenuItem(
    text: String, on: Boolean = false, hint: String = "", color: Color? = null, enabled: Boolean = true,
    box: Boolean? = null, onBox: () -> Unit = {}, action: String? = null, onAction: () -> Unit = {}, onClick: () -> Unit,
) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Row(
        Modifier.fillMaxWidth().height(22.dp).background(if (hovered && enabled) Palette.panelRaised else Color.Transparent).hoverable(hover)
            .pointerHoverIcon(PointerIcon.Hand).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (box != null) {
            Box(
                Modifier.width(11.dp).height(11.dp).border(1.dp, if (box) Palette.gold else Palette.secondary).background(if (box) Palette.gold else Color.Transparent)
                    .clickable(onClick = onBox),
            )
            Spacer(Modifier.width(7.dp))
        }
        // A tick box already marks what is on; the bar marks the current choice in a plain list.
        if (box == null) { Box(Modifier.width(3.dp).height(12.dp).background(if (on) Palette.gold else Color.Transparent)); Spacer(Modifier.width(5.dp)) }
        if (color != null) { Box(Modifier.width(9.dp).height(9.dp).background(color)); Spacer(Modifier.width(6.dp)) }
        Text(
            text, color = if (!enabled) Palette.secondary.copy(alpha = 0.5f) else if (on) Palette.gold else Palette.primaryText, fontSize = 12.sp,
            maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f),
        )
        if (action != null) Text(action.uppercase(), color = Palette.secondary, fontFamily = Pixel.font, fontSize = 9.sp, modifier = Modifier.border(1.dp, Palette.stroke).clickable(onClick = onAction).padding(horizontal = 4.dp, vertical = 1.dp))
        else if (hint.isNotEmpty()) Text(hint, color = Palette.secondary, style = GridFigures, fontSize = 11.sp, maxLines = 1)
    }
}

@Composable
private fun ViewMenu(view: ViewState, modified: Boolean, book: ViewBook, pick: (ViewState) -> Unit, onView: (ViewState) -> Unit, onMenu: (String?) -> Unit, phone: Boolean) {
    val clip = LocalClipboardManager.current
    var naming by remember { mutableStateOf(false) }
    var name by remember { mutableStateOf("") }
    MenuHeading("Views", if (phone) "" else "keys 1 to 9")
    book.all.forEachIndexed { i, v ->
        val active = v.name == view.name
        MenuItem(
            v.name, on = active, hint = if (active && modified) "changed" else if (i < 9) "${i + 1}" else "",
            action = if (!book.isBuiltIn(v.name)) "delete" else null, onAction = { book.delete(v.name); if (active) book.named(book.activeName)?.let(onView) },
        ) { pick(v) }
    }
    MenuRule()
    // Only what can be done now: Save and Reset appear once the view has changed.
    if (modified) MenuItem("Save \"${view.name}\"") { book.save(view); onView(view); onMenu(null) }
    if (!naming) MenuItem("Save as a new view") { naming = true }
    else Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        GridSearch(name, "Name", 170.dp, {}, { name = it }, {
            if (name.isNotBlank()) { val v = view.copy(name = name.trim()); book.save(v); onView(v); onMenu(null) }
        })
        Spacer(Modifier.width(6.dp))
        GridButton("Save", false) { if (name.isNotBlank()) { val v = view.copy(name = name.trim()); book.save(v); onView(v); onMenu(null) } }
    }
    if (modified) MenuItem("Reset view") { book.named(view.name)?.let(onView); onMenu(null) }
    if (book.isBuiltIn(view.name)) {
        val built = book.builtIn.first { it.name == view.name }
        if (!view.sameShape(built)) MenuItem("Back to the view as it came") { book.restoreBuiltIn(view.name); onView(built); onMenu(null) }
    }
    MenuRule()
    MenuItem("Copy this view to share") { clip.setText(AnnotatedString(ViewJson.exportView(view))); onMenu(null) }
    MenuItem("Add a copied view") {
        clip.getText()?.text?.let(ViewJson::importView)?.let { v ->
            val unique = generateSequence(v.name) { "$it 2" }.first { book.named(it) == null }
            val mine = v.copy(name = unique)
            book.save(mine); onView(mine)
        }
        onMenu(null)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TimeframeMenu(view: ViewState, tf: Timeframe, onView: (ViewState) -> Unit) {
    MenuHeading("Timeframe", "changes, trends, bars")
    MenuItem("Follow the game: ${GridPrefs.timeframe.label}", on = view.timeframe == null) { onView(view.copy(timeframe = null)) }
    Text("Or keep this grid at:", color = Palette.secondary, fontSize = 11.sp, modifier = Modifier.padding(horizontal = 16.dp, vertical = 2.dp))
    FlowRow(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, bottom = 4.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
        Timeframe.entries.forEach { t -> GridButton(t.label, view.timeframe == t, 10.sp) { onView(view.copy(timeframe = t)) } }
    }
}

@Composable
private fun <T> ScopeMenu(view: ViewState, views: GridViews<T>, onView: (ViewState) -> Unit) {
    fun only(id: String?) = onView(view.copy(scope = listOfNotNull(id)))
    fun toggle(id: String) = onView(view.copy(scope = if (id in view.scope) view.scope - id else view.scope + id))
    MenuHeading("Scope", "tick to combine")
    MenuItem("Everything", on = view.scope.isEmpty()) { only(null) }
    MenuItem("Mine", on = view.scope == listOf("mine"), box = "mine" in view.scope, onBox = { toggle("mine") }) { only("mine") }
    views.scopes.groupBy { it.kind }.forEach { (kind, options) ->
        MenuHeading(kind)
        options.forEach { o -> MenuItem(o.label, on = view.scope == listOf(o.id), color = o.color, box = o.id in view.scope, onBox = { toggle(o.id) }) { only(o.id) } }
    }
}

@Composable
private fun <T> FilterMenu(view: ViewState, columns: List<GridColumn<T>>, menu: String, onView: (ViewState) -> Unit, onMenu: (String?) -> Unit, distinct: (String) -> List<String>) {
    val id = menu.removePrefix("filter").removePrefix(":")
    val c = columns.firstOrNull { it.id == id }
    if (c == null) {
        MenuHeading("Filter by")
        MenuItem("Only mine", on = view.filters.any { it.op == FilterOp.Mine }) { onView(view.withFilter(Filter(FilterOp.Mine))); onMenu(null) }
        columns.drop(1).filter { it.sort != null || !it.numeric }.groupBy { it.topic }.forEach { (topic, cols) ->
            MenuHeading(topic)
            cols.forEach { col -> MenuItem(col.title, hint = if (col.numeric) "range, top" else "pick") { onMenu("filter:${col.id}") } }
        }
        return
    }
    MenuHeading(c.title)
    if (!c.numeric) {
        distinct(c.id).forEach { v ->
            MenuItem(v, color = Sectors.order.firstOrNull { Sectors.name(it) == v }?.let(Sectors::color), action = "not",
                onAction = { onView(view.withFilter(Filter(FilterOp.Not, c.id, v))); onMenu(null) }) { onView(view.withFilter(Filter(FilterOp.Is, c.id, v))); onMenu(null) }
        }
        return
    }
    listOf(5, 10).forEach { n ->
        MenuItem("Top $n") { onView(view.withFilter(Filter(FilterOp.Top, c.id, number = n.toDouble()))); onMenu(null) }
        MenuItem("Bottom $n") { onView(view.withFilter(Filter(FilterOp.Bottom, c.id, number = n.toDouble()))); onMenu(null) }
    }
    var above by remember { mutableStateOf("") }
    var below by remember { mutableStateOf("") }
    fun num(s: String) = s.replace(",", "").replace("%", "").trim().toDoubleOrNull()
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
        Text("Above", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.width(52.dp))
        GridSearch(above, "a figure", 120.dp, {}, { above = it }, { num(above)?.let { onView(view.withFilter(Filter(FilterOp.Above, c.id, number = it))); onMenu(null) } })
    }
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
        Text("Below", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.width(52.dp))
        GridSearch(below, "a figure", 120.dp, {}, { below = it }, { num(below)?.let { onView(view.withFilter(Filter(FilterOp.Below, c.id, number = it))); onMenu(null) } })
    }
    Text("Money in whole units, percent in percent. Press Enter.", color = Palette.secondary, fontSize = 10.sp, modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp))
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun <T> ColumnsMenu(view: ViewState, columns: List<GridColumn<T>>, onView: (ViewState) -> Unit, onExtra: (String) -> Unit) {
    var find by remember { mutableStateOf("") }
    var open by remember { mutableStateOf<String?>(null) }
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 3.dp)) { GridSearch(find, "Find a column", 300.dp, {}, { find = it }, {}) }
    columns.drop(1).filter { find.isBlank() || it.title.contains(find, true) || it.topic.contains(find, true) }.groupBy { it.topic }.forEach { (topic, cols) ->
        MenuHeading(topic)
        cols.forEach { c ->
            val specs = view.columns.filter { it.id == c.id }
            val on = specs.isNotEmpty()
            val modes = modesOf(c)
            // How it shows stays folded away until asked: one line per column, "show as" opens the ways.
            MenuItem(
                c.title, on = on, box = on, onBox = { onView(view.withColumn(c.id, !on)); if (!on) onExtra(c.id) },
                hint = if (on && open != c.id) specs.joinToString(", ") { it.mode.label.lowercase() } else "",
                action = if (on && modes.size > 1 && open != c.id) "show as" else null, onAction = { open = c.id },
            ) { onView(view.withColumn(c.id, !on)); if (!on) onExtra(c.id) }
            if (open == c.id) FlowRow(Modifier.fillMaxWidth().padding(start = 34.dp, end = 8.dp, bottom = 3.dp), horizontalArrangement = Arrangement.spacedBy(3.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                modes.forEach { m -> GridButton(m.label, specs.any { it.mode == m }, 9.sp) { onView(view.toggleMode(c.id, m)); onExtra(ColumnSpec(c.id, m).key) } }
            }
        }
    }
}

/** The chips under the bar: scope, every filter and the grouping, each with its own cross, and Clear all. Not drawn when there are none. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun <T> ChipRow(view: ViewState, views: GridViews<T>, columns: List<GridColumn<T>>, onView: (ViewState) -> Unit) {
    if (view.filters.isEmpty() && view.scope.isEmpty() && view.groupBy == null) return
    val titles = columns.associate { it.id to it.title }
    FlowRow(
        Modifier.fillMaxWidth().background(Palette.ink).padding(horizontal = 6.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (view.scope.isNotEmpty()) Chip("Scope: " + view.scope.joinToString(" + ") { scopeLabel(it, views) }, views.scopes.firstOrNull { it.id == view.scope.first() }?.color) { onView(view.copy(scope = emptyList())) }
        view.filters.forEach { f -> Chip(f.label { titles[it] ?: it }, null) { onView(view.withoutFilter(f)) } }
        view.groupBy?.let { g -> Chip("Grouped by ${titles[g] ?: g}", null) { onView(view.copy(groupBy = null)) } }
        Text(
            "CLEAR ALL", color = Palette.secondary, fontFamily = Pixel.font, fontSize = 9.sp,
            modifier = Modifier.pointerHoverIcon(PointerIcon.Hand).clickable { onView(view.copy(filters = emptyList(), scope = emptyList(), groupBy = null)) }.padding(horizontal = 6.dp, vertical = 3.dp),
        )
    }
}

@Composable
private fun Chip(text: String, color: Color?, onRemove: () -> Unit) {
    Row(Modifier.background(Palette.panelRaised).border(1.dp, Palette.gold.copy(alpha = 0.6f)).padding(start = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        if (color != null) { Box(Modifier.width(8.dp).height(8.dp).background(color)); Spacer(Modifier.width(5.dp)) }
        Text(text, color = Palette.primaryText, fontSize = 11.sp, maxLines = 1)
        // The cross that removes the chip (a letter, not a pixel icon).
        val cross = Modifier.pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onRemove).padding(horizontal = 6.dp, vertical = 2.dp)
        Text("x", color = Palette.gold, fontFamily = Pixel.font, fontSize = 10.sp, modifier = cross)
    }
}
