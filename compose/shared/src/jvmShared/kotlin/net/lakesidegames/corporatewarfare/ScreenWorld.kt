package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.ElectionRaceDto
import uniffi.cw_ffi.LawsDto
import uniffi.cw_ffi.LetterDto
import uniffi.cw_ffi.PublicTenderDto

// MARK: Politics and laws

private fun partyLabel(key: String) = if (key == "business") "Commerce Party" else if (key.isBlank()) DASH else "Workers' Party"

/**
 * World: Politics and laws. Governors and their races in one grid (laws in force and pending
 * beside it, gifts with the cap), the public tenders under it with their bid action.
 */
@Composable
fun PoliticsScreen(store: GameStore) {
    DisposableEffect(store) {
        store.politicsOpen = true
        store.refreshPolitics()
        onDispose { store.politicsOpen = false }
    }
    val data = store.politics
    val races = data?.races.orEmpty()
    val tenders = data?.tenders.orEmpty()
    val now = store.overview?.tick?.toInt() ?: 0
    var racePick by remember { mutableStateOf<UInt?>(null) }
    var tenderPick by remember { mutableStateOf<UInt?>(null) }
    val race = races.firstOrNull { it.office == racePick } ?: races.firstOrNull { it.nearYou } ?: races.firstOrNull()
    val tender = tenders.firstOrNull { it.id == tenderPick } ?: tenders.firstOrNull { it.status == "open" } ?: tenders.firstOrNull()
    val act = LocalActions.current
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(Modifier.weight(1f).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            DataGrid(
                "Governors and races", races, raceColumns(now), key = { it.office },
                views = GridViews("politics", PoliticsViews, mine = { it.nearYou || it.youGaveCents > 0 }, scopes = raceScopes(races)),
                modifier = Modifier.weight(3f).fillMaxWidth(),
                filterText = { "${it.place} ${it.governor} ${partyLabel(it.governorParty)}" },
                selected = race?.office, onSelect = { racePick = it.office },
                breakdown = { r -> "${r.place}: governor ${r.governor} (${partyLabel(r.governorParty)}), ${r.lawsInForce.size} laws in force, ${r.lawsPending.size} coming, election in ${r.weeksLeft} weeks" },
                initialSort = "left", initialDescending = false,
            )
            DataGrid(
                "Public tenders", tenders, tenderColumns(now), key = { it.id },
                views = GridViews("tenders", TenderViews, mine = { it.yourPriceCents > 0 }),
                modifier = Modifier.weight(2f).fillMaxWidth(),
                filterText = { "${it.place} ${it.kindLabel} ${it.winner}" },
                selected = tender?.id, onSelect = { tenderPick = it.id },
                breakdown = { t -> "${t.kindLabel} in ${t.place}: budget ${Format.compact(t.budgetCents)}, ${t.bids} bids, ${t.status}" },
                initialSort = "closes", initialDescending = false,
            )
        }
        DetailPane(race?.place ?: "Politics", Modifier.width(420.dp).fillMaxHeight()) {
            val e = store.economy
            if (e != null) {
                FigureLine(
                    Triple("Governing party", e.partyName.ifBlank { partyLabel(e.party) }, Palette.gold),
                    Triple("Approval", "%.0f%%".format(e.approvalBps / 100.0), Palette.primaryText),
                    Triple("Election", if (e.nextElectionWeek > 0u) ResultBuckets.weekLabel(e.nextElectionWeek.toInt()) else DASH, Palette.primaryText),
                )
                DataGrid(
                    "National laws", lawRows(e.laws, e.pendingLaws), lawColumns(e.pendingLaws != null), key = { it.name },
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(10)), initialSort = null, phoneBelow = 200.dp,
                )
                if (e.pendingLaws != null && e.pendingEffectiveWeek > 0u) Text("The change takes effect in week ${e.pendingEffectiveWeek}.", color = Palette.secondary, fontSize = 11.sp)
            }
            if (race != null) {
                Text("${race.place}: governor ${race.governor}, ${partyLabel(race.governorParty)}. " + (if (race.open) "Election in ${race.weeksLeft} weeks, poll ${"%.0f%%".format(race.pollBps / 100.0)}." else "No election is open."), color = Palette.primaryText, fontSize = 12.sp)
                if (race.lawsInForce.isNotEmpty()) Text("In force: " + race.lawsInForce.joinToString("; "), color = Palette.secondary, fontSize = 11.sp)
                if (race.lawsPending.isNotEmpty()) Text("Coming in week ${race.pendingWeek}: " + race.lawsPending.joinToString("; "), color = Palette.warning, fontSize = 11.sp)
                if (race.open) {
                    race.candidates.forEach { c ->
                        Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Text("${c.name}, ${partyLabel(c.party)}" + (if (c.fundsCents > 0) ", gifts ${Format.compact(c.fundsCents)}" else ""), color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f))
                            GridButton("Give", false) { act("GiveToCandidate", mapOf("office" to "${race.office}", "candidate" to if (c.party == "business") "0" else "1")) }
                        }
                    }
                    Text("Gifts are capped at ${Format.compact(race.capCents)} per company for this race; you have given ${Format.compact(race.youGaveCents)}. ${data?.disclosureNote.orEmpty()}", color = Palette.secondary, fontSize = 10.sp)
                }
            }
            if (tender != null) {
                Text("${tender.kindLabel.replaceFirstChar { it.uppercase() }} in ${tender.place}: budget ${Format.compact(tender.budgetCents)}, ${tender.bids} bids" + (if (tender.status == "open") ", closes ${ResultBuckets.weekLabel(tender.closesWeek.toInt())}" else ", ${tender.status}") + ".", color = Palette.primaryText, fontSize = 12.sp)
                if (tender.status == "open") {
                    GoldButton(if (tender.yourPriceCents > 0) "Change your bid" else "Bid on this tender") {
                        act("BidOnTender", buildMap {
                            put("tender", "${tender.id}")
                            put("price_cents", "${if (tender.yourPriceCents > 0) tender.yourPriceCents else tender.budgetCents * 9 / 10}")
                            put("weeks", "${if (tender.yourWeeks > 0u) tender.yourWeeks else (tender.minWeeks + tender.maxWeeks) / 2u}")
                        })
                    }
                    Text("The lowest price wins, with each week of delivery counted as a hundredth of the budget. Late work forfeits a tenth of the price a week.", color = Palette.secondary, fontSize = 10.sp)
                }
            }
        }
    }
}

