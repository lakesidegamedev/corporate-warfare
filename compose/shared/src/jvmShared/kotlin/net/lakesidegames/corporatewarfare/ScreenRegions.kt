package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.RegionBoardDto
import uniffi.cw_ffi.RegionDossierDto

/**
 * Markets: Regions. Every region as one row: wages, skills, roads, power, what households want and
 * go without, migration, your plants and the company that leads against you. The region's dossier
 * opens beside the grid, with Build here.
 */

/** What the grid reads for one region, worked out once a week from the boards, the dossiers and the openings. */
internal class RegionRow(
    val board: RegionBoardDto,
    val dossier: RegionDossierDto?,
    val myBlocks: Int,
    val myDivisions: Int,
    val unmetLots: Long,
    val rival: String?,
    val rivalShareBps: Int,
    val rivalSector: String?,
)

private fun pctBps(bps: Int) = "${(bps + 50) / 100}%"

private fun series(d: RegionDossierDto?, key: String): List<Long> = d?.series?.firstOrNull { it.key == key }?.values.orEmpty()

@Composable
fun RegionsScreen(store: GameStore, startPick: UInt? = null) {
    val boards = store.boards
    val firm = store.selectedFirm ?: 0uL
    val dossiers = rememberEngine(store, store.overview?.tick, boards.size) { h -> boards.associate { it.index to h.regionDossier(it.index, firm) } }.orEmpty()
    // What households go without, by region: the engine's capacity openings, one figure per good.
    val unmet = rememberEngine(store, store.overview?.tick) { h ->
        h.capacityOpenings(2000u).groupBy { it.region }.mapValues { (_, rows) -> rows.groupBy { it.productId }.values.sumOf { g -> g.maxOf { it.unmetLots } } }
    }.orEmpty()
    val rows = boards.map { b ->
        val mine = store.divisions.filter { it.region == b.code }
        // The company that leads the most sectors here, other than yours.
        val lead = b.sectors.filter { it.leaderFirm != firm && it.leaderShareBps > 0 }.maxByOrNull { it.leaderShareBps }
        RegionRow(b, dossiers[b.index], mine.sumOf { it.blocks.toInt() }, mine.size, unmet[b.index] ?: 0L, lead?.leader, lead?.leaderShareBps ?: 0, lead?.industry)
    }
    var pick by remember { mutableStateOf(startPick) }
    val chosen = rows.firstOrNull { it.board.index == pick } ?: rows.firstOrNull { it.myBlocks > 0 } ?: rows.firstOrNull()
    val act = LocalActions.current
    BoxWithConstraints(Modifier.fillMaxSize().padding(8.dp)) {
        val wide = maxWidth >= 900.dp
        Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DataGrid(
                "Regions", rows, regionColumns(), key = { it.board.index },
                watchKey = { "region:" + it.board.name },
                views = GridViews("regions", RegionViews, mine = { it.myBlocks > 0 }, scopes = regionScopes(rows)),
                modifier = Modifier.weight(1f).fillMaxHeight(),
                filterText = { "${it.board.name} ${it.board.stateName} ${it.board.hubCity}" },
                selected = chosen?.board?.index, onSelect = { pick = it.board.index }, onOpen = if (wide) null else { r: RegionRow -> pick = r.board.index },
                breakdown = { r ->
                    "${r.board.name}, ${r.board.stateName}: wages ${pctBps(r.board.wageIndexBps)} of the corridor, skills ${pctBps(r.board.skillBps)}, roads ${pctBps(r.board.roadBps)}, power ${pctBps(r.board.powerGridBps)}, " +
                        "households go without ${Format.count(r.unmetLots)} lots a week, net migration ${signed(r.board.netMigrationYear)} a year" + (if (r.myBlocks > 0) ", you run ${r.myBlocks} plants" else "")
                },
                initialSort = "plants",
            )
            if (wide) {
                DetailPane(chosen?.board?.name ?: "Region", Modifier.width(470.dp).fillMaxHeight()) {
                    if (chosen == null) {
                        Text("No regions in this game.", color = Palette.secondary, fontSize = 12.sp)
                        return@DetailPane
                    }
                    FigureLine(
                        Triple("Your plants", "${chosen.myBlocks}", if (chosen.myBlocks > 0) Palette.gold else Palette.primaryText),
                        Triple("Going without", Format.count(chosen.unmetLots), if (chosen.unmetLots > 0) Palette.warning else Palette.primaryText),
                        Triple("Migration", signed(chosen.board.netMigrationYear) + " a yr", if (chosen.board.netMigrationYear >= 0) Palette.positive else Palette.negative),
                    )
                    chosen.rival?.let { Text("Strongest company here besides you: $it, ${pctBps(chosen.rivalShareBps)} of ${Sectors.name(chosen.rivalSector.orEmpty())}.", color = Palette.primaryText, fontSize = 12.sp) }
                    GoldButton("Build here") { act("BuildFacility", mapOf("region" to "${chosen.board.index}")) }
                    Text("Opens the build form with this region filled in; you choose the strategy and how many plants.", color = Palette.secondary, fontSize = 10.sp)
                    chosen.dossier?.let { RegionDossierView(it, onDivision = { industry -> store.divisions.firstOrNull { d -> d.industry == industry && d.regionName == it.name }?.let(store::showDivision) }) }
                }
            }
        }
    }
}

