package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.clickable
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountBalance
import androidx.compose.material.icons.filled.BarChart
import androidx.compose.material.icons.filled.Business
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Map
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.EventDto
import uniffi.cw_ffi.ScenarioDto
import uniffi.cw_ffi.scenarios
import uniffi.cw_ffi.ProductLineDto

private enum class Tab(val title: String, val icon: ImageVector) {
    Map("Map", Icons.Filled.Map),
    Office("Office", Icons.Filled.Business),
    Markets("Markets", Icons.Filled.BarChart),
    Inbox("Inbox", Icons.Filled.Inbox),
    Portfolio("Portfolio", Icons.Filled.AccountBalance),
}

/** Settings that live outside a career, for any screen that needs them. */
val LocalAppSettings = androidx.compose.runtime.staticCompositionLocalOf<AppSettings?> { null }

/**
 * The whole app: the main menu until a career is open, then the five tabs. [onQuit] is set on
 * desktop, where the menu offers Quit and the settings page offers the window options.
 */
@Composable
fun CorporateWarfareApp(store: GameStore, settings: AppSettings, updater: Updater? = null, onQuit: (() -> Unit)? = null) {
    val density = androidx.compose.ui.platform.LocalDensity.current
    var showSettings by remember { mutableStateOf(false) }
    androidx.compose.runtime.LaunchedEffect(updater) {
        Tutorial.load(settings)
        if (updater != null && settings.autoUpdate) updater.check()
    }
    androidx.compose.runtime.LaunchedEffect(store.overview?.tick) {
        store.overview?.tick?.let { Tutorial.week(it) }
    }
    androidx.compose.runtime.CompositionLocalProvider(
        LocalAppSettings provides settings,
        androidx.compose.ui.platform.LocalDensity provides androidx.compose.ui.unit.Density(density.density * settings.uiScale, density.fontScale),
    ) {
        CorporateWarfareTheme {
            Box(Modifier.fillMaxSize().background(Palette.ink)) {
                when {
                    showSettings -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
                        Box(Modifier.widthIn(max = 820.dp)) {
                            SettingsScreen(store, settings, desktop = onQuit != null, updater = updater) { showSettings = false }
                        }
                    }
                    store.ready -> androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxSize()) {
                        if (maxWidth >= DesktopBreakpoint) DesktopShell(store, onSettings = { showSettings = true })
                        else MainScreen(store, onSettings = { showSettings = true })
                    }
                    else -> StartScreen(store, updater, onSettings = { showSettings = true }, onQuit = onQuit)
                }
            store.error?.let { message ->
                AlertDialog(
                    onDismissRequest = { store.error = null },
                    confirmButton = { TextButton(onClick = { store.error = null }) { Text("OK") } },
                    title = { Text("Not done") },
                    text = { Text(message) },
                    containerColor = Palette.panel,
                )
            }
            }
        }
    }
}

/** The pixel logo, scaled to fit with square pixels. */
@Composable
fun LogoArt(modifier: Modifier) {
    val file = PixelLogo.file ?: return
    androidx.compose.foundation.Canvas(modifier.aspectRatio(file.width.toFloat() / file.height)) {
        val cell = minOf(size.width / file.width, size.height / file.height)
        file.rows.forEachIndexed { y, row ->
            row.forEachIndexed { x, character ->
                val hex = if (character == '.') null else file.palette[character.toString()]
                if (hex != null) {
                    drawRect(
                        SectorIcons.color(hex),
                        topLeft = androidx.compose.ui.geometry.Offset(x * cell, y * cell),
                        size = androidx.compose.ui.geometry.Size(cell + 0.5f, cell + 0.5f),
                    )
                }
            }
        }
    }
}

@Composable
fun GoldButton(text: String, modifier: Modifier = Modifier, enabled: Boolean = true, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        enabled = enabled,
        modifier = modifier,
        colors = ButtonDefaults.buttonColors(containerColor = Palette.gold, contentColor = Palette.ink),
    ) { Text(text, fontWeight = FontWeight.SemiBold) }
}

