package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccountBalance
import androidx.compose.material.icons.filled.BarChart
import androidx.compose.material.icons.filled.Business
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
import uniffi.cw_ffi.ProductLineDto

private enum class Tab(val title: String, val icon: ImageVector) {
    Map("Map", Icons.Filled.Map),
    Office("CEO Office", Icons.Filled.Business),
    Markets("Markets", Icons.Filled.BarChart),
    Inbox("Inbox", Icons.Filled.Inbox),
    Portfolio("Portfolio", Icons.Filled.AccountBalance),
}

/** The whole app: the start screen until a career is open, then the five tabs. */
@Composable
fun CorporateWarfareApp(store: GameStore) {
    CorporateWarfareTheme {
        Box(Modifier.fillMaxSize().background(Palette.ink)) {
            if (store.ready) MainScreen(store) else StartScreen(store)
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

@Composable
private fun StartScreen(store: GameStore) {
    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("Corporate Warfare", color = Palette.primaryText, fontSize = 32.sp, fontWeight = FontWeight.Bold)
        Text(
            "Build an industrial empire across the Rust Belt corridor, 2000.",
            color = Palette.secondary,
            fontSize = 14.sp,
            textAlign = androidx.compose.ui.text.style.TextAlign.Center,
        )
        Spacer(Modifier.height(32.dp))
        if (store.hasAutosave) {
            GoldButton("Continue career", enabled = !store.busy) { store.continueCareer() }
            Spacer(Modifier.height(10.dp))
        }
        GoldButton("New guided career", enabled = !store.busy) { store.newCareer(System.currentTimeMillis() % 1_000_000, guided = true) }
        Spacer(Modifier.height(10.dp))
        OutlinedButton(onClick = { store.newCareer(System.currentTimeMillis() % 1_000_000, guided = false) }, enabled = !store.busy) {
            Text("New sandbox career", color = Palette.primaryText)
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
private fun MainScreen(store: GameStore) {
    var tab by remember { mutableStateOf(Tab.Office) }
    Scaffold(
        containerColor = Palette.ink,
        bottomBar = {
            NavigationBar(containerColor = Palette.panel) {
                Tab.entries.forEach { item ->
                    NavigationBarItem(
                        selected = tab == item,
                        onClick = { tab = item },
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
            }
        },
    ) { padding ->
        // Content stops at the navigation bar: the scaffold's padding keeps it clear.
        Box(Modifier.padding(padding)) {
            when (tab) {
                Tab.Map -> MapScreen(store)
                Tab.Office -> OfficeScreen(store)
                Tab.Markets -> MarketsScreen(store)
                Tab.Inbox -> InboxScreen(store)
                Tab.Portfolio -> PortfolioScreen(store)
            }
        }
    }
}

@Composable
private fun TabList(content: LazyListScope.() -> Unit) {
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
        content = content,
    )
}

// MARK: Map (regions until the drawn map arrives)

@Composable
private fun MapScreen(store: GameStore) {
    TabList {
        item { ScreenHeader("Regions", store.overview?.weekLabel) }
        item { RunControls(store) }
        items(store.boards.size) { index ->
            val board = store.boards[index]
            Panel {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(board.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                        Text("${board.stateName} · hub ${board.hubCity}", color = Palette.secondary, fontSize = 12.sp)
                    }
                    Text("wages ${board.wageIndexBps / 100}%", color = Palette.secondary, fontSize = 12.sp)
                }
                val room = board.room.filter { it.importLots > 0 }.sortedByDescending { it.importLots * it.importPriceCents }.take(3)
                if (room.isNotEmpty()) {
                    Text(
                        "Room to build: " + room.joinToString(" · ") { "${it.productName} ${it.importLots}/wk" },
                        color = Palette.positive,
                        fontSize = 12.sp,
                    )
                }
            }
        }
    }
}

// MARK: CEO Office

@Composable
private fun OfficeScreen(store: GameStore) {
    val firm = store.selectedFirmDto
    var editing by remember { mutableStateOf<ProductLineDto?>(null) }
    TabList {
        item { ScreenHeader("CEO Office", store.overview?.weekLabel) }
        if (firm == null) {
            item { Callout("Choose a company from the portfolio.") }
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

@Composable
private fun ProfitDrivers(weeks: List<uniffi.cw_ffi.WeekDto>) {
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

private fun postureLabel(line: ProductLineDto): String = when (line.mode) {
    "market" -> if (line.postureBps == 0) "At market" else "%+d%% vs market".format(line.postureBps / 100)
    "cost_plus" -> "Cost +${line.postureBps / 100}%"
    "fixed" -> "Fixed price"
    else -> "Auto"
}

@Composable
private fun PricingRow(line: ProductLineDto, onClick: () -> Unit) {
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
private fun PostureDialog(store: GameStore, line: ProductLineDto, onDismiss: () -> Unit) {
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
                OutlinedButton(onClick = { store.setPosture(line.product, 0, 0); onDismiss() }) { Text("Automatic", color = Palette.primaryText) }
                Eyebrow("Against the market")
                ChipRow(listOf(-2000, -1000, -500, 0, 500, 1000, 2000)) { bps ->
                    store.setPosture(line.product, 1, bps); onDismiss()
                }
                Eyebrow("Over your cost")
                ChipRow(listOf(1000, 2000, 3000, 5000), prefix = "+") { bps ->
                    store.setPosture(line.product, 2, bps); onDismiss()
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } },
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

// MARK: Markets

@Composable
private fun MarketsScreen(store: GameStore) {
    var pickingRegion by remember { mutableStateOf(false) }
    val board = store.boards.firstOrNull { it.code == store.marketRegion }
    TabList {
        item {
            ScreenHeader("Markets", board?.let { "${it.stateName} · prices per lot" }) {
                OutlinedButton(onClick = { pickingRegion = true }) { Text(board?.name ?: "Region", color = Palette.primaryText) }
            }
        }
        val rows = store.marketRows.filter { it.product.referencePriceCents > 0 }
        item {
            Panel {
                rows.forEachIndexed { index, row ->
                    if (index > 0) HorizontalDivider(color = Palette.stroke)
                    val market = row.market
                    val price = market?.lastPriceCents ?: row.product.referencePriceCents
                    val sold = market?.lastWeekSalesLots ?: 0
                    val stock = market?.regionalStockLots ?: 0
                    val imported = board?.room?.firstOrNull { it.product == market?.productId }?.importLots ?: 0
                    Column(Modifier.padding(vertical = 4.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(row.product.name, color = Palette.primaryText, modifier = Modifier.weight(1f), maxLines = 1)
                            when {
                                imported > 0 -> SignalPill("Imported", Palette.positive)
                                (sold == 0L && stock > 0) || (sold > 0 && stock > sold * 4) -> SignalPill("Glut", Palette.negative)
                                sold > 0 && stock < sold -> SignalPill("Tight", Palette.gold)
                                else -> {}
                            }
                            Spacer(Modifier.width(8.dp))
                            Text(Format.money(price), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                        }
                        val parts = buildList {
                            add(if (sold > 0) "${Format.count(sold)}/wk sold" else "nothing sold")
                            if (sold > 0) add("%.1f wk of stock".format(stock.toDouble() / sold))
                            if (imported > 0) add("${Format.count(imported)}/wk imported")
                        }
                        Text(parts.joinToString(" · "), color = Palette.secondary, fontSize = 12.sp)
                    }
                }
            }
        }
    }
    if (pickingRegion) {
        AlertDialog(
            onDismissRequest = { pickingRegion = false },
            containerColor = Palette.panel,
            title = { Text("Region") },
            text = {
                LazyColumn(Modifier.height(420.dp)) {
                    items(store.boards.size) { index ->
                        val item = store.boards[index]
                        Text(
                            "${item.name} · ${item.stateName}",
                            color = Palette.primaryText,
                            modifier = Modifier.fillMaxWidth().clickable {
                                store.selectRegion(item.code)
                                pickingRegion = false
                            }.padding(vertical = 10.dp),
                        )
                    }
                }
            },
            confirmButton = { TextButton(onClick = { pickingRegion = false }) { Text("Close") } },
        )
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
private fun DecisionCard(store: GameStore, event: EventDto) {
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
private fun PortfolioScreen(store: GameStore) {
    val overview = store.overview ?: return
    TabList {
        item { ScreenHeader("Portfolio", overview.weekLabel) }
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
        item {
            Panel {
                Eyebrow("Game")
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Save", color = Palette.primaryText)
                        Text("The game also saves itself every four weeks.", color = Palette.secondary, fontSize = 12.sp)
                    }
                    TextButton(onClick = { store.saveNow() }) { Text("Save now", color = Palette.gold) }
                }
                if (store.status.isNotEmpty()) Text(store.status, color = Palette.secondary, fontSize = 12.sp)
            }
        }
    }
}

@Composable
fun RunControls(store: GameStore) {
    Column {
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            GoldButton("Next week", Modifier.weight(1f), enabled = !store.busy && !store.running) { store.advance() }
            OutlinedButton(onClick = { store.toggleRun() }, modifier = Modifier.weight(1f)) {
                Text(if (store.running) "Pause" else "Auto-advance", color = Palette.gold)
            }
        }
        Text("Next week settles one week. Auto-advance plays a week a second and stops for any decision.", color = Palette.secondary, fontSize = 11.sp)
    }
}
