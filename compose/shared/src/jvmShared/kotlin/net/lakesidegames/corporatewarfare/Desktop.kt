package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.material3.Icon
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.CommodityDto
import uniffi.cw_ffi.EventDto
import uniffi.cw_ffi.ProductLineDto
import uniffi.cw_ffi.RegionBoardDto
import uniffi.cw_ffi.StockDto

/** Windows at least this wide get the desktop layout instead of the phone tabs. */
val DesktopBreakpoint: Dp = 1000.dp

private enum class Page(val id: String, val title: String, val icon: String, val group: String, val key: String) {
    Map("map", "Map", "map", "Play", "F1"),
    Office("office", "CEO Office", "office", "Company", "F2"),
    Research("research", "Research", "research", "Company", "F3"),
    Stats("stats", "Stats", "stats", "Company", "F4"),
    Markets("markets", "Markets", "markets", "World", "F5"),
    Actions("actions", "Actions", "command", "World", "Ctrl K"),
    Inbox("inbox", "Inbox", "inbox", "World", "F6"),
    Portfolio("portfolio", "Portfolio", "portfolio", "You", "F7"),
}

/**
 * The desktop shell: a sidebar for every screen, a status bar with the company, the economy and
 * the time controls always in view, keyboard shortcuts, and dense multi-pane screens.
 */
@Composable
fun DesktopShell(store: GameStore, onSettings: () -> Unit, startPage: Int = 0) {
    var page by remember { mutableStateOf(Page.entries[startPage]) }
    var query by remember { mutableStateOf("") }
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    fun open(next: Page) {
        page = next
        Tutorial.visited(next.id)
    }
    // "Show me" in the tutorial opens the page its step needs.
    LaunchedEffect(Tutorial.requestedPage) {
        val wanted = Tutorial.requestedPage ?: return@LaunchedEffect
        Page.entries.firstOrNull { it.id == wanted }?.let(::open)
        Tutorial.requestedPage = null
    }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    LaunchedEffect(page) { Tutorial.visited(page.id) }
    Box(
        Modifier.fillMaxSize().background(Palette.ink).focusRequester(focus).focusable()
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when {
                    event.isCtrlPressed && event.key == Key.K -> { open(Page.Actions); true }
                    event.key == Key.Spacebar && page != Page.Actions -> { store.playAt(if (store.running) 0 else store.speed.coerceAtLeast(1)); true }
                    event.key == Key.N && page != Page.Actions -> { if (!store.busy && !store.running) store.advance(); true }
                    event.key == Key.One && page != Page.Actions -> { store.playAt(1); true }
                    event.key == Key.Two && page != Page.Actions -> { store.playAt(2); true }
                    event.key == Key.Three && page != Page.Actions -> { store.playAt(4); true }
                    event.key == Key.F1 -> { open(Page.Map); true }
                    event.key == Key.F2 -> { open(Page.Office); true }
                    event.key == Key.F3 -> { open(Page.Research); true }
                    event.key == Key.F4 -> { open(Page.Stats); true }
                    event.key == Key.F5 -> { open(Page.Markets); true }
                    event.key == Key.F6 -> { open(Page.Inbox); true }
                    event.key == Key.F7 -> { open(Page.Portfolio); true }
                    else -> false
                }
            },
    ) {
        Row(Modifier.fillMaxSize()) {
            Sidebar(store, page, ::open, onSettings)
            Box(Modifier.width(1.dp).fillMaxHeight().background(Palette.stroke))
            Column(Modifier.weight(1f).fillMaxHeight()) {
                StatusBar(store) { text ->
                    query = text
                    open(Page.Actions)
                }
                Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    when (page) {
                        Page.Map -> DesktopMap(store)
                        Page.Office -> DesktopOffice(store) { open(it) }
                        Page.Research -> Box(Modifier.fillMaxSize().padding(10.dp)) { ResearchScreen(store) }
                        Page.Markets -> DesktopMarkets(store)
                        Page.Actions -> key(query) { CommandCenterScreen(store, initialQuery = query) }
                        Page.Stats -> DesktopStats(store)
                        Page.Inbox -> DesktopInbox(store)
                        Page.Portfolio -> DesktopPortfolio(store)
                    }
                }
            }
        }
        TutorialPanel(compact = false, modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp).width(360.dp))
    }
}

