package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.AchievementDto
import uniffi.cw_ffi.CareerEventDto
import uniffi.cw_ffi.CareerYearDto
import uniffi.cw_ffi.ChapterDto

/**
 * Research (the ladder stays, with a grid of every level beside it), You: Career (chapters and the
 * board's mandates) and Hall of Careers (achievements and earlier careers), in the 2.0 shell.
 * Ambition, medals and the end of career belong to the careers engine (PR 241) and are not here yet.
 */

// MARK: Research

private class LevelRow(val level: Int, val state: String, val intensityBps: Int?, val qualityBps: Int?)

internal val LevelViews = listOf(
    ViewState("Ladder", cols("level", "state", "intensity", "quality"), sort = "level", descending = false),
    ViewState("Still to reach", cols("level", "state", "intensity", "quality"), filters = listOf(Filter(FilterOp.Not, "state", "Reached")), sort = "level", descending = false),
)

private fun levelColumns(): List<GridColumn<LevelRow>> = listOf(
    GridColumn("level", "Level", null, numeric = false, text = { "Level ${it.level}" }, sort = { it.level.toLong() }, minWidth = 90.dp, bold = true, topic = "About"),
    GridColumn("state", "State", 110.dp, numeric = false, text = { it.state }, tone = { if (it.state == "Reached") Palette.gold else if (it.state == "Next") Palette.positive else Palette.secondary }, topic = "About"),
    GridColumn("intensity", "Inputs per unit", 110.dp, text = { l -> l.intensityBps?.let { "%.0f%%".format(it / 100.0) } ?: DASH }, sort = { (it.intensityBps ?: 0).toLong() }, scale = 100.0, topic = "Effect",
        tip = "Inputs and labour per unit, as a share of the baseline. The engine gives it for your level and the next only."),
    GridColumn("quality", "Quality", 84.dp, text = { l -> l.qualityBps?.let { "%.0f".format(it / 100.0) } ?: DASH }, sort = { (it.qualityBps ?: 0).toLong() }, scale = 100.0, topic = "Effect", tip = "Out of 200."),
)

@Composable
fun ResearchGridScreen(store: GameStore) {
    val r = store.research
    SplitOrStack(Modifier.fillMaxSize(), 1.3f, 1f, 900.dp, 560.dp, left = { m -> Box(m) { ResearchScreen(store) } }, right = { m ->
        if (r != null) {
            val rows = (1..r.maxLevel.toInt()).map { n ->
                LevelRow(
                    n,
                    when { n <= r.level.toInt() -> "Reached"; n == r.level.toInt() + 1 -> "Next"; n <= r.frontier.toInt() + 1 -> "Within reach"; else -> "Beyond" },
                    when (n) { r.level.toInt() -> r.intensityBps; r.level.toInt() + 1 -> r.nextIntensityBps; else -> null },
                    when (n) { r.level.toInt() -> r.qualityBps; r.level.toInt() + 1 -> r.nextQualityBps; else -> null },
                )
            }
            DataGrid(
                "Levels", rows, levelColumns(), key = { it.level },
                views = GridViews("research-levels", LevelViews),
                modifier = m.padding(top = 8.dp, end = 8.dp, bottom = 8.dp), phoneBelow = 380.dp,
                filterText = { "Level ${it.level} ${it.state}" },
                breakdown = { l -> "Level ${l.level}: ${l.state}" + (if (l.level == r.frontier.toInt() + 1) ". One past the industry frontier, the furthest you can research." else "") },
                initialSort = "level", initialDescending = false,
            )
        }
    })
}

// MARK: Career

internal val ChapterViews = listOf(
    ViewState("Chapters", cols("name", "weeks", "state", "goal", "outcome", "bonus"), sort = "num", descending = true),
    ViewState("Results", cols("name", "outcome", "bonus", "rep"), filters = listOf(Filter(FilterOp.Not, "outcome", "Open")), sort = "bonus"),
)

private fun chosenOf(c: ChapterDto) = c.chosen?.let { c.offered.getOrNull(it.toInt()) }

private fun outcomeWords(c: ChapterDto) = when (c.outcome) { "open" -> "Open"; "met" -> "Met"; "missed" -> "Missed"; "lapsed" -> "Not taken"; "cancelled" -> "Cancelled"; else -> c.outcome }

