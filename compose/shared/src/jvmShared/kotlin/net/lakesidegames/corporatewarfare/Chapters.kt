package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import uniffi.cw_ffi.AchievementDto
import uniffi.cw_ffi.ChapterDto
import uniffi.cw_ffi.HallDto
import uniffi.cw_ffi.MandateDto

/** Careers the main menu remembers. */
internal const val MAX_CAREERS = 12

/**
 * One finished or left career, as the main menu lists it. Small on purpose: it lives in the
 * settings file beside the player's other choices.
 */
@Serializable
data class CareerSummary(
    val title: String,
    val scenario: String,
    val weeks: Int,
    val chapters: Int,
    val met: Int,
    val missed: Int,
    val achievements: Int,
) {
    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        fun of(title: String, scenario: String, hall: HallDto) = CareerSummary(
            title = title.ifBlank { "Untitled career" },
            scenario = scenario,
            weeks = hall.weeks.toInt(),
            chapters = hall.chapters.count { it.status == "closed" },
            met = hall.mandatesMet.toInt(),
            missed = hall.mandatesMissed.toInt(),
            achievements = hall.achievementsEarned.toInt(),
        )

        fun encode(list: List<CareerSummary>): String = json.encodeToString(ListSerializer(serializer()), list)
        fun decode(text: String): List<CareerSummary> = json.decodeFromString(ListSerializer(serializer()), text)
    }
}

/** A scenario's name from its id, or the id when the scenario is not bundled. */
internal fun scenarioName(id: String): String =
    runCatching { uniffi.cw_ffi.scenarios().firstOrNull { it.id == id }?.name }.getOrNull() ?: id

/** What a met or missed goal does, from content/mandates.json (the engine's numbers, in words). */
private const val REWARD_LINE = "A management bonus of 2% of a year's operating profit (at least \$50K) and +1.5 reputation."
private const val RISK_LINE = "Missing it costs 1.0 reputation. Two misses in a row can bring an activist campaign."
private const val CASH_COVER_RISK = "One week under the line loses it. Missing costs 1.0 reputation, and two misses in a row can bring an activist campaign."

private fun riskOf(mandate: MandateDto) = if (mandate.key == "cash_cover") CASH_COVER_RISK else RISK_LINE

private fun numberWord(n: Int) = when (n) { 1 -> "one"; 2 -> "two"; 3 -> "three"; else -> "$n" }

/** A thin stepped bar: twenty blocks, the filled ones in [tint]. */
@Composable
fun PixelBar(fraction: Float, tint: Color, modifier: Modifier = Modifier, height: Dp = 8.dp) {
    Canvas(modifier.fillMaxWidth().height(height)) {
        val blocks = 20
        val gap = 2.dp.toPx()
        val cell = (size.width - gap * (blocks - 1)) / blocks
        val lit = (fraction.coerceIn(0f, 1f) * blocks + 0.001f).toInt()
        for (i in 0 until blocks) {
            drawRect(
                if (i < lit) tint else Palette.stroke,
                Offset(i * (cell + gap), 0f),
                Size(cell, size.height),
            )
        }
    }
}

// MARK: The board's offer

/**
 * The board's offer at the top of the CEO Office: the chapter, three goals side by side (stacked
 * on a phone), each with its target, how it is measured, what it pays, what a miss costs and a
 * Choose button. Not a pause: time keeps running while the player decides.
 */
@Composable
fun BoardOfferCard(store: GameStore, chapter: ChapterDto, modifier: Modifier = Modifier) {
    val weeks = chapter.offerWeeksLeft.toInt()
    Panel(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PixelIcon("flag", size = 20.dp, tint = Palette.gold)
            Column(Modifier.weight(1f)) {
                Text(chapter.name, color = Palette.primaryText, fontSize = 17.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                Text(
                    "The board offers ${numberWord(chapter.offered.size)} ${if (chapter.offered.size == 1) "goal" else "goals"} for this year. Pick one, or leave it and nothing is lost.",
                    color = Palette.secondary, fontSize = 12.sp,
                )
            }
            SignalPill(if (weeks == 1) "Decide within 1 week" else "Decide within $weeks weeks", if (weeks <= 1) Palette.warning else Palette.gold)
        }
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val wide = maxWidth >= 760.dp
            if (wide) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    chapter.offered.forEach { OfferChoice(store, chapter, it, Modifier.weight(1f)) }
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    chapter.offered.forEach { OfferChoice(store, chapter, it, Modifier.fillMaxWidth()) }
                }
            }
        }
    }
}

