package net.lakesidegames.corporatewarfare

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
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerIcon
import androidx.compose.ui.input.pointer.pointerHoverIcon
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.CommodityDto
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.HeadToHeadDto
import uniffi.cw_ffi.PeriodDto
import uniffi.cw_ffi.ProductLineDto
import uniffi.cw_ffi.RivalMoveFeedDto
import uniffi.cw_ffi.RivalRowDto

/**
 * The places of the 2.0 desktop shell, grouped round what the player runs. Places not yet
 * converted show the 1.x page, so nothing is lost while the rest moves over.
 */
enum class Place(val title: String, val group: String, val key: String, val converted: Boolean) {
    Overview("Overview", "Company", "F2", true),
    Finance("Finance", "Company", "F4", false),
    Research("Research", "Company", "F3", false),
    Office("Office room", "Company", "", false),
    Goods("Goods", "Markets", "F5", true),
    Exchange("Stocks, economy", "Markets", "", false),
    Map("Map, freight", "Markets", "F1", false),
    League("League table", "Rivals", "F6", true),
    Paper("The paper", "World", "F8", false),
    Inbox("Letters", "World", "", false),
    Portfolio("Portfolio", "You", "F7", false),
    Hall("Hall of Careers", "You", "F9", false),
}

/** Switch the 2.0 shell on for a session: CW_UI=2.0 in the environment or -Dcw.ui=2.0. */
object NewShell {
    val enabled: Boolean by lazy { System.getenv("CW_UI") == "2.0" || System.getProperty("cw.ui") == "2.0" }
}

/**
 * The 2.0 desktop shell: a persistent top strip with the numbers that matter, grouped places on
 * the left, the place in the middle (with the map dockable beside it), and an alert rail on the
 * right with only what the player can act on. Ctrl K opens the command palette over anything.
 */