@Composable
private fun MainScreen(store: GameStore, onSettings: () -> Unit) {
    var tab by remember { mutableStateOf(Tab.Map) }
    var menu by remember { mutableStateOf(false) }
    // Full screens opened over the tabs: actions, research, stats.
    var overlay by remember { mutableStateOf<String?>(null) }
    fun open(page: String) {
        Tutorial.visited(page)
        when (page) {
            "map" -> { overlay = null; tab = Tab.Map }
            "office" -> { overlay = null; tab = Tab.Office }
            "markets" -> { overlay = null; tab = Tab.Markets }
            "inbox" -> { overlay = null; tab = Tab.Inbox }
            "portfolio" -> { overlay = null; tab = Tab.Portfolio }
            else -> overlay = page
        }
    }
    androidx.compose.runtime.LaunchedEffect(Tutorial.requestedPage) {
        val wanted = Tutorial.requestedPage ?: return@LaunchedEffect
        open(wanted)
        Tutorial.requestedPage = null
    }
    overlay?.let { page ->
        Column(Modifier.fillMaxSize().background(Palette.ink)) {
            Row(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = { overlay = null }) { Text("‹ Back", color = Palette.gold) }
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when (page) {
                    "actions" -> CommandCenterScreen(store)
                    "research" -> ResearchScreen(store)
                    else -> StatsScreen(store) { overlay = null }
                }
            }
        }
        return
    }
    Scaffold(
        containerColor = Palette.ink,
        bottomBar = {
            NavigationBar(containerColor = Palette.panel) {
                Tab.entries.forEach { item ->
                    NavigationBarItem(
                        selected = tab == item,
                        onClick = { open(item.name.lowercase()) },
                        icon = {
                            val due = store.events.count { it.mandatory }
                            if (item == Tab.Inbox && due > 0) {
                                BadgedBox(badge = { Badge { Text("$due") } }) { Icon(item.icon, item.title) }
                            } else {
                                Icon(item.icon, item.title)
                            }
                        },
                        label = { Text(item.title, fontSize = 11.sp) },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = Palette.gold,
                            selectedTextColor = Palette.gold,
                            indicatorColor = Palette.panelRaised,
                            unselectedIconColor = Palette.secondary,
                            unselectedTextColor = Palette.secondary,
                        ),
                    )
                }
                NavigationBarItem(
                    selected = false,
                    onClick = { menu = true },
                    icon = { Icon(Icons.Filled.Menu, "Menu") },
                    label = { Text("Menu", fontSize = 11.sp) },
                    colors = NavigationBarItemDefaults.colors(unselectedIconColor = Palette.secondary, unselectedTextColor = Palette.secondary),
                )
            }
        },
    ) { padding ->
        // Content stops at the navigation bar: the scaffold's padding keeps it clear.
        Column(Modifier.padding(padding)) {
            TutorialPanel(compact = true, modifier = Modifier.fillMaxWidth())
            Box(Modifier.weight(1f)) {
            when (tab) {
                Tab.Map -> MapScreen(store)
                Tab.Office -> OfficeScreen(store, ::open)
                Tab.Markets -> MarketsScreen(store)
                Tab.Inbox -> InboxScreen(store)
                Tab.Portfolio -> PortfolioScreen(store, onSettings)
            }
            }
        }
    }
    if (menu) {
        AlertDialog(
            onDismissRequest = { menu = false },
            containerColor = Palette.panel,
            title = { Text("Menu") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    GoldButton("Resume", Modifier.fillMaxWidth()) { menu = false }
                    OutlinedButton(onClick = { store.saveNow(); menu = false }, modifier = Modifier.fillMaxWidth()) { Text("Save now", color = Palette.primaryText) }
                    OutlinedButton(onClick = { menu = false; onSettings() }, modifier = Modifier.fillMaxWidth()) { Text("Settings", color = Palette.primaryText) }
                    OutlinedButton(onClick = { menu = false; store.exitToMenu() }, modifier = Modifier.fillMaxWidth()) { Text("Save and exit to main menu", color = Palette.primaryText) }
                }
            },
            confirmButton = {},
        )
    }
}

