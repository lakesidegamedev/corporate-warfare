package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The five entries of the phone's bottom bar. Each opens a list of the places in its groups. */
enum class PhoneTab(val title: String, val icon: String, val groups: List<String>) {
    Map("Map", "map", emptyList()),
    Company("Company", "office", listOf("Company")),
    Markets("Markets", "markets", listOf("Markets")),
    Rivals("Rivals", "target", listOf("Rivals")),
    More("More", "menu", listOf("World", "You")),
}

/** Where the player is on a phone: a bottom bar entry, and a place inside it (none means that entry's list). Lives above the shell so Settings can come and go. */
class PhoneNav {
    var tab by mutableStateOf(PhoneTab.Map)
    var place by mutableStateOf<Place?>(null)
    var leaguePick by mutableStateOf<ULong?>(null)
    var goodsPick by mutableStateOf<UInt?>(null)
    /** The bell sheet (alerts), Find, the pull-down of the top strip. */
    var alerts by mutableStateOf(false)
    var find by mutableStateOf(false)
    var pulled by mutableStateOf(false)
    /** An action opened with its fields filled in, or all actions. */
    var action by mutableStateOf<Pair<String, Map<String, String>>?>(null)
    var allActions by mutableStateOf(false)
    /** Detail panes of the open place: shown as a page of their own when a row is tapped. */
    val panes = PhonePanes()

    fun tabOf(p: Place): PhoneTab = PhoneTab.entries.firstOrNull { p.group in it.groups } ?: PhoneTab.More

    /** Opens a place under its own bottom bar entry, closing whatever sheet was over the screen. */
    fun open(p: Place) {
        closeSheets(); panes.open = false
        if (p == Place.Map) { tab = PhoneTab.Map; place = null; return }
        tab = tabOf(p); place = p
        Tutorial.visited(tutorialPage(p))
    }

    fun goTab(t: PhoneTab) { closeSheets(); panes.open = false; tab = t; place = null }
    fun closeSheets() { alerts = false; find = false; pulled = false; action = null; allActions = false }
    val sheetOpen: Boolean get() = alerts || find || action != null || allActions
}

/**
 * The 2.0 layout on a phone: a folded top strip (cash, profit, date, speed and Next week always in view, the rest
 * in a pull-down), a bell for the alert rail, a search icon for Find, and a bottom bar of Map, Company, Markets,
 * Rivals and More. Each bar entry opens its places as a list with the key figure of each, then the place.
 */
