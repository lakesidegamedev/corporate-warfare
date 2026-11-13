package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.DealDto
import uniffi.cw_ffi.StockDto

/**
 * Rivals: Activity (raids, rival moves, mergers and plant sales as one grid) and Deals and stakes
 * (open deals, the stakes you hold, the stakes held in you, tender offers with their answers).
 */

// MARK: Activity

/** One thing that happened or is on offer: who did it, to whom, where and when. */
internal class ActivityRow(
    val key: String,
    val week: Int,
    val kind: String,
    val who: String,
    val whoFirm: ULong?,
    val whom: String,
    val whomFirm: ULong?,
    val place: String,
    val sector: String,
    val result: String,
    val plants: Int,
    val text: String,
    val mine: Boolean,
)

/** The engine records a rival's move as a sentence; its kind is read from the words. */
internal fun moveKind(text: String): String {
    val t = text.lowercase()
    return when {
        "raid" in t -> "Raid"
        "merg" in t || "takeover" in t || "acquire" in t || "bid for" in t -> "Merger"
        "listed" in t || "sold" in t || "auction" in t || "bought" in t && "plant" in t -> "Plant sale"
        "chief executive" in t || "replaces" in t -> "New chief"
        else -> "Move"
    }
}

@Composable
fun ActivityScreen(store: GameStore, onRival: (ULong) -> Unit) {
    val now = store.overview?.tick?.toInt() ?: 0
    val lots = rememberEngine(store, store.overview?.tick) { it.plantLots() }.orEmpty()
    val deals = rememberEngine(store, store.overview?.tick) { it.deals() }.orEmpty()
    val feed = rememberEngine(store, store.overview?.tick) { it.rivalFeed(80u) }.orEmpty()
    val mineNames = store.firms.filter { it.controlled }.map { it.name }
    val rows = buildList {
        store.recentRaids.forEachIndexed { i, r ->
            val firms = store.stocks
            add(
                ActivityRow(
                    "raid$i-${r.week}", r.week.toInt(), "Raid", r.attacker, firms.firstOrNull { it.name == r.attacker }?.firm, r.defender, firms.firstOrNull { it.name == r.defender }?.firm,
                    r.region, Sectors.name(r.industry), if (r.success) "Took ${r.blocks} ${if (r.blocks == 1u) "plant" else "plants"}" else "Failed", r.blocks.toInt(),
                    "${r.attacker} ${if (r.success) "took ${r.blocks} ${if (r.blocks == 1u) "plant" else "plants"} from" else "failed to take plants from"} ${r.defender} in ${Sectors.name(r.industry)}, ${r.region}.",
                    r.playerAttacker || r.playerDefender,
                ),
            )
        }
        feed.forEachIndexed { i, m ->
            val kind = moveKind(m.text)
            // A raid already in the raid log above is not listed twice.
            val other = store.stocks.filter { it.name != m.firmName && it.name in m.text }.maxByOrNull { it.name.length }
            if (kind != "Raid") add(ActivityRow("move$i-${m.week}", m.week.toInt(), kind, m.firmName, m.firm, other?.name.orEmpty(), other?.firm, "", "", "", 0, m.text, mineNames.any { it in m.text }))
        }
        lots.forEach { l ->
            add(ActivityRow("lot${l.lot}", now, "Plant sale", l.sellerName, null, if (l.bids.isEmpty()) "" else "${l.bids.size} bids", null, l.regionName, "", "On offer, ${l.weeksLeft} weeks left", l.blocks.toInt(),
                "${l.plantName}: ${l.blocks} plants in ${l.regionName} ${if (l.route == "auction") "at auction" else "listed for sale"} by ${l.sellerName}, scrap price ${Format.compact(l.reserveCents)}.", l.yours))
        }
        deals.filter { it.kind == "merger" }.forEach { d ->
            add(ActivityRow("deal${d.handle}", now, "Merger", d.buyer, d.buyerHandle, d.target, d.targetHandle, "", "", dealStateText(d.state), 0,
                "${d.buyer} and ${d.target}: ${dealStateText(d.state).lowercase()}.", store.firms.any { it.controlled && (it.handle == d.buyerHandle || it.handle == d.targetHandle) }))
        }
    }
    // The game's timeframe sets how far back the log reaches; "Ago" in the column list narrows it further.
    val reach = GridPrefs.timeframe.weeks
    val shown = if (reach == null) rows else rows.filter { now - it.week <= reach }
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = shown.firstOrNull { it.key == pick } ?: shown.firstOrNull()
    val act = LocalActions.current
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Rival activity", shown, activityColumns(now), key = { it.key },
            views = GridViews("activity", ActivityViews, mine = { it.mine }, scopes = activityScopes(shown)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.who} ${it.whom} ${it.place} ${it.sector} ${it.kind} ${it.text}" },
            selected = chosen?.key, onSelect = { pick = it.key },
            breakdown = { it.text },
            initialSort = "ago", initialDescending = false,
        )
        DetailPane(chosen?.let { "${it.kind}: ${it.who}" } ?: "Activity", Modifier.width(380.dp).fillMaxHeight()) {
            if (chosen == null) {
                Text("Nothing has happened in this timeframe. Choose a longer one in the top strip.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("When", if (chosen.week == now && chosen.kind != "Raid") "Now" else ResultBuckets.weekLabel(chosen.week), Palette.primaryText),
                Triple("Kind", chosen.kind, Palette.gold),
                Triple("Weeks ago", "${(now - chosen.week).coerceAtLeast(0)}", Palette.primaryText),
            )
            Text(chosen.text, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp)
            chosen.whoFirm?.let { f -> GridButton("Open ${chosen.who}", false) { onRival(f) } }
            chosen.whomFirm?.let { f -> GridButton("Open ${chosen.whom}", false) { onRival(f) } }
            if (chosen.kind == "Plant sale") GridButton("Open plants on offer", false) { act("BidForPlant", emptyMap()) }
            GridButton("Raid a rival's market", false) { act("RaidMarket", emptyMap()) }
        }
    }
}

