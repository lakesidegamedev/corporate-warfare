package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.CurrencyDto
import uniffi.cw_ffi.EconomyDto
import uniffi.cw_ffi.FxDto
import uniffi.cw_ffi.LawsDto

internal fun economyPhaseTint(phase: String): Color = when (phase) {
    "recession" -> Palette.negative
    "boom" -> Palette.gold
    "slowdown" -> Palette.warning
    "recovery" -> Palette.positive
    else -> Color(0xFF42A5F5)
}

private fun pct(bps: Int, digits: Int = 1) = "%.${digits}f%%".format(bps / 100.0)

@Composable
private fun PhasePill(phase: String) {
    Text(
        phase.replaceFirstChar { it.uppercase() }, color = Palette.ink, fontSize = 11.sp, fontWeight = FontWeight.Bold,
        modifier = Modifier.clip(Pixel.small).background(economyPhaseTint(phase)).padding(horizontal = 9.dp, vertical = 2.dp),
    )
}

/**
 * The economy, the central bank and the government. On phones it is a one-line header that
 * opens on tap; elsewhere the full card.
 */
@Composable
fun EconomyCard(store: GameStore, collapsible: Boolean) {
    val e = store.economy ?: return
    var open by remember { mutableStateOf(!collapsible) }
    Panel {
        Row(
            Modifier.fillMaxWidth().then(if (collapsible) Modifier.clickable { open = !open } else Modifier),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Eyebrow("Economy")
            Spacer(Modifier.width(8.dp))
            PhasePill(e.phase)
            Spacer(Modifier.weight(1f))
            Text("Rate ${pct(e.policyRateBps, 2)} · inflation ${pct(e.inflationBps)}", color = Palette.secondary, fontSize = 12.sp)
            if (collapsible) PixelIcon(if (open) "chevron_up" else "chevron_down", size = 12.dp, tint = Palette.secondary, modifier = Modifier.padding(start = 6.dp))
        }
        if (!open) return@Panel
        StatGroup(
            null,
            listOf(
                Stat("Growth, last year", pct(e.growthBps), if (e.growthBps >= 0) Palette.positive else Palette.negative),
                Stat("Inflation", pct(e.inflationBps), if (e.inflationBps > e.targetInflationBps + 100) Palette.warning else Palette.primaryText, "target ${pct(e.targetInflationBps)}"),
                Stat("Output vs trend", "%+.1f%%".format(e.outputGapBps / 100.0), if (e.outputGapBps >= 0) Palette.positive else Palette.negative),
                Stat("Central bank rate", pct(e.policyRateBps, 2)),
                Stat("Prime lending rate", pct(e.primeRateBps, 2)),
                Stat("10-year bond", pct(e.tenYearYieldBps, 2)),
            ),
        )
        History(e)
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            Key(Color(0xFF42A5F5), "Output vs trend")
            Key(Palette.warning, "Inflation")
            Key(Palette.gold, "Central bank rate")
        }
        val credit = when {
            e.creditBps >= 800 -> "Credit is badly stretched: a bust could come at any time." to Palette.negative
            e.creditBps >= 450 -> "Borrowing is running high: the risk of a bust is rising." to Palette.warning
            else -> "Borrowing is moderate." to Palette.secondary
        }
        Text(credit.first, color = credit.second, fontSize = 12.sp)

        e.fx?.let { FxSection(store, it) }

        if (e.worldPrices.isNotEmpty()) {
            Eyebrow("World prices")
            Text(
                when (e.pricePath) {
                    "historical" -> "Outside prices follow the real path of the period. They set what gateways charge to import and pay to export."
                    "historical_variance" -> "Outside prices are shaped like the period, with their own timing and depth. They set what gateways charge to import and pay to export."
                    else -> "Outside prices drift around where they began. They set what gateways charge to import and pay to export."
                },
                color = Palette.secondary, fontSize = 12.sp,
            )
            e.worldPrices.forEach { w ->
                val level = (w.indexBps - 10_000) / 100.0
                val trend = w.trendBps / 100.0
                Row {
                    Text(w.label, color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Text("%+.0f%% since day one".format(level), color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.width(150.dp))
                    Text(
                        if (trend > 0.5) "up %.0f%% this quarter".format(trend) else if (trend < -0.5) "down %.0f%% this quarter".format(-trend) else "steady",
                        color = if (trend > 0.5) Palette.warning else if (trend < -0.5) Palette.positive else Palette.secondary,
                        fontSize = 12.sp, modifier = Modifier.width(150.dp),
                    )
                }
            }
        }

        Eyebrow("Central bank")
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (e.chairPerson.isNotEmpty()) PixelPortrait(e.chairPerson, size = 36.dp)
            Text(e.chairName, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            Spacer(Modifier.width(8.dp))
            Text(
                e.chairStance, color = Palette.ink, fontSize = 11.sp, fontWeight = FontWeight.Bold,
                modifier = Modifier.clip(Pixel.small).background(if (e.chairStance.contains("hawk")) Palette.negative.copy(alpha = 0.8f) else if (e.chairStance.contains("dov")) Color(0xFF42A5F5) else Palette.secondary).padding(horizontal = 8.dp, vertical = 2.dp),
            )
        }
        Text("Next meeting ${ResultBuckets.weekLabel(e.nextMeetingWeek.toInt())} · term ends ${ResultBuckets.weekLabel(e.chairTermEnds.toInt())}", color = Palette.secondary, fontSize = 12.sp)
        e.decisions.take(4).forEach { d ->
            val what = when {
                d.changeBps > 0 -> "Raised ${pct(d.changeBps, 2)} to ${pct(d.rateBps, 2)}"
                d.changeBps < 0 -> "Cut ${pct(-d.changeBps, 2)} to ${pct(d.rateBps, 2)}"
                else -> "Held at ${pct(d.rateBps, 2)}"
            }
            Text("${ResultBuckets.weekLabel(d.week.toInt())}: $what, citing ${d.reason}.", color = Palette.primaryText, fontSize = 12.sp)
        }

        Eyebrow("Government")
        e.posts.filter { it.post != "chair" }.forEach { post ->
            PersonLine(post.person, when (post.post) {
                "country_leader" -> "Leads the country, ${post.place}"
                "party_leader" -> "Opposition, ${post.place}"
                else -> "Governor of ${post.place}"
            })
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(e.partyName, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, modifier = Modifier.weight(1f))
            Text("Approval ${e.approvalBps / 100}%", color = Palette.secondary, fontSize = 12.sp)
        }
        Box(Modifier.fillMaxWidth().height(6.dp).clip(Pixel.small).background(Palette.panelRaised)) {
            Box(Modifier.fillMaxWidth((e.approvalBps / 10_000f).coerceIn(0f, 1f)).height(6.dp).clip(Pixel.small).background(if (e.approvalBps >= 5_000) Palette.positive else Palette.warning))
        }
        Text("Next election ${ResultBuckets.weekLabel(e.nextElectionWeek.toInt())}", color = Palette.secondary, fontSize = 12.sp)
        LawsTable(e.laws, null)
        e.pendingLaws?.let { LawsTable(it, "From ${ResultBuckets.weekLabel(e.pendingEffectiveWeek.toInt())}") }
        MarketRules(e)
        if (e.news.isNotEmpty()) {
            Eyebrow("News")
            e.news.take(5).forEach { n ->
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    n.person?.let { PixelPortrait(it, size = 28.dp) }
                    n.firm?.let { f -> n.firmName?.let { name -> FirmChipLogo(store, name, f) } }
                    Text("${ResultBuckets.weekLabel(n.week.toInt())}: ${n.text}", color = Palette.secondary, fontSize = 12.sp)
                }
            }
        }
    }
}