@Composable
fun TycoonShell(store: GameStore, onSettings: () -> Unit, onMenu: () -> Unit = {}, start: Place = Place.Overview, startPick: ULong? = null) {
    var place by remember { mutableStateOf(start) }
    var palette by remember { mutableStateOf(false) }
    var leaguePick by remember { mutableStateOf(startPick) }
    var docked by remember { mutableStateOf(false) }
    var dockShare by remember { mutableStateOf(DOCK_DEFAULT) }
    var dockLayer by remember { mutableStateOf(MapLayer.Presence) }
    var dockSelected by remember { mutableStateOf<String?>(null) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    fun open(p: Place) { place = p; palette = false }
    fun openRival(firm: ULong) { leaguePick = firm; open(Place.League) }
    Box(
        Modifier.fillMaxSize().background(Palette.ink).focusRequester(focus).focusable()
            .onPreviewKeyEvent { e ->
                if (e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when {
                    e.isCtrlPressed && e.key == Key.K -> { palette = !palette; true }
                    e.isCtrlPressed && e.key == Key.M -> { docked = !docked; true }
                    e.key == Key.Escape && palette -> { palette = false; true }
                    else -> Place.entries.firstOrNull { it.key.isNotEmpty() && keyName(e.key) == it.key }?.let { open(it); true } ?: false
                }
            },
    ) {
        DivisionDetailHost(store)
        Column(Modifier.fillMaxSize()) {
            TopStrip(store, onOpen = ::open, onPalette = { palette = true })
            Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
            Row(Modifier.fillMaxWidth().weight(1f)) {
                PlaceNav(place, store.inboxBadge, ::open, onSettings, onMenu, docked) { docked = !docked }
                Box(Modifier.width(1.dp).fillMaxHeight().background(Palette.stroke))
                Box(Modifier.weight(1f).fillMaxHeight()) {
                    val content: @Composable () -> Unit = { PlaceContent(store, place, leaguePick, { leaguePick = it }, ::open, ::openRival) }
                    if (docked && place != Place.Map) {
                        DockedSplit(dockShare, { dockShare = it }, map = {
                            DockedMap(store, dockLayer, { dockLayer = it }, dockSelected, { dockSelected = it }) { open(Place.Map) }
                        }, page = content)
                    } else content()
                }
                Box(Modifier.width(1.dp).fillMaxHeight().background(Palette.stroke))
                AlertRail(store, Modifier.width(250.dp).fillMaxHeight(), ::open, ::openRival)
            }
        }
        if (palette) {
            Box(Modifier.fillMaxSize().background(Palette.ink.copy(alpha = 0.7f)).clickable { palette = false }, contentAlignment = Alignment.TopCenter) {
                Box(Modifier.padding(top = 60.dp).width(900.dp).height(640.dp).border(2.dp, Palette.gold).background(Palette.panel).clickable(enabled = false) {}) {
                    CommandCenterScreen(store, onClose = { palette = false })
                }
            }
        }
    }
}

private fun keyName(k: Key): String = when (k) {
    Key.F1 -> "F1"; Key.F2 -> "F2"; Key.F3 -> "F3"; Key.F4 -> "F4"; Key.F5 -> "F5"
    Key.F6 -> "F6"; Key.F7 -> "F7"; Key.F8 -> "F8"; Key.F9 -> "F9"; else -> ""
}

@Composable
private fun PlaceContent(store: GameStore, place: Place, leaguePick: ULong?, onLeaguePick: (ULong) -> Unit, open: (Place) -> Unit, openRival: (ULong) -> Unit) {
    when (place) {
        Place.Overview -> CompanyOverviewScreen(store)
        Place.Goods -> GoodsScreen(store)
        Place.League -> LeagueScreen(store, leaguePick, onLeaguePick)
        Place.Finance -> DesktopStats(store)
        Place.Research -> Box(Modifier.fillMaxSize().padding(10.dp)) { ResearchScreen(store) }
        Place.Office -> DesktopOffice(store) { page -> Place.entries.firstOrNull { it.name == page.name }?.let(open) ?: open(Place.Overview) }
        Place.Exchange -> DesktopMarkets(store)
        Place.Map -> DesktopMap(store)
        Place.Paper -> PaperScreen(store)
        Place.Inbox -> DesktopInbox(store)
        Place.Portfolio -> DesktopPortfolio(store)
        Place.Hall -> HallScreen(store)
    }
}

// MARK: Top strip

/** Cash, profit, share price, league rank, net worth, the date and time controls, always in view. Each figure opens its breakdown. */
@Composable
fun TopStrip(store: GameStore, onOpen: (Place) -> Unit, onPalette: () -> Unit) {
    val o = store.overview
    val firm = store.selectedFirmDto
    val stock = firm?.let { f -> store.stocks.firstOrNull { it.firm == f.handle } }
    val ranked = store.stocks.sortedByDescending { it.marketCapCents }
    val rank = firm?.let { f -> ranked.indexOfFirst { it.firm == f.handle } + 1 } ?: 0
    val cur = o?.primaryCurrency.orEmpty()
    Row(
        Modifier.fillMaxWidth().height(44.dp).background(Palette.panel).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        firm?.let { SectorBadge(it.industry, 22) }
        Spacer(Modifier.width(6.dp))
        Text((firm?.name ?: "No company").uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 12.sp, maxLines = 1, modifier = Modifier.width(210.dp), overflow = TextOverflow.Ellipsis)
        val profit = o?.quarterNetIncomeCents ?: 0
        val weeks = o?.quarterWeeks?.toInt() ?: 0
        StripFigure("Cash", Format.compact(o?.firmCashCents ?: 0, cur)) { onOpen(Place.Finance) }
        StripFigure(if (weeks >= 13) "Profit, quarter" else "Profit, $weeks wk", Format.compact(profit, cur), if (profit >= 0) Palette.positive else Palette.negative) { onOpen(Place.Finance) }
        StripFigure("Cash in 4 wk", Format.compact(o?.forecastFourWeekCents ?: 0, cur), if ((o?.forecastFourWeekCents ?: 0) >= (o?.firmCashCents ?: 0)) Palette.primaryText else Palette.negative) { onOpen(Place.Finance) }
        stock?.let { s ->
            StripFigure("Share price", Format.price(s.priceCents) + " " + changeText(s.changeWeekBps), changeTint(s.changeWeekBps)) { onOpen(Place.League) }
        }
        if (rank > 0) StripFigure("League, by value", "#$rank of ${ranked.size}", if (rank <= 3) Palette.gold else Palette.primaryText) { onOpen(Place.League) }
        StripFigure("Net worth", Format.compact(o?.investorNetWorthCents ?: 0)) { onOpen(Place.Portfolio) }
        store.economy?.let { e -> StripFigure("Economy", "${e.phase.replaceFirstChar { it.uppercase() }} ${"%.2f%%".format(e.policyRateBps / 100.0)}", economyPhaseTint(e.phase)) { onOpen(Place.Exchange) } }
        Spacer(Modifier.weight(1f))
        TimeframeStrip()
        Spacer(Modifier.width(10.dp))
        GridButton("Ctrl K  Actions", false, 10.sp, onPalette)
        Spacer(Modifier.width(10.dp))
        Text(o?.weekLabel.orEmpty(), color = Palette.primaryText, style = GridFigures, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        Spacer(Modifier.width(10.dp))
        SpeedControl(store)
        Spacer(Modifier.width(8.dp))
        GoldButton("Next week", enabled = !store.busy && !store.running) { store.advance() }
    }
}

/** The game's one timeframe: every change, trend and bar on every grid follows it unless a grid overrides it. Remembered between sessions. */
@Composable
fun TimeframeStrip() {
    val prefs = LocalAppSettings.current
    androidx.compose.runtime.remember(prefs) { prefs?.let(GridPrefs::load); 0 }
    Explain("How far back every change, trend and bar on the grids looks. A grid can keep its own in its timeframe button.") {
        Row(horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
            Timeframe.entries.forEach { t -> GridButton(t.label, GridPrefs.timeframe == t, 10.sp) { GridPrefs.chooseTimeframe(t, prefs) } }
        }
    }
}

@Composable
private fun StripFigure(label: String, value: String, tone: Color = Palette.primaryText, onClick: () -> Unit) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Explain(Words.about(label)) {
        Column(
            Modifier.background(if (hovered) Palette.panelRaised else Color.Transparent).hoverable(hover).pointerHoverIcon(PointerIcon.Hand)
                .clickable(onClick = onClick).padding(horizontal = 8.dp, vertical = 3.dp),
        ) {
            Text(label.uppercase(), color = Palette.secondary, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp, maxLines = 1)
            Text(value, color = tone, style = GridFigures, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
        }
    }
}

// MARK: Navigation

@Composable
private fun PlaceNav(place: Place, inbox: Int, onOpen: (Place) -> Unit, onSettings: () -> Unit, onMenu: () -> Unit, docked: Boolean, onDock: () -> Unit) {
    Column(Modifier.width(168.dp).fillMaxHeight().background(Palette.panel).padding(vertical = 6.dp)) {
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            var group = ""
            Place.entries.forEach { p ->
                if (p.group != group) {
                    group = p.group
                    Text(group.uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 10.sp, modifier = Modifier.padding(start = 12.dp, top = 10.dp, bottom = 2.dp))
                }
                NavItem(p.title, p.key, p == place, if (p == Place.Inbox) inbox else 0) { onOpen(p) }
            }
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
        if (place != Place.Map) NavItem(if (docked) "Undock map" else "Dock map", "Ctrl M", docked, 0, onDock)
        NavItem("Settings", "", false, 0, onSettings)
        NavItem("Game menu", "Esc", false, 0, onMenu)
    }
}

@Composable
private fun NavItem(title: String, key: String, selected: Boolean, badge: Int, onClick: () -> Unit) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Row(
        Modifier.fillMaxWidth().height(24.dp).background(if (selected) Palette.panelRaised else if (hovered) Palette.panelRaised.copy(alpha = 0.5f) else Color.Transparent)
            .hoverable(hover).pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(3.dp).fillMaxHeight().background(if (selected) Palette.gold else Color.Transparent))
        Spacer(Modifier.width(10.dp))
        Text(title, color = if (selected) Palette.primaryText else Palette.secondary, fontSize = 12.sp, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal, maxLines = 1, modifier = Modifier.weight(1f))
        if (badge > 0) Text("$badge", color = Palette.ink, style = GridFigures, fontSize = 10.sp, fontWeight = FontWeight.Bold, modifier = Modifier.background(Palette.warning).padding(horizontal = 4.dp))
        else if (key.isNotEmpty()) Text(key, color = Palette.secondary.copy(alpha = 0.6f), fontSize = 10.sp)
        Spacer(Modifier.width(8.dp))
    }
}

// MARK: Alert rail

/** How far back the alert rail looks for raids and rival moves. */
private const val RAIL_WEEKS = 13L

/** What needs the player, newest first, each line a link to where they act on it. Empty sections are not shown. */
@Composable
fun AlertRail(store: GameStore, modifier: Modifier, onOpen: (Place) -> Unit, onRival: (ULong) -> Unit) {
    val o = store.overview
    val decisions = store.events.filter { it.mandatory }
    // Only what the player can act on: raids and rival moves from the last 13 weeks. Older ones live in the dossier.
    val now = o?.tick?.toLong() ?: 0L
    val recent = { week: UInt -> now - week.toLong() <= RAIL_WEEKS }
    val raids = store.recentRaids.filter { (it.playerDefender || it.playerAttacker) && recent(it.week) }.take(5)
    val mine = store.selectedFirmDto?.name.orEmpty()
    // The moves that touch the player's company come first, then the rest, newest first.
    val moves = store.rivalFeed.filter { recent(it.week) }
        .sortedWith(compareByDescending<RivalMoveFeedDto> { mine.isNotEmpty() && it.text.contains(mine) }.thenByDescending { it.week })
        .take(8)
    Column(modifier.background(Palette.panel).verticalScroll(rememberScrollState()).padding(vertical = 6.dp)) {
        if (decisions.isNotEmpty() || o?.pauseInfo != null) {
            RailHeading("Needs you", decisions.size)
            o?.pauseInfo?.let { RailLine(it.title, it.detail, Palette.warning) { onOpen(Place.Inbox) } }
            decisions.forEach { e -> RailLine(e.title, if (e.deadline > 0u) "Decide by ${ResultBuckets.weekLabel(e.deadline.toInt())}" else "Decision", Palette.warning) { onOpen(Place.Inbox) } }
        }
        // A decision already shows under Needs you; do not list it twice.
        val alerts = o?.alerts.orEmpty().filter { a -> decisions.none { d -> a.contains(d.title) } }
        if (alerts.isNotEmpty()) {
            RailHeading("Company", alerts.size)
            alerts.forEach { a -> RailLine(a, null, Palette.gold) { onOpen(Place.Overview) } }
        }
        if (store.unreadLetters > 0) {
            RailHeading("Letters", store.unreadLetters)
            store.letters.filter { !it.read }.take(4).forEach { l -> RailLine(l.summary.ifEmpty { l.salutation }, l.sender.name, Palette.primaryText) { onOpen(Place.Inbox) } }
        }
        if (raids.isNotEmpty()) {
            RailHeading("Raids on you", raids.size)
            raids.forEach { r ->
                val tone = if (r.playerDefender && r.success) Palette.negative else if (r.playerAttacker && r.success) Palette.positive else Palette.secondary
                val other = if (r.playerDefender) r.attacker else r.defender
                val firm = store.stocks.firstOrNull { it.name == other }?.firm
                RailLine(if (r.success) "${r.attacker} took ${r.blocks} ${if (r.blocks == 1u) "plant" else "plants"} from ${r.defender}" else "${r.attacker}'s raid on ${r.defender} failed", "${ResultBuckets.weekLabel(r.week.toInt())} · ${Sectors.name(r.industry)}", tone) { firm?.let(onRival) ?: onOpen(Place.League) }
            }
        }
        if (moves.isNotEmpty()) {
            RailHeading("Rival moves", moves.size)
            moves.forEach { m -> RailLine("${m.firmName}: ${m.text}", ResultBuckets.weekLabel(m.week.toInt()), Palette.primaryText) { onRival(m.firm) } }
        }
    }
}

@Composable
private fun RailHeading(title: String, count: Int) {
    Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, top = 8.dp, bottom = 3.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title.uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 10.sp, modifier = Modifier.weight(1f))
        Text("$count", color = Palette.secondary, style = GridFigures, fontSize = 10.sp)
    }
}