private fun activityColumns(now: Int): List<GridColumn<ActivityRow>> = listOf(
    GridColumn("who", "Who", null, minWidth = 170.dp, numeric = false, text = { it.who }, tone = { if (it.mine) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("kind", "Kind", 92.dp, numeric = false, text = { it.kind }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("whom", "To whom", 150.dp, numeric = false, text = { it.whom.ifBlank { DASH } }, tone = { Palette.primaryText }, topic = "About"),
    GridColumn("place", "Where", 110.dp, numeric = false, text = { it.place.ifBlank { DASH } }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("sector", "Sector", 100.dp, numeric = false, text = { it.sector.ifBlank { DASH } }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("result", "What came of it", 150.dp, numeric = false, text = { it.result.ifBlank { DASH } }, tone = { if (it.result == "Failed") Palette.negative else Palette.primaryText }, topic = "About"),
    GridColumn("plants", "Plants", 56.dp, text = { if (it.plants > 0) "${it.plants}" else "" }, sort = { it.plants.toLong() }, total = true, topic = "About"),
    GridColumn("when", "When", 84.dp, numeric = false, text = { ResultBuckets.weekLabel(it.week) }, sort = { it.week.toLong() }, topic = "Time"),
    GridColumn("ago", "Weeks ago", 80.dp, text = { "${(now - it.week).coerceAtLeast(0)}" }, sort = { (now - it.week).coerceAtLeast(0).toLong() }, topic = "Time", tip = "Weeks since it happened. Plants on offer and open deals count as now."),
    GridColumn("text", "What happened", 300.dp, numeric = false, text = { it.text }, tone = { Palette.secondary }, topic = "About", shown = false),
)

private fun activityScopes(rows: List<ActivityRow>): List<ScopeOption<ActivityRow>> =
    rows.map { it.kind }.distinct().sorted().map { k -> ScopeOption<ActivityRow>("kind:$k", k, "Kind") { it.kind == k } }

internal val ActivityViews = listOf(
    ViewState("Everything", cols("who", "kind", "whom", "place", "sector", "result", "when"), sort = "when"),
    ViewState("Raids", cols("who", "whom", "place", "sector", "result", "plants", "when"), sort = "when", filters = listOf(Filter(FilterOp.Is, "kind", "Raid"))),
    ViewState("Mergers and sales", cols("who", "kind", "whom", "place", "result", "plants", "when"), sort = "when", filters = listOf(Filter(FilterOp.Is, "kind", "Merger"))),
    ViewState("Involving me", cols("who", "kind", "whom", "place", "sector", "result", "when"), sort = "when", filters = listOf(Filter(FilterOp.Mine))),
    ViewState("Rival moves", cols("who", "kind", "text", "when"), sort = "when", filters = listOf(Filter(FilterOp.Is, "kind", "Move"))),
)

// MARK: Deals and stakes

internal fun dealStateText(state: String): String = when (state) {
    "draft" -> "Draft"
    "funded" -> "Funded"
    "diligence" -> "Looking at the books"
    "offer_open" -> "Offer open"
    "review" -> "Under review"
    "settled" -> "Done"
    "withdrawn" -> "Withdrawn"
    "expired" -> "Lapsed"
    "rejected" -> "Rejected"
    else -> state.replace('_', ' ').replaceFirstChar { it.uppercase() }
}

private fun dealKindText(kind: String) = when (kind) {
    "share_purchase" -> "Share offer"
    "asset_purchase" -> "Asset purchase"
    "merger" -> "Merger"
    else -> kind.replace('_', ' ').replaceFirstChar { it.uppercase() }
}

private fun terminal(state: String) = state in setOf("settled", "withdrawn", "expired", "rejected")

/** A deal, or a stake held by you or in you, as one row. */
internal class DealRow(
    val key: String,
    val type: String,
    val with: String,
    val withFirm: ULong?,
    val kind: String,
    val state: String,
    val shares: Long,
    val stakeBps: Long,
    val priceCents: Long,
    val valueCents: Long,
    val escrowCents: Long,
    val deadline: Int?,
    val changeQuarterBps: Long,
    val mine: Boolean,
    val deal: DealDto?,
    val stock: StockDto?,
)

@Composable
fun DealsScreen(store: GameStore, onRival: (ULong) -> Unit) {
    val now = store.overview?.tick?.toInt() ?: 0
    val deals = rememberEngine(store, store.overview?.tick) { it.deals() }.orEmpty()
    val mineFirms = store.firms.filter { it.controlled }.map { it.handle }.toSet()
    val rows = buildList {
        deals.forEach { d ->
            val mine = d.buyerHandle in mineFirms || d.targetHandle in mineFirms
            val stock = store.stocks.firstOrNull { it.firm == d.targetHandle }
            add(
                DealRow("deal${d.handle}", if (terminal(d.state)) "Finished deal" else "Deal", if (d.buyerHandle in mineFirms) d.target else d.buyer, if (d.buyerHandle in mineFirms) d.targetHandle else d.buyerHandle, dealKindText(d.kind), d.state,
                    d.committedShares, stock?.sharesIssued?.takeIf { it > 0 }?.let { d.committedShares * 10_000 / it } ?: 0, d.offerPriceCents, d.committedShares * d.offerPriceCents, d.escrowCents,
                    d.deadline?.toInt(), stock?.changeQuarterBps ?: 0, mine, d, stock),
            )
        }
        store.stocks.filter { it.yourShares > 0 && !it.controlledByYou }.forEach { s ->
            add(DealRow("held${s.firm}", "You hold", s.name, s.firm, "Shares", "held", s.yourShares, if (s.sharesIssued > 0) s.yourShares * 10_000 / s.sharesIssued else 0, s.priceCents, s.yourShares * s.priceCents, 0, null, s.changeQuarterBps, true, null, s))
        }
        store.ownership?.holders.orEmpty().filter { it.quantity > 0 }.forEach { h ->
            add(DealRow("in${h.holder}", "Holds you", h.holder, store.stocks.firstOrNull { it.name == h.holder }?.firm, "Shares", "held", h.quantity, h.votesBps.toLong(), 0, h.costCents, 0, null, 0, false, null, null))
        }
    }
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = rows.firstOrNull { it.key == pick } ?: rows.firstOrNull { it.deal != null && !terminal(it.state) } ?: rows.firstOrNull()
    val act = LocalActions.current
    var trading by remember { mutableStateOf<StockDto?>(null) }
    trading?.let { StockDialog(store, it, showPage = false) { trading = null } }
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Deals and stakes", rows, dealColumns(now), key = { it.key },
            views = GridViews("deals", DealViews, mine = { it.mine }, scopes = dealScopes()),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.with} ${it.type} ${it.kind} ${dealStateText(it.state)}" },
            selected = chosen?.key, onSelect = { pick = it.key },
            breakdown = { r -> "${r.with}: ${r.type.lowercase()}, ${r.kind.lowercase()}" + (if (r.shares > 0) ", ${Format.count(r.shares)} shares" else "") + (if (r.state != "held") ", ${dealStateText(r.state).lowercase()}" else "") },
            initialSort = "value",
        )
        DetailPane(chosen?.with ?: "Deals and stakes", Modifier.width(380.dp).fillMaxHeight()) {
            GridButton("Make a takeover approach", false) { act("StartDiligence", emptyMap()) }
            if (chosen == null) {
                Text("You have no deals and hold no shares in other companies. Buy shares from the League table, or make an approach.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("Shares", if (chosen.shares > 0) Format.count(chosen.shares) else DASH, Palette.primaryText),
                Triple("Stake", if (chosen.stakeBps > 0) "%.1f%%".format(chosen.stakeBps / 100.0) else DASH, Palette.gold),
                Triple("Price", if (chosen.priceCents > 0) Format.price(chosen.priceCents) else DASH, Palette.primaryText),
            )
            chosen.withFirm?.let { f -> GridButton("Open ${chosen.with}", false) { onRival(f) } }
            chosen.stock?.let { s ->
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    GoldButton("Buy shares") { trading = s }
                    if (s.yourShares > 0) GridButton("Sell", false) { trading = s }
                }
            }
            val d = chosen.deal
            if (d != null) {
                Text(
                    "${dealKindText(d.kind)}: ${d.buyer} for ${d.target}. ${dealStateText(d.state)}" +
                        (d.deadline?.let { ", closes ${ResultBuckets.weekLabel(it.toInt())}" } ?: "") + ". " +
                        (if (d.offerPriceCents > 0) "Offer ${Format.price(d.offerPriceCents)} a share, ${Format.count(d.committedShares)} shares committed, ${Format.compact(d.escrowCents)} in escrow." else ""),
                    color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
                )
                val handle = "${d.handle}"
                val iBuy = d.buyerHandle in mineFirms
                if (!terminal(d.state)) {
                    if (iBuy) {
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            if (d.state in setOf("draft", "funded", "diligence")) GoldButton("Fund the offer") { act("FundTender", mapOf("deal" to handle)) }
                            if (d.kind == "merger" && d.state != "review") GridButton("Put to a vote", false) { act("ProposeMerger", mapOf("deal" to handle)) }
                            if (d.state == "review") GoldButton("Offer a remedy") { act("SubmitRemedy", mapOf("deal" to handle)) }
                            GridButton("Withdraw", false) { act("WithdrawOffer", mapOf("deal" to handle)) }
                        }
                    } else if (d.state == "offer_open" && (chosen.stock?.yourShares ?: 0) > 0) {
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            GoldButton("Tender my shares") { act("RespondTender", mapOf("deal" to handle, "accept" to "1")) }
                            GridButton("Decline", false) { act("RespondTender", mapOf("deal" to handle, "accept" to "0")) }
                        }
                        Text("Tendering sells your shares into the offer at its price. Declining keeps them.", color = Palette.secondary, fontSize = 10.sp)
                    }
                }
            } else if (chosen.type == "Holds you") {
                Text("${chosen.with} holds ${Format.count(chosen.shares)} of your shares, ${"%.1f%%".format(chosen.stakeBps / 100.0)} of the votes.", color = Palette.primaryText, fontSize = 12.sp)
            }
        }
    }
}

