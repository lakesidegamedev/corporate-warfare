package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.DivisionOpsDto
import uniffi.cw_ffi.GoodDepthDto
import uniffi.cw_ffi.HeadToHeadDto
import uniffi.cw_ffi.LogisticsStatsDto
import uniffi.cw_ffi.PricePointDto
import uniffi.cw_ffi.RegionDossierDto
import uniffi.cw_ffi.RivalRowDto
import uniffi.cw_ffi.SeriesDto
import uniffi.cw_ffi.SessionHandle
import uniffi.cw_ffi.ShareRowDto
import uniffi.cw_ffi.StatementDto

// The deeper systems displays: a good's market depth, a region's dossier, a division's
// operations, a company's freight, the rivals board and the economy dashboard. Every figure is
// read from the engine; the views only lay it out and chart it.

private val blue = Color(0xFF6EC6FF)
private val violet = Color(0xFFC792EA)
private val systemsTierNames = listOf("Low skill", "Mid skill", "High skill")

/** Read from the engine off the main thread and again every time the week or a command moves. */
@Composable
fun <T> rememberEngine(store: GameStore, vararg keys: Any?, block: (SessionHandle) -> T): T? {
    val revision = store.overview?.revision
    return produceState<T?>(null, revision, *keys) { value = store.read(block) }.value
}

private fun SeriesDto.points(scale: (Long) -> Long = { it }) = weeks.zip(values).map { (w, v) -> ChartPoint(w.toInt(), scale(v)) }
private fun List<SeriesDto>.of(key: String, scale: (Long) -> Long = { it }) = firstOrNull { it.key == key }?.points(scale).orEmpty()
private fun pctOf(bps: Long) = "%.1f%%".format(bps / 100.0)
private fun milliLots(milli: Long) = Format.count(milli / 1_000)

/** A solid tag, no translucency, in the 8-bit style. */
@Composable
private fun Tag(text: String, tint: Color) {
    Text(
        text, color = Palette.ink, fontSize = 10.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, maxLines = 1,
        modifier = Modifier.clip(Pixel.small).background(tint).padding(horizontal = 5.dp, vertical = 1.dp),
    )
}

/**
 * A compact table: the first column left, the rest right. Rows flagged [yours] are gold.
 * A tap on a header calls [onSort] with its column; a tap on a row calls [onRow].
 */
@Composable
fun SystemsTable(
    headers: List<String>,
    rows: List<List<String>>,
    weights: List<Float> = listOf(2.2f) + List(headers.size - 1) { 1f },
    yours: (Int) -> Boolean = { false },
    sort: Int = -1,
    onSort: ((Int) -> Unit)? = null,
    onRow: ((Int) -> Unit)? = null,
    selected: Int = -1,
) {
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 6.dp, vertical = 4.dp)) {
            headers.forEachIndexed { i, h ->
                Text(
                    if (i == sort) "$h *" else h,
                    color = if (i == sort) Palette.gold else Palette.secondary, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, maxLines = 1,
                    overflow = TextOverflow.Ellipsis, textAlign = if (i == 0) TextAlign.Start else TextAlign.End,
                    modifier = Modifier.weight(weights[i]).then(if (onSort != null) Modifier.clickable { onSort(i) } else Modifier),
                )
            }
        }
        rows.forEachIndexed { r, cells ->
            val tint = if (yours(r)) Palette.gold else Palette.primaryText
            Row(
                Modifier.fillMaxWidth().background(if (r == selected) Palette.stroke else if (r % 2 == 1) Palette.panel else Palette.ink)
                    .then(if (onRow != null) Modifier.clickable { onRow(r) } else Modifier)
                    .padding(horizontal = 6.dp, vertical = 3.dp),
            ) {
                cells.forEachIndexed { i, c ->
                    Text(
                        c, color = tint, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        textAlign = if (i == 0) TextAlign.Start else TextAlign.End, modifier = Modifier.weight(weights[i]),
                    )
                }
            }
        }
    }
}

/** Two columns side by side when there is room, one under the other on a phone. */
@Composable
private fun Split(left: @Composable () -> Unit, right: @Composable () -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        if (maxWidth >= 700.dp) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) { left() }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) { right() }
            }
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { left(); right() }
        }
    }
}

@Composable
private fun ChartPanel(
    title: String, series: List<ChartSeries>, kind: ChartKind = ChartKind.Line, format: (Long) -> String = { Format.compact(it) },
    scale: ChartScale = ChartScale.Zero,
) {
    if (series.all { it.points.size < 2 }) return
    Panel { Chart(series, title = title, initialKind = kind, initialRange = ChartRange.Y1, height = 130.dp, format = format, scale = scale) }
}