@Composable
private fun RailLine(text: String, note: String?, tone: Color, onClick: () -> Unit) {
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Row(
        Modifier.fillMaxWidth().background(if (hovered) Palette.panelRaised else Color.Transparent).hoverable(hover)
            .pointerHoverIcon(PointerIcon.Hand).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 3.dp),
    ) {
        Box(Modifier.padding(top = 4.dp).width(4.dp).height(4.dp).background(tone))
        Spacer(Modifier.width(6.dp))
        Column(Modifier.weight(1f)) {
            Text(text, color = Palette.primaryText, fontSize = 11.sp, maxLines = 2, overflow = TextOverflow.Ellipsis, lineHeight = 13.sp)
            if (!note.isNullOrEmpty()) Text(note, color = Palette.secondary, fontSize = 10.sp, maxLines = 1)
        }
    }
}

// MARK: Pieces shared by the screens

/** A row of headline figures above a screen: label over value, tabular, no cards. */
@Composable
fun FigureLine(vararg figures: Triple<String, String, Color>) {
    Row(Modifier.fillMaxWidth().background(Palette.panel).border(1.dp, Palette.stroke).padding(horizontal = 6.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(18.dp)) {
        figures.forEach { (label, value, tone) ->
            Explain(Words.about(label)) {
                Column(Modifier.padding(horizontal = 4.dp)) {
                    Text(label.uppercase(), color = Palette.secondary, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp, maxLines = 1)
                    Text(value, color = tone, style = GridFigures, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
                }
            }
        }
    }
}

/** A titled side pane that scrolls on its own: the detail half of a split. */
@Composable
fun DetailPane(title: String, modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.border(1.dp, Palette.stroke).background(Palette.panel)) {
        Text(
            title.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
            modifier = Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 8.dp, vertical = 5.dp),
        )
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(10.dp), verticalArrangement = Arrangement.spacedBy(8.dp), content = content)
    }
}


private fun gridPct(bps: Long): String = "%.1f%%".format(bps / 100.0)
private fun tint(v: Long): Color = if (v > 0) Palette.positive else if (v < 0) Palette.negative else Palette.secondary

// MARK: Company overview

/**
 * The boardroom: headline figures, every division and product line as sortable grids on the left,
 * results by period and the selected division's operations on the right.
 */
