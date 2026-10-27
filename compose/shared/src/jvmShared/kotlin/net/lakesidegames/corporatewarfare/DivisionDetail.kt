package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.DivisionDetailDto
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.DivisionInputDto
import uniffi.cw_ffi.DivisionOutputDto

/** Lots to one decimal below ten, whole above, from the engine's milli-lots. */
internal fun lots(milli: Long): String =
    if (milli in 1..9_999) "%.1f".format(milli / 1000.0) else Format.count(milli / 1000)

private val costNames = mapOf(
    "materials" to "Materials",
    "wages" to "Wages",
    "maintenance" to "Maintenance",
    "land" to "Land rent",
    "depreciation" to "Depreciation",
)

private val tierNames = listOf("Low skill", "Mid skill", "High skill")

/** The detail view of the division open in the store; draws nothing when none is. */
@Composable
fun DivisionDetailHost(store: GameStore) {
    val division = store.openDivision ?: return
    DivisionDetailDialog(store, division) { store.showDivision(null) }
}

@Composable
internal fun DivisionDetailDialog(store: GameStore, division: DivisionDto, onClose: () -> Unit) {
    var detail by remember(division.region, division.industry) { mutableStateOf<DivisionDetailDto?>(null) }
    var editing by remember { mutableStateOf(false) }
    // Reload after every command and every week: the divisions list is replaced on each reload.
    LaunchedEffect(division.region, division.industry, store.divisions, store.overview?.tick) {
        store.divisionDetail(division) { detail = it }
    }
    val live = store.divisions.firstOrNull { it.region == division.region && it.industry == division.industry } ?: division
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        modifier = Modifier.widthIn(min = 360.dp, max = 640.dp),
        title = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                SectorBadge(division.industry, 28)
                Spacer(Modifier.width(10.dp))
                Column {
                    Text("${Sectors.name(division.industry)} · ${division.regionName}")
                    Text("${live.strategyName} · ${live.blocks} plants · ${live.shareBps / 100}% share", color = Palette.secondary, fontSize = 12.sp)
                }
            }
        },
        text = {
            val d = detail
            Column(Modifier.heightIn(max = 520.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                if (d == null) {
                    Text("Loading the division.", color = Palette.secondary, fontSize = 13.sp)
                } else {
                    Financials(d)
                    Strategy(store, d, live) { editing = true }
                    Inputs(d)
                    Outputs(d)
                    Plants(d)
                    Labour(store, d)
                }
            }
            if (editing) {
                val d2 = detail
                val line = d2?.let { x -> store.productLines.firstOrNull { it.product == x.primaryProduct } }
                if (line != null) PostureDialog(store, line) { editing = false } else editing = false
            }
        },
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Eyebrow(title)
        content()
    }
}

