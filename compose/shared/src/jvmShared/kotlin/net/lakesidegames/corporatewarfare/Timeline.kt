package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.TimelineDto

/** The new-game choice for the scenario's dated history. [key] is what NewGameDto.timeline carries. */
internal enum class ScenarioHistory(val key: String, val title: String, val note: String) {
    Historical("historical", "Historical", "Real events from the period happen on their real dates."),
    Off("off", "Off", "No dated events. Only the business cycle and your rivals move the world."),
    Shuffled("shuffled", "Shuffled", "The same events happen, but on different dates, up to half a year either side."),
}

/** What the History card shows: the timeline and the weeks the paper has an issue for. */
internal data class TimelineView(val timeline: TimelineDto, val issues: Set<Int>)

internal fun inAboutWeeks(weeks: UInt): String = if (weeks == 1u) "in about 1 week" else "in about $weeks weeks"
internal fun eventDate(year: Int, weekOfYear: UInt): String = "week $weekOfYear, $year"

/**
 * The History card of the economy panel: announced events still to come, then the ones that
 * happened, newest first. It reads the engine when the card is on screen and the week changes,
 * and not at all while time runs fast.
 */
@Composable
fun TimelineSection(store: GameStore) {
    val tick = store.overview?.tick?.toInt() ?: return
    val live = !store.running || store.speed <= 1
    var view by remember { mutableStateOf<TimelineView?>(null) }
    LaunchedEffect(tick, live) {
        if (live) view = store.read { h -> TimelineView(h.timeline(), h.paperArchive().map { it.toInt() }.toSet()) } ?: view
    }
    val v = view ?: return
    TimelineCard(v) { week ->
        store.paperWeek = week
        Tutorial.requestedPage = "paper"
    }
}

@Composable
internal fun TimelineCard(view: TimelineView, onOpenIssue: (Int) -> Unit) {
    val t = view.timeline
    if (t.mode == "off") {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            HistoryHead()
            Text("Scenario history is off for this career. No dated events will happen.", color = Palette.secondary, fontSize = 12.sp)
        }
        return
    }
    val past = t.past.sortedByDescending { it.week }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        HistoryHead()
        if (t.upcoming.isEmpty() && past.isEmpty()) {
            Text("Nothing announced yet. Big events are flagged a few weeks ahead.", color = Palette.secondary, fontSize = 12.sp)
        }
        t.upcoming.sortedBy { it.week }.forEach { e ->
            Entry(e.title, e.summary, "${eventDate(e.year, e.weekOfYear)} · ${inAboutWeeks(e.weeksAway)}", Palette.gold, null)
        }
        if (past.isNotEmpty()) {
            if (t.upcoming.isNotEmpty()) Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.primaryText.copy(alpha = 0.3f)))
            past.forEach { e ->
                val open = e.week.toInt() in view.issues
                Entry(e.title, e.summary, eventDate(e.year, e.weekOfYear), Palette.secondary, if (open) ({ onOpenIssue(e.week.toInt()) }) else null)
            }
        }
    }
}

@Composable
private fun HistoryHead() {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Eyebrow("History")
    }
    Box(Modifier.fillMaxWidth().height(3.dp).background(Palette.primaryText.copy(alpha = 0.9f)))
}

@Composable
private fun Entry(title: String, summary: String, when_: String, tint: androidx.compose.ui.graphics.Color, onClick: (() -> Unit)?) {
    Column(
        Modifier.fillMaxWidth().clip(Pixel.small).background(Palette.ink)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(horizontal = 10.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(when_.uppercase(), color = tint, fontFamily = Pixel.font, fontSize = 10.sp, letterSpacing = 1.sp)
        Text(title, color = Palette.primaryText, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, fontSize = 14.sp)
        Text(summary, color = Palette.secondary, fontSize = 12.sp)
        if (onClick != null) Text("Read the paper from that week", color = Palette.gold, fontSize = 11.sp)
    }
}