@Composable
fun CompanyOverviewScreen(store: GameStore, divisionRows: List<DivisionDto> = store.divisions, lineRows: List<ProductLineDto> = store.productLines) {
    val firm = store.selectedFirmDto ?: run { Box(Modifier.fillMaxSize().padding(16.dp)) { FoundCompanyPanel(store) }; return }
    var division by remember { mutableStateOf(divisionRows.firstOrNull()?.let { it.region + "/" + it.industry }) }
    var editing by remember { mutableStateOf<ProductLineDto?>(null) }
    val cur = firm.currency
    val last = store.history.lastOrNull()
    Column(Modifier.fillMaxSize().padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FigureLine(
            Triple("Company cash", Format.compact(firm.cashCents, cur), Palette.primaryText),
            Triple("Equity", Format.compact(firm.equityCents, cur), Palette.primaryText),
            Triple("Fair value", Format.compact(firm.fairValueCents, Format.numeraire), Palette.primaryText),
            Triple("Share price", Format.price(firm.markCents, Format.numeraire), Palette.primaryText),
            Triple("Your votes", "%.1f%%".format(firm.investorVotesBps / 100.0), Palette.gold),
            Triple("Credit", firm.grade, Palette.primaryText),
            Triple("Plants", "${firm.facilities}", Palette.primaryText),
            Triple("Divisions", "${divisionRows.size}", Palette.primaryText),
            Triple("Revenue, last period", Format.compact(last?.revenueCents ?: 0, cur), Palette.primaryText),
            Triple("Net income, last period", Format.compact(last?.netIncomeCents ?: 0, cur), tint(last?.netIncomeCents ?: 0)),
        )
        Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // Left: the tables. A table is as tall as its rows, up to 38 percent of the page, and the
            // room it does not use goes to the results table below, which scrolls when it is long.
            BoxWithConstraints(Modifier.weight(1.3f).fillMaxHeight()) {
                val cap = maxHeight * 0.38f
                val divHeight = minOf(gridNaturalHeight(divisionRows.size), cap)
                val lineHeight = minOf(gridNaturalHeight(lineRows.size), cap)
                Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    val opsAll = rememberEngine(store, store.overview?.tick, divisionRows.size) { h -> divisionRows.associate { d -> (d.region + "/" + d.industry) to h.divisionOps(firm.handle, d.region, d.industry) } }.orEmpty()
                    DataGrid(
                        "Divisions", divisionRows, divisionColumns(opsAll, cur), key = { it.region + "/" + it.industry },
                        views = GridViews("divisions", DivisionViews, scopes = divisionScopes(divisionRows)),
                        modifier = Modifier.fillMaxWidth().height(divHeight),
                        filterText = { "${Sectors.name(it.industry)} ${it.regionName} ${it.strategyName}" },
                        selected = division, onSelect = { division = it.region + "/" + it.industry }, onOpen = { store.showDivision(it) },
                        breakdown = { d -> "${Sectors.name(d.industry)} in ${d.regionName}: ${d.blocks} plants, ${gridPct(d.shareBps.toLong())} of the market, ${d.strategyName}, " +
                            (if (d.stockWeeks > 0) "${d.stockWeeks} weeks of stock held" else "the company's usual ${d.stockWeeksDefault} weeks of stock") + ", " +
                            (if (d.serves.isEmpty()) "ships wherever the best lane is" else "serves ${d.serves.size} regions") },
                        initialSort = "share",
                    )
                    DataGrid(
                        "Product lines", lineRows, productColumns(), key = { it.product },
                        modifier = Modifier.fillMaxWidth().height(lineHeight),
                        filterText = { it.name }, onSelect = { editing = it }, onOpen = { editing = it },
                        breakdown = { l -> "${l.name}: price ${Format.money(l.priceCents)}, market ${Format.money(l.marketCents)}, normal ${Format.money(l.referenceCents)}, cost ${Format.money(l.unitCostCents)}; made ${Format.count(l.madeLastWeekLots)}, sold ${Format.count(l.soldLastWeekLots)}, in stock ${Format.count(l.stockLots)} lots" },
                        initialSort = "made",
                    )
                    val periods = store.history.takeLast(24)
                    DataGrid(
                        "Results by period", periods, periodColumns(cur), key = { it.firstWeek },
                        modifier = Modifier.fillMaxWidth().weight(1f),
                        filterText = { ResultBuckets.weekLabel(it.firstWeek.toInt()) },
                        breakdown = { p -> "From ${ResultBuckets.weekLabel(p.firstWeek.toInt())}, ${p.weeks} weeks: revenue ${Format.compact(p.revenueCents, cur)}, goods ${Format.compact(p.costOfGoodsCents, cur)}, wages ${Format.compact(p.wagesCents, cur)}, running ${Format.compact(p.operatingExpenseCents, cur)}, wear ${Format.compact(p.depreciationCents, cur)}, interest ${Format.compact(p.netInterestCents, cur)}, tax ${Format.compact(p.taxCents, cur)}" },
                        initialSort = "week",
                    )
                }
            }
            // Right: the picked division's operations take the whole column.
            val d = divisionRows.firstOrNull { it.region + "/" + it.industry == division }
            DetailPane(d?.let { "${Sectors.name(it.industry)} · ${it.regionName}" } ?: "Division", Modifier.weight(1f).fillMaxHeight()) {
                if (d == null) Text("Pick a division on the left.", color = Palette.secondary, fontSize = 12.sp)
                else {
                    val me = firm.handle
                    val ops = rememberEngine(store, d.region, d.industry, store.overview?.tick) { h -> h.divisionOps(me, d.region, d.industry) }
                    if (ops != null) DivisionOpsView(ops)
                    GridButton("Open the full division report", false) { store.showDivision(d) }
                }
            }
        }
    }
    editing?.let { line -> PostureDialog(store, line) { editing = null } }
}