private fun signed(n: Long): String = (if (n > 0) "+" else if (n < 0) "-" else "") + Format.count(kotlin.math.abs(n))

private fun regionColumns(): List<GridColumn<RegionRow>> = listOf(
    GridColumn("name", "Region", null, minWidth = 170.dp, numeric = false, text = { it.board.name }, tone = { if (it.myBlocks > 0) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("state", "State", 110.dp, numeric = false, text = { it.board.stateName }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("pop", "Workers", 80.dp, text = { it.dossier?.population?.let(Format::count) ?: DASH }, sort = { it.dossier?.population ?: Long.MIN_VALUE }, series = { series(it.dossier, "population") },
        fmt = { Format.count(it) }, total = true, topic = "Labour", tip = "People of working age here. Its change and trend use the weekly figures the engine keeps."),
    GridColumn("idle", "Without a job", 92.dp, text = { r -> r.dossier?.let { Format.count(it.population - it.employed) } ?: DASH }, sort = { r -> r.dossier?.let { it.population - it.employed } ?: Long.MIN_VALUE },
        fmt = { Format.count(it) }, total = true, topic = "Labour"),
    GridColumn("wage", "Wage, week", 84.dp, text = { it.dossier?.wageCents?.let { c -> Format.money(c) } ?: DASH }, sort = { it.dossier?.wageCents ?: Long.MIN_VALUE }, series = { series(it.dossier, "wage") },
        fmt = { Format.money(it) }, scale = 100.0, topic = "Labour", tip = "The average weekly wage. Its change and trend use the weekly figures."),
    GridColumn("wageidx", "Wage level", 76.dp, text = { pctBps(it.board.wageIndexBps) }, sort = { it.board.wageIndexBps.toLong() }, scale = 100.0, topic = "Labour", tip = "Wages here against the corridor average (100% is the average)."),
    GridColumn("skill", "Skills", 64.dp, text = { pctBps(it.board.skillBps) }, sort = { it.board.skillBps.toLong() }, bar = { it.board.skillBps / 10_000f }, scale = 100.0, topic = "Labour", tip = "How skilled the workforce is."),
    GridColumn("roads", "Roads", 64.dp, text = { pctBps(it.board.roadBps) }, sort = { it.board.roadBps.toLong() }, bar = { it.board.roadBps / 10_000f }, scale = 100.0, topic = "Infrastructure"),
    GridColumn("power", "Power", 64.dp, text = { pctBps(it.board.powerGridBps) }, sort = { it.board.powerGridBps.toLong() }, bar = { it.board.powerGridBps / 10_000f }, scale = 100.0, topic = "Infrastructure"),
    GridColumn("broadband", "Broadband", 80.dp, text = { pctBps(it.board.broadbandBps) }, sort = { it.board.broadbandBps.toLong() }, bar = { it.board.broadbandBps / 10_000f }, scale = 100.0, topic = "Infrastructure"),
    GridColumn("land", "Land", 62.dp, text = { pctBps(it.board.landIndexBps) }, sort = { it.board.landIndexBps.toLong() }, series = { series(it.dossier, "land_index") }, fmt = { pctBps(it.toInt()) }, scale = 100.0, topic = "Cost",
        tip = "The land price against the corridor average."),
    GridColumn("demand", "Demand", 70.dp, text = { pctBps(it.board.demandIndexBps) }, sort = { it.board.demandIndexBps.toLong() }, scale = 100.0, topic = "Households", tip = "How much households here ask for, against the corridor average."),
    GridColumn("spend", "Spend, week", 90.dp, text = { it.dossier?.spendCents?.let { c -> Format.compact(c) } ?: DASH }, sort = { it.dossier?.spendCents ?: Long.MIN_VALUE }, series = { series(it.dossier, "spend") },
        fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Households", tip = "What households here spend each week. Its change and trend use the weekly figures."),
    GridColumn("unmet", "Going without", 96.dp, text = { if (it.unmetLots > 0) Format.count(it.unmetLots) else "" }, sort = { it.unmetLots }, tone = { Palette.warning }, total = true, topic = "Households",
        tip = "Lots a week households asked for here and did not get, counted once per good."),
    GridColumn("migration", "Migration, yr", 96.dp, text = { signed(it.board.netMigrationYear) }, sort = { it.board.netMigrationYear }, tone = { changeTint(it.board.netMigrationYear) }, total = true, topic = "Households",
        tip = "Workers moving in (plus) or out (minus) over the last year."),
    GridColumn("plants", "Your plants", 80.dp, text = { if (it.myBlocks > 0) "${it.myBlocks}" else "" }, sort = { it.myBlocks.toLong() }, tone = { Palette.gold }, total = true, topic = "You"),
    GridColumn("divs", "Your divisions", 96.dp, text = { if (it.myDivisions > 0) "${it.myDivisions}" else "" }, sort = { it.myDivisions.toLong() }, tone = { Palette.gold }, total = true, topic = "You"),
    GridColumn("rival", "Leading rival", 150.dp, numeric = false, text = { it.rival?.let { n -> "$n ${pctBps(it.rivalShareBps)}" } ?: DASH }, tone = { Palette.secondary }, topic = "Rivals",
        tip = "The company with the biggest share of a sector here, besides you."),
)

private fun regionScopes(rows: List<RegionRow>): List<ScopeOption<RegionRow>> =
    rows.map { it.board.stateName }.distinct().sorted().map { s -> ScopeOption<RegionRow>("state:$s", s, "State") { it.board.stateName == s } }

internal val RegionViews = listOf(
    ViewState("Overview", cols("name", "wage", "skill", "roads", "power", "demand", "unmet", "migration", "plants", "rival"), sort = "plants"),
    ViewState("Labour", cols("name", "pop", "pop.Percent", "idle", "wage", "wage.Percent", "wage.Spark", "wageidx", "skill"), sort = "wageidx", descending = false),
    ViewState("Infrastructure", cols("name", "roads", "power", "broadband", "skill", "land"), sort = "roads"),
    ViewState("Households", cols("name", "demand", "spend", "spend.Percent", "spend.Spark", "unmet", "migration"), sort = "unmet"),
    ViewState("My plants", cols("name", "plants", "divs", "wage", "wage.Percent", "roads", "power", "unmet", "rival"), sort = "plants", filters = listOf(Filter(FilterOp.Mine))),
)