/** One chart for a [ChartGrid]: what [ChartPanel] takes. */
private class PanelSpec(
    val title: String, val series: List<ChartSeries>, val kind: ChartKind = ChartKind.Line,
    val format: (Long) -> String = { Format.compact(it) }, val scale: ChartScale = ChartScale.Zero,
)

/** Charts in rows of two when there is room, so the columns stay level; one per row on a phone. */
@Composable
private fun ChartGrid(specs: List<PanelSpec>) {
    val shown = specs.filter { spec -> spec.series.any { it.points.size >= 2 } }
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val perRow = if (maxWidth >= 700.dp) 2 else 1
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            shown.chunked(perRow).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    row.forEach { p -> Column(Modifier.weight(1f)) { ChartPanel(p.title, p.series, p.kind, p.format, p.scale) } }
                    repeat(perRow - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
}

private fun signedCount(n: Long): String = (if (n > 0) "+" else if (n < 0) "-" else "") + Format.count(kotlin.math.abs(n))

@Composable
private fun EmptyHistory() = Callout("History builds up as weeks close. A career saved before this version starts it now.", Palette.secondary)

private fun shareTable(rows: List<ShareRowDto>): List<List<String>> = rows.map { listOf(it.name, Format.count(it.lots), pctOf(it.shareBps)) }

// 1. Market depth per good.

/** A good's market depth, loaded for the commodity dialog. */
@Composable
fun GoodDepthSection(store: GameStore, product: UInt) {
    val d = rememberEngine(store, product) { it.goodDepth(product) } ?: return
    GoodDepthView(d)
}

@Composable
fun GoodDepthView(d: GoodDepthDto) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        val cover = d.coverX100?.let { "%.1f wk".format(it / 100.0) } ?: "no sales"
        StatGroup(
            null,
            listOf(
                Stat("Weeks of cover", cover, note = "stock over sales"),
                Stat("Unsold on shelves", Format.count(d.unsoldLots)),
                Stat("Your share of sales", pctOf(d.yourShareBps), if (d.yourShareBps > 0) Palette.gold else Palette.primaryText),
                Stat("Imported / wk", Format.count(d.importedLots)),
                Stat("Exported / wk", Format.count(d.exportedLots)),
                Stat("Sales-weighted ask", Format.money(d.askCents)),
            ),
        )
        Panel {
            Eyebrow("Why the price is moving")
            d.drivers.forEach { driver ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    when (driver.push) {
                        1 -> Tag("UP", Palette.warning)
                        -1 -> Tag("DOWN", blue)
                        else -> Tag("NOTE", Palette.secondary)
                    }
                    Text(driver.text, color = Palette.primaryText, fontSize = 12.sp)
                }
            }
        }
        val s = d.series
        if (s.all { it.weeks.size < 2 }) EmptyHistory()
        ChartGrid(
            listOfNotNull(
                PanelSpec(
                    "Made, sold and trade per week",
                    listOf(
                        ChartSeries("Made", Palette.gold, s.of("made")), ChartSeries("Sold", Palette.positive, s.of("sold")),
                        ChartSeries("Imported", Palette.negative, s.of("imported")), ChartSeries("Exported", blue, s.of("exported")),
                    ),
                    format = { Format.count(it) },
                ),
                PanelSpec("Stock held", listOf(ChartSeries("Stock", violet, s.of("stock"))), ChartKind.Area, format = { Format.count(it) }, scale = ChartScale.Level),
                PanelSpec("Weeks of cover", listOf(ChartSeries("Cover", blue, s.of("cover_x100"))), format = { "%.1f".format(it / 100.0) }, scale = ChartScale.Level),
                if (s.of("your_share_bps").any { it.value > 0 }) {
                    PanelSpec("Your share of sales", listOf(ChartSeries("You", Palette.gold, s.of("your_share_bps"))), ChartKind.Area, format = { pctOf(it) }, scale = ChartScale.Level)
                } else null,
            ),
        )
        Split(
            {
                Eyebrow("Top sellers last week")
                if (d.sellers.isEmpty()) Text("Nobody sold it last week.", color = Palette.secondary, fontSize = 12.sp)
                else SystemsTable(listOf("Company", "Lots", "Share"), shareTable(d.sellers), yours = { d.sellers[it].yours })
            },
            {
                Eyebrow("Top buyers (weekly use)")
                if (d.buyers.isEmpty()) Text("No plant uses it and households do not buy it.", color = Palette.secondary, fontSize = 12.sp)
                else SystemsTable(listOf("Buyer", "Lots", "Share"), shareTable(d.buyers), yours = { d.buyers[it].yours })
            },
        )
    }
}