@Composable
private fun Sidebar(store: GameStore, page: Page, onPage: (Page) -> Unit, onSettings: () -> Unit) {
    Column(Modifier.width(206.dp).fillMaxHeight().background(Palette.panel).padding(vertical = 14.dp)) {
        Box(Modifier.fillMaxWidth().padding(bottom = 10.dp), contentAlignment = Alignment.Center) { LogoArt(Modifier.width(112.dp)) }
        CompanySwitcher(store)
        var group = ""
        Page.entries.forEach { item ->
            if (item.group != group) {
                group = item.group
                Text(
                    group.uppercase(), color = Palette.secondary.copy(alpha = 0.7f), fontSize = 10.sp, fontWeight = FontWeight.SemiBold,
                    letterSpacing = 1.sp, modifier = Modifier.padding(start = 20.dp, top = 12.dp, bottom = 4.dp),
                )
            }
            val due = if (item == Page.Inbox) store.events.count { it.mandatory } else 0
            SidebarItem(item.icon, item.title, item.key, page == item, badge = due) { onPage(item) }
        }
        Spacer(Modifier.weight(1f))
        SidebarItem("save", "Save", "", false) { store.saveNow() }
        SidebarItem("settings", "Settings", "", false, onClick = onSettings)
        SidebarItem("quit", "Main menu", "", false) { store.exitToMenu() }
    }
}

/** Which company the screens act on; a list of the companies you control. */
@Composable
private fun CompanySwitcher(store: GameStore) {
    var open by remember { mutableStateOf(false) }
    val firm = store.selectedFirmDto
    val controlled = store.firms.filter { it.controlled }
    Column(Modifier.fillMaxWidth().padding(horizontal = 10.dp)) {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(Palette.panelRaised)
                .clickable(enabled = controlled.size > 1) { open = !open }.padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            firm?.let { SectorBadge(it.industry, 22) }
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) {
                Text(firm?.name ?: "No company yet", color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(firm?.let { "${Sectors.name(it.industry)} · ${it.region}" } ?: "Found one in the CEO Office", color = Palette.secondary, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (controlled.size > 1) PixelIcon(if (open) "chevron_up" else "chevron_down", size = 12.dp, tint = Palette.secondary)
        }
        if (open) {
            controlled.forEach { f ->
                Text(
                    f.name, color = if (f.handle == store.selectedFirm) Palette.gold else Palette.primaryText, fontSize = 12.sp,
                    modifier = Modifier.fillMaxWidth().clickable { store.selectFirm(f.handle); open = false }.padding(horizontal = 12.dp, vertical = 6.dp),
                )
            }
        }
    }
}

@Composable
private fun SidebarItem(icon: String, title: String, shortcut: String, selected: Boolean, badge: Int = 0, onClick: () -> Unit) {
    val hover = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 1.dp).clip(RoundedCornerShape(8.dp))
            .background(if (selected) Palette.panelRaised else if (hovered) Palette.panelRaised.copy(alpha = 0.5f) else Color.Transparent)
            .hoverable(hover).pointerHoverIcon(PointerIcon.Hand)
            .clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (selected) Box(Modifier.width(3.dp).height(18.dp).clip(RoundedCornerShape(2.dp)).background(Palette.gold))
        Spacer(Modifier.width(if (selected) 7.dp else 10.dp))
        PixelIcon(icon, size = 18.dp, tint = if (selected) Palette.gold else Palette.secondary)
        Spacer(Modifier.width(10.dp))
        Text(title, color = if (selected) Palette.primaryText else Palette.secondary, fontSize = 13.sp, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal, modifier = Modifier.weight(1f))
        if (badge > 0) SignalPill("$badge", Palette.warning)
        else if (hovered && shortcut.isNotEmpty()) Text(shortcut, color = Palette.secondary.copy(alpha = 0.7f), fontSize = 10.sp)
    }
}

private fun phaseTint(phase: String): Color = when (phase) {
    "recession" -> Palette.negative
    "boom" -> Palette.gold
    "slowdown" -> Palette.warning
    "recovery" -> Palette.positive
    else -> Color(0xFF42A5F5)
}

