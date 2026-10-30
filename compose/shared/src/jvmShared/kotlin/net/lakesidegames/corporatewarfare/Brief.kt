package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.BriefDto
import uniffi.cw_ffi.BriefMoveDto

/**
 * The weekly brief: what changed, what is due and what to do, in the engine's own numbers. A
 * suggestion's Review button opens the existing dialog already filled in; nothing is sent until
 * the player confirms there.
 */
@Composable
internal fun BriefPanel(store: GameStore, currency: String) {
    // The store reads the brief only while it is on screen.
    DisposableEffect(store) {
        store.briefOpen = true
        store.refreshBrief()
        onDispose { store.briefOpen = false }
    }
    var review by remember { mutableStateOf<BriefMoveDto?>(null) }
    Panel {
        Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
            PixelIcon("flag", size = 14.dp, tint = Palette.gold)
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) { Eyebrow("This week's brief") }
            Text(
                if (store.briefCollapsed) "Show" else "Hide",
                color = Palette.secondary, fontSize = 12.sp,
                modifier = Modifier.clickable { store.foldBrief(!store.briefCollapsed) }.padding(4.dp),
            )
        }
        if (!store.briefCollapsed) {
            val brief = store.brief
            if (brief == null) Text("Reading the week...", color = Palette.secondary, fontSize = 12.sp)
            else BriefBody(brief, currency) { review = it }
        }
    }
    review?.let { BriefReview(store, it) { review = null } }
}

/** Plain notes for the brief's lines, by the kind the engine gives each line. The moves explain themselves in their own detail. */
internal object BriefWords {
    fun change(kind: String): String? = when (kind) {
        "profit" -> "Last week's profit: what the company sold minus what it spent on goods, wages, running costs, interest and tax, set against the week before."
        "price" -> "A good you sell moved enough in the market to matter. The percentage compares this week's market price with last week's."
        "rival" -> "A rival spent money to take plants from you in one of your markets. If the raid wins, the plants change owners."
        "event" -> "Something that happened in the world this week and may reach your company through prices, wages or demand."
        "mandate" -> "Your board set you a goal. This says where it stands."
        else -> null
    }

    fun due(kind: String): String? = when (kind) {
        "card" -> "A decision is waiting in your Inbox. A decision that falls due stops the clock until you answer."
        "mandate_offer" -> "The board has offered you a goal. The offer lapses on the date shown."
        "mandate" -> "A board goal that must be met by the date shown."
        "loan" -> "What your loans cost each week: the planned repayment, anything overdue and a week of interest. Missed payments hurt your credit grade."
        "results" -> "The company publishes its quarterly results on this date. Analysts compare them with what they expected."
        "bid" -> "Another company has bid to buy yours. The bid closes on the date shown, so decide before then."
        "tender" -> "A public works contract open for bids. The date shown is when bidding closes."
        "election" -> "An election near your plants. The winner can change taxes, spending and rules there."
        else -> null
    }

    /** How the engine reached a move's numbers, in words. */
    fun figures(move: BriefMoveDto): String = buildList {
        if (move.weeklyGainCents > 0) add("The extra profit each week the engine expects if you do this, from this week's prices and sales.")
        if (move.costCents > 0) add("The cost is paid once, up front.")
        if (move.hasPayback && move.costCents > 0 && move.paybackWeeks > 0) add("The payback time is the cost divided by the weekly gain.")
    }.joinToString(" ")
}

/** The brief itself, with no store behind it so it can be drawn from a saved game's numbers. */
@Composable
internal fun BriefBody(brief: BriefDto, currency: String, onReview: (BriefMoveDto) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        BriefHeading("What changed")
        if (brief.changes.isEmpty()) Text("A quiet week.", color = Palette.secondary, fontSize = 12.sp)
        brief.changes.forEach { Explain(BriefWords.change(it.kind), Modifier.fillMaxWidth()) { Text(it.text, color = Palette.primaryText, fontSize = 13.sp) } }

        BriefHeading("What is due")
        if (brief.due.isEmpty()) Text("Nothing is waiting on you.", color = Palette.secondary, fontSize = 12.sp)
        brief.due.forEach { due ->
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                Explain(BriefWords.due(due.kind), Modifier.weight(1f)) { Text(due.text, color = Palette.primaryText, fontSize = 13.sp) }
                Spacer(Modifier.width(8.dp))
                SignalPill(
                    when (due.weeksLeft.toInt()) { 0 -> "This week"; 1 -> "1 week" else -> "${due.weeksLeft} weeks" },
                    if (due.weeksLeft <= 1u) Palette.warning else Palette.gold,
                )
            }
        }

        BriefHeading("What to do")
        if (brief.moves.isEmpty()) {
            Text(
                if (brief.hold.isEmpty()) "No move stands out. Hold your course." else "Hold your course. ${brief.hold}",
                color = Palette.secondary, fontSize = 12.sp,
            )
        }
        brief.moves.forEach { move ->
            Column(
                Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.shape).padding(10.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(move.headline, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                Text(move.detail, color = Palette.secondary, fontSize = 12.sp)
                Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                    Explain(BriefWords.figures(move), Modifier.weight(1f)) { Text(figures(move, currency), color = Palette.positive, fontSize = 12.sp) }
                    OutlinedButton(onClick = { onReview(move) }) { Text("Review", color = Palette.primaryText) }
                }
            }
        }
    }
}