// 2. Region dossier.

@Composable
fun RegionDossierSection(store: GameStore, region: UInt) {
    val firm = store.selectedFirm ?: 0uL
    val d = rememberEngine(store, region, firm) { it.regionDossier(region, firm) } ?: return
    RegionDossierView(d, onDivision = { industry -> store.divisions.firstOrNull { it.industry == industry && it.regionName == d.name }?.let { store.showDivision(it) } })
}

@Composable
fun RegionDossierView(d: RegionDossierDto, onDivision: (String) -> Unit = {}) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        val idle = d.population - d.employed
        StatGroup(
            null,
            listOf(
                Stat("Workforce", Format.count(d.population)),
                Stat("Without a job", Format.count(idle), note = if (d.population > 0) pctOf(idle * 10_000 / d.population) else null),
                Stat("Average wage / wk", Format.money(d.wageCents)),
                Stat("Land price", "${d.landIndexBps / 100}%", note = "of the corridor"),
                Stat("Demand index", "${d.demandIndexBps / 100}%"),
                Stat("Spend / wk", Format.compact(d.spendCents), note = "households"),
                Stat(
                    "Net migration / yr", signedCount(d.netMigrationYear),
                    if (d.netMigrationYear > 0) Palette.positive else if (d.netMigrationYear < 0) Palette.negative else Palette.primaryText,
                    note = "workers moving in or out",
                ),
            ),
        )
        Split(
            {
                Eyebrow("Labour pool")
                SystemsTable(
                    listOf("Tier", "Workers", "Idle", "Wage"),
                    d.tiers.mapIndexed { i, t -> listOf(systemsTierNames.getOrElse(i) { "Tier $i" }, Format.count(t.population), Format.count(t.idle), Format.money(t.wageCents)) },
                )
                Eyebrow("What households buy here")
                SystemsTable(listOf("Good", "Spend / wk", "Share"), d.demand.map { listOf(it.name, Format.compact(it.spendCents), pctOf(it.shareBps)) })
            },
            {
                Eyebrow("Who competes here")
                if (d.competitors.isEmpty()) Text("No company operates here yet.", color = Palette.secondary, fontSize = 12.sp)
                else d.competitors.groupBy { it.industry }.forEach { (industry, rows) ->
                    SystemsTable(
                        listOf(Sectors.name(industry), "Plants", "Share", "Profit"),
                        rows.map { listOf(it.name, "${it.blocks}", pctOf(it.shareBps), if (it.yours) Format.compact(it.profitCents) else "") },
                        weights = listOf(2.4f, 0.8f, 1f, 1.1f),
                        yours = { rows[it].yours },
                        onRow = { r -> rows[r].takeIf { it.yours }?.let { onDivision(it.industry) } },
                    )
                }
            },
        )
        val s = d.series
        if (s.all { it.weeks.size < 2 }) EmptyHistory()
        val shares = s.filter { it.key.startsWith("share:") }
        val colours = listOf(Palette.gold, Palette.positive, blue, violet, Palette.warning)
        ChartGrid(
            listOfNotNull(
                PanelSpec("Workers and jobs", listOf(ChartSeries("Workforce", blue, s.of("population")), ChartSeries("Employed", Palette.positive, s.of("employed"))), format = { Format.count(it) }, scale = ChartScale.Level),
                PanelSpec("Average wage", listOf(ChartSeries("Wage", Palette.gold, s.of("wage"))), format = { Format.money(it) }, scale = ChartScale.Level),
                PanelSpec("Workers moving in or out each week", listOf(ChartSeries("Net movers", violet, s.of("migration"))), ChartKind.Bar, format = { signedCount(it) }, scale = ChartScale.Level),
                PanelSpec("Land price (100 is the corridor)", listOf(ChartSeries("Land", Palette.warning, s.of("land_index"))), format = { "%.0f".format(it / 100.0) }, scale = ChartScale.Level),
                PanelSpec("Household spending per week", listOf(ChartSeries("Spend", violet, s.of("spend"))), ChartKind.Bar),
                if (shares.isNotEmpty()) {
                    PanelSpec("Your share by sector", shares.mapIndexed { i, x -> ChartSeries(Sectors.name(x.key.removePrefix("share:")), colours[i % colours.size], x.points()) }, format = { pctOf(it) }, scale = ChartScale.Level)
                } else null,
            ),
        )
    }
}