private fun divisionColumns(ops: Map<String, uniffi.cw_ffi.DivisionOpsDto?>, cur: String): List<GridColumn<DivisionDto>> {
    fun o(d: DivisionDto) = ops[d.region + "/" + d.industry]
    return listOf(
        GridColumn("name", "Division", null, numeric = false, text = { "${Sectors.name(it.industry)} · ${it.regionName}" }, bold = true),
        GridColumn("sector", "Sector", 110.dp, numeric = false, text = { Sectors.name(it.industry) }, tone = { Palette.secondary }, topic = "About", dropOrder = 6),
        GridColumn("region", "Region", 110.dp, numeric = false, text = { it.regionName }, tone = { Palette.secondary }, topic = "About", dropOrder = 6),
        GridColumn("strategy", "Strategy", 150.dp, numeric = false, text = { it.strategyName + if (it.retooling) " (retooling)" else "" }, tone = { if (it.retooling) Palette.warning else Palette.secondary }, topic = "About", dropOrder = 2),
        GridColumn("plants", "Plants", 56.dp, text = { "${it.blocks}" }, sort = { it.blocks.toLong() }, total = true, topic = "Operations"),
        GridColumn("share", "Share", 84.dp, text = { gridPct(it.shareBps.toLong()) }, sort = { it.shareBps.toLong() }, bar = { it.shareBps / 10_000f }, tone = { Palette.gold }, scale = 100.0, topic = "Market"),
        // The engine reports 0 weeks for "use the company's usual cover" and no regions for "ship wherever the best lane is": neither is a real zero.
        GridColumn("stock", "Stock wks", 70.dp, text = { if (it.stockWeeks > 0) "${it.stockWeeks}" else DASH }, sort = { it.stockWeeks.toLong() }, tone = { if (it.stockWeeks > 0) Palette.primaryText else Palette.secondary }, tip = "Weeks of inputs this division keeps in store. A dash means the company's usual cover.", topic = "Operations"),
        GridColumn("serves", "Serves", 60.dp, text = { if (it.serves.isEmpty()) DASH else "${it.serves.size}" }, sort = { it.serves.size.toLong() }, tone = { if (it.serves.isEmpty()) Palette.secondary else Palette.primaryText }, tip = "Regions this division ships to. A dash means it ships wherever the best lane is.", topic = "Operations"),
        GridColumn("made", "Made", 64.dp, text = { d -> o(d)?.let { Format.count(it.madeMilli / 1000) } ?: DASH }, sort = { d -> o(d)?.madeMilli?.div(1000) ?: Long.MIN_VALUE }, total = true, topic = "Operations", tip = "Lots made last week."),
        GridColumn("sold", "Sold", 64.dp, text = { d -> o(d)?.let { Format.count(it.soldMilli / 1000) } ?: DASH }, sort = { d -> o(d)?.soldMilli?.div(1000) ?: Long.MIN_VALUE }, total = true, topic = "Operations", tip = "Lots sold last week."),
        GridColumn("util", "Running", 70.dp, text = { d -> o(d)?.let { gridPct(it.utilisationBps) } ?: DASH }, sort = { d -> o(d)?.utilisationBps ?: Long.MIN_VALUE }, bar = { d -> (o(d)?.utilisationBps ?: 0) / 10_000f }, scale = 100.0, topic = "Operations", tip = "How much of its capacity the division used last week."),
        GridColumn("price", "Price", 80.dp, text = { d -> o(d)?.let { Format.money(it.priceCents) } ?: DASH }, sort = { d -> o(d)?.priceCents ?: Long.MIN_VALUE }, scale = 100.0, topic = "Money"),
        GridColumn("cost", "Unit cost", 80.dp, text = { d -> o(d)?.let { Format.money(it.unitCostCents) } ?: DASH }, sort = { d -> o(d)?.unitCostCents ?: Long.MIN_VALUE }, scale = 100.0, topic = "Money"),
        GridColumn("margin", "Margin", 80.dp, text = { d -> o(d)?.let { Format.signed(it.marginPerLotCents) } ?: DASH }, sort = { d -> o(d)?.marginPerLotCents ?: Long.MIN_VALUE }, tone = { d -> tint(o(d)?.marginPerLotCents ?: 0) }, scale = 100.0, topic = "Money", tip = "Price less unit cost, per lot."),
        GridColumn("weekly", "Margin, week", 92.dp, text = { d -> o(d)?.let { Format.compact(it.marginPerLotCents * it.soldMilli / 1000, cur) } ?: DASH }, sort = { d -> o(d)?.let { it.marginPerLotCents * it.soldMilli / 1000 } ?: Long.MIN_VALUE },
            tone = { d -> tint(o(d)?.marginPerLotCents ?: 0) }, scale = 100.0, total = true, fmt = { Format.compact(it, cur) }, topic = "Money", tip = "Margin per lot times lots sold last week: what the division made over its unit costs."),
    )
}

private fun divisionScopes(rows: List<DivisionDto>): List<ScopeOption<DivisionDto>> =
    rows.map { it.industry }.distinct().sortedBy { Sectors.order.indexOf(it) }.map { ind -> ScopeOption<DivisionDto>("sector:$ind", Sectors.name(ind), "Sector", Sectors.color(ind)) { it.industry == ind } } +
        rows.map { it.region to it.regionName }.distinct().sortedBy { it.second }.map { (id, name) -> ScopeOption<DivisionDto>("region:$id", name, "Region") { it.region == id } }

/** Column keys for a built-in view: "price" shows the figure, "price.Percent" its change in percent over the timeframe. */
internal fun cols(vararg keys: String): List<ColumnSpec> = keys.map { k -> k.split('.').let { ColumnSpec(it[0], if (it.size > 1) ColumnMode.valueOf(it[1]) else ColumnMode.Value) } }

internal val DivisionViews = listOf(
    ViewState("Overview", cols("name", "strategy", "plants", "share", "util", "weekly"), sort = "share"),
    ViewState("Operations", cols("name", "strategy", "plants", "made", "sold", "util", "stock", "serves"), sort = "made"),
    ViewState("Money", cols("name", "plants", "price", "cost", "margin", "weekly", "util", "share"), sort = "weekly"),
)

private fun productColumns(): List<GridColumn<ProductLineDto>> = listOf(
    GridColumn("name", "Product", null, numeric = false, text = { it.name }, bold = true),
    GridColumn("price", "Price", 80.dp, text = { Format.money(it.priceCents) }, sort = { it.priceCents }),
    GridColumn("market", "Market", 80.dp, text = { Format.money(it.marketCents) }, sort = { it.marketCents }, tone = { Palette.secondary }),
    GridColumn("cost", "Cost", 80.dp, text = { if (it.unitCostCents > 0) Format.money(it.unitCostCents) else "n/a" }, sort = { it.unitCostCents }),
    GridColumn("margin", "Margin", 80.dp, text = { if (it.unitCostCents > 0) Format.signed(it.priceCents - it.unitCostCents) else "" }, sort = { it.priceCents - it.unitCostCents }, tone = { tint(it.priceCents - it.unitCostCents) }),
    GridColumn("made", "Made", 64.dp, text = { Format.count(it.madeLastWeekLots) }, sort = { it.madeLastWeekLots }),
    GridColumn("sold", "Sold", 70.dp, text = { if (it.madeLastWeekLots > 0) "${it.soldLastWeekLots * 100 / it.madeLastWeekLots}%" else "" }, sort = { if (it.madeLastWeekLots > 0) it.soldLastWeekLots * 100 / it.madeLastWeekLots else -1 },
        bar = { if (it.madeLastWeekLots > 0) it.soldLastWeekLots.toFloat() / it.madeLastWeekLots else 0f }, tip = "The share of last week's output that sold."),
    GridColumn("stock", "Stock", 64.dp, text = { Format.count(it.stockLots) }, sort = { it.stockLots }),
    GridColumn("posture", "Posture", 110.dp, numeric = false, text = { postureLabel(it) }, tone = { Palette.secondary }),
)