@Composable
internal fun TabList(content: LazyListScope.() -> Unit) {
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        content = content,
    )
}

// MARK: CEO Office

@Composable
private fun OfficeScreen(store: GameStore, onOpen: (String) -> Unit = {}) {
    val firm = store.selectedFirmDto
    var editing by remember { mutableStateOf<ProductLineDto?>(null) }
    var stats by remember { mutableStateOf(false) }
    if (stats) {
        StatsScreen(store) { stats = false }
        return
    }
    TabList {
        item {
            ScreenHeader("CEO Office", store.overview?.weekLabel) {
                if (firm != null) {
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        OutlinedButton(onClick = { onOpen("research") }) { Text("Research", color = Palette.primaryText, fontSize = 12.sp) }
                        OutlinedButton(onClick = { onOpen("stats") }) { Text("Stats", color = Palette.primaryText, fontSize = 12.sp) }
                        GoldButton("Actions") { onOpen("actions") }
                    }
                }
            }
        }
        if (firm == null) {
            item { FoundCompanyPanel(store, scroll = false) }
            return@TabList
        }
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                SectorBadge(firm.industry, 40)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(firm.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 17.sp, maxLines = 1)
                    Text("${Sectors.name(firm.industry)} · HQ ${firm.region} · Credit ${firm.grade}", color = Palette.secondary, fontSize = 12.sp, maxLines = 1)
                }
                Text("${Format.price(firm.markCents)}/share", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        item {
            val overview = store.overview
            val quarter = if (overview?.primaryFirm == firm.handle) overview.quarterNetIncomeCents else null
            val weeks = overview?.quarterWeeks?.toInt() ?: 0
            StatGroup(
                Stat("Company cash", Format.compact(firm.cashCents)),
                listOfNotNull(
                    quarter?.let {
                        Stat(if (weeks >= 13) "Profit, quarter" else "Profit, $weeks wk", Format.compact(it), if (it >= 0) Palette.positive else Palette.negative)
                    },
                    Stat("Equity", Format.compact(firm.equityCents)),
                    Stat("Fair value", Format.compact(firm.fairValueCents)),
                    Stat("Plants", "${firm.facilities}"),
                    Stat("Your votes", "%.1f%%".format(firm.investorVotesBps / 100.0)),
                ),
            )
        }
        store.finance?.takeIf { it.firm == firm.handle && it.recentWeeks.isNotEmpty() }?.let { finance ->
            item { ProfitDrivers(finance.recentWeeks) }
        }
        if (store.productLines.isNotEmpty()) {
            item {
                Panel {
                    Eyebrow("Pricing")
                    store.productLines.forEachIndexed { index, line ->
                        if (index > 0) HorizontalDivider(color = Palette.stroke)
                        PricingRow(line) { editing = line }
                    }
                }
            }
        }
        if (store.facilities.isNotEmpty()) item { ProductionTarget(store) }
        if (store.divisions.isNotEmpty()) {
            item { Eyebrow("Divisions") }
            items(store.divisions.size) { index ->
                val division = store.divisions[index]
                Panel {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        SectorBadge(division.industry, 28)
                        Spacer(Modifier.width(10.dp))
                        Column(Modifier.weight(1f)) {
                            Text("${Sectors.name(division.industry)} · ${division.regionName}", color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                            Text("${division.strategyName} · ${division.blocks} plants", color = Palette.secondary, fontSize = 12.sp)
                        }
                        Text("${division.shareBps / 100}% share", color = Palette.gold, fontSize = 13.sp)
                    }
                }
            }
        }
    }
    editing?.let { line -> PostureDialog(store, line) { editing = null } }
}

