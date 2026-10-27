package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.PeriodDto
import uniffi.cw_ffi.WeekDto
import kotlin.math.abs
import kotlin.math.max

/** How the stats screen buckets results. */
enum class StatsPeriod(val title: String) { Week("Weekly"), Month("Four-weekly"), Year("Yearly") }

/** One bucket of a company's results, with balances at its close when the engine kept them. */
data class ResultBucket(
    val label: String,
    val revenue: Long,
    val costOfGoods: Long,
    val wages: Long,
    val operating: Long,
    val depreciation: Long,
    val interest: Long,
    val tax: Long,
    val netIncome: Long,
    val cash: Long? = null,
    val equity: Long? = null,
    val liabilities: Long? = null,
) {
    val operatingProfit: Long get() = revenue - costOfGoods - wages - operating - depreciation
}

object ResultBuckets {
    /** The game year is exactly 52 weeks: thirteen periods of four. */
    fun weekLabel(week: Int): String {
        val w = max(week, 1)
        return "W%02d %d".format((w - 1) % 52 + 1, 2000 + (w - 1) / 52)
    }

    fun make(period: StatsPeriod, weeks: List<WeekDto>, history: List<PeriodDto>): List<ResultBucket> = when (period) {
        StatsPeriod.Week -> weeks.map {
            ResultBucket(
                weekLabel(it.week.toInt()), it.revenueCents, it.costOfGoodsCents, it.wagesCents, it.operatingExpenseCents,
                it.depreciationCents, it.netInterestCents, it.taxCents, it.netIncomeCents,
            )
        }
        StatsPeriod.Month -> history.takeLast(13).map {
            val w = max(it.firstWeek.toInt(), 1)
            bucket("P${(w - 1) % 52 / 4 + 1} ${2000 + (w - 1) / 52}", listOf(it))
        }
        StatsPeriod.Year -> history.groupBy { (max(it.firstWeek.toInt(), 1) - 1) / 52 }.toSortedMap()
            .map { (year, periods) -> bucket("${2000 + year}", periods) }
    }

    private fun bucket(label: String, periods: List<PeriodDto>): ResultBucket {
        val last = periods.lastOrNull()
        return ResultBucket(
            label,
            periods.sumOf { it.revenueCents }, periods.sumOf { it.costOfGoodsCents }, periods.sumOf { it.wagesCents },
            periods.sumOf { it.operatingExpenseCents }, periods.sumOf { it.depreciationCents }, periods.sumOf { it.netInterestCents },
            periods.sumOf { it.taxCents }, periods.sumOf { it.netIncomeCents },
            cash = last?.cashCents, equity = last?.equityCents, liabilities = last?.liabilitiesCents,
        )
    }
}

/**
 * The selected company's numbers in depth: results by week, period or year, the income
 * statement, the balance sheet over time, the share price and market value, and who owns it.
 */