private fun periodColumns(cur: String): List<GridColumn<PeriodDto>> = listOf(
    GridColumn("week", "Period", null, numeric = false, text = { ResultBuckets.weekLabel(it.firstWeek.toInt()) }, sort = { it.firstWeek.toLong() }, bold = true),
    GridColumn("revenue", "Revenue", 82.dp, text = { Format.compact(it.revenueCents, cur) }, sort = { it.revenueCents }),
    GridColumn("goods", "Goods", 76.dp, text = { Format.compact(it.costOfGoodsCents, cur) }, sort = { it.costOfGoodsCents }, tone = { Palette.secondary }),
    GridColumn("wages", "Wages", 76.dp, text = { Format.compact(it.wagesCents, cur) }, sort = { it.wagesCents }, tone = { Palette.secondary }),
    GridColumn("net", "Net income", 84.dp, text = { Format.compact(it.netIncomeCents, cur) }, sort = { it.netIncomeCents }, tone = { tint(it.netIncomeCents) }),
    GridColumn("cash", "Cash", 80.dp, text = { Format.compact(it.cashCents, cur) }, sort = { it.cashCents }),
    GridColumn("equity", "Equity", 80.dp, text = { Format.compact(it.equityCents, cur) }, sort = { it.equityCents }, shown = true),
)

// MARK: Markets: goods

/** Every good in one grid, with the selected good's depth (sellers, buyers, drivers) beside it. */
@Composable
fun GoodsScreen(store: GameStore, startPick: UInt? = null) {
    val goods = store.commodities
    var pick by remember { mutableStateOf(startPick ?: goods.maxByOrNull { it.soldLots }?.product) }
    val shares = rememberEngine(store, store.overview?.tick, goods.size) { h -> goods.associate { g -> g.product to (h.goodDepth(g.product)?.yourShareBps ?: 0L) } }.orEmpty()
    BoxWithConstraints(Modifier.fillMaxSize().padding(8.dp)) {
        val wide = maxWidth >= 900.dp
        Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DataGrid(
                "Goods", goods, goodsColumns(shares), key = { it.product },
                views = GridViews("goods", GoodsViews, mine = { it.yours || (shares[it.product] ?: 0L) > 0 }, scopes = goodsScopes(goods)),
                modifier = Modifier.weight(1f).fillMaxHeight(),
                filterText = { "${it.name} ${Sectors.name(it.sector)}" },
                selected = pick, onSelect = { pick = it.product }, onOpen = if (wide) null else { g: CommodityDto -> pick = g.product },
                breakdown = { g ->
                    "${g.name}: ${Format.money(g.priceCents)} a lot, normal ${Format.money(g.referenceCents)}. Made ${Format.count(g.madeLots)}, sold ${Format.count(g.soldLots)}, " +
                        "imported ${Format.count(g.importedLots)}, exported ${Format.count(g.exportedLots)}, in stock ${Format.count(g.stockLots)}. Your share ${gridPct(shares[g.product] ?: 0)}"
                },
                initialSort = "sold",
            )
            if (wide) {
                val g = goods.firstOrNull { it.product == pick }
                DetailPane(g?.name ?: "Good", Modifier.width(460.dp).fillMaxHeight()) {
                    if (g == null) Text("Pick a good on the left.", color = Palette.secondary, fontSize = 12.sp) else GoodDepthSection(store, g.product)
                }
            }
        }
    }
}

