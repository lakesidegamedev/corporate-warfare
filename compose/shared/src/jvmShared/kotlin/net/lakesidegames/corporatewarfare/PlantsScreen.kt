package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import uniffi.cw_ffi.FacilityDto

/**
 * Company: Plants. One row per plant across every company you control, with its operations
 * figures; the side pane carries the plant's actions (mothball, reopen, expand, retool, list for
 * sale), each through the action catalogue with the plant filled in.
 */

private class PlantRow(val f: FacilityDto, val company: String, val strategy: String)

private fun statusWords(s: String): String = when {
    s.startsWith("operating") -> "Running"
    s.startsWith("building") -> "Building"
    s.startsWith("mothballed") -> "Mothballed"
    s.startsWith("retooling") -> "Retooling"
    else -> s.replaceFirstChar { it.uppercase() }
}

private fun use(f: FacilityDto): Long = if (f.capacityLots > 0) f.lastOutputLots * 10_000 / f.capacityLots else 0L

internal val PlantViews = listOf(
    ViewState("Operations", cols("name", "company", "region", "status", "plants", "capacity", "output", "use", "bottleneck"), sort = "use", descending = false),
    ViewState("Staff", cols("name", "region", "status", "low", "mid", "high", "target")),
    ViewState("Idle and closed", cols("name", "company", "region", "status", "capacity", "reopen"), filters = listOf(Filter(FilterOp.Not, "status", "Running")), sort = "reopen"),
    ViewState("By region", cols("name", "company", "status", "plants", "capacity", "output", "use"), groupBy = "region", sort = "capacity"),
)