@Composable
private fun OfferChoice(store: GameStore, chapter: ChapterDto, mandate: MandateDto, modifier: Modifier) {
    Column(
        modifier.clip(Pixel.shape).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.shape).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(mandate.title, color = Palette.primaryText, fontSize = 15.sp, fontWeight = FontWeight.Bold)
        OfferLine("MEASURED", mandate.detail, Palette.secondary)
        OfferLine("REWARD", REWARD_LINE, Palette.positive)
        OfferLine("RISK", riskOf(mandate), Palette.warning)
        Spacer(Modifier.height(2.dp))
        GoldButton("Choose", Modifier.fillMaxWidth(), enabled = !store.busy) {
            store.chooseMandate(chapter.firm, mandate.index.toInt())
        }
    }
}

@Composable
private fun OfferLine(label: String, text: String, tint: Color) {
    Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
        Text(label, color = tint, fontSize = 10.sp, fontFamily = Pixel.font, letterSpacing = 0.8.sp)
        Text(text, color = Palette.primaryText.copy(alpha = 0.9f), fontSize = 12.sp, lineHeight = 16.sp)
    }
}

// MARK: The goal in force

/** The chosen goal: what it is, a progress bar, the number and a sentence, and the weeks left in the chapter. */
@Composable
fun MandateStrip(store: GameStore, chapter: ChapterDto, mandate: MandateDto, modifier: Modifier = Modifier) {
    val now = store.overview?.tick?.toInt() ?: chapter.startWeek.toInt()
    val left = (chapter.endWeek.toInt() - now).coerceAtLeast(0)
    val percent = (mandate.progressBps / 100).coerceIn(0, 100)
    val tint = when {
        mandate.lost -> Palette.negative
        mandate.progressBps >= 10_000 -> Palette.positive
        else -> Palette.gold
    }
    Panel(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PixelIcon("target", size = 18.dp, tint = Palette.gold)
            Column(Modifier.weight(1f)) {
                Eyebrow("Board goal")
                Text(mandate.title, color = Palette.primaryText, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            }
            Text(if (mandate.lost) "LOST" else "$percent%", color = tint, fontFamily = Pixel.font, fontSize = 18.sp, fontWeight = FontWeight.Bold)
        }
        PixelBar(mandate.progressBps / 10_000f, tint)
        Text(mandate.progressText, color = Palette.secondary, fontSize = 12.sp)
        Text(
            if (left == 1) "1 week left in ${chapter.name.substringBefore(":")}" else "$left weeks left in ${chapter.name.substringBefore(":")}",
            color = Palette.secondary, fontSize = 11.sp,
        )
    }
}

// MARK: A chapter closes

private fun bonusText(chapter: ChapterDto): String {
    val rep = chapter.reputationBps / 100.0
    val repText = "%s%.1f reputation".format(if (rep >= 0) "+" else "", rep)
    return if (chapter.bonusCents > 0) "Paid a bonus of ${Format.compact(chapter.bonusCents)} and $repText." else "Cost $repText."
}

/** Shown once when a chapter closes with a goal met or missed. */
@Composable
fun ChapterResultPanel(chapter: ChapterDto, onDismiss: () -> Unit, modifier: Modifier = Modifier) {
    val met = chapter.outcome == "met"
    val goal = chapter.chosen?.let { chapter.offered.getOrNull(it.toInt()) }
    Panel(modifier) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PixelIcon(if (met) "check_circle" else "close", size = 20.dp, tint = if (met) Palette.positive else Palette.negative)
            Column(Modifier.weight(1f)) {
                Text(chapter.name, color = Palette.primaryText, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                Text(if (met) "The board's goal was met." else "The board's goal was missed.", color = if (met) Palette.positive else Palette.negative, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            }
            TextButton(onClick = onDismiss) { Text("Close", color = Palette.gold) }
        }
        goal?.let { Text(it.title, color = Palette.primaryText, fontSize = 13.sp) }
        Text(bonusText(chapter), color = Palette.secondary, fontSize = 12.sp)
    }
}