private fun goodsColumns(shares: Map<UInt, Long>): List<GridColumn<CommodityDto>> = listOf(
    GridColumn("name", "Good", null, minWidth = 190.dp, numeric = false, text = { it.name + if (it.yours) " *" else "" }, tone = { if (it.yours) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("sector", "Sector", 120.dp, numeric = false, text = { if (it.sector == "other") DASH else Sectors.name(it.sector) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("price", "Price", 84.dp, text = { Format.money(it.priceCents) }, sort = { it.priceCents }, series = { it.historyCents }, fmt = { Format.money(it) }, scale = 100.0, topic = "Price",
        tip = "The price of a lot this week. Its change, percent and trend use the weekly closing prices the game keeps (one year)."),
    GridColumn("vsnormal", "Vs normal", 76.dp, text = { if (it.referenceCents > 0) changeText((it.priceCents - it.referenceCents) * 10_000 / it.referenceCents) else "" },
        sort = { if (it.referenceCents > 0) (it.priceCents - it.referenceCents) * 10_000 / it.referenceCents else 0 }, tone = { tint(it.priceCents - it.referenceCents) }, tip = "The price now against the good's normal price.", dropOrder = 1, scale = 100.0, topic = "Price"),
    GridColumn("week", "Week", 62.dp, text = { changeText(it.changeWeekBps) }, sort = { it.changeWeekBps }, tone = { changeTint(it.changeWeekBps) }, dropOrder = 2, scale = 100.0, topic = "Price", tip = "The price change over the last week, whatever the timeframe."),
    GridColumn("made", "Made", 72.dp, text = { Format.count(it.madeLots) }, sort = { it.madeLots }, total = true, topic = "Supply", tip = "Lots made last week. The game keeps no history of it, so it has no change or trend."),
    GridColumn("sold", "Sold", 72.dp, text = { Format.count(it.soldLots) }, sort = { it.soldLots }, total = true, topic = "Supply", tip = "Lots sold last week. The game keeps no history of it, so it has no change or trend."),
    GridColumn("through", "Sold %", 84.dp, text = { if (it.madeLots > 0) "${it.soldLots * 100 / it.madeLots}%" else "" }, sort = { if (it.madeLots > 0) it.soldLots * 100 / it.madeLots else -1 },
        bar = { if (it.madeLots > 0) (it.soldLots.toFloat() / it.madeLots) else 0f }, tip = "Sold as a share of what was made.", topic = "Supply"),
    GridColumn("stock", "Stock", 70.dp, text = { Format.count(it.stockLots) }, sort = { it.stockLots }, total = true, topic = "Supply"),
    GridColumn("imported", "Imported", 72.dp, text = { if (it.importedLots > 0) Format.count(it.importedLots) else "" }, sort = { it.importedLots }, tone = { Palette.positive }, dropOrder = 4, total = true, topic = "Trade"),
    GridColumn("exported", "Exported", 72.dp, text = { if (it.exportedLots > 0) Format.count(it.exportedLots) else "" }, sort = { it.exportedLots }, tone = { Palette.secondary }, dropOrder = 4, total = true, topic = "Trade"),
    GridColumn("yours", "Your share", 80.dp, text = { (shares[it.product] ?: 0L).let { s -> if (s > 0) gridPct(s) else "" } }, sort = { shares[it.product] ?: 0L },
        bar = { (shares[it.product] ?: 0L) / 10_000f }, tone = { Palette.gold }, tip = "Your share of what was sold of this good.", scale = 100.0, topic = "You"),
)

private fun goodsScopes(goods: List<CommodityDto>): List<ScopeOption<CommodityDto>> =
    goods.map { it.sector }.filter { it != "other" }.distinct().sortedBy { Sectors.order.indexOf(it) }.map { sec -> ScopeOption<CommodityDto>("sector:$sec", Sectors.name(sec), "Sector", Sectors.color(sec)) { it.sector == sec } }

internal val GoodsViews = listOf(
    ViewState("Overview", cols("name", "sector", "price", "price.Percent", "price.Spark", "made", "sold", "through", "stock", "yours"), sort = "sold"),
    ViewState("Prices", cols("name", "sector", "price", "vsnormal", "week", "price.Change", "price.Percent", "price.Spark"), sort = "price.Percent"),
    ViewState("Supply", cols("name", "sector", "made", "sold", "through", "stock", "imported", "exported"), sort = "made"),
    ViewState("My business", cols("name", "sector", "price", "price.Percent", "yours", "sold", "stock"), sort = "yours", filters = listOf(Filter(FilterOp.Mine))),
    ViewState("Trade", cols("name", "sector", "price", "price.Percent", "imported", "exported", "made", "sold"), sort = "imported"),
)

// MARK: Rivals: league table

/** Every company ranked, with the picked company's dossier and your head to head beside it. */
@Composable
fun LeagueScreen(store: GameStore, pick: ULong?, onPick: (ULong) -> Unit) {
    val rows = rememberEngine(store, store.overview?.tick) { it.rivalsBoard() }.orEmpty()
    val me = store.selectedFirm
    val ranked = rows.sortedByDescending { it.marketCapCents }
    val rank = ranked.mapIndexed { i, r -> r.firm to (i + 1) }.toMap()
    val maxCap = ranked.firstOrNull()?.marketCapCents?.coerceAtLeast(1) ?: 1
    val stocks = store.stocks.associateBy { it.firm }
    val chosen = pick ?: ranked.firstOrNull { !it.yours }?.firm
    // Four-week results for every company (up to five years) and, for the Against me view, the goods each shares with you.
    val periods = rememberEngine(store, store.overview?.tick, rows.size) { h -> rows.associate { it.firm to h.history(it.firm) } }.orEmpty()
    val contest = rememberEngine(store, store.overview?.tick, rows.size, me) { h -> if (me == null) emptyMap<ULong, List<HeadToHeadDto>>() else rows.filter { !it.yours }.associate { it.firm to h.headToHead(me, it.firm) } }.orEmpty()
    val raidsOnYou = store.recentRaids.filter { it.playerDefender }.groupingBy { it.attacker }.eachCount()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "League table", rows, leagueColumns(rank, maxCap, stocks, periods, contest, raidsOnYou), key = { it.firm },
            views = GridViews("league", LeagueViews, mine = { it.yours }, scopes = leagueScopes(rows)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.name} ${Sectors.name(it.industry)}" },
            selected = chosen, onSelect = { onPick(it.firm) },
            breakdown = { r ->
                "${r.name}: revenue ${Format.compact(r.revenueCents)}, profit ${Format.compact(r.profitCents)}, cash ${Format.compact(r.cashCents)}, debt ${Format.compact(r.debtCents)}, " +
                    "value ${Format.compact(r.marketCapCents)}, ${r.divisions} divisions" + (stocks[r.firm]?.takeIf { it.yourShares > 0 }?.let { ", you hold ${Format.count(it.yourShares)} shares" } ?: "")
            },
            initialSort = "cap",
        )
        val them = rows.firstOrNull { it.firm == chosen }
        DetailPane(them?.let { "Dossier: ${it.name}" } ?: "Dossier", Modifier.width(560.dp).fillMaxHeight()) {
            if (them == null || chosen == null) {
                Text("Pick a company on the left.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            if (me != null && !them.yours) {
                val h2h = rememberEngine(store, chosen, me, store.overview?.tick) { h -> h.headToHead(me, chosen) }.orEmpty()
                if (h2h.isNotEmpty()) {
                    DataGrid(
                        "Head to head", h2h, headToHeadColumns(), key = { it.product },
                        modifier = Modifier.fillMaxWidth().height((70 + 24 * h2h.size.coerceAtMost(8)).dp),
                        filterText = { it.name }, initialSort = "theirs",
                    )
                }
            }
            val page = rememberEngine(store, chosen, store.overview?.tick) { h -> h.companyPage(chosen) }
            if (page != null) CompanyPageBody(store, page, onOpenFirm = onPick, onTrade = null)
        }
    }
}

private fun leagueColumns(
    rank: Map<ULong, Int>, maxCap: Long, stocks: Map<ULong, uniffi.cw_ffi.StockDto>, periods: Map<ULong, List<PeriodDto>>,
    contest: Map<ULong, List<HeadToHeadDto>>, raidsOnYou: Map<String, Int>,
): List<GridColumn<RivalRowDto>> = listOf(
    // The company's name leads: its rank and sector badge sit in front of it. Your own company is gold.
    GridColumn("name", "Company", null, minWidth = 210.dp, numeric = false, text = { it.name }, tone = { if (it.yours) Palette.gold else Palette.primaryText }, bold = true,
        lead = { r ->
            Text("${rank[r.firm] ?: ""}", color = Palette.secondary, style = GridFigures, textAlign = androidx.compose.ui.text.style.TextAlign.End, modifier = Modifier.width(20.dp), maxLines = 1)
            Spacer(Modifier.width(6.dp))
            SectorBadge(r.industry, 14)
        }),
    GridColumn("sector", "Sector", 118.dp, numeric = false, text = { Sectors.name(it.industry) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("revenue", "Revenue", 84.dp, text = { Format.compact(it.revenueCents) }, sort = { it.revenueCents }, series = { r -> periods[r.firm].orEmpty().map { it.revenueCents } }, step = 4,
        fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Results", tip = "Revenue. Its change, percent and trend compare four-week results, which the game keeps for five years."),
    GridColumn("profit", "Profit", 80.dp, text = { Format.compact(it.profitCents) }, sort = { it.profitCents }, tone = { tint(it.profitCents) }, series = { r -> periods[r.firm].orEmpty().map { it.netIncomeCents } }, step = 4,
        fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Results", tip = "Profit after interest and tax. Its change and trend compare four-week results."),
    GridColumn("margin", "Margin", 62.dp, text = { it.netMarginBps?.let(::gridPct) ?: "" }, sort = { it.netMarginBps ?: Long.MIN_VALUE }, tone = { tint(it.netMarginBps ?: 0) }, dropOrder = 2, scale = 100.0, topic = "Results"),
    GridColumn("cash", "Cash", 78.dp, text = { Format.compact(it.cashCents) }, sort = { it.cashCents }, series = { r -> periods[r.firm].orEmpty().map { it.cashCents } }, step = 4, fmt = { Format.compact(it) }, dropOrder = 3, scale = 100.0, total = true, topic = "Balance sheet"),
    GridColumn("debt", "Debt", 78.dp, text = { Format.compact(it.debtCents) }, sort = { it.debtCents }, tone = { Palette.secondary }, dropOrder = 4, scale = 100.0, total = true, topic = "Balance sheet"),
    GridColumn("price", "Share", 72.dp, text = { Format.price(it.sharePriceCents) }, sort = { it.sharePriceCents }, series = { stocks[it.firm]?.historyCents.orEmpty() }, fmt = { Format.price(it) }, scale = 100.0, topic = "Shares",
        tip = "The share price. Its change, percent and trend use the weekly closing prices the game keeps (one year)."),
    GridColumn("week", "Week", 60.dp, text = { stocks[it.firm]?.let { s -> changeText(s.changeWeekBps) } ?: "" }, sort = { stocks[it.firm]?.changeWeekBps ?: 0 }, tone = { changeTint(stocks[it.firm]?.changeWeekBps ?: 0) }, scale = 100.0, topic = "Shares"),
    GridColumn("cap", "Value", 96.dp, text = { Format.compact(it.marketCapCents) }, sort = { it.marketCapCents }, bar = { it.marketCapCents.toFloat() / maxCap }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Shares",
        tip = "Market value: share price times shares issued."),
    GridColumn("fair", "Fair price", 76.dp, text = { stocks[it.firm]?.let { s -> Format.price(s.fairCents) } ?: DASH }, sort = { stocks[it.firm]?.fairCents ?: Long.MIN_VALUE }, scale = 100.0, topic = "Shares", tip = "What the market's own valuation says a share is worth."),
    GridColumn("float", "Float", 62.dp, text = { stocks[it.firm]?.let { s -> gridPct(s.floatBps) } ?: DASH }, sort = { stocks[it.firm]?.floatBps ?: Long.MIN_VALUE }, scale = 100.0, topic = "Shares", tip = "The share of the company held by outside investors, which anyone can buy."),
    GridColumn("divisions", "Divs", 44.dp, text = { "${it.divisions}" }, sort = { it.divisions.toLong() }, dropOrder = 5, total = true, topic = "About"),
    GridColumn("held", "You hold", 72.dp, text = { stocks[it.firm]?.yourShares?.takeIf { s -> s > 0 }?.let { s -> Format.count(s) } ?: "" }, sort = { stocks[it.firm]?.yourShares ?: 0 }, tone = { Palette.gold }, dropOrder = 6, topic = "You"),
    GridColumn("overlap", "Shared goods", 92.dp, text = { r -> contest[r.firm]?.size?.takeIf { it > 0 }?.toString() ?: "" }, sort = { r -> contest[r.firm]?.size?.toLong() ?: 0 }, topic = "Against you", tip = "Goods both of you sold last week."),
    GridColumn("theirlots", "Their lots", 80.dp, text = { r -> contest[r.firm]?.sumOf { it.theirLots }?.takeIf { it > 0 }?.let(Format::count) ?: "" }, sort = { r -> contest[r.firm]?.sumOf { it.theirLots } ?: 0 }, total = true, topic = "Against you", tip = "What they sold last week of the goods you also sell."),
    GridColumn("raids", "Raids on you", 92.dp, text = { r -> raidsOnYou[r.name]?.toString() ?: "" }, sort = { r -> raidsOnYou[r.name]?.toLong() ?: 0 }, tone = { Palette.negative }, total = true, topic = "Against you", tip = "Their raids on your businesses in the raid log."),
)

private fun leagueScopes(rows: List<RivalRowDto>): List<ScopeOption<RivalRowDto>> =
    rows.map { it.industry }.distinct().sortedBy { Sectors.order.indexOf(it) }.map { ind -> ScopeOption<RivalRowDto>("sector:$ind", Sectors.name(ind), "Sector", Sectors.color(ind)) { it.industry == ind } }

internal val LeagueViews = listOf(
    ViewState("Overview", cols("name", "sector", "revenue", "profit", "margin", "price", "price.Spark", "cap"), sort = "cap"),
    ViewState("Finance", cols("name", "revenue", "revenue.Percent", "profit", "profit.Change", "margin", "cash", "debt"), sort = "revenue"),
    ViewState("Shares", cols("name", "price", "price.Percent", "price.Spark", "cap", "fair", "float", "held"), sort = "cap"),
    ViewState("Against me", cols("name", "sector", "overlap", "theirlots", "raids", "cap", "profit"), sort = "theirlots", filters = listOf(Filter(FilterOp.Above, "overlap", number = 0.0))),
)

private fun headToHeadColumns(): List<GridColumn<HeadToHeadDto>> = listOf(
    GridColumn("name", "Good", null, numeric = false, text = { it.name }, bold = true),
    GridColumn("yours", "Your lots", 74.dp, text = { Format.count(it.yourLots) }, sort = { it.yourLots }),
    GridColumn("theirs", "Their lots", 74.dp, text = { Format.count(it.theirLots) }, sort = { it.theirLots }),
    GridColumn("yshare", "You", 70.dp, text = { gridPct(it.yourShareBps) }, sort = { it.yourShareBps }, bar = { it.yourShareBps / 10_000f }, tone = { Palette.gold }),
    GridColumn("tshare", "Them", 70.dp, text = { gridPct(it.theirShareBps) }, sort = { it.theirShareBps }, bar = { it.theirShareBps / 10_000f }, tone = { Palette.negative }),
)