@Composable
fun PhoneShell(store: GameStore, nav: PhoneNav, onSettings: () -> Unit, onMenu: () -> Unit) {
    DisposableEffect(Unit) { Tutorial.newLayout = true; onDispose { Tutorial.newLayout = false } }
    // Back goes up one level, one press at a time: a sheet, the pull-down, a place, the bar entry's list, the map.
    // On the map there is nowhere higher, so back opens the game menu rather than leaving the game.
    BackAction {
        when {
            nav.sheetOpen -> nav.closeSheets()
            nav.panes.open -> nav.panes.open = false
            nav.pulled -> nav.pulled = false
            nav.place != null -> nav.place = null
            nav.tab != PhoneTab.Map -> nav.goTab(PhoneTab.Map)
            else -> onMenu()
        }
    }
    LaunchedEffect(Tutorial.requestedPage) {
        val wanted = Tutorial.requestedPage ?: return@LaunchedEffect
        when (wanted) {
            "office" -> nav.open(Place.Overview)
            "research" -> nav.open(Place.Research)
            "markets" -> nav.open(Place.Economy)
            "map" -> nav.open(Place.Regions)
            "stats" -> nav.open(Place.Finance)
            "actions" -> { nav.closeSheets(); nav.find = true }
        }
        Tutorial.requestedPage = null
    }
    Box(Modifier.fillMaxSize().background(Palette.ink)) {
        Column(Modifier.fillMaxSize()) {
            PhoneStrip(store, nav)
            Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
            if (nav.pulled) PhonePullDown(store, nav)
            PauseBanner(store, Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), compact = true)
            TutorialPanel(compact = true, modifier = Modifier.fillMaxWidth(), researchHint = store.research?.let { r -> store.selectedFirmDto?.let { Tutorial.researchHint(it.cashCents, r) } })
            Box(Modifier.weight(1f).fillMaxWidth()) {
                androidx.compose.runtime.CompositionLocalProvider(LocalActions provides { cmd, preset -> nav.action = cmd to preset }, LocalPhonePanes provides nav.panes) {
                    PhoneBody(store, nav, onSettings, onMenu)
                    // A tapped row's detail pane, as a page with a back arrow.
                    if (nav.panes.open && nav.panes.entries.isNotEmpty()) Column(Modifier.fillMaxSize().background(Palette.ink)) {
                        PhoneBackBar("Back to the list") { nav.panes.open = false }
                        nav.panes.entries.toList().forEach { e -> DetailPaneBody(e.title, Modifier.fillMaxWidth().weight(1f)) { e.content?.invoke(this) } }
                    }
                }
                if (SidePanes.items.isNotEmpty()) PhoneSidePanes()
            }
            Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
            PhoneBar(store, nav)
        }
        DivisionDetailHost(store)
        if (nav.alerts) PhoneSheet("Alerts", { nav.alerts = false }) {
            AlertRail(store, Modifier.fillMaxSize(), onOpen = nav::open, onRival = { nav.leaguePick = it; nav.open(Place.League) }, onGood = { nav.goodsPick = it; nav.open(Place.Goods) }, sheet = true)
        }
        if (nav.find || nav.action != null || nav.allActions) PhoneSheet(if (nav.action != null || nav.allActions) "Actions" else "Find", { nav.closeSheets() }, header = false) {
            val a = nav.action
            when {
                a != null -> CommandCenterScreen(store, onClose = { nav.closeSheets() }, initialCommand = a.first, preset = a.second)
                nav.allActions -> CommandCenterScreen(store, onClose = { nav.closeSheets() })
                else -> FindPalette(
                    store, "", onPlace = nav::open, onGood = { nav.goodsPick = it; nav.open(Place.Goods) }, onRival = { nav.leaguePick = it; nav.open(Place.League) },
                    onAction = { cmd -> nav.action = cmd to emptyMap() }, onAllActions = { nav.allActions = true }, onClose = { nav.closeSheets() }, phone = true,
                )
            }
        }
    }
}

// MARK: Body

@Composable
private fun PhoneBody(store: GameStore, nav: PhoneNav, onSettings: () -> Unit, onMenu: () -> Unit) {
    val place = nav.place
    when {
        nav.tab == PhoneTab.Map -> MapScreen(store)
        place == null -> PhoneList(store, nav, onSettings, onMenu)
        else -> key(place, nav.goodsPick, nav.leaguePick) { PhonePlace(store, nav, place) }
    }
}

/** One place on a phone. The three places that are still 1.x desktop pages show the 1.x phone pages instead. */
@Composable
private fun PhonePlace(store: GameStore, nav: PhoneNav, place: Place) {
    Column(Modifier.fillMaxSize()) {
        PhoneBackBar(place.title) { nav.place = null }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (place) {
                Place.Office -> OfficeScreen(store) { page ->
                    when (page) {
                        "actions" -> nav.find = true
                        "research" -> nav.open(Place.Research)
                        "paper" -> nav.open(Place.Paper)
                        "hall" -> nav.open(Place.Hall)
                        "stats" -> nav.open(Place.Finance)
                        "map" -> nav.goTab(PhoneTab.Map)
                        "markets" -> nav.open(Place.Goods)
                        "inbox" -> nav.open(Place.Inbox)
                        "portfolio" -> nav.open(Place.Portfolio)
                        else -> nav.open(Place.Overview)
                    }
                }
                Place.Exchange -> MarketsScreen(store)
                Place.Map -> MapScreen(store)
                else -> PlaceContent(store, place, nav.leaguePick, { nav.leaguePick = it }, nav::open, { nav.leaguePick = it; nav.open(Place.League) }, nav.goodsPick)
            }
        }
    }
}