/** How hard every plant runs, 0 to 100 percent of capacity. */
@Composable
internal fun ProductionTarget(store: GameStore) {
    val capacity = store.facilities.sumOf { it.capacityLots }
    var target by remember(store.selectedFirm) { mutableStateOf((store.facilities.firstOrNull()?.targetBps ?: 8_500) / 100.0) }
    Panel {
        Eyebrow("Production target")
        ValueSlider(
            target, { target = it }, 0.0..100.0, 1.0,
            label = { "%.0f%% of capacity".format(it) },
            detail = { "About ${Format.count((capacity * it / 100).toLong())} of ${Format.count(capacity)} lots a week. Lower targets save inputs and wages when stock piles up." },
            onCommit = { store.setTarget((it * 100).toInt()) },
        )
    }
}

@Composable
internal fun ProfitDrivers(weeks: List<uniffi.cw_ffi.WeekDto>) {
    val revenue = weeks.sumOf { it.revenueCents }
    val net = weeks.sumOf { it.netIncomeCents }
    val costs = listOf(
        "Inputs and goods" to weeks.sumOf { it.costOfGoodsCents },
        "Wages" to weeks.sumOf { it.wagesCents },
        "Overheads" to weeks.sumOf { it.operatingExpenseCents },
        "Depreciation" to weeks.sumOf { it.depreciationCents },
        "Interest" to weeks.sumOf { it.netInterestCents },
        "Tax" to weeks.sumOf { it.taxCents },
    ).filter { it.second != 0L }
    val scale = maxOf(revenue, costs.maxOfOrNull { it.second } ?: 0L, 1L).toDouble()
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Eyebrow(if (weeks.size >= 13) "Last quarter" else "Last ${weeks.size} weeks")
            Spacer(Modifier.weight(1f))
            Text(Format.signed(net), color = if (net >= 0) Palette.positive else Palette.negative, fontWeight = FontWeight.SemiBold)
            Text(" profit", color = Palette.secondary, fontSize = 12.sp)
        }
        BarRow("Revenue", revenue, scale, Palette.positive)
        costs.forEach { (name, cents) -> BarRow(name, cents, scale, Palette.negative.copy(alpha = 0.75f)) }
        val biggest = costs.maxByOrNull { it.second }
        val text = when {
            revenue <= 0 -> "No sales closed yet this quarter: every cost is a loss until the first sale settles."
            biggest == null -> "No costs booked."
            net < 0 -> "${biggest.first} alone take ${biggest.second * 100 / revenue}% of revenue. Sell more of what you make, raise prices if stock is selling out, or cut what is idle."
            else -> "${net * 100 / revenue}% net margin. The biggest cost is ${biggest.first.lowercase()} at ${biggest.second * 100 / revenue}% of revenue."
        }
        Text(text, color = Palette.secondary, fontSize = 12.sp)
    }
}

internal fun postureLabel(line: ProductLineDto): String = when (line.mode) {
    "market" -> if (line.postureBps == 0) "At market" else "%+d%% vs market".format(line.postureBps / 100)
    "cost_plus" -> "Cost +${line.postureBps / 100}%"
    "fixed" -> "Fixed price"
    else -> "Auto"
}

@Composable
internal fun PricingRow(line: ProductLineDto, onClick: () -> Unit) {
    val perLot = line.priceCents - line.unitCostCents
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(line.name, color = Palette.primaryText, fontWeight = FontWeight.Medium, maxLines = 1)
            Text("${Format.money(line.priceCents)} · ${postureLabel(line)}", color = Palette.secondary, fontSize = 12.sp)
        }
        if (line.unitCostCents > 0) {
            Text(Format.signed(perLot), color = if (perLot >= 0) Palette.positive else Palette.negative, fontWeight = FontWeight.SemiBold)
            Spacer(Modifier.width(8.dp))
        }
        if (line.madeLastWeekLots > 0) {
            val sold = (line.soldLastWeekLots * 100 / line.madeLastWeekLots).coerceAtMost(999)
            SignalPill("$sold%", if (sold >= 95) Palette.positive else if (sold >= 70) Palette.gold else Palette.negative)
        }
    }
}