// 3. Division operations.

@Composable
fun DivisionOpsSection(store: GameStore, division: uniffi.cw_ffi.DivisionDto) {
    val firm = store.selectedFirm ?: return
    val d = rememberEngine(store, firm, division.region, division.industry) { it.divisionOps(firm, division.region, division.industry) } ?: return
    DivisionOpsView(d)
}

private val costNames = mapOf(
    "materials" to "Inputs", "wages" to "Labour", "maintenance" to "Upkeep", "land" to "Land and rent",
    "depreciation" to "Depreciation", "freight" to "Freight",
)

@Composable
fun DivisionOpsView(d: DivisionOpsDto) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Eyebrow("Operations")
        StatGroup(
            null,
            listOf(
                Stat("Utilisation", pctOf(d.utilisationBps), if (d.utilisationBps < 6_000) Palette.warning else Palette.primaryText, note = "of this week's full run"),
                Stat("Made / wk", milliLots(d.madeMilli)),
                Stat("Sold / wk", milliLots(d.soldMilli), if (d.soldMilli < d.madeMilli * 9 / 10) Palette.warning else Palette.primaryText),
                Stat("Capacity / wk", milliLots(d.capacityMilli)),
                Stat("Cost per lot", Format.money(d.unitCostCents)),
                Stat("Margin per lot", Format.signed(d.marginPerLotCents), if (d.marginPerLotCents < 0) Palette.negative else Palette.positive),
            ),
        )
        Eyebrow("What a lot costs against its price")
        if (d.madeMilli <= 0) Text("Nothing made last week, so there is no cost per lot.", color = Palette.secondary, fontSize = 12.sp)
        else {
            val scale = maxOf(d.priceCents, d.unitCostCents, 1L).toDouble()
            d.unitCosts.forEach { BarRow(costNames[it.key] ?: it.key, it.cents, scale, Palette.negative) }
            BarRow("Total cost", d.unitCostCents, scale, Palette.warning)
            BarRow("Price", d.priceCents, scale, Palette.gold)
        }
        val s = d.series
        if (s.all { it.weeks.size < 2 }) EmptyHistory()
        ChartGrid(
            listOf(
                PanelSpec(
                    "Output against sales",
                    listOf(
                        ChartSeries("Made", Palette.gold, s.of("made") { it / 1_000 }), ChartSeries("Sold", Palette.positive, s.of("sold") { it / 1_000 }),
                        ChartSeries("Capacity", Palette.secondary, s.of("capacity") { it / 1_000 }),
                    ),
                    format = { Format.count(it) },
                ),
                PanelSpec("Utilisation", listOf(ChartSeries("Used", blue, s.of("utilisation_bps"))), ChartKind.Area, format = { pctOf(it) }),
                PanelSpec("Cost per lot", listOf(ChartSeries("Cost", Palette.negative, s.of("unit_cost"))), format = { Format.money(it) }, scale = ChartScale.Level),
                PanelSpec("Revenue and costs", listOf(ChartSeries("Revenue", Palette.gold, s.of("revenue")), ChartSeries("Costs", Palette.negative, s.of("costs"))), ChartKind.Bar),
            ),
        )
    }
}

// 4. Logistics.

@Composable
fun LogisticsStatsSection(store: GameStore) {
    val firm = store.selectedFirm ?: return
    val d = rememberEngine(store, firm) { it.logisticsStats(firm) } ?: return
    LogisticsStatsView(d)
}