/** Exchange rates against the reporting currency, each central bank and peg, and booked exchange results. */
@Composable
private fun FxSection(store: GameStore, fx: FxDto) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        PixelIcon("swap", size = 14.dp, tint = Palette.secondary)
        Spacer(Modifier.width(6.dp))
        Eyebrow("Currencies")
    }
    fx.currencies.forEach { c -> CurrencyRow(c, c.id == fx.reporting, c.symbol) }
    Text("Rates are what one unit buys in ${fx.reporting}.", color = Palette.secondary, fontSize = 11.sp)
    if (fx.gainsCents != 0L || fx.lossesCents != 0L || fx.openForeignSalesCents != 0L) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("coin", size = 14.dp, tint = Palette.secondary)
            Spacer(Modifier.width(6.dp))
            Text(
                "Exchange results: gains ${Format.money(fx.gainsCents, fx.reporting)}, losses ${Format.money(fx.lossesCents, fx.reporting)}, " +
                    "foreign sales not yet collected ${Format.money(fx.openForeignSalesCents, fx.reporting)}",
                color = Palette.secondary, fontSize = 12.sp,
            )
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        PixelIcon("coin", size = 14.dp, tint = Palette.gold)
        Spacer(Modifier.width(6.dp))
        Text("Show my figures in", color = Palette.primaryText, fontSize = 12.sp)
    }
    CurrencyChoices(store, fx)
}