@Composable
private fun PhoneBackBar(title: String, onBack: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().height(36.dp).background(Palette.panelRaised).clickable(onClick = onBack).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PixelIcon("back", size = 16.dp, tint = Palette.gold, description = "Back")
        Spacer(Modifier.width(8.dp))
        Text(title.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

// MARK: Place lists

/** A bar entry's places as a clean list, each with its key figure on the right. More also holds Settings and the game menu. */
@Composable
private fun PhoneList(store: GameStore, nav: PhoneNav, onSettings: () -> Unit, onMenu: () -> Unit) {
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 8.dp)) {
        nav.tab.groups.forEach { group ->
            Text(group.uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp, modifier = Modifier.padding(start = 12.dp, top = 12.dp, bottom = 4.dp))
            Place.entries.filter { it.group == group && it != Place.Map }.forEach { p ->
                PhoneRow(p.title, phoneNote(p), phoneFigure(store, p)) { nav.open(p) }
            }
        }
        if (nav.tab == PhoneTab.More) {
            Text("THE GAME", color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp, modifier = Modifier.padding(start = 12.dp, top = 16.dp, bottom = 4.dp))
            PhoneRow("Settings", "Layout, currency, display", null, onSettings)
            PhoneRow("Game menu", "Save, resume, back to the main menu", null, onMenu)
        }
    }
}

@Composable
private fun PhoneRow(title: String, note: String, figure: String?, onClick: () -> Unit) {
    Column(Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(title, color = Palette.primaryText, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
                if (note.isNotEmpty()) Text(note, color = Palette.secondary, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            if (figure != null) Text(figure, color = Palette.primaryText, style = GridFigures, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, modifier = Modifier.padding(start = 10.dp))
            Spacer(Modifier.width(8.dp))
            PixelIcon("chevron_right", size = 14.dp, tint = Palette.secondary)
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke.copy(alpha = 0.6f)))
    }
}

/** What a place is for, in a few plain words, under its name in the list. */
internal fun phoneNote(p: Place): String = when (p) {
    Place.Overview -> "Divisions, product lines, results"
    Place.Finance -> "Statements, debt, shares"
    Place.Research -> "Levels and what the next one needs"
    Place.People -> "Board, executives, workforce"
    Place.Plants -> "Every site you run"
    Place.Office -> "The 1.x office page"
    Place.Goods -> "Prices, supply and demand"
    Place.Openings -> "Where a new plant would pay"
    Place.Regions -> "Wages, labour, infrastructure"
    Place.Freight -> "Lanes, prices, reservations"
    Place.Auctions -> "Plants for sale and your listings"
    Place.Exchange -> "Commodities and stocks, 1.x page"
    Place.Map -> ""
    Place.League -> "Every company ranked by value"
    Place.Raids -> "Attacks on markets, yours and theirs"
    Place.Activity -> "What rivals are doing"
    Place.Deals -> "Mergers, stakes and tender offers"
    Place.Economy -> "Cycle, rates and prices"
    Place.Politics -> "Races, laws and tenders"
    Place.Timeline -> "What happened, week by week"
    Place.Paper -> "The week's news"
    Place.Inbox -> "Decisions and letters"
    Place.Portfolio -> "Shares you hold"
    Place.Career -> "Your standing and ambition"
    Place.Hall -> "Earlier careers"
}

/** The key figure beside a place in the list, from what the game already holds. Null when the engine gives no single figure. */
internal fun phoneFigure(store: GameStore, p: Place): String? {
    val o = store.overview
    val cur = o?.primaryCurrency.orEmpty()
    val ranked = store.stocks.sortedByDescending { it.marketCapCents }
    val rank = store.selectedFirmDto?.let { f -> ranked.indexOfFirst { it.firm == f.handle } + 1 } ?: 0
    return when (p) {
        Place.Overview -> o?.let { Format.compact(it.quarterNetIncomeCents, cur) }
        Place.Finance -> o?.let { Format.compact(it.firmCashCents, cur) }
        Place.Research -> store.research?.let { "Level ${it.level}" }
        Place.People -> store.executives?.let { Format.compact(it.weeklyPayCents, cur) + " / wk" }
        Place.Plants -> store.facilities.size.takeIf { it > 0 }?.let { "$it" }
        Place.Goods -> store.commodities.size.takeIf { it > 0 }?.let { "$it" }
        Place.Regions -> store.boards.size.takeIf { it > 0 }?.let { "$it" }
        Place.League -> if (rank > 0) "#$rank of ${ranked.size}" else null
        Place.Raids -> store.recentRaids.count { it.playerDefender || it.playerAttacker }.takeIf { it > 0 }?.let { "$it" }
        Place.Economy -> store.economy?.let { it.phase.replaceFirstChar { c -> c.uppercase() } + " " + "%.2f%%".format(it.policyRateBps / 100.0) }
        Place.Inbox -> store.unreadLetters.takeIf { it > 0 }?.let { "$it new" }
        Place.Portfolio -> o?.let { Format.compact(it.investorNetWorthCents) }
        else -> null
    }
}

