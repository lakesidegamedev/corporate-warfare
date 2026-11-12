package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.CareerSummaryDto

/**
 * The end of a career, full screen (2.0): the medal and the ambition, the one-sentence verdict, the year by year
 * grid, the moments that mattered and three ways on. It opens by itself when the engine says the career has
 * ended, and can be opened again from Career and from the Hall of Careers.
 */
object EndOfCareer {
    /** Set by the Career and Hall screens to bring the summary back after it was closed. */
    var requested by mutableStateOf(false)
}

internal fun endingTitle(s: CareerSummaryDto): String = when (s.ending) {
    "won" -> "You won"
    "completed" -> "Career complete"
    "no_company" -> "No company left"
    "removed" -> "Removed by the board"
    "taken_over" -> "Taken over"
    else -> if (s.lost) "Career lost" else "Career complete"
}

internal fun medalColor(key: String): Color = when (key) {
    "gold" -> Palette.gold
    "silver" -> Color(0xFFC3CCD6)
    "bronze" -> Color(0xFFB8733C)
    else -> Palette.stroke
}

/** A medal drawn in hard-edged blocks: two ribbon ends and a square disc with a square inside. */
@Composable
internal fun MedalArt(medal: String, modifier: Modifier = Modifier) {
    val disc = medalColor(medal)
    Canvas(modifier.size(72.dp, 88.dp)) {
        val u = size.width / 12f
        drawRect(Color(0xFFB8403E), Offset(2 * u, 0f), Size(3 * u, 5 * u))
        drawRect(Color(0xFF3E6FB8), Offset(7 * u, 0f), Size(3 * u, 5 * u))
        drawRect(Color(0xFF0B1622), Offset(0f, 4 * u), Size(12 * u, 12 * u))
        drawRect(disc, Offset(1 * u, 5 * u), Size(10 * u, 10 * u))
        drawRect(disc.copy(alpha = 1f).let { Color(it.red * 0.6f, it.green * 0.6f, it.blue * 0.6f) }, Offset(3 * u, 7 * u), Size(6 * u, 6 * u))
        drawRect(disc, Offset(4 * u, 8 * u), Size(4 * u, 4 * u))
    }
}

/** Shows the end screen when the career has ended (and stays closed once the player has chosen to go on). */
@Composable
fun EndOfCareerHost(store: GameStore, onNewCareer: () -> Unit, onMainMenu: () -> Unit) {
    val ended = rememberEngine(store, store.overview?.tick) { h -> h.careerStanding()?.ended == true } == true
    var dismissed by remember { mutableStateOf(false) }
    // Time stops when the career ends, once.
    LaunchedEffect(ended) { if (ended && !dismissed) store.playAt(0) }
    if (!(ended && !dismissed) && !EndOfCareer.requested) return
    val summary = rememberEngine(store, store.overview?.tick, ended) { h -> h.careerSummary() }?.takeIf { it.ended } ?: return
    EndOfCareerScreen(
        summary,
        onKeepPlaying = { dismissed = true; EndOfCareer.requested = false },
        onNewCareer = { EndOfCareer.requested = false; onNewCareer() },
        onMainMenu = { EndOfCareer.requested = false; onMainMenu() },
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun EndOfCareerScreen(summary: CareerSummaryDto, onKeepPlaying: () -> Unit, onNewCareer: () -> Unit, onMainMenu: () -> Unit) {
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink).clickable(enabled = false) {}) {
        val wide = maxWidth >= 760.dp
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = if (wide) 28.dp else 14.dp, vertical = 20.dp),
            horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Column(Modifier.widthIn(max = 1000.dp).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Row(horizontalArrangement = Arrangement.spacedBy(20.dp), verticalAlignment = Alignment.CenterVertically) {
                    MedalArt(summary.medal)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(endingTitle(summary).uppercase(), color = if (summary.lost) Palette.negative else Palette.gold, fontFamily = Pixel.font, fontSize = if (wide) 26.sp else 20.sp, fontWeight = FontWeight.Bold)
                        Text(
                            (summary.ambition?.let { "${ambitionWords(it)}: ${medalWords(summary.medal).let { m -> if (summary.medal == "none") "no medal" else "$m medal" }}" } ?: "No ambition: a sandbox career, no medal"),
                            color = medalColor(summary.medal).takeIf { summary.medal != "none" } ?: Palette.primaryText, fontSize = 16.sp, fontFamily = Pixel.font,
                        )
                        Text(startWords(summary), color = Palette.secondary, fontSize = 12.sp)
                    }
                }
                if (summary.sentence.isNotEmpty()) Text(summary.sentence, color = Palette.primaryText, fontSize = 16.sp, lineHeight = 22.sp)
                val figures = listOf(
                    Triple("Net worth at the start", Format.compact(summary.startWorthCents), Palette.primaryText),
                    Triple("Net worth at the end", Format.compact(summary.netWorthCents), Palette.primaryText),
                    Triple("Net worth vs start", growthWords(summary.growthBps), changeTint(summary.growthBps - 10_000)),
                    summary.events.maxOfOrNull { it.week }?.let { Triple("Ended in week", "$it", Palette.primaryText) } ?: Triple("Years recorded", "${summary.yearsLog.size}", Palette.primaryText),
                    Triple("Career length", "${summary.years} years", Palette.secondary),
                )
                if (wide) FigureLine(*figures.toTypedArray())
                else FlowRow(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    figures.forEach { (label, value, tone) ->
                        Column {
                            Text(label.uppercase(), color = Palette.secondary, fontSize = 10.sp, fontFamily = Pixel.font)
                            Text(value, color = tone, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                        }
                    }
                }
                if (summary.yearsLog.isNotEmpty()) DataGrid(
                    "Year by year", summary.yearsLog, yearColumns(), key = { it.year },
                    views = GridViews("career-years", YearViews),
                    modifier = Modifier.fillMaxWidth().height(if (wide) gridNaturalHeight(summary.yearsLog.size.coerceAtMost(10)) + 30.dp else 520.dp),
                    filterText = { "year ${it.year}" },
                    initialSort = "year", initialDescending = false,
                )
                Column(Modifier.fillMaxWidth().border(1.dp, Palette.stroke).background(Palette.panel).padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text("THE MOMENTS THAT MATTERED", color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp)
                    if (summary.events.isEmpty()) Text("Nothing was recorded.", color = Palette.secondary, fontSize = 12.sp)
                    summary.events.sortedBy { it.week }.take(10).forEach { e ->
                        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Text("Week ${e.week}", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.widthIn(min = 64.dp))
                            Text(e.text, color = Palette.primaryText, fontSize = 13.sp, lineHeight = 17.sp, modifier = Modifier.weight(1f))
                        }
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    GoldButton("Start a new career", onClick = onNewCareer)
                    GridButton("Keep playing in this world", false, 13.sp, onKeepPlaying)
                    GridButton("Main menu", false, 13.sp, onMainMenu)
                }
                Spacer(Modifier.height(6.dp))
            }
        }
    }
}

private fun startWords(s: CareerSummaryDto): String = when {
    s.start == "helm" && s.helmCompany != null ->
        "Took the helm of ${s.helmCompany}." + if (s.helmFortuneScaled) " The starting fortune was scaled up to pay for control." else ""
    s.start == "helm" -> "Took the helm of an established company."
    else -> "Started as a founder."
}
