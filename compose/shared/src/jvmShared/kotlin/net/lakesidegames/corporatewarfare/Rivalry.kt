package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToLong
import uniffi.cw_ffi.RaidCheckDto
import uniffi.cw_ffi.RaidOptionDto
import uniffi.cw_ffi.RaidQuoteDto
import uniffi.cw_ffi.RivalMoveFeedDto
import uniffi.cw_ffi.RivalSummaryDto
import uniffi.cw_ffi.RivalTagDto

/** A small capsule naming how a rival's chief executive plays. */
@Composable
fun TemperamentTag(key: String) {
    Text(
        temperamentLabel(key), color = Palette.gold, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(Pixel.small).background(Palette.panelRaised).padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

/** Grudge and ally marks for a rival's standing with you. */
@Composable
fun StandingTags(tag: RivalTagDto) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        if (tag.grudgeBps > 0) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                PixelIcon("target", size = 10.dp, tint = Palette.negative)
                Text(if (tag.grudgeBps >= 6_000) "Bitter grudge" else "Grudge", color = Palette.negative, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
        }
        if (tag.ally) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                PixelIcon("people", size = 10.dp, tint = Palette.positive)
                Text("Ally", color = Palette.positive, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
        }
    }
}

/** A rival's chief executive: portrait, name, temperament, and grudge or ally marks. */
@Composable
fun RivalTagLine(tag: RivalTagDto, portrait: androidx.compose.ui.unit.Dp = 28.dp) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (tag.ceoId.isNotEmpty()) PixelPortrait(tag.ceoId, size = portrait)
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                tag.ceoName.ifEmpty { tag.firmName }, color = Palette.primaryText, fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold, maxLines = 1,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (tag.temperament.isNotEmpty()) TemperamentTag(tag.temperament)
                StandingTags(tag)
            }
        }
    }
}

/** The chief and marks of one company, loaded from the engine. Draws nothing without a chief. */
@Composable
fun RivalIdentity(store: GameStore, firm: ULong, portrait: androidx.compose.ui.unit.Dp = 28.dp) {
    var tag by remember(firm) { mutableStateOf<RivalTagDto?>(null) }
    LaunchedEffect(firm) { store.rivalTag(firm) { tag = it } }
    tag?.let { if (it.ceoName.isNotEmpty() || it.grudgeBps > 0 || it.ally) RivalTagLine(it, portrait) }
}

/** A Raid button for one rival market; it opens the odds, or says why the raid is closed. */
@Composable
fun RaidMarketButton(store: GameStore, target: ULong, region: String, industry: String) {
    var open by remember { mutableStateOf(false) }
    OutlinedButton(onClick = { open = true }, enabled = !store.busy) {
        PixelIcon("flag", size = 14.dp, tint = Palette.negative)
        Spacer(Modifier.width(4.dp))
        Text("Raid", color = Palette.negative, fontSize = 12.sp)
    }
    if (open) RaidDialog(store, target, region, industry) { open = false }
}

/** The campaign when the market is open; otherwise one line of why and a way to build in. */
@Composable
fun RaidDialog(store: GameStore, target: ULong, region: String, industry: String, onClose: () -> Unit) {
    var check by remember { mutableStateOf<RaidCheckDto?>(null) }
    var refresh by remember { mutableIntStateOf(0) }
    var multiple by remember { mutableStateOf(2.0) }
    var quote by remember { mutableStateOf<RaidQuoteDto?>(null) }
    var building by remember { mutableStateOf(false) }
    var launchedFrom by remember { mutableStateOf<uniffi.cw_ffi.RaidRecordDto?>(null) }
    var launched by remember { mutableStateOf(false) }
    LaunchedEffect(target, region, industry, refresh) { store.raidCheck(target, region, industry) { check = it } }
    val option = check?.option
    val budget = option?.let { (it.quote.defenceCents * multiple).roundToLong() } ?: 0L
    LaunchedEffect(option, budget) { if (option != null) store.raidQuote(option, budget) { quote = it } }
    val regionName = store.boards.firstOrNull { it.code == region }?.name ?: region
    val result = if (launched) store.recentRaids.firstOrNull()?.takeIf { it != launchedFrom } else null

    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Market raid") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val c = check
                when {
                    c == null -> Text("Checking the market", color = Palette.secondary)
                    option == null -> {
                        Callout(c.reason, Palette.warning)
                        if (c.needsBuild) GoldButton("Build in $regionName") { building = true }
                    }
                    else -> {
                        Text(option.targetName, color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                        RivalIdentity(store, option.target, 36.dp)
                        Text("${Sectors.name(industry)} in $regionName · ${option.rivalBlocks} plants", color = Palette.secondary, fontSize = 12.sp)
                        ValueSlider(
                            value = multiple, onValue = { multiple = it }, range = 0.5..4.0, step = 0.1,
                            label = { "%.1f to 1".format(it) },
                            detail = { "Budget ${Format.money((option.quote.defenceCents * it).roundToLong())} against their defence of ${Format.money(option.quote.defenceCents)}" },
                            ticks = 35,
                        )
                        Line("Budget", Format.money(budget))
                        quote?.let { q ->
                            Line("Chance", "${q.chanceBps / 100}%")
                            Line("Their defence", Format.money(q.defenceCents))
                            Line("Plants at stake", "${q.blocksAtStake} plant${if (q.blocksAtStake == 1u) "" else "s"}")
                            Line("Price if it lands", Format.money(q.priceCents))
                            if (q.surchargeCents > 0) Line("Repeat-raid surcharge", Format.money(q.surchargeCents))
                            Line("Most it can cost", Format.money(budget + q.priceCents + q.surchargeCents))
                        }
                        Text(
                            "The budget buys advertising whatever happens. If the raid lands you take the plants into your division at your strategy and pay the rival half their book value.",
                            color = Palette.secondary, fontSize = 11.sp,
                        )
                        result?.let { r ->
                            Callout(
                                if (r.success) "Took ${r.blocks} plant${if (r.blocks == 1u) "" else "s"} from ${r.defender}" else "The campaign did not move ${r.defender}'s customers",
                                if (r.success) Palette.positive else Palette.warning,
                            )
                        }
                        store.error?.let { Text(it, color = Palette.negative, fontSize = 12.sp) }
                    }
                }
            }
        },
        confirmButton = {
            if (option != null && result == null) {
                GoldButton("Launch campaign", enabled = !store.busy) {
                    launchedFrom = store.recentRaids.firstOrNull()
                    launched = true
                    store.raid(option, budget)
                }
            }
        },
        dismissButton = { TextButton(onClick = onClose) { Text(if (result == null) "Cancel" else "Done") } },
    )
    if (building) {
        store.boards.firstOrNull { it.code == region }?.let { board ->
            BuildDialog(store, board) { building = false; refresh++ }
        }
    }
}