private fun plantColumns(): List<GridColumn<PlantRow>> = listOf(
    GridColumn("name", "Plant", null, numeric = false, text = { it.strategy }, minWidth = 170.dp, bold = true, topic = "About"),
    GridColumn("company", "Company", 130.dp, numeric = false, text = { it.company }, tone = { Palette.secondary }, dropOrder = 1, topic = "About"),
    GridColumn("region", "Region", 120.dp, numeric = false, text = { it.f.region }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("status", "Status", 90.dp, numeric = false, text = { statusWords(it.f.status) }, tone = { if (it.f.status.startsWith("operating")) Palette.positive else Palette.warning }, topic = "About"),
    GridColumn("plants", "Plants", 60.dp, text = { "${it.f.blocks}" }, sort = { it.f.blocks.toLong() }, total = true, topic = "Operations"),
    GridColumn("capacity", "Capacity", 80.dp, text = { Format.count(it.f.capacityLots) }, sort = { it.f.capacityLots }, total = true, topic = "Operations", tip = "Lots a week at full tilt."),
    GridColumn("output", "Made last week", 100.dp, text = { Format.count(it.f.lastOutputLots) }, sort = { it.f.lastOutputLots }, total = true, topic = "Operations"),
    GridColumn("use", "Used", 76.dp, text = { "%.0f%%".format(use(it.f) / 100.0) }, sort = { use(it.f) }, bar = { (use(it.f) / 10_000f).coerceIn(0f, 1f) }, scale = 100.0, topic = "Operations", tip = "Last week's output as a share of capacity."),
    GridColumn("bottleneck", "Held back by", 130.dp, numeric = false, text = { if (it.f.bottleneck.isEmpty() || it.f.bottleneck == "none") DASH else it.f.bottleneck }, tone = { Palette.secondary }, dropOrder = 2, topic = "Operations"),
    GridColumn("target", "Target", 64.dp, text = { "%.0f%%".format(it.f.targetBps / 100.0) }, sort = { it.f.targetBps.toLong() }, scale = 100.0, topic = "Operations"),
    GridColumn("low", "Low skill", 76.dp, text = { Format.count(it.f.workersLow) }, sort = { it.f.workersLow }, total = true, topic = "Staff"),
    GridColumn("mid", "Mid skill", 76.dp, text = { Format.count(it.f.workersMid) }, sort = { it.f.workersMid }, total = true, topic = "Staff"),
    GridColumn("high", "High skill", 76.dp, text = { Format.count(it.f.workersHigh) }, sort = { it.f.workersHigh }, total = true, topic = "Staff"),
    GridColumn("reopen", "To reopen", 84.dp, text = { if (it.f.reopenCostCents > 0) Format.compact(it.f.reopenCostCents) else DASH }, sort = { it.f.reopenCostCents }, scale = 100.0, topic = "Money"),
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PlantsScreen(store: GameStore) {
    if (store.selectedFirmDto == null) { Box(Modifier.fillMaxSize().padding(16.dp)) { FoundCompanyPanel(store) }; return }
    val act = LocalActions.current
    val mine = store.firms.filter { it.controlled }
    val rows = rememberEngine(store, store.overview?.tick, mine.size) { h ->
        val names = h.strategies().associate { it.id to it.name }
        mine.flatMap { firm -> h.facilities(firm.handle).map { PlantRow(it, firm.name, names[it.recipe] ?: it.recipe) } }
    }.orEmpty()
    var pick by remember { mutableStateOf<ULong?>(null) }
    val chosen = rows.firstOrNull { it.f.handle == pick } ?: rows.firstOrNull()
    SplitOrStack(Modifier.fillMaxSize().padding(8.dp), 2.4f, 1f, 520.dp, 360.dp, left = { m ->
        DataGrid(
            "Plants", rows, plantColumns(), key = { it.f.handle },
            views = GridViews("plants", PlantViews, mine = { true }, scopes = plantScopes(rows)),
            modifier = m,
            filterText = { "${it.strategy} ${it.company} ${it.f.region} ${statusWords(it.f.status)}" },
            selected = chosen?.f?.handle, onSelect = { pick = it.f.handle },
            breakdown = { p -> "${p.strategy} in ${p.f.region}: ${statusWords(p.f.status)}, ${p.f.blocks} plants, made ${Format.count(p.f.lastOutputLots)} of ${Format.count(p.f.capacityLots)} lots" + (if (p.f.bottleneck.isNotEmpty() && p.f.bottleneck != "none") ", held back by ${p.f.bottleneck}" else "") },
            initialSort = "use", initialDescending = false,
        )
    }, right = { m ->
        DetailPane(chosen?.strategy?.let { "$it, ${chosen.f.region}" } ?: "Plant", m) {
            if (chosen == null) {
                Text("You have no plants yet. Openings (under Markets) shows where a new one would pay.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            val f = chosen.f
            FigureLine(
                Triple("Status", statusWords(f.status), if (f.status.startsWith("operating")) Palette.positive else Palette.warning),
                Triple("Used", "%.0f%%".format(use(f) / 100.0), Palette.primaryText),
                Triple("Plants", "${f.blocks}", Palette.primaryText),
            )
            Text(
                "${chosen.company}. Made ${Format.count(f.lastOutputLots)} of ${Format.count(f.capacityLots)} lots last week with ${Format.count(f.workersLow + f.workersMid + f.workersHigh)} workers." +
                    if (f.bottleneck.isNotEmpty() && f.bottleneck != "none") " Held back by ${f.bottleneck}." else "",
                color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
            )
            val fields = mapOf("facility" to "${f.handle}")
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (f.status.startsWith("mothballed")) GoldButton("Reopen (${Format.compact(f.reopenCostCents)})") { act("ReopenFacility", fields) }
                else if (f.status.startsWith("operating")) {
                    GridButton("Expand", false) { act("ExpandFacility", fields) }
                    GridButton("Mothball", false) { act("MothballFacility", fields) }
                    GridButton("Retool", false) { act("RetoolFacility", fields) }
                }
                GridButton("List for sale", false) { act("ListPlant", fields) }
            }
            Text("Each button opens the action with this plant filled in; you see the cost before you confirm.", color = Palette.secondary, fontSize = 10.sp)
        }
    })
}

private fun plantScopes(rows: List<PlantRow>): List<ScopeOption<PlantRow>> =
    rows.map { it.company }.distinct().sorted().map { c -> ScopeOption<PlantRow>("company:$c", c, "Company") { it.company == c } } +
        rows.map { it.f.region }.distinct().sorted().map { r -> ScopeOption<PlantRow>("region:$r", r, "Region") { it.f.region == r } }