/** The company, the economy and the time controls, always in view, with action search. */
@Composable
private fun StatusBar(store: GameStore, onSearch: (String) -> Unit) {
    val overview = store.overview
    val economy = store.economy
    Row(
        Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Column {
            Text(overview?.weekLabel ?: "", color = Palette.primaryText, fontSize = 16.sp, fontWeight = FontWeight.Bold)
            if (economy != null) {
                Text(
                    economy.phase.replaceFirstChar { it.uppercase() },
                    color = Palette.ink, fontSize = 10.sp, fontWeight = FontWeight.Bold,
                    modifier = Modifier.clip(CircleShape).background(phaseTint(economy.phase)).padding(horizontal = 7.dp, vertical = 1.dp),
                )
            }
        }
        val profit = overview?.quarterNetIncomeCents ?: 0
        val weeks = overview?.quarterWeeks?.toInt() ?: 0
        Figure("Company cash", Format.compact(overview?.firmCashCents ?: 0))
        Figure(if (weeks >= 13) "Profit, quarter" else "Profit, $weeks wk", Format.compact(profit), if (profit >= 0) Palette.positive else Palette.negative)
        val outlook = (overview?.forecastFourWeekCents ?: 0) - (overview?.firmCashCents ?: 0)
        Figure("Cash in 4 wk", Format.compact(overview?.forecastFourWeekCents ?: 0), if (outlook >= 0) Palette.primaryText else Palette.negative)
        Figure("Net worth", Format.compact(overview?.investorNetWorthCents ?: 0))
        if (economy != null) {
            Figure("Rate", "%.2f%%".format(economy.policyRateBps / 100.0))
            Figure("Inflation", "%.1f%%".format(economy.inflationBps / 100.0), if (economy.inflationBps > economy.targetInflationBps + 100) Palette.warning else Palette.primaryText)
        }
        Spacer(Modifier.weight(1f))
        var text by remember { mutableStateOf("") }
        androidx.compose.foundation.text.BasicTextField(
            text, { text = it },
            singleLine = true,
            textStyle = androidx.compose.ui.text.TextStyle(color = Palette.primaryText, fontSize = 13.sp),
            cursorBrush = androidx.compose.ui.graphics.SolidColor(Palette.gold),
            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Search),
            keyboardActions = androidx.compose.foundation.text.KeyboardActions(onSearch = { onSearch(text) }),
            modifier = Modifier.width(220.dp).onPreviewKeyEvent { e ->
                if (e.type == KeyEventType.KeyDown && e.key == Key.Enter) { onSearch(text); true } else false
            },
            decorationBox = { inner ->
                Row(
                    Modifier.clip(RoundedCornerShape(9.dp)).background(Palette.ink).border(1.dp, Palette.stroke, RoundedCornerShape(9.dp))
                        .padding(horizontal = 10.dp, vertical = 7.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    PixelIcon("search", size = 16.dp, tint = Palette.secondary)
                    Spacer(Modifier.width(6.dp))
                    Box(Modifier.weight(1f)) {
                        if (text.isEmpty()) Text("Search actions", color = Palette.secondary, fontSize = 13.sp)
                        inner()
                    }
                    Text("Ctrl K", color = Palette.secondary.copy(alpha = 0.6f), fontSize = 10.sp)
                }
            },
        )
        GoldButton("Next week", enabled = !store.busy && !store.running) { store.advance() }
        SpeedControl(store)
    }
}

@Composable
private fun Figure(label: String, value: String, tone: Color = Palette.primaryText) {
    Column {
        Text(label.uppercase(), color = Palette.secondary, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.6.sp)
        Text(value, color = tone, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

// MARK: Table pieces

/** A column: header text, share of the row's width, and alignment. */
data class Col(val title: String, val weight: Float, val end: Boolean = false)

@Composable
private fun TableHeader(cols: List<Col>) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 5.dp)) {
        cols.forEach { col ->
            Text(
                col.title.uppercase(), color = Palette.secondary, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp,
                textAlign = if (col.end) TextAlign.End else TextAlign.Start, modifier = Modifier.weight(col.weight).padding(horizontal = 4.dp), maxLines = 1,
            )
        }
    }
    HorizontalDivider(color = Palette.stroke)
}

