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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.BoardVoteDto
import uniffi.cw_ffi.CandidateDto
import uniffi.cw_ffi.ExecutivePostDto

/**
 * Company: People in the 2.0 shell. Executives (a grid of posts with hire, dismiss and poach going
 * through the action catalogue, and the people you could hire for the picked post), the board
 * (shareholder resolutions, seats, the chief executive and the mandate in force) and the
 * workforce (wages and the union).
 */

internal val PostViews = listOf(
    ViewState("Executives", cols("name", "who", "level", "effect", "pay", "left"), sort = null),
    ViewState("Cost", cols("name", "who", "pay", "left", "sever")),
)

private fun postColumns(): List<GridColumn<ExecutivePostDto>> = listOf(
    GridColumn("name", "Post", null, numeric = false, text = { it.title }, minWidth = 150.dp, bold = true, topic = "About"),
    GridColumn("who", "Held by", 150.dp, numeric = false, text = { it.holder?.name ?: "Empty" }, tone = { if (it.holder == null) Palette.warning else Palette.primaryText }, topic = "About"),
    GridColumn("level", "Level", 56.dp, text = { it.holder?.level?.toString() ?: DASH }, sort = { (it.holder?.level ?: 0u).toLong() }, topic = "Person"),
    GridColumn("effect", "What they change", 300.dp, numeric = false, text = { it.holder?.effect ?: it.blurb }, tone = { Palette.secondary }, dropOrder = 2, topic = "Person"),
    GridColumn("pay", "Pay a week", 84.dp, text = { it.holder?.let { h -> Format.compact(h.payCents) } ?: DASH }, sort = { it.holder?.payCents ?: 0L }, scale = 100.0, total = true, topic = "Money"),
    GridColumn("left", "Contract left", 84.dp, text = { it.holder?.let { h -> "${h.weeksLeft} wk" } ?: DASH }, sort = { (it.holder?.weeksLeft ?: 0u).toLong() }, topic = "Money"),
    GridColumn("sever", "To dismiss", 84.dp, text = { it.holder?.let { h -> Format.compact(h.severanceCents) } ?: DASH }, sort = { it.holder?.severanceCents ?: 0L }, scale = 100.0, topic = "Money", tip = "Severance you would pay to let them go now."),
)

internal val CandidateViews = listOf(
    ViewState("For hire", cols("name", "level", "effect", "ask", "fee", "payback"), sort = "payback", descending = false),
    ViewState("Best level", cols("name", "level", "background", "ask", "yearly"), sort = "level"),
)

private fun candidateColumns(): List<GridColumn<CandidateDto>> = listOf(
    GridColumn("name", "Name", null, numeric = false, text = { it.name + if (it.named) " *" else "" }, minWidth = 140.dp, bold = true, topic = "About", tip = "A star marks a named person of the cast."),
    GridColumn("level", "Level", 56.dp, text = { "${it.level}" }, sort = { it.level.toLong() }, topic = "Person"),
    GridColumn("background", "Background", 150.dp, numeric = false, text = { it.background }, tone = { Palette.secondary }, dropOrder = 3, topic = "Person"),
    GridColumn("effect", "What they would change", 300.dp, numeric = false, text = { it.effect }, tone = { Palette.secondary }, dropOrder = 2, topic = "Person"),
    GridColumn("ask", "Asks a week", 84.dp, text = { Format.compact(it.askCents) }, sort = { it.askCents }, scale = 100.0, topic = "Money"),
    GridColumn("fee", "Signing fee", 84.dp, text = { Format.compact(it.signingFeeCents) }, sort = { it.signingFeeCents }, scale = 100.0, topic = "Money"),
    GridColumn("yearly", "Worth a year", 90.dp, text = { Format.compact(it.yearlyValueCents) }, sort = { it.yearlyValueCents }, scale = 100.0, topic = "Money", tip = "What the effect is worth to this company over a year."),
    GridColumn("payback", "Pays back", 76.dp, text = { if (it.paybackWeeks == 0u) DASH else "${it.paybackWeeks} wk" }, sort = { if (it.paybackWeeks == 0u) Long.MAX_VALUE else it.paybackWeeks.toLong() },
        tone = { if (it.affordable) Palette.gold else Palette.secondary }, topic = "Money", tip = "Weeks for the effect to earn back the signing fee. A dash means it does not."),
)

