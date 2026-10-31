package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.ElectionRaceDto
import uniffi.cw_ffi.PoliticalCandidateDto
import uniffi.cw_ffi.PublicTenderDto

/**
 * Politics: every state's governor race and the national leader race with the two candidates, their
 * portraits and stated positions, the weekly poll, the laws in force and coming, a Give button with the
 * cap and the disclosure warning, and the public tenders with a bid form that quotes cost and profit.
 */
@Composable
internal fun PoliticsPanel(store: GameStore, currency: String) {
    DisposableEffect(store) {
        store.politicsOpen = true
        store.refreshPolitics()
        onDispose { store.politicsOpen = false }
    }
    val data = store.politics ?: return
    if (!data.ready) return
    var giveTo by remember { mutableStateOf<Pair<ElectionRaceDto, PoliticalCandidateDto>?>(null) }
    var bidOn by remember { mutableStateOf<PublicTenderDto?>(null) }
    var showAll by remember { mutableStateOf(false) }
    Eyebrow("Politics")
    val races = if (showAll) data.races else data.races.take(3)
    races.forEach { race -> RaceCard(race, currency) { giveTo = race to it } }
    if (data.races.size > 3) {
        TextButton(onClick = { showAll = !showAll }) { Text(if (showAll) "Show fewer races" else "Show all ${data.races.size} races", fontSize = 12.sp) }
    }
    val open = data.tenders.filter { it.status == "open" }
    val other = data.tenders.filter { it.status != "open" }.take(4)
    if (open.isNotEmpty() || other.isNotEmpty()) {
        Eyebrow("Public tenders")
        open.forEach { t -> TenderRow(t, currency) { bidOn = t } }
        other.forEach { t -> TenderRow(t, currency, null) }
    }
    giveTo?.let { (race, cand) ->
        GiveDialog(store, race, cand, currency, data.disclosureNote) { giveTo = null }
    }
    bidOn?.let { t -> BidDialog(store, t, currency) { bidOn = null } }
}

private fun partyName(key: String) = if (key == "business") "Commerce Party" else "Workers' Party"

@Composable
private fun RaceCard(race: ElectionRaceDto, currency: String, onGive: (PoliticalCandidateDto) -> Unit) {
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(
                    if (race.office == 255u) "Leader of the country" else "Governor of ${race.place}",
                    color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 14.sp,
                )
                Text(
                    "In office: ${race.governor} (${partyName(race.governorParty)}) · vote ${ResultBuckets.weekLabel(race.electionWeek.toInt())}, in ${race.weeksLeft} weeks",
                    color = Palette.secondary, fontSize = 12.sp,
                )
            }
            if (race.nearYou && race.office != 255u) Text("Your plants", color = Palette.ink, fontSize = 11.sp, fontWeight = FontWeight.Bold,
                modifier = Modifier.clip(Pixel.small).background(Palette.gold).padding(horizontal = 6.dp, vertical = 2.dp))
        }
        // The poll: the business candidate's share against the labour candidate's, with the last weeks.
        val a = race.candidates.getOrNull(0)
        val b = race.candidates.getOrNull(1)
        if (a != null && b != null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("${race.pollBps / 100}%", color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.width(40.dp))
                Box(Modifier.weight(1f).height(8.dp).clip(Pixel.small).background(Palette.negative.copy(alpha = 0.7f))) {
                    Box(Modifier.fillMaxWidth((race.pollBps / 10_000f).coerceIn(0f, 1f)).height(8.dp).background(Palette.positive))
                }
                Text("${(10_000 - race.pollBps) / 100}%", color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.width(40.dp))
            }
            PollLine(race.history)
        }
        race.candidates.forEach { c -> CandidateRow(race, c, currency, onGive) }
        if (race.office != 255u) {
            if (race.lawsInForce.isNotEmpty()) {
                Text("Laws in force", color = Palette.secondary, fontSize = 11.sp)
                race.lawsInForce.forEach { Text("· $it", color = Palette.primaryText, fontSize = 12.sp) }
            }
            if (race.lawsPending.isNotEmpty()) {
                Text("Coming ${ResultBuckets.weekLabel(race.pendingWeek.toInt())}", color = Palette.warning, fontSize = 11.sp)
                race.lawsPending.forEach { Text("· $it", color = Palette.primaryText, fontSize = 12.sp) }
            }
        }
    }
}

@Composable
private fun PollLine(history: List<Int>) {
    if (history.size < 2) return
    Canvas(Modifier.fillMaxWidth().height(28.dp)) {
        val lo = (history.minOrNull() ?: 0).coerceAtMost(400)
        val hi = (history.maxOrNull() ?: 1000).coerceAtLeast(600)
        val span = (hi - lo).coerceAtLeast(1).toFloat()
        val step = size.width / (history.size - 1)
        val half = size.height * (1f - (500 - lo) / span)
        drawLine(Palette.panelRaised, Offset(0f, half), Offset(size.width, half), strokeWidth = 1f)
        for (i in 1 until history.size) {
            val y0 = size.height * (1f - (history[i - 1] - lo) / span)
            val y1 = size.height * (1f - (history[i] - lo) / span)
            drawLine(Palette.gold, Offset((i - 1) * step, y0), Offset(i * step, y1), strokeWidth = 2f)
        }
    }
}