@Composable
private fun Line(label: String, value: String) {
    Row(Modifier.fillMaxWidth()) {
        Text(label, color = Palette.secondary, fontSize = 13.sp, modifier = Modifier.weight(1f))
        Text(value, color = Palette.primaryText, fontSize = 13.sp)
    }
}

/** Every market a rival has plants in, each with a Raid button. */
@Composable
fun RaidMarketsList(store: GameStore, firm: ULong) {
    if (firm == store.selectedFirm) return
    val markets = store.mapPlants.filter { it.firm == firm && !it.player }
        .map { it.region to it.industry }.distinct().sortedBy { "${it.first}/${it.second}" }
    if (markets.isEmpty()) return
    Eyebrow("Raid their markets")
    markets.take(8).forEach { (region, industry) ->
        val regionName = store.boards.firstOrNull { it.code == region }?.name ?: region
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SectorBadge(industry, 20)
            Text("${Sectors.name(industry)} · $regionName", color = Palette.primaryText, fontSize = 12.sp, maxLines = 1, modifier = Modifier.weight(1f))
            RaidMarketButton(store, firm, region, industry)
        }
    }
}

/** The CEO Office's compact rival board: threats, allies and who you can raid now, with odds. */
@Composable
fun RivalsCard(store: GameStore) {
    val rivals = store.rivals
    if (rivals.isEmpty()) return
    var raiding by remember { mutableStateOf<RaidOptionDto?>(null) }
    val threats = rivals.filter { it.tag.grudgeBps > 0 }.take(3)
    val allies = rivals.filter { it.tag.ally }.take(3)
    val targets = rivals.filter { it.bestRaid != null }.sortedByDescending { it.bestRaid!!.quote.chanceBps }.take(3)
    var board by remember { mutableStateOf(false) }
    Panel {
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Eyebrow("Rivals")
            Spacer(Modifier.weight(1f))
            TextButton(onClick = { board = true }) { Text("Rivals board", color = Palette.gold, fontSize = 12.sp) }
        }
        RivalGroup("Threats", "target", Palette.negative, threats, "Nobody holds a grudge against you.")
        RivalGroup("Allies", "people", Palette.positive, allies, "No allies yet. A joint venture or a friendly deal makes one.")
        Text("Raid now", color = Palette.secondary, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
        if (targets.isEmpty()) {
            Text("No rival shares a market with you yet. Build where a rival operates to raid them.", color = Palette.secondary, fontSize = 12.sp)
        }
        targets.forEach { row ->
            val option = row.bestRaid ?: return@forEach
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Column(Modifier.weight(1f)) { RivalTagLine(row.tag) }
                Column(horizontalAlignment = Alignment.End) {
                    Text("${option.quote.chanceBps / 100}%", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                    Text(Sectors.name(option.industry), color = Palette.secondary, fontSize = 11.sp)
                }
                OutlinedButton(onClick = { raiding = option }, enabled = !store.busy) { Text("Raid", color = Palette.negative, fontSize = 12.sp) }
            }
        }
    }
    raiding?.let { o -> RaidDialog(store, o.target, o.region, o.industry) { raiding = null } }
    if (board) RivalsBoardDialog(store) { board = false }
}

@Composable
private fun RivalGroup(title: String, icon: String, tint: androidx.compose.ui.graphics.Color, rows: List<RivalSummaryDto>, empty: String) {
    Text(title, color = Palette.secondary, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
    if (rows.isEmpty()) Text(empty, color = Palette.secondary, fontSize = 12.sp)
    rows.forEach { row ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PixelIcon(icon, size = 12.dp, tint = tint)
            Column {
                Text(row.tag.firmName, color = Palette.primaryText, fontSize = 13.sp, maxLines = 1)
                Text(
                    if (row.tag.ceoName.isEmpty()) "${row.sharedMarkets} shared markets"
                    else "${row.tag.ceoName} · ${temperamentLabel(row.tag.temperament)}",
                    color = Palette.secondary, fontSize = 11.sp, maxLines = 1,
                )
            }
        }
    }
}

/** One rival move in the news, naming the chief and temperament behind it. */
@Composable
fun RivalMoveRow(move: RivalMoveFeedDto) {
    val who = if (move.ceoName.isEmpty()) move.firmName else "${move.ceoName} (${temperamentLabel(move.temperament)}) at ${move.firmName}"
    Column(Modifier.fillMaxWidth().padding(vertical = 2.dp)) {
        Text(move.text, color = Palette.primaryText, fontSize = 12.sp)
        Text("${ResultBuckets.weekLabel(move.week.toInt())} · $who", color = Palette.secondary, fontSize = 11.sp)
    }
}