private fun chapterColumns(): List<GridColumn<ChapterDto>> = listOf(
    GridColumn("name", "Chapter", null, numeric = false, text = { it.name }, minWidth = 190.dp, bold = true, topic = "About"),
    GridColumn("num", "No.", 50.dp, text = { "${it.number}" }, sort = { it.number.toLong() }, shown = false, topic = "About"),
    GridColumn("weeks", "Weeks", 100.dp, numeric = false, text = { "${it.startWeek} to ${if (it.status == "current") "now" else "${it.endWeek}"}" }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("state", "State", 80.dp, numeric = false, text = { if (it.status == "current") "Current" else "Closed" }, tone = { if (it.status == "current") Palette.gold else Palette.secondary }, topic = "About"),
    GridColumn("goal", "Goal taken", 240.dp, numeric = false, text = { chosenOf(it)?.title ?: DASH }, dropOrder = 2, topic = "Goal"),
    GridColumn("outcome", "Result", 84.dp, numeric = false, text = { outcomeWords(it) }, tone = { when (it.outcome) { "met" -> Palette.positive; "missed" -> Palette.negative; else -> Palette.secondary } }, topic = "Goal"),
    GridColumn("bonus", "Bonus", 84.dp, text = { if (it.bonusCents > 0) Format.compact(it.bonusCents) else DASH }, sort = { it.bonusCents }, scale = 100.0, total = true, topic = "Goal"),
    GridColumn("rep", "Reputation", 84.dp, text = { if (it.outcome == "open") DASH else changeText(it.reputationBps.toLong()) }, sort = { it.reputationBps.toLong() }, tone = { changeTint(it.reputationBps.toLong()) }, scale = 100.0, topic = "Goal"),
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CareerScreen(store: GameStore) {
    DisposableEffect(store) { store.hallOpen = true; onDispose { store.hallOpen = false } }
    LaunchedEffect(store.overview?.tick) { store.refreshHall() }
    val hall = store.hall
    val act = LocalActions.current
    var pick by remember { mutableStateOf<UInt?>(null) }
    if (hall == null) { Box(Modifier.fillMaxSize().padding(16.dp)) { Text("The record opens once the career is running.", color = Palette.secondary, fontSize = 12.sp) }; return }
    val chapters = hall.chapters
    val standing = rememberEngine(store, store.overview?.tick) { h -> h.careerStanding() }
    val summary = rememberEngine(store, store.overview?.tick) { h -> h.careerSummary() }
    val years = summary?.yearsLog.orEmpty()
    val chosen = chapters.firstOrNull { it.number == pick } ?: chapters.lastOrNull()
    val score = store.score
    Column(Modifier.fillMaxSize().padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FigureLine(
            *listOfNotNull(
                standing?.ambition?.let { Triple("Ambition", ambitionWords(it), Palette.gold) },
                standing?.let { Triple("Medal", medalWords(it.medal), medalTint(it.medal)) },
                standing?.takeIf { it.league > 0u }?.let { Triple("Rank", "#${it.rank} of ${it.league}", Palette.primaryText) },
                standing?.let { Triple("Growth", changeText(it.growthBps), changeTint(it.growthBps)) },
                standing?.takeIf { it.weeksLeft > 0u && !it.ended }?.let { Triple("Weeks left", "${it.weeksLeft}", Palette.primaryText) },
                score?.let { Triple("Five-year score", it.total.toString(), Palette.gold) },
                score?.let { Triple("Return", it.returnComponent.toString(), Palette.primaryText) },
                score?.let { Triple("Expansion", it.expansionComponent.toString(), Palette.primaryText) },
                score?.let { Triple("Leadership", it.leadershipComponent.toString(), Palette.primaryText) },
                score?.let { Triple("Resilience", it.resilienceComponent.toString(), Palette.primaryText) },
                Triple("Goals met", "${hall.mandatesMet} of ${hall.mandatesMet + hall.mandatesMissed}", Palette.positive),
                Triple("Years in", "${hall.weeks / 52u}", Palette.primaryText),
            ).toTypedArray(),
        )
        SplitOrStack(Modifier.fillMaxWidth().weight(1f), 1.8f, 1f, 760.dp, 560.dp, left = { m ->
          Column(m, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            DataGrid(
                "Chapters", chapters, chapterColumns(), key = { it.number },
                views = GridViews("career-chapters", ChapterViews, mine = { true }),
                modifier = Modifier.fillMaxWidth().weight(1f),
                filterText = { "${it.name} ${chosenOf(it)?.title.orEmpty()}" },
                selected = chosen?.number, onSelect = { pick = it.number },
                breakdown = { c -> "${c.name}: ${outcomeWords(c)}" + (chosenOf(c)?.let { ", goal: ${it.title}" } ?: "") },
                initialSort = "num",
            )
            if (years.isNotEmpty()) DataGrid(
                "Year by year", years, yearColumns(), key = { it.year },
                views = GridViews("career-years", YearViews),
                modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(years.size.coerceAtMost(8)) + 30.dp),
                filterText = { "year ${it.year}" },
                initialSort = "year", initialDescending = false,
            )
          }
        }, right = { m ->
            DetailPane(chosen?.name ?: "Chapter", m) {
                if (standing != null) {
                    if (standing.ended && summary != null) {
                        Text(summary.ending.replace('_', ' ').replaceFirstChar { it.uppercase() }, color = if (summary.lost) Palette.negative else Palette.gold, fontFamily = Pixel.font, fontSize = 14.sp)
                        if (summary.sentence.isNotEmpty()) Text(summary.sentence, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
                        Text("Net worth ${Format.compact(summary.startWorthCents)} to ${Format.compact(summary.netWorthCents)}, ${changeText(summary.growthBps)}. Medal: ${medalWords(summary.medal)}.", color = Palette.primaryText, fontSize = 12.sp)
                    } else {
                        if (standing.detail.isNotEmpty()) Text(standing.detail, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
                        if (standing.next.isNotEmpty()) Text("Next medal: ${standing.next}", color = Palette.gold, fontSize = 12.sp, lineHeight = 16.sp)
                    }
                    if (standing.boardWarned && standing.boardWarning.isNotEmpty()) Text(standing.boardWarning, color = Palette.warning, fontSize = 12.sp, lineHeight = 16.sp)
                    summary?.events?.takeIf { it.isNotEmpty() }?.takeLast(4)?.forEach { e -> Text("Week ${e.week}: ${e.text}", color = Palette.secondary, fontSize = 11.sp, lineHeight = 15.sp) }
                }
                if (chosen == null) { Text("The first chapter opens in week 1.", color = Palette.secondary, fontSize = 12.sp); return@DetailPane }
                val taken = chosenOf(chosen)
                if (chosen.offered.isEmpty()) Text("The board made no offer this chapter.", color = Palette.secondary, fontSize = 12.sp)
                chosen.offered.forEachIndexed { i, m ->
                    val isTaken = chosen.chosen == i.toUInt()
                    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Text((if (isTaken) "TAKEN: " else "") + m.title, color = if (isTaken) Palette.gold else Palette.primaryText, fontSize = 13.sp)
                        Text(m.detail, color = Palette.secondary, fontSize = 11.sp, lineHeight = 15.sp)
                        if (isTaken && chosen.outcome == "open") {
                            Text(m.progressText, color = if (m.lost) Palette.negative else Palette.primaryText, fontSize = 11.sp)
                        }
                    }
                }
                if (chosen.status == "current" && chosen.chosen == null && chosen.offerWeeksLeft > 0u) {
                    Text("The offer closes in ${chosen.offerWeeksLeft} weeks.", color = Palette.gold, fontSize = 12.sp)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        chosen.offered.forEachIndexed { i, m -> GridButton("Take goal ${i + 1}", false) { act("ChooseMandate", mapOf("firm" to "${chosen.firm}", "pick" to "$i")) } }
                    }
                }
                if (taken == null && chosen.status != "current") Text("You did not take a goal this chapter.", color = Palette.secondary, fontSize = 12.sp)
                if (taken != null && chosen.outcome != "open") Text("Result: ${outcomeWords(chosen)}. Bonus ${Format.compact(chosen.bonusCents)}, reputation ${changeText(chosen.reputationBps.toLong())}.", color = Palette.primaryText, fontSize = 12.sp)
            }
        })
    }
}

// MARK: Hall of Careers

internal val AchievementViews = listOf(
    ViewState("Achievements", cols("name", "state", "week", "detail"), sort = "state"),
    ViewState("Still to earn", cols("name", "detail"), filters = listOf(Filter(FilterOp.Not, "state", "Earned"))),
)

private fun achievementColumns(): List<GridColumn<AchievementDto>> = listOf(
    GridColumn("name", "Achievement", null, numeric = false, text = { it.title }, minWidth = 190.dp, bold = true, topic = "About"),
    GridColumn("state", "State", 80.dp, numeric = false, text = { if (it.earned) "Earned" else "Locked" }, tone = { if (it.earned) Palette.gold else Palette.secondary }, topic = "About"),
    GridColumn("week", "Week", 64.dp, text = { if (it.earned) "${it.week}" else DASH }, sort = { if (it.earned) it.week.toLong() else 0L }, topic = "About"),
    GridColumn("detail", "What it takes", 420.dp, numeric = false, text = { it.detail }, tone = { Palette.secondary }, dropOrder = 1, topic = "About"),
)

internal val EarlierViews = listOf(ViewState("Earlier careers", cols("name", "scenario", "years", "chapters", "met", "missed", "ach"), sort = null))

private fun earlierColumns(): List<GridColumn<CareerSummary>> = listOf(
    GridColumn("name", "Career", null, numeric = false, text = { it.title }, minWidth = 170.dp, bold = true, topic = "About"),
    GridColumn("scenario", "Scenario", 150.dp, numeric = false, text = { it.scenario }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("years", "Years", 60.dp, text = { "${it.weeks / 52}" }, sort = { it.weeks.toLong() }, topic = "Result"),
    GridColumn("chapters", "Chapters", 76.dp, text = { "${it.chapters}" }, sort = { it.chapters.toLong() }, topic = "Result"),
    GridColumn("met", "Met", 56.dp, text = { "${it.met}" }, sort = { it.met.toLong() }, tone = { Palette.positive }, topic = "Result"),
    GridColumn("missed", "Missed", 64.dp, text = { "${it.missed}" }, sort = { it.missed.toLong() }, topic = "Result"),
    GridColumn("ach", "Achievements", 96.dp, text = { "${it.achievements} of 20" }, sort = { it.achievements.toLong() }, topic = "Result"),
)

@Composable
fun HallGridScreen(store: GameStore, settings: AppSettings? = LocalAppSettings.current) {
    DisposableEffect(store) { store.hallOpen = true; onDispose { store.hallOpen = false } }
    LaunchedEffect(store.overview?.tick) { store.refreshHall() }
    val hall = store.hall
    val earlier = settings?.careers.orEmpty()
    Column(Modifier.fillMaxSize().padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FigureLine(
            Triple("This career", "${(hall?.weeks ?: 0u) / 52u} years", Palette.primaryText),
            Triple("Goals met", "${hall?.mandatesMet ?: 0}", Palette.positive),
            Triple("Goals missed", "${hall?.mandatesMissed ?: 0}", Palette.primaryText),
            Triple("Achievements", "${hall?.achievementsEarned ?: 0} of ${hall?.achievements?.size ?: 0}", Palette.gold),
            Triple("Earlier careers", "${earlier.size}", Palette.primaryText),
        )
        SplitOrStack(Modifier.fillMaxWidth().weight(1f), 1.4f, 1f, 560.dp, 300.dp, left = { m ->
            DataGrid(
                "Achievements", hall?.achievements.orEmpty(), achievementColumns(), key = { it.id },
                views = GridViews("hall-achievements", AchievementViews),
                modifier = m,
                filterText = { "${it.title} ${it.detail}" },
                breakdown = { a -> "${a.title}: ${a.detail}" + if (a.earned) " Earned in week ${a.week}." else "" },
                initialSort = "state",
            )
        }, right = { m ->
            if (earlier.isEmpty()) Box(m.padding(8.dp)) { Text("Careers you finish are kept here.", color = Palette.secondary, fontSize = 12.sp) } else DataGrid(
                "Earlier careers", earlier, earlierColumns(), key = { it.title + it.weeks + it.scenario },
                views = GridViews("hall-earlier", EarlierViews),
                modifier = m, phoneBelow = 380.dp,
                filterText = { "${it.title} ${it.scenario}" },
            )
        })
    }
}

private fun ambitionWords(key: String) = when (key) {
    "richest" -> "Richest"
    "biggest" -> "Biggest"
    "dominant" -> "Dominant"
    "survivor" -> "Survivor"
    else -> key.replaceFirstChar { it.uppercase() }
}

private fun medalWords(key: String) = when (key) { "bronze" -> "Bronze"; "silver" -> "Silver"; "gold" -> "Gold"; else -> "None yet" }

private fun medalTint(key: String) = if (key == "none") Palette.secondary else Palette.gold

internal val YearViews = listOf(
    ViewState("Years", cols("year", "rank", "value", "worth", "wealth", "medal"), sort = "year", descending = false),
)

private fun yearColumns(): List<GridColumn<CareerYearDto>> = listOf(
    GridColumn("year", "Year", null, numeric = false, text = { "Year ${it.year}" }, sort = { it.year.toLong() }, minWidth = 80.dp, bold = true, topic = "About"),
    GridColumn("rank", "Company rank", 100.dp, text = { "#${it.rank} of ${it.league}" }, sort = { it.rank.toLong() }, topic = "Company"),
    GridColumn("value", "Company value", 100.dp, text = { Format.compact(it.companyValueCents) }, sort = { it.companyValueCents }, scale = 100.0, topic = "Company"),
    GridColumn("worth", "Net worth", 90.dp, text = { Format.compact(it.netWorthCents) }, sort = { it.netWorthCents }, scale = 100.0, topic = "You"),
    GridColumn("wealth", "Wealth rank", 90.dp, text = { "#${it.wealthRank}" }, sort = { it.wealthRank.toLong() }, topic = "You"),
    GridColumn("medal", "Medal", 70.dp, numeric = false, text = { medalWords(it.medal) }, tone = { medalTint(it.medal) }, topic = "You"),
)