@Composable
private fun BriefHeading(text: String) {
    Text(text.uppercase(), color = Palette.gold, fontSize = 11.sp, fontWeight = FontWeight.Bold)
}

private fun figures(move: BriefMoveDto, currency: String): String = buildList {
    if (move.weeklyGainCents > 0) add("About ${Format.compact(move.weeklyGainCents, currency)} a week")
    if (move.costCents > 0) add("costs ${Format.compact(move.costCents, currency)}")
    if (move.hasPayback && move.costCents > 0 && move.paybackWeeks > 0) add("pays back in about ${paybackText(move.paybackWeeks)}")
}.joinToString(", ")

/** Weeks under a year, years to one decimal after. */
internal fun paybackText(weeks: Long): String =
    if (weeks < 52) "$weeks weeks" else "${weeks * 10 / 52 / 10}.${weeks * 10 / 52 % 10} years"

/** Opens the existing dialog for a suggestion, filled in. */
@Composable
internal fun BriefReview(store: GameStore, move: BriefMoveDto, onClose: () -> Unit) {
    val t = move.target
    when (t.dialog) {
        "build" -> {
            val board = store.boards.firstOrNull { it.code == t.region }
            if (board == null) onClose() else BuildDialog(store, board, initialRecipe = t.recipe.toInt(), initialBlocks = t.blocks.toInt(), onClose = onClose)
        }
        "pricing" -> {
            val line = store.productLines.firstOrNull { it.product == t.product }
            val bps = move.commands.firstOrNull()?.let { Regex("\"bps\":(-?\\d+)").find(it)?.groupValues?.get(1)?.toIntOrNull() }
            if (line == null) onClose() else PostureDialog(store, line, presetBps = bps, onDismiss = onClose)
        }
        "lane" -> {
            val lane = store.lanes.firstOrNull { it.a == t.region && it.b == t.regionTo && it.mode == t.mode }
            AlertDialog(
                onDismissRequest = onClose,
                containerColor = Palette.panel,
                title = { Text(move.headline) },
                text = {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(move.detail, color = Palette.secondary, fontSize = 12.sp)
                        if (lane != null) ReserveLane(store, lane, initialLots = t.lots.toInt(), initialWeeks = t.weeks.toInt())
                    }
                },
                confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
            )
        }
        "mandate" -> {
            val offer = store.boardOffer
            AlertDialog(
                onDismissRequest = onClose,
                containerColor = Palette.panel,
                title = { Text("The board's offer") },
                text = {
                    if (offer != null) BoardOfferCard(store, offer)
                    else Text("The offer has lapsed or been taken.", color = Palette.secondary)
                },
                confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
            )
        }
        "raid" -> RaidDialog(store, t.target, t.region, t.industry, onClose)
        "division" -> {
            val division = store.divisions.firstOrNull { it.region == t.region && it.industry == t.industry }
            if (division == null) onClose() else DivisionDetailDialog(store, division, onClose)
        }
        else -> AlertDialog(
            onDismissRequest = onClose,
            containerColor = Palette.panel,
            title = { Text(move.headline) },
            text = { Text(move.detail, color = Palette.secondary, fontSize = 13.sp) },
            confirmButton = {
                GoldButton(move.actionLabel) {
                    move.commands.forEach { store.submit(it) }
                    onClose()
                }
            },
            dismissButton = { TextButton(onClick = onClose) { Text("Not now") } },
        )
    }
}

/**
 * Desktop: when a new week starts with time paused or at 1x, show the brief once. Never at 2x or
 * 4x, where a week lasts a moment.
 */
@Composable
internal fun BriefWeeklyHost(store: GameStore) {
    var shown by remember { mutableStateOf(-1) }
    var open by remember { mutableStateOf(false) }
    val tick = store.overview?.tick?.toInt() ?: -1
    val slow = !store.running || store.speed <= 1
    LaunchedEffect(tick, slow) {
        if (tick > 0 && slow && tick != shown && store.selectedFirm != null) {
            shown = tick
            store.briefOpen = true
            store.refreshBrief()
            open = true
        }
    }
    if (open) {
        var review by remember { mutableStateOf<BriefMoveDto?>(null) }
        val brief = store.brief
        val currency = store.selectedFirmDto?.currency.orEmpty()
        AlertDialog(
            onDismissRequest = { open = false; store.briefOpen = false },
            containerColor = Palette.panel,
            title = { Text("This week's brief") },
            text = {
                if (brief == null) Text("Reading the week...", color = Palette.secondary)
                else Column(Modifier.width(520.dp)) { BriefBody(brief, currency) { review = it } }
            },
            confirmButton = { TextButton(onClick = { open = false; store.briefOpen = false }) { Text("Close") } },
        )
        review?.let { BriefReview(store, it) { review = null } }
    }
}