/** Automatic pricing, a step against the market, or a markup over the company's own cost. */
@Composable
internal fun PostureDialog(store: GameStore, line: ProductLineDto, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Palette.panel,
        title = { Text(line.name) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(
                    "Your price ${Format.money(line.priceCents)} · market ${Format.money(line.marketCents)} · your cost " +
                        (if (line.unitCostCents > 0) Format.money(line.unitCostCents) else "n/a"),
                    color = Palette.secondary,
                    fontSize = 13.sp,
                )
                Text("Buyers take the cheapest offer first. Undercut to sell out; price above the market for margin and risk unsold stock.", color = Palette.secondary, fontSize = 12.sp)
                OutlinedButton(onClick = { store.setPosture(line.product, 0, 0); onDismiss() }) {
                    Text(if (line.mode == "auto") "Automatic (on)" else "Automatic", color = Palette.primaryText)
                }
                var market by remember { mutableStateOf(if (line.mode == "market") line.postureBps / 100.0 else 0.0) }
                var cost by remember { mutableStateOf(if (line.mode == "cost_plus") line.postureBps / 100.0 else 20.0) }
                fun margin(price: Long) = if (line.unitCostCents > 0) " · ${Format.signed(price - line.unitCostCents)} a lot over cost" else ""
                Eyebrow("Against the market" + if (line.mode == "market") " · active" else "")
                ValueSlider(
                    market, { market = it }, -50.0..50.0, 1.0,
                    label = { if (it == 0.0) "At market" else "%+.0f%%".format(it) },
                    origin = 0.0,
                    detail = { val p = (line.marketCents * (1 + it / 100)).toLong(); "${Format.money(p)} a lot" + margin(p) },
                    tint = if (market <= 0) Palette.positive else Palette.warning,
                    onCommit = { store.setPosture(line.product, 1, (it * 100).toInt()) },
                )
                Eyebrow("Over your cost" + if (line.mode == "cost_plus") " · active" else "")
                if (line.unitCostCents > 0) {
                    ValueSlider(
                        cost, { cost = it }, 0.0..100.0, 1.0,
                        label = { "+%.0f%%".format(it) },
                        detail = { val p = (line.unitCostCents * (1 + it / 100)).toLong(); "${Format.money(p)} a lot · ${Format.money(p - line.unitCostCents)} margin" },
                        onCommit = { store.setPosture(line.product, 2, (it * 100).toInt()) },
                    )
                } else {
                    Text("Available once the good has a production cost.", color = Palette.secondary, fontSize = 12.sp)
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } },
    )
}

@Composable
private fun ChipRow(steps: List<Int>, prefix: String = "", onPick: (Int) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        steps.forEach { bps ->
            val label = when {
                prefix.isNotEmpty() -> "$prefix${bps / 100}%"
                bps == 0 -> "Mkt"
                else -> "%+d".format(bps / 100)
            }
            Text(
                label,
                color = Palette.primaryText,
                fontSize = 12.sp,
                modifier = Modifier.background(Palette.panelRaised).clickable { onPick(bps) }.padding(horizontal = 8.dp, vertical = 6.dp),
            )
        }
    }
}

// MARK: Inbox

@Composable
private fun InboxScreen(store: GameStore) {
    TabList {
        item { ScreenHeader("Inbox", listOfNotNull(store.selectedFirmDto?.name, store.overview?.weekLabel).joinToString(" · ")) }
        val due = store.events.count { it.mandatory }
        if (due > 0) item { Callout("$due decision${if (due == 1) "" else "s"} needed before time can advance.", Palette.warning) }
        if (store.events.isEmpty()) item { Callout("Quiet week. Decisions show up here when they happen.", Palette.secondary) }
        items(store.events.size) { index -> DecisionCard(store, store.events[index]) }
    }
}

@Composable
internal fun DecisionCard(store: GameStore, event: EventDto) {
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(event.title, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            if (event.mandatory) SignalPill("Due week ${event.deadline}", Palette.warning)
        }
        Text(event.subject, color = Palette.secondary, fontSize = 12.sp)
        event.choices.forEach { choice ->
            OutlinedButton(onClick = { store.respond(event.id, choice.id) }, enabled = !store.busy, modifier = Modifier.fillMaxWidth()) {
                Text(choice.label, color = Palette.primaryText)
            }
        }
    }
}