@Composable
fun LogisticsStatsView(d: LogisticsStatsDto) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Eyebrow("Freight stats")
        StatGroup(
            null,
            listOf(
                Stat("Lots moved / wk", Format.count(d.lots)),
                Stat("Cost per lot", Format.money(d.centsPerLot)),
                Stat("Overflow to outside", Format.count(d.outsideLots), if (d.outsideLots > 0) Palette.warning else Palette.primaryText),
                Stat("Own carriers", Format.count(d.ownLots)),
                Stat("Own fleet used", if (d.carrierCap > 0) "${Format.count(d.carrierUsed)} of ${Format.count(d.carrierCap)}" else "none"),
                Stat("Idle capacity", Format.count(d.carrierIdle), if (d.carrierCap > 0 && d.carrierFillBps < 6_000) Palette.warning else Palette.primaryText),
            ),
        )
        val s = d.series
        if (d.lots == 0L && d.carrierCap == 0L && s.of("lots").all { it.value == 0L }) {
            Callout("No freight moved yet: everything you sell is sold where it is made. Shipping starts when you sell into another region or buy inputs from one.", Palette.secondary)
            return@Column
        }
        if (s.all { it.weeks.size < 2 }) EmptyHistory()
        Split(
            {
                ChartPanel(
                    "Who carried your freight",
                    listOf(ChartSeries("Own", Palette.gold, s.of("own")), ChartSeries("Other carriers", blue, s.of("other")), ChartSeries("Outside", Palette.negative, s.of("outside"))),
                    ChartKind.Bar, format = { Format.count(it) },
                )
                ChartPanel("Cost per lot", listOf(ChartSeries("Per lot", Palette.warning, s.of("cents_per_lot"))), format = { Format.money(it) }, scale = ChartScale.Level)
            },
            {
                if (s.of("carrier_cap").any { it.value > 0 }) {
                    ChartPanel(
                        "Your carriers: capacity and use",
                        listOf(ChartSeries("Capacity", Palette.secondary, s.of("carrier_cap")), ChartSeries("Used", Palette.positive, s.of("carrier_used"))),
                        format = { Format.count(it) },
                    )
                    ChartPanel("Carrier fill", listOf(ChartSeries("Fill", blue, s.of("fill_bps"))), ChartKind.Area, format = { pctOf(it) })
                }
                ChartPanel("Freight bill per week", listOf(ChartSeries("Paid", Palette.negative, s.of("cents"))), ChartKind.Bar)
            },
        )
    }
}

// 5. Rivals board.

enum class RivalSort(val title: String, val key: (RivalRowDto) -> Long) {
    Revenue("Revenue", { it.revenueCents }), Profit("Profit", { it.profitCents }), Cash("Cash", { it.cashCents }),
    Debt("Debt", { it.debtCents }), Price("Share", { it.sharePriceCents }), Divisions("Divs", { it.divisions.toLong() }),
    // No revenue sorts below every margin, losses included.
    Margin("Net margin", { it.netMarginBps ?: Long.MIN_VALUE }),
}

/** What the rival comparison shows: both companies by quarter and their goods head to head. */
data class RivalCompare(val you: List<StatementDto>, val them: List<StatementDto>, val goods: List<HeadToHeadDto>)

@Composable
fun RivalsBoardDialog(store: GameStore, onClose: () -> Unit) {
    val rows = rememberEngine(store) { it.rivalsBoard() }
    var picked by remember { mutableStateOf<ULong?>(null) }
    val you = store.selectedFirm
    val compare = rememberEngine(store, picked, you) { h ->
        val p = picked ?: return@rememberEngine null
        val me = you ?: return@rememberEngine null
        RivalCompare(h.statements(me, "quarter"), h.statements(p, "quarter"), h.headToHead(me, p))
    }
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        modifier = Modifier.widthIn(min = 360.dp, max = 980.dp),
        title = { Text("Rivals board") },
        text = {
            Column(Modifier.heightIn(max = 620.dp).verticalScroll(rememberScrollState())) {
                if (rows == null) Text("Loading.", color = Palette.secondary) else RivalsBoardView(rows, picked, { picked = it }, compare)
            }
        },
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
}

