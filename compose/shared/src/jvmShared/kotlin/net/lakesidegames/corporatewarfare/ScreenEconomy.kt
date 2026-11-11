package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import uniffi.cw_ffi.IncidentDto
import uniffi.cw_ffi.WorldPriceDto

/**
 * World: Economy. The macro indicators as series that follow the timeframe, with the cycle phase,
 * the rates and inflation beside them, the world price families, and the named shocks now running.
 * Series are kept in hundredths of their unit (basis points for rates, hundredths of a company for
 * counts), so a change reads the same way in every row.
 */
internal class Indicator(val id: String, val name: String, val group: String, val now: String, val value: Long, val series: List<Long>, val tip: String)

private fun pct2(bps: Long) = "%.2f%%".format(bps / 100.0)

/** A figure kept in hundredths, read back in plain units and shortened when large. */
internal fun unitless(v: Long): String {
    val x = v / 100.0
    val a = kotlin.math.abs(x)
    return when {
        a >= 1_000_000 -> "%.1fM".format(x / 1_000_000)
        a >= 10_000 -> "%.1fK".format(x / 1_000)
        a >= 100 -> "%.0f".format(x)
        else -> "%.1f".format(x)
    }
}

@Composable
fun EconomyScreen(store: GameStore) {
    val e = store.economy
    val data = rememberEngine(store, store.overview?.tick) { h -> h.economySeries() to h.corridorIndexHistory() }
    val series = data?.first.orEmpty()
    fun of(key: String) = series.firstOrNull { it.key == key }?.values.orEmpty()
    val rows = buildList {
        if (e != null) {
            val gap = e.history.map { it.outputGapBps.toLong() }
            add(Indicator("gap", "Output against trend", "Cycle", pct2(e.outputGapBps.toLong()), e.outputGapBps.toLong(), gap, "How far the economy runs above or below its trend. Above is a boom, below a slump."))
        }
        val inflation = of("inflation_bps")
        add(Indicator("inflation", "Inflation", "Prices", inflation.lastOrNull()?.let(::pct2) ?: DASH, inflation.lastOrNull() ?: 0, inflation, "How fast prices are rising."))
        val rate = of("policy_rate_bps")
        add(Indicator("rate", "Bank rate", "Rates", rate.lastOrNull()?.let(::pct2) ?: DASH, rate.lastOrNull() ?: 0, rate, "The central bank's rate."))
        val level = of("price_index_bps")
        add(Indicator("level", "Price level", "Prices", level.lastOrNull()?.let(::unitless) ?: DASH, level.lastOrNull() ?: 0, level, "What an ordinary basket costs, against 100 at the start."))
        val corridor = data?.second.orEmpty().map { it.cents }
        add(Indicator("corridor", "Corridor share index", "Markets", corridor.lastOrNull()?.let(::unitless) ?: DASH, corridor.lastOrNull() ?: 0, corridor, "All the corridor's companies' shares as one index."))
        val operating = of("firms_operating").map { it * 100 }
        add(Indicator("operating", "Companies operating", "Business", operating.lastOrNull()?.let(::unitless) ?: DASH, operating.lastOrNull() ?: 0, operating, "Companies still trading."))
        val gone = of("firms_gone").map { it * 100 }
        add(Indicator("gone", "Companies gone", "Business", gone.lastOrNull()?.let(::unitless) ?: DASH, gone.lastOrNull() ?: 0, gone, "Companies dissolved or merged away, added up."))
        val raids = of("raids").map { it * 100 }
        add(Indicator("raids", "Raids a week", "Business", raids.lastOrNull()?.let(::unitless) ?: DASH, raids.lastOrNull() ?: 0, raids, "Raids decided in the week."))
        val spend = of("household_spend")
        add(Indicator("spend", "Household spending, week", "Households", spend.lastOrNull()?.let { Format.compact(it) } ?: DASH, spend.lastOrNull() ?: 0, spend, "What households spent in the week."))
    }
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = rows.firstOrNull { it.id == pick } ?: rows.firstOrNull()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Indicators", rows, indicatorColumns(), key = { it.id },
            views = GridViews("economy", EconomyViews, scopes = indicatorScopes(rows)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.name} ${it.group}" },
            selected = chosen?.id, onSelect = { pick = it.id },
            breakdown = { it.tip },
            initialSort = "name", initialDescending = false,
        )
        DetailPane("The economy", Modifier.width(520.dp).fillMaxHeight()) {
            if (e == null) {
                Text("The economy is not ready yet.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("Phase", e.phase.replaceFirstChar { it.uppercase() }, economyPhaseTint(e.phase)),
                Triple("Growth", pct2(e.growthBps.toLong()), if (e.growthBps >= 0) Palette.positive else Palette.negative),
                Triple("Inflation", pct2(e.inflationBps.toLong()), if (e.inflationBps > e.targetInflationBps + 100) Palette.warning else Palette.primaryText),
            )
            FigureLine(
                Triple("Bank rate", pct2(e.policyRateBps.toLong()), Palette.gold),
                Triple("Prime rate", pct2(e.primeRateBps.toLong()), Palette.primaryText),
                Triple("10-year bond", pct2(e.tenYearYieldBps.toLong()), Palette.primaryText),
            )
            Text(
                "The bank targets ${pct2(e.targetInflationBps.toLong())} inflation and expects ${pct2(e.expectedInflationBps.toLong())}. " +
                    "It meets again in week ${e.nextMeetingWeek}" + (if (e.chairName.isNotEmpty()) ", chaired by ${e.chairName} (${e.chairStance})" else "") + ". " +
                    when {
                        e.creditBps >= 800 -> "Credit is badly stretched: a bust could come at any time."
                        e.creditBps >= 450 -> "Borrowing is running high: the risk of a bust is rising."
                        else -> "Credit is at a normal level."
                    },
                color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
            )
            if (e.worldPrices.isNotEmpty()) {
                DataGrid(
                    "World price families", e.worldPrices, familyColumns(), key = { it.family },
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(e.worldPrices.size.coerceAtMost(10))),
                    initialSort = "index",
                )
            }
            val shocks = e.incidents
            if (shocks.isNotEmpty()) {
                DataGrid(
                    "Shocks", shocks, shockColumns(), key = { "${it.week}${it.kind}${it.text.hashCode()}" },
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(shocks.size.coerceAtMost(8))),
                    initialSort = "week",
                )
            } else {
                Text("No shock is running. A named shock (a bank run, a supply crunch) appears here while it lasts.", color = Palette.secondary, fontSize = 11.sp)
            }
        }
    }
}