// MARK: Strip, pull-down, bell

/** The folded top strip: cash and profit with the three icons on the first line; date, speed and Next week on the second. */
@Composable
private fun PhoneStrip(store: GameStore, nav: PhoneNav) {
    val o = store.overview
    val cur = o?.primaryCurrency.orEmpty()
    val profit = o?.quarterNetIncomeCents ?: 0
    val weeks = o?.quarterWeeks?.toInt() ?: 0
    Column(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 8.dp, vertical = 2.dp)) {
        Row(Modifier.fillMaxWidth().height(46.dp), verticalAlignment = Alignment.CenterVertically) {
            PhoneFigure("Cash", Format.compact(o?.firmCashCents ?: 0, cur), Palette.primaryText, Modifier.weight(1f)) { nav.open(Place.Finance) }
            PhoneFigure(if (weeks >= 13) "Profit, quarter" else "Profit, $weeks wk", Format.compact(profit, cur), if (profit >= 0) Palette.positive else Palette.negative, Modifier.weight(1.3f)) { nav.open(Place.Finance) }
            StripIcon("search", "Find", 0) { nav.closeSheets(); nav.find = true }
            val needs = alertCount(store)
            StripIcon("alert", "Alerts", needs) { val on = !nav.alerts; nav.closeSheets(); nav.alerts = on }
            StripIcon(if (nav.pulled) "chevron_up" else "chevron_down", "More figures", 0) { nav.pulled = !nav.pulled }
        }
        Row(Modifier.fillMaxWidth().height(40.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(o?.weekLabel.orEmpty(), color = Palette.primaryText, style = GridFigures, fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false, modifier = Modifier.padding(start = 4.dp))
            Spacer(Modifier.weight(1f))
            SpeedControl(store)
            Spacer(Modifier.width(8.dp))
            val ready = !store.busy && !store.running
            Text(
                "NEXT WEEK", color = Palette.ink, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, fontSize = 11.sp, maxLines = 1, softWrap = false,
                modifier = Modifier.background(if (ready) Palette.gold else Palette.gold.copy(alpha = 0.4f)).clickable(enabled = ready) { store.advance() }.padding(horizontal = 12.dp, vertical = 11.dp),
            )
        }
    }
}