internal val VoteViews = listOf(ViewState("Resolutions", cols("name", "yes", "no", "state", "deadline"), sort = "deadline", descending = false))

private fun voteColumns(): List<GridColumn<BoardVoteDto>> = listOf(
    GridColumn("name", "Resolution", null, numeric = false, text = { it.subject }, minWidth = 150.dp, topic = "About"),
    GridColumn("yes", "For", 56.dp, text = { "${it.yes}" }, sort = { it.yes.toLong() }, topic = "Votes"),
    GridColumn("no", "Against", 64.dp, text = { "${it.no}" }, sort = { it.no.toLong() }, topic = "Votes"),
    GridColumn("state", "State", 76.dp, numeric = false, text = { if (!it.resolved) "Open" else if (it.passed) "Passed" else "Failed" }, tone = { if (!it.resolved) Palette.gold else Palette.secondary }, topic = "Votes"),
    GridColumn("deadline", "Closes", 70.dp, text = { "wk ${it.deadline}" }, sort = { it.deadline.toLong() }, topic = "Votes"),
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PeopleScreen(store: GameStore) {
    val firm = store.selectedFirmDto ?: run { Box(Modifier.fillMaxSize().padding(16.dp)) { FoundCompanyPanel(store) }; return }
    val act = LocalActions.current
    val tick = store.overview?.tick
    val execs = rememberEngine(store, firm.handle, tick) { h -> h.executives(firm.handle) }
    val votes = rememberEngine(store, firm.handle, tick) { h -> h.boardVotes(firm.handle) }.orEmpty()
    val labour = rememberEngine(store, firm.handle, tick) { h -> h.labour(firm.handle) }
    val own = rememberEngine(store, firm.handle, tick) { h -> h.ownership(firm.handle) }
    var postPick by remember { mutableStateOf<UInt?>(null) }
    val posts = execs?.posts.orEmpty()
    val post = posts.firstOrNull { it.postCode == postPick } ?: posts.firstOrNull()
    val people = execs?.candidates?.firstOrNull { it.postCode == post?.postCode }?.people.orEmpty()
    val cur = firm.currency
    Column(Modifier.fillMaxSize().padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FigureLine(
            Triple("Executive pay a week", Format.compact(execs?.weeklyPayCents ?: 0, cur), Palette.primaryText),
            Triple("Posts filled", "${posts.count { it.holder != null }} of ${posts.size}", Palette.primaryText),
            Triple("Chief executive", own?.ceo ?: DASH, Palette.primaryText),
            Triple("Employees", labour?.let { Format.count(it.employees) } ?: DASH, Palette.primaryText),
            Triple("Union", labour?.let { unionState(it.state) } ?: DASH, if (labour?.state == "striking") Palette.negative else Palette.primaryText),
        )
        Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Column(Modifier.weight(1.6f).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                DataGrid(
                    "Executives", posts, postColumns(), key = { it.postCode },
                    views = GridViews("people-posts", PostViews),
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(posts.size.coerceAtLeast(3)) + 30.dp),
                    filterText = { "${it.title} ${it.holder?.name.orEmpty()}" },
                    selected = post?.postCode, onSelect = { postPick = it.postCode },
                    breakdown = { p -> p.holder?.let { "${p.title}: ${it.name}, level ${it.level}, ${it.background}. ${it.effect}" } ?: "${p.title} is empty. ${p.blurb}" },
                )
                DataGrid(
                    "People you could hire as ${post?.title ?: "an executive"}", people, candidateColumns(), key = { it.id },
                    views = GridViews("people-hire", CandidateViews),
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    filterText = { "${it.name} ${it.background} ${it.home}" },
                    onOpen = { c -> post?.let { p -> act("HireExecutive", mapOf("post" to "${p.postCode}", "person" to "${c.id}")) } },
                    breakdown = { c -> "${c.name}, ${c.age}, from ${c.home}. ${c.hireNote.ifEmpty { c.background }} Asks ${Format.compact(c.askCents)} a week, ${Format.compact(c.signingFeeCents)} to sign. Press Enter to hire." },
                    initialSort = "payback", initialDescending = false,
                )
            }
            Column(Modifier.weight(1f).fillMaxHeight(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                DetailPane(post?.title ?: "Post", Modifier.fillMaxWidth().height(190.dp)) {
                    val h = post?.holder
                    if (post == null) Text("No posts yet.", color = Palette.secondary, fontSize = 12.sp)
                    else if (h == null) {
                        Text(post.blurb, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
                        Text("The post is empty. Pick someone below and press Enter to hire.", color = Palette.secondary, fontSize = 11.sp)
                    } else {
                        Text("${h.name}, ${h.age}, from ${h.home}. ${h.effect}", color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            GridButton("Dismiss (${Format.compact(h.severanceCents)})", false) { act("DismissExecutive", mapOf("post" to "${post.postCode}")) }
                            GridButton("Poach a rival's", false) { act("PoachExecutive", mapOf("post" to "${post.postCode}")) }
                        }
                    }
                }
                DataGrid(
                    "Board and shareholders", votes, voteColumns(), key = { it.id },
                    views = GridViews("people-board", VoteViews),
                    modifier = Modifier.fillMaxWidth().weight(1f), phoneBelow = 380.dp,
                    filterText = { it.subject },
                    onOpen = { v -> act("CastVote", mapOf("firm" to "${firm.handle}", "vote_id" to "${v.id}")) },
                    breakdown = { v -> "${v.subject}: ${v.yes} for, ${v.no} against, closes in week ${v.deadline}." + if (!v.resolved) " Press Enter to vote." else "" },
                    initialSort = "deadline", initialDescending = false,
                )
                DetailPane("Board and workforce", Modifier.fillMaxWidth().height(250.dp)) {
                    own?.let {
                        Text("Controlled by ${it.controller ?: "nobody"}. Board seats: ${it.boardSeats.joinToString().ifEmpty { "none filled" }}. Next shareholder meeting: week ${it.nextMeeting}.", color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
                    }
                    store.activeMandate?.let { (_, m) -> Text("Board goal in force: ${m.title}. ${m.progressText}", color = Palette.gold, fontSize = 12.sp, lineHeight = 16.sp) }
                    labour?.let { l ->
                        Text(
                            "Workforce: ${Format.count(l.employees)} people at ${Format.compact(l.currentWageCents)} a week; union ${unionState(l.state)}, ${"%.0f".format(l.organisationBps / 100.0)}% organised" +
                                (if (l.demandCents > 0) ", asking ${Format.compact(l.demandCents)} a week" else "") + ".",
                            color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
                        )
                    }
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        GridButton("Call a meeting", false) { act("CallMeeting", emptyMap()) }
                        GridButton("Set wages", false) { act("SetWageOffer", emptyMap()) }
                        GridButton("Training budget", false) { act("SetTrainingBudget", emptyMap()) }
                        if (labour?.state == "demanding") GridButton("Answer the union", true) { act("RespondBargaining", emptyMap()) }
                        if (labour?.state == "striking") GridButton("Offer a settlement", true) { act("ProposeSettlement", emptyMap()) }
                    }
                }
            }
        }
    }
}

private fun unionState(key: String): String = when (key) {
    "idle" -> "quiet"
    "ballot" -> "balloting"
    "demanding" -> "making a demand"
    "striking" -> "on strike"
    else -> key
}