/** The offer, the goal in force, or a chapter result, whichever applies, for the top of the CEO Office. */
@Composable
fun ChapterBlock(store: GameStore, modifier: Modifier = Modifier) {
    val offer = store.boardOffer
    val active = store.activeMandate
    val result = store.chapterResult
    Column(modifier, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        result?.let { ChapterResultPanel(it, onDismiss = store::dismissChapterResult) }
        offer?.let { BoardOfferCard(store, it) }
        active?.let { (chapter, mandate) -> MandateStrip(store, chapter, mandate) }
    }
}

/** Phone Office: the same block as list items. */
internal fun LazyListScope.chapterItems(store: GameStore) {
    if (store.boardOffer != null || store.activeMandate != null || store.chapterResult != null) {
        item { ChapterBlock(store) }
    }
}

// MARK: Hall of Careers

private fun badgeIcon(id: String) = when (id) {
    "year_one" -> "calendar"
    "first_mandate" -> "check_circle"
    "five_mandates" -> "star"
    "three_in_a_row" -> "trend_up"
    "first_raid" -> "target"
    "first_listing" -> "markets"
    "subsidiary_listed" -> "branch"
    "first_merger" -> "swap"
    "ten_companies" -> "briefcase"
    "three_states" -> "globe"
    "five_divisions" -> "stack"
    "market_leader" -> "flag"
    "thousand_workers" -> "people"
    "net_worth_double" -> "coin"
    "net_worth_tenfold" -> "cash"
    "debt_free" -> "bank"
    "hostile_survived" -> "umbrella"
    "bank_rescued" -> "hand"
    "all_events" -> "history"
    "thirty_years" -> "sparkle"
    else -> "star"
}

/** An 8-bit badge: the pixel icon in a stepped frame. Locked badges are dim and grey. */
@Composable
fun AchievementBadge(item: AchievementDto, size: Dp = 52.dp) {
    val earned = item.earned
    Box(
        Modifier.size(size).padding(end = 2.dp, bottom = 2.dp)
            .pixelFrame(if (earned) Palette.panelRaised else Palette.panel, if (earned) Palette.gold else Palette.stroke, shadow = 2.dp, raised = earned)
            .alpha(if (earned) 1f else 0.35f),
        contentAlignment = Alignment.Center,
    ) {
        PixelIcon(badgeIcon(item.id), size = size * 0.6f, tint = if (earned) null else Palette.secondary)
    }
}

@Composable
private fun AchievementTile(item: AchievementDto, modifier: Modifier) {
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        AchievementBadge(item)
        Column(Modifier.weight(1f)) {
            Text(item.title, color = if (item.earned) Palette.primaryText else Palette.secondary.copy(alpha = 0.7f), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 2)
            Text(
                if (item.earned) "Earned ${ResultBuckets.weekLabel(item.week.toInt())}" else item.detail,
                color = Palette.secondary.copy(alpha = if (item.earned) 1f else 0.6f), fontSize = 11.sp, maxLines = 2,
            )
        }
    }
}

private fun outcomeLabel(outcome: String) = when (outcome) {
    "met" -> "Met"
    "missed" -> "Missed"
    "lapsed" -> "No goal chosen"
    "cancelled" -> "Cancelled"
    else -> "In progress"
}

private fun outcomeTint(outcome: String) = when (outcome) {
    "met" -> Palette.positive
    "missed" -> Palette.negative
    "open" -> Palette.gold
    else -> Palette.secondary
}