@Composable
fun RivalsBoardView(rows: List<RivalRowDto>, picked: ULong?, onPick: (ULong) -> Unit, compare: RivalCompare?) {
    var sort by remember { mutableStateOf(RivalSort.Revenue) }
    val sorted = rows.sortedByDescending { sort.key(it) }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Last 13 weeks. Tap a heading to sort, a company to compare it with yours.", color = Palette.secondary, fontSize = 11.sp)
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val wide = maxWidth >= 700.dp
            val columns = if (wide) RivalSort.entries else listOf(RivalSort.Revenue, RivalSort.Profit, RivalSort.Cash, RivalSort.Price)
            fun cell(r: RivalRowDto, c: RivalSort) = when (c) {
                RivalSort.Divisions -> "${r.divisions}"
                RivalSort.Margin -> r.netMarginBps?.let { pctOf(it) } ?: "n/a"
                RivalSort.Price -> Format.price(r.sharePriceCents)
                else -> Format.compact(c.key(r))
            }
            SystemsTable(
                listOf("Company") + columns.map { it.title },
                sorted.map { r -> listOf(r.name) + columns.map { cell(r, it) } },
                weights = listOf(if (wide) 2.4f else 1.8f) + columns.map { 1f },
                yours = { sorted[it].yours },
                sort = columns.indexOf(sort) + 1,
                onSort = { i -> if (i > 0) sort = columns[i - 1] },
                onRow = { i -> onPick(sorted[i].firm) },
                selected = sorted.indexOfFirst { it.firm == picked },
            )
        }
        val rival = rows.firstOrNull { it.firm == picked } ?: return@Column
        val c = compare ?: return@Column
        Eyebrow("You against ${rival.name}")
        fun pts(s: List<StatementDto>, v: (StatementDto) -> Long) = s.map { ChartPoint(it.firstWeek.toInt(), v(it)) }
        Split(
            {
                ChartPanel("Revenue by quarter", listOf(ChartSeries("You", Palette.gold, pts(c.you) { it.revenueCents }), ChartSeries(rival.name, blue, pts(c.them) { it.revenueCents })), ChartKind.Bar)
                ChartPanel("Net income by quarter", listOf(ChartSeries("You", Palette.gold, pts(c.you) { it.netIncomeCents }), ChartSeries(rival.name, blue, pts(c.them) { it.netIncomeCents })), ChartKind.Bar)
            },
            {
                ChartPanel("Cash at each close", listOf(ChartSeries("You", Palette.gold, pts(c.you) { it.cashCents }), ChartSeries(rival.name, blue, pts(c.them) { it.cashCents })), scale = ChartScale.Level)
                ChartPanel("Share price", listOf(ChartSeries("You", Palette.gold, pts(c.you) { it.sharePriceCents }), ChartSeries(rival.name, blue, pts(c.them) { it.sharePriceCents })), format = { Format.price(it) }, scale = ChartScale.Level)
            },
        )
        Eyebrow("Head to head, last week")
        if (c.goods.isEmpty()) Text("Neither company sold goods last week.", color = Palette.secondary, fontSize = 12.sp)
        else SystemsTable(
            listOf("Good", "You", "Share", "Them", "Share"),
            c.goods.sortedByDescending { it.yourLots + it.theirLots }.map { listOf(it.name, Format.count(it.yourLots), pctOf(it.yourShareBps), Format.count(it.theirLots), pctOf(it.theirShareBps)) },
            yours = { i -> c.goods.sortedByDescending { it.yourLots + it.theirLots }[i].let { it.yourLots > 0 && it.theirLots > 0 } },
        )
    }
}

// 6. Economy dashboard.

/** The economy's weekly series and the corridor index, loaded together. */
data class EconomyHistory(val series: List<SeriesDto>, val index: List<PricePointDto>)

@Composable
fun EconomyDashboardSection(store: GameStore) {
    val d = rememberEngine(store) { EconomyHistory(it.economySeries(), it.corridorIndexHistory()) } ?: return
    EconomyDashboardView(d)
}

@Composable
fun EconomyDashboardView(d: EconomyHistory) {
    val s = d.series
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Eyebrow("Economy history")
        if (s.all { it.weeks.size < 2 }) EmptyHistory()
        Split(
            {
                ChartPanel("Corridor share index", listOf(ChartSeries("Index", Palette.gold, d.index.map { ChartPoint(it.week.toInt(), it.cents) })), ChartKind.Area, format = { "%.1f".format(it / 100.0) }, scale = ChartScale.Level)
                ChartPanel(
                    "Bank rate and inflation",
                    listOf(ChartSeries("Bank rate", Palette.gold, s.of("policy_rate_bps")), ChartSeries("Inflation", Palette.negative, s.of("inflation_bps"))),
                    format = { pctOf(it) },
                )
                ChartPanel("Price level", listOf(ChartSeries("Prices", violet, s.of("price_index_bps"))), format = { "%.1f".format(it / 100.0) }, scale = ChartScale.Level)
            },
            {
                ChartPanel(
                    "Companies operating and gone",
                    listOf(ChartSeries("Operating", Palette.positive, s.of("firms_operating")), ChartSeries("Dissolved or merged", Palette.negative, s.of("firms_gone"))),
                    format = { Format.count(it) },
                )
                if (s.of("raids").any { it.value > 0 }) ChartPanel("Raids per week", listOf(ChartSeries("Raids", Palette.warning, s.of("raids")), ChartSeries("Won", Palette.negative, s.of("raids_won"))), ChartKind.Bar, format = { Format.count(it) })
                ChartPanel("Household spending per week", listOf(ChartSeries("Spending", blue, s.of("household_spend"))), ChartKind.Bar)
            },
        )
    }
}