@Composable
private fun CandidateRow(race: ElectionRaceDto, c: PoliticalCandidateDto, currency: String, onGive: (PoliticalCandidateDto) -> Unit) {
    Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        PixelPortrait(c.id, size = 40.dp)
        Column(Modifier.weight(1f)) {
            Text("${c.name} · ${partyName(c.party)}", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 13.sp)
            c.positions.forEach { Text("· $it", color = Palette.secondary, fontSize = 12.sp) }
            if (c.fundsCents > 0) Text("Gifts ${Format.compact(c.fundsCents, currency)}, worth +${c.bonusBps / 100}.${c.bonusBps % 100 / 10} points in the poll", color = Palette.secondary, fontSize = 11.sp)
        }
        if (race.open) OutlinedButton(onClick = { onGive(c) }) { Text("Give", color = Palette.primaryText, fontSize = 12.sp) }
    }
}

@Composable
private fun GiveDialog(store: GameStore, race: ElectionRaceDto, c: PoliticalCandidateDto, currency: String, note: String, onClose: () -> Unit) {
    val firm = store.selectedFirm
    val room = (race.capCents - race.youGaveCents).coerceAtLeast(0)
    var text by remember { mutableStateOf((minOf(room, 5_000_000L) / 100).toString()) }
    val cents = (text.toLongOrNull() ?: 0L) * 100
    val bad = firm == null || cents < 100_000 || cents > room
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Give to ${c.name}") },
        text = {
            Column(Modifier.dialogScroll(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("The law caps gifts at ${Format.compact(race.capCents, currency)} per company for this race. You have given ${Format.compact(race.youGaveCents, currency)}, so ${Format.compact(room, currency)} is left.", color = Palette.secondary, fontSize = 12.sp)
                OutlinedTextField(value = text, onValueChange = { text = it.filter(Char::isDigit) }, label = { Text("Amount") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true, modifier = Modifier.textEntry())
                Text(note, color = Palette.warning, fontSize = 12.sp)
            }
        },
        confirmButton = {
            TextButton(enabled = !bad && !store.busy, onClick = { store.giveToCandidate(firm!!, race.office.toInt(), if (c.party == "business") 0 else 1, cents); onClose() }) { Text("Give") }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
    )
}

@Composable
private fun TenderRow(t: PublicTenderDto, currency: String, onBid: (() -> Unit)? = null) {
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("${t.kindLabel.replaceFirstChar { it.uppercase() }} in ${t.place}", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 13.sp)
                Text(
                    when (t.status) {
                        "open" -> "Up to ${Format.compact(t.budgetCents, currency)} · ${t.minWeeks} to ${t.maxWeeks} weeks · closes ${ResultBuckets.weekLabel(t.closesWeek.toInt())} · ${t.bids} bids" + if (t.yourPriceCents > 0) " · your bid ${Format.compact(t.yourPriceCents, currency)} in ${t.yourWeeks} weeks" else ""
                        "building" -> "${t.winner} builds it for ${Format.compact(t.priceCents, currency)} in ${t.promisedWeeks} weeks · ${t.progressBps / 100}% built, ${Format.compact(t.paidCents, currency)} paid"
                        "done" -> "Finished by ${t.winner}"
                        else -> "Lapsed"
                    },
                    color = Palette.secondary, fontSize = 12.sp,
                )
            }
            if (onBid != null) OutlinedButton(onClick = onBid) { Text("Bid", color = Palette.primaryText, fontSize = 12.sp) }
        }
    }
}

@Composable
private fun BidDialog(store: GameStore, t: PublicTenderDto, currency: String, onClose: () -> Unit) {
    val firm = store.selectedFirm
    var weeks by remember { mutableStateOf(((t.minWeeks + t.maxWeeks) / 2u).toString()) }
    val w = (weeks.toUIntOrNull() ?: t.minWeeks).coerceIn(t.minWeeks, t.maxWeeks)
    // The cost to deliver falls in a straight line from the quickest to the slowest delivery.
    val span = (t.maxWeeks - t.minWeeks).coerceAtLeast(1u).toLong()
    val cost = t.costQuickCents + (t.costSlowCents - t.costQuickCents) * (w - t.minWeeks).toLong() / span
    var price by remember { mutableStateOf((cost * 115 / 100 / 100).coerceAtMost(t.budgetCents / 100).toString()) }
    val cents = (price.toLongOrNull() ?: 0L) * 100
    val profit = cents - cost
    val bad = firm == null || cents <= 0 || cents > t.budgetCents
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Bid on the ${t.kindLabel} in ${t.place}") },
        text = {
            Column(Modifier.dialogScroll(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("The state pays up to ${Format.compact(t.budgetCents, currency)}. The best bid wins: lowest price, with each week of delivery counted as a hundredth of that budget. You build it, paying for contractors and materials as you go, and are paid in four stages as it is built. A week late costs a tenth of the price.", color = Palette.secondary, fontSize = 12.sp)
                OutlinedTextField(value = weeks, onValueChange = { weeks = it.filter(Char::isDigit) }, label = { Text("Delivery in weeks (${t.minWeeks} to ${t.maxWeeks})") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true, modifier = Modifier.textEntry())
                OutlinedTextField(value = price, onValueChange = { price = it.filter(Char::isDigit) }, label = { Text("Your price") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true, modifier = Modifier.textEntry())
                Text("It costs you about ${Format.compact(cost, currency)} to deliver in $w weeks. At this price your profit is ${Format.compact(profit, currency)}.",
                    color = if (profit >= 0) Palette.positive else Palette.negative, fontSize = 12.sp)
            }
        },
        confirmButton = {
            TextButton(enabled = !bad && !store.busy, onClick = { store.bidOnTender(firm!!, t.id.toInt(), cents, w.toInt()); onClose() }) { Text("Submit bid") }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
    )
}