@Composable
private fun TableRow(index: Int, selected: Boolean = false, onClick: (() -> Unit)? = null, content: @Composable RowScope.() -> Unit) {
    val hover = remember { androidx.compose.foundation.interaction.MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    val base = if (index % 2 == 1) Palette.panelRaised.copy(alpha = 0.35f) else Color.Transparent
    Row(
        Modifier.fillMaxWidth()
            .background(if (selected) Palette.gold.copy(alpha = 0.14f) else if (hovered && onClick != null) Palette.gold.copy(alpha = 0.06f) else base)
            .hoverable(hover)
            .then(if (onClick != null) Modifier.pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick) else Modifier)
            .padding(horizontal = 8.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}

@Composable
private fun RowScope.Cell(text: String, col: Col, tone: Color = Palette.primaryText, bold: Boolean = false) {
    Text(
        text, color = tone, fontSize = 12.sp, fontWeight = if (bold) FontWeight.SemiBold else FontWeight.Normal,
        textAlign = if (col.end) TextAlign.End else TextAlign.Start, maxLines = 1, overflow = TextOverflow.Ellipsis,
        modifier = Modifier.weight(col.weight).padding(horizontal = 4.dp),
    )
}

/** A titled pane that fills its slot and scrolls on its own. */
@Composable
private fun Pane(title: String, modifier: Modifier, scroll: Boolean = true, trailing: @Composable () -> Unit = {}, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.clip(RoundedCornerShape(10.dp)).background(Palette.panel).border(1.dp, Palette.stroke, RoundedCornerShape(10.dp))) {
        Row(Modifier.fillMaxWidth().padding(start = 12.dp, end = 8.dp, top = 8.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(title.uppercase(), color = Palette.secondary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.8.sp, modifier = Modifier.weight(1f))
            trailing()
        }
        if (scroll) Column(Modifier.fillMaxWidth().weight(1f, fill = false).verticalScroll(rememberScrollState()), content = content)
        else Column(Modifier.fillMaxWidth().padding(bottom = 6.dp), content = content)
    }
}

@Composable
private fun Chips(options: List<String>, selected: String, onPick: (String) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        options.forEach { option ->
            Text(
                option, color = if (option == selected) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(CircleShape).background(if (option == selected) Palette.gold else Palette.panelRaised)
                    .clickable { onPick(option) }.padding(horizontal = 10.dp, vertical = 4.dp),
            )
        }
    }
}

// MARK: Map

/** Outliner on the left, the world map in the middle, the selected region's board on the right. */
@Composable
private fun DesktopMap(store: GameStore) {
    var layer by remember { mutableStateOf(MapLayer.Presence) }
    var selected by remember { mutableStateOf<String?>(null) }
    var building by remember { mutableStateOf<RegionBoardDto?>(null) }
    Row(Modifier.fillMaxSize().padding(10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Column(Modifier.width(300.dp).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            val overview = store.overview
            val alerts = buildList {
                val due = store.events.count { it.mandatory }
                if (due > 0) add("$due decision${if (due == 1) "" else "s"} waiting in the Inbox" to Palette.warning)
                overview?.pause?.let { add("Paused: $it" to Palette.warning) }
                overview?.alerts?.forEach { add(it to Palette.warning) }
                overview?.nextActionReason?.takeIf { it.isNotEmpty() }?.let { add(it to Palette.gold) }
            }
            Pane("Alerts", Modifier.fillMaxWidth().weight(0.8f)) {
                if (alerts.isEmpty()) Text("Nothing needs you this week.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(12.dp))
                alerts.forEach { (text, tone) ->
                    Row(Modifier.padding(horizontal = 12.dp, vertical = 4.dp)) {
                        PixelIcon("alert", size = 12.dp, tint = tone, modifier = Modifier.padding(top = 1.dp, end = 6.dp))
                        Text(text, color = Palette.primaryText, fontSize = 12.sp)
                    }
                }
            }
            val cols = listOf(Col("Division", 1.6f), Col("Plants", 0.6f, true), Col("Share", 0.6f, true))
            Pane("Your divisions", Modifier.fillMaxWidth().weight(1.2f)) {
                TableHeader(cols)
                if (store.divisions.isEmpty()) Text("No divisions yet. Pick a region and build.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(12.dp))
                store.divisions.forEachIndexed { i, d ->
                    TableRow(i, selected = d.region == selected, onClick = { selected = d.region }) {
                        SectorBadge(d.industry, 18)
                        Spacer(Modifier.width(6.dp))
                        Cell("${Sectors.name(d.industry)} · ${d.regionName}", cols[0].copy(weight = cols[0].weight - 0.2f))
                        Cell("${d.blocks}", cols[1])
                        Cell("${d.shareBps / 100}%", cols[2], Palette.gold, bold = true)
                    }
                }
                store.divisions.firstOrNull { it.region == selected }?.let { d ->
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        DivisionStock(store, d)
                        DivisionShipping(store, d)
                    }
                }
            }
            Pane("Rival moves", Modifier.fillMaxWidth().weight(1f)) {
                val raids = store.recentRaids.take(14)
                if (raids.isEmpty()) Text("No raids yet.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(12.dp))
                raids.forEach { r ->
                    val tone = when {
                        r.playerDefender && r.success -> Palette.negative
                        r.playerDefender -> Palette.warning
                        r.playerAttacker -> Palette.positive
                        else -> Palette.secondary
                    }
                    val region = store.boards.firstOrNull { it.code == r.region }?.name ?: r.region
                    Column(Modifier.fillMaxWidth().clickable { selected = r.region }.padding(horizontal = 12.dp, vertical = 4.dp)) {
                        Text(
                            "${r.attacker} ${if (r.success) "took" else "failed to take"} ${r.blocks} plants from ${r.defender}",
                            color = tone, fontSize = 12.sp,
                        )
                        Text("${ResultBuckets.weekLabel(r.week.toInt())} · ${Sectors.name(r.industry)} · $region", color = Palette.secondary, fontSize = 11.sp)
                    }
                }
            }
        }
        Box(Modifier.weight(1f).fillMaxHeight().clip(RoundedCornerShape(10.dp)).border(1.dp, Palette.stroke, RoundedCornerShape(10.dp))) {
            WorldMapView(store, layer, selected, { selected = it }, Modifier.fillMaxSize())
            Row(Modifier.align(Alignment.TopStart).padding(10.dp)) {
                Chips(MapLayer.entries.map { it.title }, layer.title) { title -> layer = MapLayer.entries.first { it.title == title } }
            }
        }
        if (layer == MapLayer.Freight) {
            Pane("Freight lanes", Modifier.width(360.dp).fillMaxHeight()) { FreightPanel(store, selected) }
        }
        val board = selected?.let { code -> store.boards.firstOrNull { it.code == code } }
        if (board != null) {
            Pane(board.name, Modifier.width(320.dp).fillMaxHeight(), trailing = {
                PixelIcon("close", size = 14.dp, tint = Palette.secondary, modifier = Modifier.clickable { selected = null }.padding(6.dp))
            }) {
                RegionBoardDetail(store, board)
                GoldButton("Build here", Modifier.fillMaxWidth().padding(12.dp), enabled = store.selectedFirm != null) { building = board }
            }
        }
    }
    building?.let { b -> BuildDialog(store, b) { building = null } }
}

@Composable
private fun RegionBoardDetail(store: GameStore, board: RegionBoardDto) {
    Column(Modifier.padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("${board.stateName} · hub ${board.hubCity}", color = Palette.secondary, fontSize = 12.sp)
        val cols = listOf(Col("Wages", 1f, true), Col("Skill", 1f, true), Col("Roads", 1f, true), Col("Power", 1f, true))
        TableHeader(cols)
        TableRow(0) {
            Cell("${board.wageIndexBps / 100}%", cols[0]); Cell("${board.skillBps / 100}", cols[1])
            Cell("${board.roadBps / 100}", cols[2]); Cell("${board.powerGridBps / 100}", cols[3])
        }
        if (board.specialization.isNotEmpty()) {
            Text("Specialized in " + board.specialization.joinToString(" and ") { Sectors.name(it) }, color = Palette.positive, fontSize = 12.sp)
        }
        val room = board.room.filter { it.importLots > 0 }.sortedByDescending { it.importLots * it.importPriceCents }
        if (room.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Eyebrow("Room to build: imported each week")
            val rc = listOf(Col("Good", 1.6f), Col("Lots", 0.7f, true), Col("Import price", 1f, true))
            TableHeader(rc)
            room.forEachIndexed { i, r ->
                TableRow(i) { Cell(r.productName, rc[0]); Cell(Format.count(r.importLots), rc[1]); Cell(Format.money(r.importPriceCents), rc[2]) }
            }
        }
        if (board.storage.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Eyebrow("Your storage here")
            StorageLines(board.storage)
        }
        val ours = store.divisions.filter { it.region == board.code }
        if (ours.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Eyebrow("Your divisions here")
            ours.forEach { Text("${Sectors.name(it.industry)} · ${it.strategyName} · ${it.blocks} plants · ${it.shareBps / 100}% share", color = Palette.primaryText, fontSize = 12.sp) }
        }
    }
}

// MARK: CEO Office

/** Figures in a strip, then results, pricing and divisions side by side. */
@Composable
private fun DesktopOffice(store: GameStore, onOpen: (Page) -> Unit) {
    val firm = store.selectedFirmDto
    var editing by remember { mutableStateOf<ProductLineDto?>(null) }
    if (firm == null) {
        Box(Modifier.fillMaxSize().padding(16.dp)) { FoundCompanyPanel(store) }
        return
    }
    Column(Modifier.fillMaxSize().padding(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(Palette.panel).padding(12.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            SectorBadge(firm.industry, 40)
            Column(Modifier.width(260.dp)) {
                Text(firm.name, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 17.sp, maxLines = 1)
                Text("${Sectors.name(firm.industry)} · HQ ${firm.region} · Credit ${firm.grade}", color = Palette.secondary, fontSize = 12.sp, maxLines = 1)
            }
            Figure("Company cash", Format.compact(firm.cashCents))
            Figure("Equity", Format.compact(firm.equityCents))
            Figure("Fair value", Format.compact(firm.fairValueCents))
            Figure("Share price", Format.price(firm.markCents))
            Figure("Plants", "${firm.facilities}")
            Figure("Your votes", "%.1f%%".format(firm.investorVotesBps / 100.0))
            Spacer(Modifier.weight(1f))
            OutlinedButton(onClick = { onOpen(Page.Research) }) { Text("Research", color = Palette.primaryText) }
            OutlinedButton(onClick = { onOpen(Page.Stats) }) { Text("Stats", color = Palette.primaryText) }
            GoldButton("Actions") { onOpen(Page.Actions) }
        }
        Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Column(Modifier.weight(0.9f).fillMaxHeight().verticalScroll(rememberScrollState())) {
                store.finance?.takeIf { it.firm == firm.handle && it.recentWeeks.isNotEmpty() }?.let { ProfitDrivers(it.recentWeeks) }
                    ?: Callout("Results appear after the first week closes.", Palette.secondary)
                if (store.facilities.isNotEmpty()) {
                    Spacer(Modifier.height(10.dp))
                    ProductionTarget(store)
                }
                val periods = ResultBuckets.make(StatsPeriod.Month, emptyList(), store.history)
                if (periods.size > 1) {
                    Spacer(Modifier.height(10.dp))
                    Panel {
                        Eyebrow("Four-weekly results")
                        ResultsChart(periods)
                    }
                }
            }
            val pc = listOf(Col("Product", 1.5f), Col("Price", 0.8f, true), Col("Posture", 1f), Col("Cost", 0.8f, true), Col("Margin", 0.8f, true), Col("Made", 0.6f, true), Col("Sold", 0.6f, true), Col("Stock", 0.75f, true))
            Pane("Pricing: click a product to change its posture", Modifier.weight(1.4f).fillMaxHeight()) {
                TableHeader(pc)
                store.productLines.forEachIndexed { i, line ->
                    val margin = line.priceCents - line.unitCostCents
                    val sold = if (line.madeLastWeekLots > 0) (line.soldLastWeekLots * 100 / line.madeLastWeekLots) else null
                    TableRow(i, onClick = { editing = line }) {
                        Cell(line.name, pc[0], bold = true)
                        Cell(Format.money(line.priceCents), pc[1])
                        Cell(postureLabel(line), pc[2], Palette.secondary)
                        Cell(if (line.unitCostCents > 0) Format.money(line.unitCostCents) else "n/a", pc[3])
                        Cell(if (line.unitCostCents > 0) Format.signed(margin) else "", pc[4], if (margin >= 0) Palette.positive else Palette.negative)
                        Cell(Format.count(line.madeLastWeekLots), pc[5])
                        Cell(sold?.let { "$it%" } ?: "", pc[6], when { sold == null -> Palette.secondary; sold >= 95 -> Palette.positive; sold >= 70 -> Palette.gold; else -> Palette.negative })
                        Cell(Format.count(line.stockLots), pc[7])
                    }
                }
            }
            val dc = listOf(Col("Division", 1.6f), Col("Strategy", 1.2f), Col("Plants", 0.6f, true), Col("Share", 0.6f, true))
            Pane("Divisions", Modifier.weight(1.1f).fillMaxHeight()) {
                TableHeader(dc)
                store.divisions.forEachIndexed { i, d ->
                    TableRow(i) {
                        Cell("${Sectors.name(d.industry)} · ${d.regionName}", dc[0], bold = true)
                        Cell(d.strategyName + if (d.retooling) " (retooling)" else "", dc[1], Palette.secondary)
                        Cell("${d.blocks}", dc[2])
                        Cell("${d.shareBps / 100}%", dc[3], Palette.gold)
                    }
                }
            }
        }
    }
    editing?.let { line -> PostureDialog(store, line) { editing = null } }
}

// MARK: Markets

/** The stock exchange and the commodity board side by side, as full tables. */
@Composable
private fun DesktopMarkets(store: GameStore) {
    var stock by remember { mutableStateOf<StockDto?>(null) }
    var commodity by remember { mutableStateOf<CommodityDto?>(null) }
    var sort by remember { mutableStateOf("Top movers") }
    var board by remember { mutableStateOf("Stocks") }
    Row(Modifier.fillMaxSize().padding(10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
      Column(Modifier.weight(1f).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Chips(listOf("Stocks", "Commodities"), board) { board = it }
        val index = corridorIndex(store.stocks)
        val sc = listOf(Col("Company", 1.7f), Col("Sector", 1f), Col("Price", 0.8f, true), Col("Week", 0.6f, true), Col("Quarter", 0.7f, true), Col("Market cap", 0.9f, true), Col("Float", 0.55f, true), Col("You hold", 0.75f, true))
        val title = if (index.isNotEmpty()) "Stocks · index ${Format.count(index.last())}" else "Stocks"
        if (board == "Stocks") Pane(title, Modifier.fillMaxWidth().weight(1f), trailing = { Chips(listOf("Top movers", "Largest", "Yours"), sort) { sort = it } }) {
            TableHeader(sc)
            val rows = when (sort) {
                "Largest" -> store.stocks.sortedByDescending { it.marketCapCents }
                "Yours" -> store.stocks.filter { it.yourShares > 0 || it.controlledByYou }
                else -> store.stocks.sortedByDescending { kotlin.math.abs(it.changeWeekBps) }
            }
            rows.forEachIndexed { i, s ->
                TableRow(i, onClick = { stock = s }) {
                    Cell(s.name + if (s.controlledByYou) " ★" else "", sc[0], bold = true)
                    Cell(Sectors.name(s.industry), sc[1], Palette.secondary)
                    Cell(Format.price(s.priceCents), sc[2])
                    Cell(changeText(s.changeWeekBps), sc[3], changeTint(s.changeWeekBps))
                    Cell(changeText(s.changeQuarterBps), sc[4], changeTint(s.changeQuarterBps))
                    Cell(Format.compact(s.marketCapCents), sc[5])
                    Cell("${s.floatBps / 100}%", sc[6], Palette.secondary)
                    Cell(if (s.yourShares > 0) Format.count(s.yourShares) else "", sc[7], Palette.gold)
                }
            }
        }
        val cc = listOf(Col("Good", 1.5f), Col("Price", 0.8f, true), Col("Week", 0.6f, true), Col("Made", 0.7f, true), Col("Sold", 0.7f, true), Col("Imported", 0.75f, true), Col("Exported", 0.75f, true))
        if (board == "Commodities") Pane("Commodities, per lot", Modifier.fillMaxWidth().weight(1f)) {
            TableHeader(cc)
            val ordered = store.commodities.sortedWith(compareBy({ (Sectors.order + "other").indexOf(it.sector).let { i -> if (i < 0) 99 else i } }, { it.name }))
            ordered.forEachIndexed { i, c ->
                TableRow(i, onClick = { commodity = c }) {
                    if (c.sector != "other") SectorBadge(c.sector, 16) else Spacer(Modifier.width(16.dp))
                    Spacer(Modifier.width(6.dp))
                    Cell(c.name + if (c.yours) " ★" else "", cc[0].copy(weight = cc[0].weight - 0.15f), bold = true)
                    Cell(Format.money(c.priceCents), cc[1])
                    Cell(changeText(c.changeWeekBps), cc[2], changeTint(c.changeWeekBps))
                    Cell(Format.count(c.madeLots), cc[3])
                    Cell(Format.count(c.soldLots), cc[4])
                    Cell(if (c.importedLots > 0) Format.count(c.importedLots) else "", cc[5], Palette.positive)
                    Cell(if (c.exportedLots > 0) Format.count(c.exportedLots) else "", cc[6], Palette.secondary)
                }
            }
        }
      }
        Column(Modifier.width(430.dp).fillMaxHeight().verticalScroll(rememberScrollState())) {
            EconomyCard(store, collapsible = false)
        }
    }
    stock?.let { StockDialog(store, it) { stock = null } }
    commodity?.let { CommodityDialog(store, it) { commodity = null } }
}

// MARK: Stats

/** Charts and statements in two columns. */
@Composable
private fun DesktopStats(store: GameStore) {
    var period by remember { mutableStateOf(StatsPeriod.Month) }
    val firm = store.selectedFirmDto
    val buckets = ResultBuckets.make(period, store.finance?.recentWeeks ?: emptyList(), store.history)
    val stock = firm?.let { f -> store.stocks.firstOrNull { it.firm == f.handle } }
    Column(Modifier.fillMaxSize().padding(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("${firm?.name ?: ""} stats", color = Palette.primaryText, fontSize = 17.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
            Chips(StatsPeriod.entries.map { it.title }, period.title) { t -> period = StatsPeriod.entries.first { it.title == t } }
        }
        if (buckets.isEmpty()) {
            Callout("Results appear after the first week closes.", Palette.secondary)
            return@Column
        }
        Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Column(Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Panel {
                    Eyebrow("Revenue and profit")
                    ResultsChart(buckets)
                    Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        Legend(Palette.gold.copy(alpha = 0.55f), "Revenue"); Legend(Palette.positive, "Net income"); Legend(Palette.negative, "Loss")
                    }
                }
                Statement(buckets)
            }
            Column(Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                if (period != StatsPeriod.Week && buckets.any { it.cash != null }) {
                    Panel {
                        Eyebrow("Balance sheet at each close")
                        LineChart(listOf(Palette.gold to buckets.map { it.cash ?: 0 }, Palette.positive to buckets.map { it.equity ?: 0 }, Palette.negative to buckets.map { it.liabilities ?: 0 }))
                        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                            Legend(Palette.gold, "Cash"); Legend(Palette.positive, "Equity"); Legend(Palette.negative, "Liabilities")
                        }
                    }
                }
                if (stock != null) {
                    val annual = if (stock.earningsWeeks > 0u) stock.earningsCents * 52 / stock.earningsWeeks.toLong() else 0L
                    StatGroup(
                        Stat("Market value", Format.money(stock.marketCapCents), note = "${Format.price(stock.priceCents)} a share"),
                        listOf(
                            Stat("Shares issued", Format.count(stock.sharesIssued)),
                            Stat("Free float", Format.count(stock.sharesIssued * stock.floatBps / 10_000)),
                            Stat("You hold", Format.count(stock.yourShares)),
                            Stat("Fair value", Format.money(stock.fairCents)),
                            Stat("Price to earnings", if (annual > 0) "%.1f".format(stock.marketCapCents.toDouble() / annual) else "n/a"),
                            Stat("Earnings a year", Format.compact(annual)),
                        ),
                    )
                    if (stock.historyCents.size > 1) {
                        Panel { Eyebrow("Share price, last ${stock.historyCents.size} weeks"); LineChart(listOf(Palette.gold to stock.historyCents)) }
                    }
                }
                val ownership = store.ownership
                if (ownership != null && ownership.firm == firm?.handle) {
                    val oc = listOf(Col("Holder", 2f), Col("Shares", 1f, true), Col("Stake", 0.7f, true), Col("Votes", 0.7f, true))
                    Pane("Who owns it", Modifier.fillMaxWidth(), scroll = false) {
                        TableHeader(oc)
                        ownership.holders.sortedByDescending { it.quantity }.forEachIndexed { i, h ->
                            TableRow(i) {
                                Cell(h.holder, oc[0], bold = true)
                                Cell(Format.count(h.quantity), oc[1])
                                Cell(if (ownership.issued > 0) "%.1f%%".format(h.quantity * 100.0 / ownership.issued) else "", oc[2], Palette.gold)
                                Cell("%.1f%%".format(h.votesBps / 100.0), oc[3], Palette.secondary)
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: Inbox

/** Decisions listed on the left, the open one on the right. */
@Composable
private fun DesktopInbox(store: GameStore) {
    var open by remember { mutableStateOf<ULong?>(null) }
    val events = store.events
    val current: EventDto? = events.firstOrNull { it.id == open } ?: events.firstOrNull()
    Row(Modifier.fillMaxSize().padding(10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        val ec = listOf(Col("Decision", 2f), Col("Due", 0.6f, true))
        Pane("Inbox · ${events.size}", Modifier.width(420.dp).fillMaxHeight()) {
            TableHeader(ec)
            if (events.isEmpty()) Text("Quiet week. Decisions show up here when they happen.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(12.dp))
            events.forEachIndexed { i, e ->
                TableRow(i, selected = e.id == current?.id, onClick = { open = e.id }) {
                    Cell(e.title, ec[0], bold = e.mandatory)
                    Cell(if (e.mandatory) "W${e.deadline}" else "", ec[1], Palette.warning)
                }
            }
        }
        Column(Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState())) {
            if (current != null) DecisionCard(store, current)
            else Callout("When a decision arrives it opens here with its choices. Time stops for any decision that is due.", Palette.secondary)
        }
    }
}

// MARK: Portfolio

/** The investor's position and every company they control, as one table. */
@Composable
private fun DesktopPortfolio(store: GameStore) {
    val overview = store.overview ?: return
    Column(Modifier.fillMaxSize().padding(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(Palette.panel).padding(12.dp),
            horizontalArrangement = Arrangement.spacedBy(28.dp), verticalAlignment = Alignment.CenterVertically,
        ) {
            Figure("Net worth", Format.compact(overview.investorNetWorthCents))
            Figure("Your cash", Format.compact(overview.investorCashCents))
            Figure("Company cash", Format.compact(overview.firmCashCents))
            Figure("Cash in 4 wk", Format.compact(overview.forecastFourWeekCents))
            Spacer(Modifier.weight(1f))
            overview.pause?.let { OutlinedButton(onClick = { store.dismissPause() }) { Text("Continue past: $it", color = Palette.primaryText, fontSize = 12.sp) } }
        }
        val fc = listOf(Col("Company", 1.8f), Col("Sector", 1f), Col("HQ", 1f), Col("Cash", 0.8f, true), Col("Equity", 0.8f, true), Col("Fair value", 0.9f, true), Col("Price", 0.7f, true), Col("Plants", 0.5f, true), Col("Votes", 0.6f, true), Col("Credit", 0.5f, true))
        Pane("Companies you control · click to manage", Modifier.fillMaxWidth().weight(1f), scroll = true) {
            TableHeader(fc)
            store.firms.filter { it.controlled }.forEachIndexed { i, f ->
                TableRow(i, selected = f.handle == store.selectedFirm, onClick = { store.selectFirm(f.handle) }) {
                    Cell(f.name, fc[0], bold = true)
                    Cell(Sectors.name(f.industry), fc[1], Palette.secondary)
                    Cell(f.region, fc[2], Palette.secondary)
                    Cell(Format.compact(f.cashCents), fc[3])
                    Cell(Format.compact(f.equityCents), fc[4])
                    Cell(Format.compact(f.fairValueCents), fc[5])
                    Cell(Format.price(f.markCents), fc[6])
                    Cell("${f.facilities}", fc[7])
                    Cell("%.0f%%".format(f.investorVotesBps / 100.0), fc[8], Palette.gold)
                    Cell(f.grade, fc[9])
                }
            }
        }
        val held = store.stocks.filter { it.yourShares > 0 }.sortedByDescending { it.yourShares * it.priceCents }
        val hc = listOf(Col("Company", 1.8f), Col("Sector", 1f), Col("Shares", 0.9f, true), Col("Stake", 0.6f, true), Col("Price", 0.7f, true), Col("Value", 0.9f, true), Col("Quarter", 0.7f, true))
        Pane("Your shareholdings · ${held.size}", Modifier.fillMaxWidth().weight(1f)) {
            TableHeader(hc)
            if (held.isEmpty()) Text("You hold no shares outside your companies. Buy on the Markets screen.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(12.dp))
            held.forEachIndexed { i, s ->
                TableRow(i) {
                    Cell(s.name + if (s.controlledByYou) " ★" else "", hc[0], bold = true)
                    Cell(Sectors.name(s.industry), hc[1], Palette.secondary)
                    Cell(Format.count(s.yourShares), hc[2])
                    Cell(if (s.sharesIssued > 0) "%.1f%%".format(s.yourShares * 100.0 / s.sharesIssued) else "", hc[3], Palette.gold)
                    Cell(Format.price(s.priceCents), hc[4])
                    Cell(Format.compact(s.yourShares * s.priceCents), hc[5], bold = true)
                    Cell(changeText(s.changeQuarterBps), hc[6], changeTint(s.changeQuarterBps))
                }
            }
        }
    }
}
