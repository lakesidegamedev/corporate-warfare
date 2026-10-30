package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.EarningsDto
import kotlin.math.abs
import kotlin.math.roundToLong

/** The analyst's rating in words and colour. */
internal fun ratingText(rating: String) = when (rating) {
    "buy" -> "Buy"
    "sell" -> "Sell"
    else -> "Hold"
}

internal fun ratingTint(rating: String): Color = when (rating) {
    "buy" -> Palette.positive
    "sell" -> Palette.negative
    else -> Palette.secondary
}

/** "Results this week", "Results in 5 weeks": when the next results day is, for the stock board and the page. */
internal fun resultsWhen(e: EarningsDto): String? {
    if (!e.covered || !e.hasEstimate) return null
    return when (e.weeksToResults.toInt()) {
        0, 1 -> "Reports this week"
        else -> "Reports in ${e.weeksToResults} weeks"
    }
}

/** Trust the market has in management's numbers, in words. */
internal fun credibilityText(bps: Int) = when {
    bps >= 7_500 -> "High"
    bps >= 4_000 -> "Fair"
    else -> "Low"
}

/**
 * Earnings season on a company page: what the analyst expects for the quarter, the last results
 * against the estimate, the rating, and (for your own company) guidance with a plain warning.
 */
@Composable
fun EarningsSection(e: EarningsDto, onGiveGuidance: (() -> Unit)?) {
    if (!e.covered) return
    Eyebrow("Earnings")
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            SignalPill("Analysts: ${ratingText(e.rating)}", ratingTint(e.rating))
            resultsWhen(e)?.let { SignalPill(it, Palette.gold) }
        }
        if (e.hasEstimate) {
            Text(
                "Analysts expect ${Format.money(e.estimateCents)} in operating profit this quarter.",
                color = Palette.primaryText, fontSize = 12.sp,
            )
        } else {
            Text("No estimate yet. The analyst writes one at the next quarter's close.", color = Palette.secondary, fontSize = 12.sp)
        }
        if (e.hasGuidance) {
            Text(
                "Guidance: ${Format.money(e.guideLowCents)} to ${Format.money(e.guideHighCents)}. Meet the low end on results day and the market trusts you more.",
                color = Palette.gold, fontSize = 12.sp,
            )
        }
        if (e.hasLast) {
            val beat = e.lastActualCents >= e.lastEstimateCents
            val tint = if (beat) Palette.positive else Palette.negative
            Text(
                "Last results (week ${e.lastWeek}): ${Format.money(e.lastActualCents)} against ${Format.money(e.lastEstimateCents)} expected, " +
                    "${if (beat) "a beat" else "a miss"}. The share price moved ${changeText(e.lastMoveBps.toLong())}.",
                color = tint, fontSize = 12.sp,
            )
            when (e.lastGuidanceMet) {
                true -> Text("Management met its guidance.", color = Palette.positive, fontSize = 12.sp)
                false -> Text("Management missed its guidance.", color = Palette.negative, fontSize = 12.sp)
                null -> {}
            }
            if (abs(e.streak) >= 2) {
                Text(
                    "${abs(e.streak)} quarters in a row ${if (e.streak > 0) "above" else "below"} the estimate.",
                    color = Palette.secondary, fontSize = 12.sp,
                )
            }
        }
        Text("Trust in management's numbers: ${credibilityText(e.credibilityBps)}", color = Palette.secondary, fontSize = 12.sp)
        if (onGiveGuidance != null && e.canGuide) TextButton(onClick = onGiveGuidance) { Text("Give guidance") }
    }
}

/**
 * Tell the analysts what to expect. Prefilled with the estimate and five percent either side.
 * [confirm] gets the low and high ends in cents.
 */
@Composable
fun GuidanceDialog(e: EarningsDto, onClose: () -> Unit, confirm: (Long, Long) -> Unit) {
    fun dollars(cents: Long) = "%.0f".format(cents / 100.0)
    val room = abs(e.estimateCents) / 20
    var low by remember { mutableStateOf(dollars(e.estimateCents - room)) }
    var high by remember { mutableStateOf(dollars(e.estimateCents + room)) }
    val lowCents = low.toDoubleOrNull()?.let { (it * 100).roundToLong() }
    val highCents = high.toDoubleOrNull()?.let { (it * 100).roundToLong() }
    val valid = lowCents != null && highCents != null && lowCents <= highCents
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Give guidance") },
        text = { GuidanceBody(e, low, { low = it }, high, { high = it }) },
        confirmButton = {
            TextButton(enabled = valid, onClick = { confirm(lowCents!!, highCents!!); onClose() }) { Text("Give guidance") }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
    )
}

/** The words and fields of the guidance dialog, apart from its buttons. */
@Composable
fun GuidanceBody(e: EarningsDto, low: String, onLow: (String) -> Unit, high: String, onHigh: (String) -> Unit) {
    Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(
            "Analysts expect ${Format.money(e.estimateCents)} in operating profit this quarter. " +
                "Tell them a range and they move their number toward the middle of it.",
            color = Palette.primaryText, fontSize = 12.sp,
        )
        Text(
            "The risk: finish under the low end and the company loses reputation, the market trusts you less, " +
                "and activist investors take notice. Finish above it and the market trusts you more, so a later miss costs less. " +
                "A low end well under what the analysts expect is seen as sandbagging and earns no trust.",
            color = Palette.warning, fontSize = 12.sp,
        )
        OutlinedTextField(low, { onLow(it.filter { c -> c.isDigit() || c == '-' }) }, label = { Text("Low end of operating profit ($)") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true, modifier = Modifier.textEntry())
        OutlinedTextField(high, { onHigh(it.filter { c -> c.isDigit() || c == '-' }) }, label = { Text("High end of operating profit ($)") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true, modifier = Modifier.textEntry())
    }
}
