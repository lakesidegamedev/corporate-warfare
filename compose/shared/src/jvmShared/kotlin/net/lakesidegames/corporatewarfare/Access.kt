package net.lakesidegames.corporatewarfare

import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import kotlin.math.abs

/** A name for a control or picture that a screen reader says aloud. */
fun Modifier.described(text: String, role: Role? = null): Modifier = semantics {
    contentDescription = text
    if (role != null) this.role = role
}

/**
 * One line a screen reader gives for a chart: the last value and which way the series has been
 * going, for example "Revenue: 4.2M, rising from 3.1M over 12 points." The trend compares the last
 * value with the first; a change under two percent of the larger one reads as flat.
 */
fun chartSummary(name: String, values: List<Long>, format: (Long) -> String = { Format.compact(it) }): String {
    if (values.isEmpty()) return "$name: no data yet."
    val last = values.last()
    if (values.size == 1) return "$name: ${format(last)}."
    val first = values.first()
    val trend = when {
        abs(last - first) * 50 < maxOf(abs(last), abs(first), 1L) -> "flat"
        last > first -> "rising"
        else -> "falling"
    }
    return if (trend == "flat") "$name: ${format(last)}, flat over ${values.size} points."
    else "$name: ${format(last)}, $trend from ${format(first)} over ${values.size} points."
}