// MARK: Portfolio

@Composable
private fun PortfolioScreen(store: GameStore, onSettings: () -> Unit) {
    val overview = store.overview ?: return
    TabList {
        item {
            ScreenHeader("Portfolio", overview.weekLabel) {
                TextButton(onClick = onSettings) { Text("⚙ Settings", color = Palette.secondary) }
            }
        }
        overview.pause?.let { pause ->
            item {
                Callout("Paused: $pause", Palette.warning)
                Spacer(Modifier.height(6.dp))
                OutlinedButton(onClick = { store.dismissPause() }) { Text("Continue", color = Palette.primaryText) }
            }
        }
        item {
            val weeks = overview.quarterWeeks.toInt()
            StatGroup(
                Stat("Net worth", Format.compact(overview.investorNetWorthCents), note = "holdings at fair value plus cash"),
                listOf(
                    Stat("Your cash", Format.compact(overview.investorCashCents)),
                    Stat("Company cash", Format.compact(overview.firmCashCents)),
                    Stat(
                        if (weeks >= 13) "Profit, quarter" else "Profit, $weeks wk",
                        Format.compact(overview.quarterNetIncomeCents),
                        if (overview.quarterNetIncomeCents >= 0) Palette.positive else Palette.negative,
                    ),
                ),
            )
        }
        item {
            val outlook = overview.forecastFourWeekCents - overview.firmCashCents
            Panel {
                Text("Company cash in four weeks: ${Format.compact(overview.forecastFourWeekCents)}", color = Palette.primaryText, fontWeight = FontWeight.Medium)
                Text("${Format.signed(outlook)} after payroll and loan principal due", color = if (outlook >= 0) Palette.secondary else Palette.negative, fontSize = 12.sp)
            }
        }
        if (overview.nextActionReason.isNotEmpty()) item { Callout(overview.nextActionReason) }
        overview.alerts.forEach { alert -> item { Callout(alert, Palette.warning) } }
        item { RunControls(store) }
        item {
            Panel {
                Eyebrow("Your companies")
                store.firms.filter { it.controlled }.forEach { firm ->
                    Row(Modifier.fillMaxWidth().clickable { store.selectFirm(firm.handle) }.padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(firm.name + if (firm.handle == store.selectedFirm) "  ✓" else "", color = Palette.primaryText, fontWeight = FontWeight.Medium)
                            Text("${Sectors.name(firm.industry)} · ${firm.region}", color = Palette.secondary, fontSize = 12.sp)
                        }
                        Text(Format.compact(firm.cashCents), color = Palette.primaryText)
                    }
                }
            }
        }
    }
}

@Composable
fun RunControls(store: GameStore) {
    Column {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            GoldButton("Next week", Modifier.weight(1f), enabled = !store.busy && !store.running) { store.advance() }
            SpeedControl(store)
        }
        Text("One tick is one week. 1x plays a week a second, 2x two, 4x four; play stops for any decision.", color = Palette.secondary, fontSize = 11.sp)
    }
}

/** Pause, 1x, 2x, 4x. */
@Composable
fun SpeedControl(store: GameStore) {
    Row(
        Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(10.dp)).background(Palette.panelRaised).padding(2.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        listOf(0 to "II", 1 to "1x", 2 to "2x", 4 to "4x").forEach { (value, label) ->
            val selected = if (value == 0) !store.running else store.running && store.speed == value
            Text(
                label,
                color = if (selected) Palette.ink else Palette.primaryText,
                fontWeight = FontWeight.Bold,
                fontSize = 13.sp,
                modifier = Modifier.clip(androidx.compose.foundation.shape.RoundedCornerShape(8.dp))
                    .background(if (selected) Palette.gold else androidx.compose.ui.graphics.Color.Transparent)
                    .clickable { store.playAt(value) }
                    .padding(horizontal = 10.dp, vertical = 7.dp),
            )
        }
    }
}