@Composable
private fun PhoneFigure(label: String, value: String, tone: Color, modifier: Modifier, onClick: () -> Unit) {
    Column(modifier.clickable(onClick = onClick).padding(horizontal = 4.dp, vertical = 2.dp)) {
        Text(label.uppercase(), color = Palette.secondary, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(value, color = tone, style = GridFigures, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

@Composable
private fun StripIcon(icon: String, description: String, count: Int, onClick: () -> Unit) {
    Box(Modifier.size(40.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        PixelIcon(icon, size = 20.dp, tint = if (count > 0) Palette.warning else Palette.primaryText, description = description)
        if (count > 0) Text(
            if (count > 99) "99+" else "$count", color = Palette.ink, style = GridFigures, fontSize = 9.sp, fontWeight = FontWeight.Bold, maxLines = 1,
            modifier = Modifier.align(Alignment.TopEnd).background(Palette.warning).padding(horizontal = 3.dp),
        )
    }
}

/** The pulled-down rest of the strip: share price, rank, net worth, the four-week cash forecast, economy, timeframe and focus. */
@Composable
private fun PhonePullDown(store: GameStore, nav: PhoneNav) {
    val o = store.overview
    val firm = store.selectedFirmDto
    val stock = firm?.let { f -> store.stocks.firstOrNull { it.firm == f.handle } }
    val ranked = store.stocks.sortedByDescending { it.marketCapCents }
    val rank = firm?.let { f -> ranked.indexOfFirst { it.firm == f.handle } + 1 } ?: 0
    val cur = o?.primaryCurrency.orEmpty()
    Column(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 8.dp, vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text((firm?.name ?: "No company").uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Row(Modifier.fillMaxWidth()) {
            stock?.let { s -> PhoneFigure("Share price", Format.price(s.priceCents) + " " + changeText(s.changeWeekBps), changeTint(s.changeWeekBps), Modifier.weight(1f)) { nav.open(Place.League) } }
            if (rank > 0) PhoneFigure("League, by value", "#$rank of ${ranked.size}", if (rank <= 3) Palette.gold else Palette.primaryText, Modifier.weight(1f)) { nav.open(Place.League) }
        }
        Row(Modifier.fillMaxWidth()) {
            PhoneFigure("Net worth", Format.compact(o?.investorNetWorthCents ?: 0), Palette.primaryText, Modifier.weight(1f)) { nav.open(Place.Portfolio) }
            PhoneFigure("Cash in 4 wk", Format.compact(o?.forecastFourWeekCents ?: 0, cur), if ((o?.forecastFourWeekCents ?: 0) >= (o?.firmCashCents ?: 0)) Palette.primaryText else Palette.negative, Modifier.weight(1f)) { nav.open(Place.Finance) }
        }
        store.economy?.let { e ->
            PhoneFigure("Economy", "${e.phase.replaceFirstChar { it.uppercase() }} ${"%.2f%%".format(e.policyRateBps / 100.0)}", economyPhaseTint(e.phase), Modifier.fillMaxWidth()) { nav.open(Place.Economy) }
        }
        Text("TIMEFRAME", color = Palette.secondary, fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.5.sp)
        TimeframeStrip()
        FocusChip()
    }
    Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
}

/** How many things need the player: decisions, alerts, unread letters, fired watch alerts and a pause. The bell's count. */
internal fun alertCount(store: GameStore): Int {
    val o = store.overview
    val decisions = store.events.filter { it.mandatory }
    val alerts = o?.alerts.orEmpty().filter { a -> decisions.none { d -> a.contains(d.title) } }
    return decisions.size + alerts.size + store.unreadLetters + firedWatchAlerts(store).size + if (o?.pauseInfo != null) 1 else 0
}

// MARK: Bottom bar, sheets

@Composable
private fun PhoneBar(store: GameStore, nav: PhoneNav) {
    Row(Modifier.fillMaxWidth().height(58.dp).background(Palette.panel)) {
        PhoneTab.entries.forEach { t ->
            val on = nav.tab == t
            val badge = if (t == PhoneTab.More) store.inboxBadge else 0
            Column(
                Modifier.weight(1f).fillMaxHeight().background(if (on) Palette.panelRaised else Color.Transparent).clickable { nav.goTab(t) },
                horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
            ) {
                Box(Modifier.width(28.dp).height(3.dp).background(if (on) Palette.gold else Color.Transparent))
                Spacer(Modifier.height(4.dp))
                Box {
                    PixelIcon(t.icon, size = 22.dp, tint = if (on) Palette.gold else Palette.secondary)
                    if (badge > 0) Text("$badge", color = Palette.ink, style = GridFigures, fontSize = 9.sp, fontWeight = FontWeight.Bold, modifier = Modifier.align(Alignment.TopEnd).background(Palette.warning).padding(horizontal = 3.dp))
                }
                Text(t.title.uppercase(), color = if (on) Palette.gold else Palette.secondary, fontFamily = Pixel.font, fontSize = 9.sp, maxLines = 1)
            }
        }
    }
}

/** A full-screen sheet over the shell, with a Close button. [header] false lets the content draw its own header. */
@Composable
private fun PhoneSheet(title: String, onClose: () -> Unit, header: Boolean = true, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxSize().background(Palette.ink).clickable(enabled = false) {}) {
        if (header) Row(Modifier.fillMaxWidth().height(44.dp).background(Palette.panelRaised).padding(horizontal = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(title.uppercase(), color = Palette.gold, fontFamily = Pixel.font, fontSize = 13.sp, modifier = Modifier.weight(1f))
            GridButton("Close", false, 11.sp, onClose)
        }
        Box(Modifier.weight(1f).fillMaxWidth()) { content() }
    }
}

/** Figures opened to the side on a desktop (hold a figure) come up as a page on a phone, with a back arrow. */
@Composable
private fun PhoneSidePanes() {
    BackAction { SidePanes.items.toList().forEach { SidePanes.close(it) } }
    Column(Modifier.fillMaxSize().background(Palette.ink)) {
        PhoneBackBar("Figures") { SidePanes.items.toList().forEach { SidePanes.close(it) } }
        SidePaneColumn(Modifier.fillMaxWidth().weight(1f))
    }
}