@Composable
private fun ChapterRow(chapter: ChapterDto) {
    val goal = chapter.chosen?.let { chapter.offered.getOrNull(it.toInt()) }
    val year = ResultBuckets.startYear + (chapter.startWeek.toInt() - 1) / 52
    Column(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.shape).padding(10.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(chapter.name, color = Palette.primaryText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                Text("$year · weeks ${chapter.startWeek} to ${chapter.endWeek}", color = Palette.secondary, fontSize = 11.sp)
            }
            SignalPill(outcomeLabel(chapter.outcome), outcomeTint(chapter.outcome))
        }
        Text(
            goal?.title ?: if (chapter.offered.isEmpty()) "The board made no offer." else "No goal was chosen.",
            color = Palette.primaryText.copy(alpha = 0.9f), fontSize = 12.sp,
        )
        if (chapter.outcome == "met" || chapter.outcome == "missed") Text(bonusText(chapter), color = Palette.secondary, fontSize = 11.sp)
    }
}

@Composable
private fun CareerLine(career: CareerSummary) {
    Column(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.shape).padding(10.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(career.title, color = Palette.primaryText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
        Text("${career.scenario} · ${career.weeks / 52} years", color = Palette.secondary, fontSize = 11.sp)
        Text(
            "${career.chapters} ${if (career.chapters == 1) "chapter" else "chapters"} · ${career.met} met, ${career.missed} missed · ${career.achievements} of 20 achievements",
            color = Palette.primaryText.copy(alpha = 0.9f), fontSize = 12.sp,
        )
    }
}

/** The Hall of Careers for the open career: chapters in order, achievements as badges, the score, then past careers. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun HallScreen(store: GameStore, settings: AppSettings = LocalAppSettings.current, modifier: Modifier = Modifier) {
    DisposableEffect(store) {
        store.hallOpen = true
        onDispose { store.hallOpen = false }
    }
    LaunchedEffect(store.overview?.tick) { store.refreshHall() }
    val hall = store.hall
    LazyColumn(
        modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item { ScreenHeader("Hall of Careers", hall?.let { "${it.weeks / 52u} years in" }) }
        if (hall == null) {
            item { Callout("The record opens once the career is running.", Palette.secondary) }
            return@LazyColumn
        }
        item {
            Panel {
                Eyebrow("This career")
                Text(
                    "${hall.mandatesMet} goals met, ${hall.mandatesMissed} missed · ${hall.achievementsEarned} of ${hall.achievements.size} achievements",
                    color = Palette.primaryText, fontSize = 14.sp,
                )
                store.score?.let { score ->
                    Text("Five-year score: ${score.total}", color = Palette.gold, fontFamily = Pixel.font, fontSize = 16.sp, fontWeight = FontWeight.Bold)
                    Text(
                        "Return ${score.returnComponent} · Expansion ${score.expansionComponent} · Leadership ${score.leadershipComponent} · Resilience ${score.resilienceComponent}",
                        color = Palette.secondary, fontSize = 11.sp,
                    )
                }
            }
        }
        item { Eyebrow("Chapters") }
        if (hall.chapters.isEmpty()) item { Callout("The first chapter opens in week 1.", Palette.secondary) }
        items(hall.chapters.size) { ChapterRow(hall.chapters[it]) }
        item { Eyebrow("Achievements") }
        item {
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                val columns = if (maxWidth >= 900.dp) 3 else if (maxWidth >= 560.dp) 2 else 1
                val tileWidth = (maxWidth - 12.dp * (columns - 1)) / columns
                FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    hall.achievements.sortedByDescending { it.earned }.forEach { AchievementTile(it, Modifier.width(tileWidth)) }
                }
            }
        }
        if (settings.careers.isNotEmpty()) {
            item { Eyebrow("Earlier careers") }
            items(settings.careers.size) { CareerLine(settings.careers[it]) }
        }
    }
}

/** The main menu's list of careers across games. */
@Composable
fun CareerListScreen(settings: AppSettings, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize().background(Palette.ink).padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onBack) { BackLabel("Back", Palette.gold) }
        }
        ScreenHeader("Hall of Careers", "Every career you have left or finished")
        if (settings.careers.isEmpty()) Callout("No finished careers yet. Leave a career from its menu and it is kept here.", Palette.secondary)
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            settings.careers.forEach { CareerLine(it) }
        }
    }
}