private class LawRow(val name: String, val now: String, val pending: String?)

private fun yesNo(b: Boolean) = if (b) "Yes" else "No"
private fun p(bps: Long) = "%.1f%%".format(bps / 100.0)

private fun lawRows(l: LawsDto, next: LawsDto?): List<LawRow> {
    fun row(name: String, now: String, nxt: String?) = LawRow(name, now, nxt?.takeIf { it != now })
    return listOf(
        row("Company tax", p(l.corporateTaxBps.toLong()), next?.let { p(it.corporateTaxBps.toLong()) }),
        row("Wage floor, an hour", Format.money(l.wageFloorHourlyCents), next?.let { Format.money(it.wageFloorHourlyCents) }),
        row("Merger review", p(l.mergerReviewBps), next?.let { p(it.mergerReviewBps) }),
        row("Unions", l.unions.replace('_', ' ').replaceFirstChar { it.uppercase() }, next?.unions?.replace('_', ' ')?.replaceFirstChar { it.uppercase() }),
        row("Common carrier rule", yesNo(l.commonCarrier), next?.let { yesNo(it.commonCarrier) }),
        row("Review of vertical deals", p(l.verticalReviewBps), next?.let { p(it.verticalReviewBps) }),
        row("State reserve", yesNo(l.reserve), next?.let { yesNo(it.reserve) }),
        row("Rescue of failing firms", l.rescue.replace('_', ' ').replaceFirstChar { it.uppercase() }, next?.rescue?.replace('_', ' ')?.replaceFirstChar { it.uppercase() }),
        row("Privatisation", yesNo(l.privatise), next?.let { yesNo(it.privatise) }),
        row("Export quota", p(l.quotaBps.toLong()), next?.let { p(it.quotaBps.toLong()) }),
    )
}

private fun lawColumns(pending: Boolean): List<GridColumn<LawRow>> = buildList {
    add(GridColumn("name", "Law", null, minWidth = 140.dp, numeric = false, text = { it.name }, bold = true))
    add(GridColumn("now", "In force", 90.dp, numeric = false, text = { it.now }))
    if (pending) add(GridColumn("next", "Coming", 90.dp, numeric = false, text = { it.pending ?: "" }, tone = { Palette.warning }))
}