@Composable
fun StatsScreen(store: GameStore, onBack: () -> Unit) {
    val firm = store.selectedFirmDto
    var period by remember { mutableStateOf(StatsPeriod.Month) }
    val buckets = ResultBuckets.make(period, store.finance?.recentWeeks ?: emptyList(), store.history)
    val stock = firm?.let { f -> store.stocks.firstOrNull { it.firm == f.handle } }
    TabList {
        item {
            ScreenHeader("Stats", firm?.name) { TextButton(onClick = onBack) { Text("Done", color = Palette.gold) } }
        }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                StatsPeriod.entries.forEach { option ->
                    Text(
                        option.title,
                        color = if (period == option) Palette.ink else Palette.primaryText,
                        fontSize = 12.sp,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(CircleShape)
                            .background(if (period == option) Palette.gold else Palette.panel)
                            .clickable { period = option }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }
            }
        }
        if (buckets.isEmpty()) {
            item { Callout("Results appear after the first week closes.") }
        } else {
            item {
                Panel {
                    Eyebrow("Revenue and profit")
                    ResultsChart(buckets)
                    Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        Legend(Palette.gold.copy(alpha = 0.55f), "Revenue")
                        Legend(Palette.positive, "Net income")
                        Legend(Palette.negative, "Loss")
                    }
                }
            }
            item { Statement(buckets) }
            if (period != StatsPeriod.Week && buckets.any { it.cash != null }) {
                item {
                    Panel {
                        Eyebrow("Balance sheet at each close")
                        LineChart(
                            listOf(
                                Palette.gold to buckets.map { it.cash ?: 0 },
                                Palette.positive to buckets.map { it.equity ?: 0 },
                                Palette.negative to buckets.map { it.liabilities ?: 0 },
                            ),
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                            Legend(Palette.gold, "Cash")
                            Legend(Palette.positive, "Equity")
                            Legend(Palette.negative, "Liabilities")
                        }
                    }
                }
            }
        }
        if (stock != null) {
            item {
                val annual = if (stock.earningsWeeks > 0u) stock.earningsCents * 52 / stock.earningsWeeks.toLong() else 0L
                val pe = if (annual > 0) "%.1f".format(stock.marketCapCents.toDouble() / annual) else "n/a"
                StatGroup(
                    Stat("Market value", Format.money(stock.marketCapCents), note = "${Format.price(stock.priceCents)} a share, ${Format.bps(stock.changeQuarterBps)} this quarter"),
                    listOf(
                        Stat("Shares issued", Format.count(stock.sharesIssued)),
                        Stat("Free float", Format.count(stock.sharesIssued * stock.floatBps / 10_000), note = "%.1f%%".format(stock.floatBps / 100.0)),
                        Stat("You hold", Format.count(stock.yourShares)),
                        Stat("Fair value", Format.money(stock.fairCents)),
                        Stat("Price to earnings", pe),
                        Stat("Earnings a year", Format.compact(annual)),
                    ),
                )
            }
            if (stock.historyCents.size > 1) {
                item {
                    Panel {
                        Eyebrow("Share price, last ${stock.historyCents.size} weeks")
                        LineChart(listOf(Palette.gold to stock.historyCents))
                    }
                }
            }
        }
        val ownership = store.ownership
        if (ownership != null && ownership.firm == firm?.handle) {
            item {
                Panel {
                    Eyebrow("Who owns it")
                    ownership.holders.sortedByDescending { it.quantity }.take(8).forEach { holder ->
                        val share = if (ownership.issued > 0) holder.quantity.toFloat() / ownership.issued else 0f
                        Row(Modifier.fillMaxWidth()) {
                            Text(holder.holder, color = Palette.primaryText, fontSize = 13.sp, modifier = Modifier.weight(1f))
                            Text("%.1f%% · %.1f%% of votes".format(share * 100, holder.votesBps / 100.0), color = Palette.primaryText, fontSize = 13.sp)
                        }
                        Box(Modifier.fillMaxWidth().height(5.dp).clip(CircleShape).background(Palette.stroke)) {
                            Box(Modifier.fillMaxWidth(share.coerceIn(0f, 1f)).height(5.dp).clip(CircleShape).background(Palette.gold))
                        }
                    }
                    ownership.controller?.let { Text("Controlled by $it", color = Palette.secondary, fontSize = 12.sp) }
                }
            }
        }
    }
}

@Composable
internal fun Legend(color: Color, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(10.dp).clip(RoundedCornerShape(2.dp)).background(color))
        Spacer(Modifier.width(5.dp))
        Text(text, color = Palette.secondary, fontSize = 11.sp)
    }
}