private fun indicatorColumns(): List<GridColumn<Indicator>> = listOf(
    GridColumn("name", "Indicator", null, minWidth = 200.dp, numeric = false, text = { it.name }, bold = true),
    GridColumn("group", "Kind", 96.dp, numeric = false, text = { it.group }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("now", "Now", 100.dp, text = { it.now }, sort = { it.value }, series = { it.series }, fmt = ::unitless, scale = 100.0, topic = "Figures",
        tip = "The latest figure. Its change, percent and trend follow the timeframe in the top strip, from the weekly record the game keeps."),
    GridColumn("high", "High", 84.dp, text = { r -> r.series.takeLast(52).maxOrNull()?.let(::unitless) ?: DASH }, sort = { r -> r.series.takeLast(52).maxOrNull() ?: Long.MIN_VALUE }, scale = 100.0, topic = "Figures", tip = "The highest week of the last year."),
    GridColumn("low", "Low", 84.dp, text = { r -> r.series.takeLast(52).minOrNull()?.let(::unitless) ?: DASH }, sort = { r -> r.series.takeLast(52).minOrNull() ?: Long.MAX_VALUE }, scale = 100.0, topic = "Figures", tip = "The lowest week of the last year."),
)

private fun indicatorScopes(rows: List<Indicator>): List<ScopeOption<Indicator>> =
    rows.map { it.group }.distinct().map { g -> ScopeOption<Indicator>("group:$g", g, "Kind") { it.group == g } }

internal val EconomyViews = listOf(
    ViewState("Overview", cols("name", "group", "now", "now.Change", "now.Spark"), sort = "name", descending = false),
    ViewState("Rates and prices", cols("name", "group", "now", "now.Change", "now.Percent", "high", "low"), sort = "group", descending = false),
    ViewState("Business", cols("name", "now", "now.Percent", "now.Spark"), sort = "name", descending = false, filters = listOf(Filter(FilterOp.Is, "group", "Business"))),
    ViewState("Year range", cols("name", "now", "high", "low", "now.Percent"), sort = "name", descending = false),
)

private fun familyColumns(): List<GridColumn<WorldPriceDto>> = listOf(
    GridColumn("name", "Family", null, minWidth = 130.dp, numeric = false, text = { it.label }, bold = true),
    GridColumn("index", "Level", 66.dp, text = { "%.0f%%".format(it.indexBps / 100.0) }, sort = { it.indexBps.toLong() }, bar = { (it.indexBps / 20_000f).coerceIn(0f, 1f) }, scale = 100.0, tip = "The family's world price against its normal level, 100% is normal."),
    GridColumn("trend", "Trend", 64.dp, text = { changeText(it.trendBps.toLong()) }, sort = { it.trendBps.toLong() }, tone = { changeTint(it.trendBps.toLong()) }, scale = 100.0, tip = "Where the price is heading."),
    GridColumn("goods", "Goods", 56.dp, text = { "${it.products.size}" }, sort = { it.products.size.toLong() }),
)

private fun shockColumns(): List<GridColumn<IncidentDto>> = listOf(
    GridColumn("what", "Shock", null, minWidth = 150.dp, numeric = false, text = { it.text }, tone = { if (it.touchesYou) Palette.warning else Palette.primaryText }),
    GridColumn("week", "Started", 76.dp, numeric = false, text = { ResultBuckets.weekLabel(it.week.toInt()) }, sort = { it.week.toLong() }, tone = { Palette.secondary }),
    GridColumn("left", "Weeks left", 76.dp, text = { "${it.weeksLeft}" }, sort = { it.weeksLeft.toLong() }),
)