@Composable
private fun Line(label: String, value: String, tone: Color = Palette.primaryText, note: String? = null) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f))
        if (note != null) Text(note, color = Palette.secondary, fontSize = 11.sp, modifier = Modifier.padding(end = 8.dp))
        Text(value, color = tone, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
private fun Financials(d: DivisionDetailDto) {
    Section("Financials, per week") {
        Line("Revenue", Format.money(d.revenueCents))
        d.costs.forEach { Line(costNames[it.key] ?: it.key, Format.money(-it.cents), Palette.secondary) }
        HorizontalDivider(color = Palette.stroke)
        val tone = if (d.operatingProfitCents >= 0) Palette.positive else Palette.negative
        Line("Operating profit", Format.money(d.operatingProfitCents), tone)
        Line("Margin", if (d.revenueCents > 0) "%.1f%%".format(d.marginBps / 100.0) else "no sales", tone)
        Line("Capital invested", Format.money(d.capexCents))
        Line("Book value", Format.money(d.bookCents))
        if (d.revenueTrend.size >= 2) {
            Spacer(Modifier.padding(top = 2.dp))
            Text("Revenue, last ${d.revenueTrend.size} weeks, at today's prices", color = Palette.secondary, fontSize = 11.sp)
            Trend(d.revenueTrend)
        }
        Text("Estimated from the plants' last week and your sales at today's asking prices.", color = Palette.secondary, fontSize = 11.sp)
    }
}

/** A row of bars, one per week, scaled to the largest. */
@Composable
private fun Trend(values: List<Long>) {
    val peak = (values.maxOrNull() ?: 0L).coerceAtLeast(1L)
    Row(Modifier.fillMaxWidth().heightIn(min = 36.dp, max = 36.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.Bottom) {
        values.forEach { v ->
            val h = (36.0 * v / peak).coerceAtLeast(1.0).dp
            Spacer(Modifier.weight(1f).heightIn(min = h, max = h).clip(CircleShape).background(Palette.gold))
        }
    }
}

@Composable
private fun Pill(text: String, onClick: () -> Unit) {
    Text(
        text, color = Palette.primaryText, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(CircleShape).background(Palette.panelRaised).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

@Composable
private fun Strategy(store: GameStore, d: DivisionDetailDto, live: DivisionDto, onPricing: () -> Unit) {
    Section("Operating strategy") {
        Text("${d.strategyName}. ${d.meaning}", color = Palette.primaryText, fontSize = 13.sp)
        val posture = when (d.priceMode) {
            "market" -> if (d.priceBps == 0) "At market" else "%+d%% against market".format(d.priceBps / 100)
            "cost_plus" -> "Cost plus ${d.priceBps / 100}%"
            "manual" -> "Set by hand"
            else -> "Automatic"
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            LineIn("Pricing", posture, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(8.dp))
            Pill("Change", onPricing)
        }
        if (d.hasGroupBuyers) {
            var bps by remember(d.transferBps) { mutableStateOf(d.transferBps.toDouble()) }
            ValueSlider(
                bps, { bps = it }, 0.0..12_000.0, 100.0,
                label = { "Transfer price ${(it / 100).toInt()}% of market" + if (it < 10_000.0) " (toward cost)" else "" },
                origin = 10_000.0,
                detail = { "What sister companies pay this division for its output." },
                onCommit = { store.setTransferPrice(d, it.toInt()) },
            )
        } else {
            Line("Transfer price", "No group buyers", Palette.secondary)
        }
        val stockText = if (d.stockWeeks > 0) "${d.stockWeeks} weeks" else "${d.stockWeeksEffective} weeks (company default)"
        Line("Stock target", stockText)
        DivisionStock(store, live)
        DivisionShipping(store, live)
    }
}

@Composable
private fun LineIn(label: String, value: String, tone: Color = Palette.primaryText, modifier: Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Text(label, color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f))
        Text(value, color = tone, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
    }
}

private fun coverTone(i: DivisionInputDto): Color = when {
    i.coverWeeksX100 <= 0L && i.needMilli > 0 -> Palette.negative
    i.coverWeeksX100 < 100L -> Palette.warning
    else -> Palette.primaryText
}

@Composable
private fun Inputs(d: DivisionDetailDto) {
    Section("Inputs, per week") {
        if (d.inputs.isEmpty()) Text("This strategy buys nothing in.", color = Palette.secondary, fontSize = 12.sp)
        d.inputs.forEachIndexed { index, i ->
            if (index > 0) HorizontalDivider(color = Palette.stroke)
            Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
                Text(i.productName, color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                Text("Needs ${lots(i.needMilli)} · used ${lots(i.usedMilli)} · ${Format.price(i.priceCents)} a lot", color = Palette.secondary, fontSize = 12.sp)
                Text("From ${i.source}", color = Palette.secondary, fontSize = 12.sp)
                if (i.storeClass.isNotEmpty()) {
                    val cover = if (i.needMilli > 0) "%.1f weeks of cover".format(i.coverWeeksX100 / 100.0) else "none needed"
                    Text("${lots(i.stockMilli)} in stock · $cover", color = coverTone(i), fontSize = 12.sp)
                }
            }
        }
    }
}

@Composable
private fun Outputs(d: DivisionDetailDto) {
    Section("Outputs, last week") {
        d.outputs.forEachIndexed { index, o: DivisionOutputDto ->
            if (index > 0) HorizontalDivider(color = Palette.stroke)
            Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
                Text(o.productName + if (o.byProduct) " (by-product)" else "", color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                Text("Made ${lots(o.madeMilli)} · sold ${lots(o.soldMilli)} · stored ${lots(o.storedMilli)}", color = Palette.secondary, fontSize = 12.sp)
                Text("Asking ${Format.price(o.priceCents)} a lot · market ${Format.price(o.marketCents)}", color = Palette.secondary, fontSize = 12.sp)
                Text("Buyers: " + if (o.buyers.isEmpty()) "the open market" else o.buyers.joinToString(", ") + " and the open market", color = Palette.secondary, fontSize = 12.sp)
            }
        }
    }
}

@Composable
private fun Plants(d: DivisionDetailDto) {
    Section("Plants") {
        d.plants.forEachIndexed { index, p ->
            val tone = when {
                p.status != "operating" -> Palette.warning
                p.inputsShort || p.staffBps < 9_000 -> Palette.warning
                else -> Palette.primaryText
            }
            val status = p.status.replaceFirstChar { it.uppercase() }
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text("Plant ${index + 1} · ${p.blocks} blocks", color = Palette.primaryText, fontSize = 13.sp, modifier = Modifier.weight(1f))
                Text(if (p.status == "operating") "${p.utilisationBps / 100}% used" else status, color = tone, fontSize = 12.sp)
            }
            val notes = buildList {
                add("${p.ageWeeks} weeks old")
                add("book ${Format.money(p.bookCents)}")
                if (p.status == "operating") add("${p.staffBps / 100}% staffed")
                if (p.inputsShort) add("short of inputs")
            }
            Text(notes.joinToString(" · "), color = Palette.secondary, fontSize = 12.sp)
        }
    }
}

@Composable
private fun Labour(store: GameStore, d: DivisionDetailDto) {
    Section("Labour") {
        d.tiers.forEachIndexed { tier, t ->
            if (t.needed <= 0L && t.employed <= 0L) return@forEachIndexed
            val short = t.employed < t.needed
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("${tierNames[tier]}: ${Format.count(t.employed)} of ${Format.count(t.needed)}", color = if (short) Palette.warning else Palette.primaryText, fontSize = 13.sp)
                    val gap = t.wageCents - t.regionalWageCents
                    val vs = if (t.regionalWageCents <= 0L) "" else if (gap >= 0) " · ${Format.money(gap)} over the region" else " · ${Format.money(-gap)} under the region"
                    Text("Offer ${Format.money(t.wageCents)} a week$vs · payroll ${Format.money(t.payrollCents)}", color = Palette.secondary, fontSize = 12.sp)
                }
                Pill("+5%") { store.setWageOffer(tier, t.wageCents * 105 / 100 + 1) }
            }
        }
        if (d.shortage) Text("Short of workers: output is capped by the thinnest tier. Raise the offer to draw more.", color = Palette.warning, fontSize = 12.sp)
        val union = when (d.unionState) {
            "balloting" -> "Union ballot under way"
            "demanding" -> "Union is demanding a settlement"
            "striking" -> "Union is on strike"
            else -> if (d.unionOrganisationBps > 0) "No union action" else "No union"
        }
        Line("Union", union + if (d.unionOrganisationBps > 0) " · ${d.unionOrganisationBps / 100}% organised" else "")
        Text("Wage offers are set company wide, so a raise reaches every plant of the company.", color = Palette.secondary, fontSize = 11.sp)
    }
}