private fun dealColumns(now: Int): List<GridColumn<DealRow>> = listOf(
    GridColumn("name", "Company", null, minWidth = 170.dp, numeric = false, text = { it.with }, tone = { if (it.mine) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("type", "What", 84.dp, numeric = false, text = { it.type }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("kind", "Kind", 100.dp, numeric = false, text = { it.kind }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("state", "Where it stands", 130.dp, numeric = false, text = { if (it.state == "held") DASH else dealStateText(it.state) }, tone = { if (terminal(it.state)) Palette.secondary else Palette.primaryText }, topic = "About"),
    GridColumn("shares", "Shares", 84.dp, text = { if (it.shares > 0) Format.count(it.shares) else DASH }, sort = { it.shares }, topic = "Stake"),
    GridColumn("stake", "Stake", 64.dp, text = { if (it.stakeBps > 0) "%.1f%%".format(it.stakeBps / 100.0) else DASH }, sort = { it.stakeBps }, bar = { it.stakeBps / 10_000f }, scale = 100.0, topic = "Stake", tip = "The share of the company (or of its votes, for a stake in you)."),
    GridColumn("price", "Price", 74.dp, text = { if (it.priceCents > 0) Format.price(it.priceCents) else DASH }, sort = { it.priceCents }, scale = 100.0, topic = "Money", tip = "The share price, or the offer per share for a deal."),
    GridColumn("value", "Value", 84.dp, text = { if (it.valueCents > 0) Format.compact(it.valueCents) else DASH }, sort = { it.valueCents }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Money"),
    GridColumn("escrow", "In escrow", 84.dp, text = { if (it.escrowCents > 0) Format.compact(it.escrowCents) else "" }, sort = { it.escrowCents }, scale = 100.0, topic = "Money"),
    GridColumn("quarter", "Quarter", 66.dp, text = { if (it.stock != null) changeText(it.changeQuarterBps) else DASH }, sort = { it.changeQuarterBps }, tone = { changeTint(it.changeQuarterBps) }, scale = 100.0, topic = "Money"),
    GridColumn("left", "Weeks left", 80.dp, text = { r -> r.deadline?.let { "${(it - now).coerceAtLeast(0)}" } ?: "" }, sort = { r -> r.deadline?.let { (it - now).coerceAtLeast(0).toLong() } ?: Long.MAX_VALUE }, topic = "Time"),
)

private fun dealScopes(): List<ScopeOption<DealRow>> =
    listOf("Deal", "Finished deal", "You hold", "Holds you").map { t -> ScopeOption<DealRow>("type:$t", t, "What") { it.type == t } }

internal val DealViews = listOf(
    ViewState("Open deals", cols("name", "kind", "state", "price", "shares", "escrow", "left"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Is, "type", "Deal"))),
    ViewState("Stakes I hold", cols("name", "shares", "stake", "price", "value", "quarter"), sort = "value", filters = listOf(Filter(FilterOp.Is, "type", "You hold"))),
    ViewState("Stakes in me", cols("name", "shares", "stake", "value"), sort = "stake", filters = listOf(Filter(FilterOp.Is, "type", "Holds you"))),
    ViewState("Tender offers", cols("name", "state", "price", "shares", "stake", "escrow", "left"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Is, "kind", "Share offer"))),
    ViewState("Finished deals", cols("name", "kind", "state", "price", "shares"), sort = "name", descending = false, filters = listOf(Filter(FilterOp.Is, "type", "Finished deal"))),
)