/** Paired bars per bucket: revenue, then net income above or below zero. */
@Composable
internal fun ResultsChart(buckets: List<ResultBucket>) {
    val measurer = androidx.compose.ui.text.rememberTextMeasurer()
    Canvas(Modifier.fillMaxWidth().height(200.dp)) {
        val top = buckets.maxOf { max(it.revenue, it.netIncome) }.coerceAtLeast(1)
        val bottom = buckets.minOf { minOf(it.netIncome, 0) }
        val span = (top - bottom).toFloat()
        val labelRoom = 18f
        val plot = size.height - labelRoom
        fun y(value: Long) = plot * (top - value) / span
        val zero = y(0)
        drawLine(Palette.stroke, Offset(0f, zero), Offset(size.width, zero), 1f)
        val slot = size.width / buckets.size
        val bar = slot * 0.34f
        buckets.forEachIndexed { i, b ->
            val x = i * slot + slot * 0.14f
            drawRect(Palette.gold.copy(alpha = 0.55f), Offset(x, y(b.revenue)), Size(bar, zero - y(b.revenue)))
            val ny = y(b.netIncome)
            drawRect(
                if (b.netIncome >= 0) Palette.positive else Palette.negative,
                Offset(x + bar + 2f, minOf(ny, zero)), Size(bar, abs(zero - ny)),
            )
            if (buckets.size <= 7 || i % ((buckets.size + 5) / 6) == 0) {
                val label = measurer.measure(b.label, androidx.compose.ui.text.TextStyle(color = Palette.secondary, fontSize = 9.sp))
                drawText(label, topLeft = Offset(x, size.height - label.size.height))
            }
        }
        val scale = measurer.measure(Format.compact(top), androidx.compose.ui.text.TextStyle(color = Palette.secondary, fontSize = 9.sp))
        drawText(scale, topLeft = Offset(size.width - scale.size.width, 0f))
    }
}

/** One or more series on shared axes. */
@Composable
internal fun LineChart(series: List<Pair<Color, List<Long>>>) {
    Canvas(Modifier.fillMaxWidth().height(160.dp)) {
        val all = series.flatMap { it.second }
        if (all.isEmpty()) return@Canvas
        val top = all.max()
        val bottom = minOf(all.min(), 0L)
        val span = max(top - bottom, 1L).toFloat()
        fun y(value: Long) = size.height * (top - value) / span
        drawLine(Palette.stroke, Offset(0f, y(0)), Offset(size.width, y(0)), 1f)
        series.forEach { (color, values) ->
            if (values.size < 2) return@forEach
            val step = size.width / (values.size - 1)
            val path = Path()
            values.forEachIndexed { i, v -> if (i == 0) path.moveTo(0f, y(v)) else path.lineTo(i * step, y(v)) }
            drawPath(path, color, style = Stroke(2.5f))
        }
    }
}

/** The latest bucket's income statement beside the one before it. */
@Composable
internal fun Statement(buckets: List<ResultBucket>) {
    val latest = buckets.last()
    val previous = buckets.getOrNull(buckets.size - 2)
    val rows: List<Triple<String, Boolean, (ResultBucket) -> Long>> = listOf(
        Triple("Revenue", false) { it.revenue },
        Triple("Cost of goods", false) { -it.costOfGoods },
        Triple("Wages", false) { -it.wages },
        Triple("Operating costs", false) { -it.operating },
        Triple("Depreciation", false) { -it.depreciation },
        Triple("Operating profit", true) { it.operatingProfit },
        Triple("Interest", false) { it.interest },
        Triple("Tax", false) { -it.tax },
        Triple("Net income", true) { it.netIncome },
    )
    Panel {
        Eyebrow("Income statement")
        Row(Modifier.fillMaxWidth()) {
            Spacer(Modifier.weight(1.4f))
            Text(latest.label, color = Palette.secondary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            if (previous != null) Text(previous.label, color = Palette.secondary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
        }
        rows.forEach { (title, total, value) ->
            Row(Modifier.fillMaxWidth()) {
                val weight = if (total) FontWeight.SemiBold else FontWeight.Normal
                Text(title, color = Palette.primaryText, fontSize = 13.sp, fontWeight = weight, modifier = Modifier.weight(1.4f))
                listOfNotNull(latest, previous).forEachIndexed { i, bucket ->
                    val amount = value(bucket)
                    val tone = if (!total) Palette.primaryText else if (amount >= 0) Palette.positive else Palette.negative
                    Text(
                        Format.money(amount), color = tone.copy(alpha = if (i == 0) 1f else 0.7f), fontSize = 13.sp, fontWeight = weight,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
        }
        if (latest.revenue > 0) {
            val margin = latest.netIncome * 100.0 / latest.revenue
            Text("Net margin %.1f%%".format(margin), color = if (margin >= 0) Palette.positive else Palette.negative, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
        }
    }
}