@Composable
private fun CurrencyRow(c: CurrencyDto, isReporting: Boolean, symbol: String) {
    val trend = c.trendBps / 100.0
    val trendColor = if (trend > 0.05) Palette.positive else if (trend < -0.05) Palette.negative else Palette.secondary
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("$symbol ${c.name} · ${c.country}", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 13.sp, modifier = Modifier.weight(1f))
                if (isReporting) {
                    Text("Reporting currency", color = Palette.gold, fontSize = 12.sp)
                } else {
                    Text(Format.rate(c.valuePpm), color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 13.sp)
                    Text("%+.1f%% over 13 wk".format(trend), color = trendColor, fontSize = 12.sp, modifier = Modifier.padding(start = 8.dp))
                }
            }
            Text("${c.centralBank}: rate ${pct(c.policyRateBps, 2)}, inflation ${pct(c.inflationBps)}", color = Palette.secondary, fontSize = 12.sp)
            if (c.chairName.isNotEmpty()) {
                Text("Chair ${c.chairName}, ${c.chairStance}", color = Palette.secondary, fontSize = 12.sp)
            }
            if (!isReporting) {
                val (text, color) = pegStatus(c)
                Text(text, color = color, fontSize = 12.sp)
            }
        }
        if (!isReporting && c.historyPpm.size >= 2) Sparkline(c.historyPpm, trendColor)
    }
}

private fun pegStatus(c: CurrencyDto): Pair<String, Color> {
    if (!c.pegged) return "Floating" to Palette.secondary
    val crisis = c.crisisWeek
    if (crisis != null) return "Band widened in ${ResultBuckets.weekLabel(crisis.toInt())}, now plus or minus ${pct(c.bandBps)}" to Palette.warning
    if (c.bandBps <= 0) return "Pegged" to Palette.secondary
    val used = (kotlin.math.abs(c.strainBps) * 100 / c.bandBps).coerceIn(0, 100)
    val color = if (used >= 75) Palette.negative else if (used >= 50) Palette.warning else Palette.secondary
    return "Pegged, plus or minus ${pct(c.bandBps)}: $used% of the way to the edge" to color
}

/** A 52 week line of one rate, scaled to its own range. */
@Composable
private fun Sparkline(values: List<Long>, color: Color) {
    Canvas(Modifier.width(96.dp).height(30.dp)) {
        val low = values.min()
        val span = (values.max() - low).coerceAtLeast(1L).toFloat()
        val step = size.width / (values.size - 1)
        val path = Path()
        values.forEachIndexed { i, v ->
            val y = size.height * (1f - (v - low) / span)
            if (i == 0) path.moveTo(0f, y) else path.lineTo(i * step, y)
        }
        drawPath(path, color, style = Stroke(2f))
    }
}

@Composable
private fun Key(color: Color, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(9.dp).clip(Pixel.small).background(color))
        Spacer(Modifier.width(5.dp))
        Text(text, color = Palette.secondary, fontSize = 11.sp)
    }
}