private fun raceColumns(now: Int): List<GridColumn<ElectionRaceDto>> = listOf(
    GridColumn("name", "Place", null, minWidth = 150.dp, numeric = false, text = { it.place }, tone = { if (it.nearYou) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("governor", "Governor", 150.dp, numeric = false, text = { it.governor }, topic = "About"),
    GridColumn("party", "Party", 120.dp, numeric = false, text = { partyLabel(it.governorParty) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("poll", "Poll", 64.dp, text = { if (it.open) "%.0f%%".format(it.pollBps / 100.0) else DASH }, sort = { if (it.open) it.pollBps.toLong() else Long.MIN_VALUE }, series = { r -> r.history.map { it.toLong() } }, fmt = { "%.0f%%".format(it / 100.0) }, scale = 100.0, topic = "Election",
        tip = "The governing party's weekly poll. Its change and trend use the poll history the engine keeps for each race."),
    GridColumn("left", "Election in", 80.dp, text = { if (it.open) "${it.weeksLeft} wk" else DASH }, sort = { if (it.open) it.weeksLeft.toLong() else Long.MAX_VALUE }, topic = "Election"),
    GridColumn("laws", "Laws in force", 92.dp, text = { "${it.lawsInForce.size}" }, sort = { it.lawsInForce.size.toLong() }, topic = "Laws"),
    GridColumn("pending", "Laws coming", 88.dp, text = { if (it.lawsPending.isEmpty()) "" else "${it.lawsPending.size}" }, sort = { it.lawsPending.size.toLong() }, tone = { Palette.warning }, topic = "Laws"),
    GridColumn("cap", "Gift cap", 80.dp, text = { Format.compact(it.capCents) }, sort = { it.capCents }, scale = 100.0, topic = "Gifts"),
    GridColumn("gave", "You gave", 80.dp, text = { if (it.youGaveCents > 0) Format.compact(it.youGaveCents) else "" }, sort = { it.youGaveCents }, tone = { Palette.gold }, scale = 100.0, total = true, topic = "Gifts"),
)

private fun raceScopes(rows: List<ElectionRaceDto>): List<ScopeOption<ElectionRaceDto>> =
    rows.map { it.governorParty }.distinct().map { k -> ScopeOption<ElectionRaceDto>("party:$k", partyLabel(k), "Party") { it.governorParty == k } }

internal val PoliticsViews = listOf(
    ViewState("Governors", cols("name", "governor", "party", "poll", "left", "laws", "pending"), sort = "left", descending = false),
    ViewState("Elections soon", cols("name", "governor", "party", "poll", "poll.Spark", "left", "gave"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Below, "left", number = 27.0))),
    ViewState("Laws", cols("name", "party", "laws", "pending"), sort = "pending"),
    ViewState("Near me", cols("name", "governor", "party", "poll", "left", "cap", "gave"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Mine))),
)

private fun tenderColumns(now: Int): List<GridColumn<PublicTenderDto>> = listOf(
    GridColumn("name", "Works", null, minWidth = 150.dp, numeric = false, text = { "${it.kindLabel}, ${it.place}" }, tone = { if (it.yourPriceCents > 0) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("state", "State", 100.dp, numeric = false, text = { it.state }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("status", "Status", 80.dp, numeric = false, text = { it.status.replaceFirstChar { c -> c.uppercase() } }, topic = "About"),
    GridColumn("budget", "Budget", 84.dp, text = { Format.compact(it.budgetCents) }, sort = { it.budgetCents }, scale = 100.0, total = true, topic = "Money"),
    GridColumn("bids", "Bids", 50.dp, text = { "${it.bids}" }, sort = { it.bids.toLong() }, topic = "Bids"),
    GridColumn("yours", "Your bid", 84.dp, text = { if (it.yourPriceCents > 0) Format.compact(it.yourPriceCents) else "" }, sort = { it.yourPriceCents }, tone = { Palette.gold }, scale = 100.0, topic = "Bids"),
    GridColumn("winner", "Winner", 120.dp, numeric = false, text = { it.winner.ifBlank { DASH } }, tone = { Palette.secondary }, topic = "Bids"),
    GridColumn("closes", "Closes", 80.dp, text = { if (it.status == "open") "${(it.closesWeek.toInt() - now).coerceAtLeast(0)} wk" else DASH }, sort = { if (it.status == "open") it.closesWeek.toLong() else Long.MAX_VALUE }, topic = "Time"),
    GridColumn("progress", "Built", 64.dp, text = { "%.0f%%".format(it.progressBps / 100.0) }, sort = { it.progressBps.toLong() }, bar = { it.progressBps / 10_000f }, scale = 100.0, topic = "Works"),
)

internal val TenderViews = listOf(
    ViewState("All", cols("name", "state", "status", "budget", "bids", "winner", "closes"), sort = "closes", descending = false),
    ViewState("Open", cols("name", "state", "budget", "bids", "yours", "closes"), sort = "closes", descending = false, filters = listOf(Filter(FilterOp.Is, "status", "Open"))),
    ViewState("My bids", cols("name", "budget", "yours", "bids", "winner", "closes"), sort = "yours", filters = listOf(Filter(FilterOp.Mine))),
    ViewState("Under way", cols("name", "state", "winner", "budget", "progress"), sort = "progress"),
)

// MARK: Timeline

private class EventRow(val id: String, val title: String, val summary: String, val week: Int, val year: Int, val weekOfYear: Int, val coming: Boolean, val away: Int, val issue: Boolean)

/** Scenario events that have happened and the ones foreshadowed, with the paper that reported them. */
@Composable
fun TimelineScreen(store: GameStore, onPaper: (Int) -> Unit) {
    val now = store.overview?.tick?.toInt() ?: 0
    val view = rememberEngine(store, store.overview?.tick) { h -> h.timeline() to h.paperArchive().map { it.toInt() }.toSet() }
    val t = view?.first
    val issues = view?.second.orEmpty()
    val rows = buildList {
        t?.upcoming.orEmpty().forEach { add(EventRow(it.id, it.title, it.summary, it.week.toInt(), it.year, it.weekOfYear.toInt(), true, it.weeksAway.toInt(), false)) }
        t?.past.orEmpty().forEach { add(EventRow(it.id, it.title, it.summary, it.week.toInt(), it.year, it.weekOfYear.toInt(), false, 0, it.week.toInt() in issues)) }
    }
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = rows.firstOrNull { it.id == pick } ?: rows.firstOrNull { it.coming } ?: rows.firstOrNull()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Timeline", rows, eventColumns(now), key = { it.id },
            views = GridViews("timeline", TimelineViews, scopes = eventScopes(rows)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.title} ${it.summary} ${it.year}" },
            selected = chosen?.id, onSelect = { pick = it.id },
            breakdown = { it.summary },
            initialSort = "when", initialDescending = true,
        )
        DetailPane(chosen?.title ?: "Timeline", Modifier.width(380.dp).fillMaxHeight()) {
            if (t?.mode == "off") {
                Text("Scenario history is off for this career. No dated events will happen.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            if (chosen == null) {
                Text("Nothing announced yet. Big events are flagged a few weeks ahead.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("When", "week ${chosen.weekOfYear}, ${chosen.year}", Palette.primaryText),
                Triple(if (chosen.coming) "Away" else "Ago", "${if (chosen.coming) chosen.away else (now - chosen.week).coerceAtLeast(0)} wk", if (chosen.coming) Palette.gold else Palette.secondary),
            )
            Text(chosen.summary, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
            if (chosen.issue) GridButton("Read that week's paper", false) { onPaper(chosen.week) }
        }
    }
}

private fun eventColumns(now: Int): List<GridColumn<EventRow>> = listOf(
    GridColumn("name", "Event", null, minWidth = 220.dp, numeric = false, text = { it.title }, tone = { if (it.coming) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("status", "Status", 80.dp, numeric = false, text = { if (it.coming) "Coming" else "Happened" }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("year", "Year", 56.dp, text = { "${it.year}" }, sort = { it.year.toLong() }, topic = "Time"),
    GridColumn("when", "When", 100.dp, numeric = false, text = { "week ${it.weekOfYear}, ${it.year}" }, sort = { it.week.toLong() }, topic = "Time"),
    GridColumn("away", "Weeks away", 90.dp, text = { r -> if (r.coming) "${r.away}" else "${(now - r.week).coerceAtLeast(0)} ago" }, sort = { r -> if (r.coming) r.away.toLong() else -(now - r.week).toLong() }, topic = "Time"),
    GridColumn("paper", "Paper", 56.dp, numeric = false, text = { if (it.issue) "Yes" else "" }, tone = { Palette.secondary }, topic = "About", tip = "Whether the game kept the newspaper issue from that week."),
    GridColumn("summary", "What happened", 340.dp, numeric = false, text = { it.summary }, tone = { Palette.secondary }, topic = "About", shown = false),
)

private fun eventScopes(rows: List<EventRow>): List<ScopeOption<EventRow>> =
    rows.map { it.year }.distinct().sorted().map { y -> ScopeOption<EventRow>("year:$y", "$y", "Year") { it.year == y } }

internal val TimelineViews = listOf(
    ViewState("Everything", cols("name", "status", "when", "away", "paper"), sort = "when"),
    ViewState("Coming", cols("name", "when", "away"), sort = "when", descending = false, filters = listOf(Filter(FilterOp.Is, "status", "Coming"))),
    ViewState("Happened", cols("name", "when", "away", "paper"), sort = "when", filters = listOf(Filter(FilterOp.Is, "status", "Happened"))),
)

// MARK: Letters

/** Letters in a grid: decisions pinned at the top, the open letter on its paper beside it. */
@Composable
fun LettersScreen(store: GameStore) {
    val letters = store.letters
    var pick by remember { mutableStateOf<String?>(null) }
    // Decisions first, then unread, then the rest, newest first inside each.
    val chosen = letters.firstOrNull { it.id == pick } ?: letters.sortedByDescending(::letterRank).firstOrNull()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Letters", letters, letterColumns(), key = { it.id },
            views = GridViews("letters", LetterViews, mine = { it.event != null }, scopes = letterScopes(letters)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.sender.name} ${it.sender.organisation} ${it.sender.role} ${it.summary}" },
            selected = chosen?.id, onSelect = { pick = it.id },
            breakdown = { it.summary },
            initialSort = "status",
        )
        DetailPane(chosen?.sender?.name ?: "Letters", Modifier.width(660.dp).fillMaxHeight()) {
            if (chosen == null) Text("Quiet week. Letters arrive when something happens. Decisions are pinned on top and stop time when they are due.", color = Palette.secondary, fontSize = 12.sp)
            else LetterPaper(store, chosen)
        }
    }
}

private fun letterRank(l: LetterDto): Long = (if (l.event != null && l.mandatory) 3_000_000L else if (l.event != null) 2_000_000L else if (!l.read) 1_000_000L else 0L) + l.week.toLong()

private fun letterColumns(): List<GridColumn<LetterDto>> = listOf(
    GridColumn("name", "From", null, minWidth = 170.dp, numeric = false, text = { it.sender.name }, tone = { if (it.read) Palette.secondary else Palette.primaryText }, bold = true),
    GridColumn("org", "Who they are", 160.dp, numeric = false, text = { listOf(it.sender.role, it.sender.organisation).filter { s -> s.isNotEmpty() && s != it.sender.name }.joinToString(", ") }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("status", "Status", 120.dp, numeric = false, text = { if (it.event != null) (if (it.mandatory) "Due ${ResultBuckets.weekLabel(it.deadline.toInt())}" else "Reply") else if (!it.read) "New" else "" },
        sort = ::letterRank, tone = { if (it.event != null && it.mandatory) Palette.warning else if (it.event != null || !it.read) Palette.gold else Palette.secondary }, topic = "About", tip = "Decisions come first, then new letters."),
    GridColumn("when", "When", 84.dp, numeric = false, text = { ResultBuckets.weekLabel(it.week.toInt()) }, sort = { it.week.toLong() }, topic = "Time"),
    GridColumn("subject", "About", 360.dp, numeric = false, text = { it.summary }, tone = { Palette.secondary }, topic = "About"),
)

private fun letterScopes(rows: List<LetterDto>): List<ScopeOption<LetterDto>> =
    rows.map { it.kind }.distinct().sorted().map { k -> ScopeOption<LetterDto>("kind:$k", k.replace('_', ' ').replaceFirstChar { it.uppercase() }, "Kind") { it.kind == k } }

internal val LetterViews = listOf(
    ViewState("Everything", cols("name", "org", "status", "when", "subject"), sort = "status"),
    ViewState("Decisions", cols("name", "status", "when", "subject"), sort = "status", filters = listOf(Filter(FilterOp.Mine))),
    ViewState("New", cols("name", "org", "when", "subject"), sort = "when", filters = listOf(Filter(FilterOp.Is, "status", "New"))),
)