@Composable
private fun LawsTable(laws: LawsDto, heading: String?) {
    Column(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(if (heading != null) Palette.gold.copy(alpha = 0.1f) else Palette.ink).padding(10.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Text(heading?.let { "Announced, in force $it" } ?: "Laws in force", color = if (heading != null) Palette.gold else Palette.secondary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
        Row { Text("Corporate tax", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f)); Text(pct(laws.corporateTaxBps, 0), color = Palette.primaryText, fontSize = 12.sp) }
        Row { Text("Low-skill wage floor", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f)); Text(Format.price(laws.wageFloorHourlyCents) + " an hour", color = Palette.primaryText, fontSize = 12.sp) }
        Row { Text("Merger review from", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f)); Text("${laws.mergerReviewBps / 100}% market share", color = Palette.primaryText, fontSize = 12.sp) }
        Row { Text("Union law", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f)); Text(laws.unions.replaceFirstChar { it.uppercase() }, color = Palette.primaryText, fontSize = 12.sp) }
        LawRow("Common carrier", if (laws.commonCarrier) "Spare lane capacity sold at a regulated rate" else "Not required")
        LawRow("Production quotas", if (laws.quotaBps > 0 && laws.quotaStates.isNotEmpty()) "${laws.quotaStates.joinToString(", ")}: up to ${laws.quotaBps / 100}% of capacity when prices are soft" else "None")
        LawRow("Supplier and customer deals", if (laws.verticalReviewBps > 0) "Reviewed from ${laws.verticalReviewBps / 100}% market share" else "Not reviewed")
        LawRow("Strategic reserve", if (laws.reserve) "Buys in busts, sells in spikes" else "None")
        LawRow(
            "Failing strategic firms",
            when (laws.rescue) { "bailout" -> "State rescue loan"; "nationalise" -> "Nationalised"; else -> "Left to the courts" } +
                if (laws.privatise && laws.rescue != "none") ", sold back when sound" else "",
        )
    }
}

@Composable
private fun LawRow(title: String, value: String) {
    Row {
        Text(title, color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f))
        Text(value, color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(2f))
    }
}

/** The strategic reserve, quotas biting now, state firms and the latest disruptions. */
@Composable
private fun MarketRules(e: EconomyDto) {
    if (e.reserve.isEmpty() && e.quotas.isEmpty() && e.incidents.isEmpty() && e.stateFirms.isEmpty()) return
    Eyebrow("Markets and rules")
    if (e.reserve.isNotEmpty()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("storage", size = 14.dp, tint = Palette.secondary)
            Spacer(Modifier.width(6.dp))
            Text("Reserve holds ${Format.price(e.reserveValueCents)} at cost", color = Palette.primaryText, fontSize = 12.sp)
        }
        e.reserve.forEach { r ->
            Text(
                "${r.label}: ${r.lots} lots, ${r.phase}",
                color = if (r.phase == "buying") Palette.positive else if (r.phase == "selling") Palette.warning else Palette.secondary,
                fontSize = 12.sp,
            )
        }
    }
    e.quotas.forEach { q ->
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("gavel", size = 14.dp, tint = Palette.warning)
            Spacer(Modifier.width(6.dp))
            Text("${q.label} in ${q.regionName} capped at ${q.capBps / 100}% of capacity", color = Palette.secondary, fontSize = 12.sp)
        }
    }
    if (e.stateFirms.isNotEmpty()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("flag", size = 14.dp, tint = Palette.secondary)
            Spacer(Modifier.width(6.dp))
            Text("State owned: ${e.stateFirms.joinToString(", ")}", color = Palette.secondary, fontSize = 12.sp)
        }
    }
    e.incidents.take(4).forEach { i ->
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon(if (i.touchesYou) "alert" else "warning", size = 14.dp, tint = if (i.touchesYou) Palette.negative else Palette.warning)
            Spacer(Modifier.width(6.dp))
            Text(i.text + if (i.weeksLeft > 0u) " ${i.weeksLeft} weeks left." else "", color = if (i.touchesYou) Palette.primaryText else Palette.secondary, fontSize = 12.sp)
        }
    }
}

/** Output gap, inflation and the central bank rate over the kept history (five years), on one scale. */
@Composable
private fun History(e: EconomyDto) {
    val points = e.history
    if (points.size < 2) return
    var period by remember { mutableStateOf(ChartPeriod.Weekly) }
    fun series(name: String, color: Color, value: (uniffi.cw_ffi.MacroPointDto) -> Int) =
        ChartSeries(name, color, GameDates.lastClose(points.map { ChartPoint(it.week.toInt(), value(it).toLong()) }, period))
    Chart(
        listOf(
            series("Output gap", Color(0xFF42A5F5)) { it.outputGapBps },
            series("Inflation", Palette.warning) { it.inflationBps },
            series("Bank rate", Palette.gold) { it.policyRateBps },
        ),
        kinds = listOf(ChartKind.Line, ChartKind.Bar), initialRange = ChartRange.Y1, period = period,
        periods = ChartPeriod.entries, onPeriod = { period = it }, height = 130.dp, format = { "%.1f%%".format(it / 100.0) },
    )
}
