package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.CapacityOpeningDto
import uniffi.cw_ffi.PlantLotDto
import uniffi.cw_ffi.RaidCampaignDto

/**
 * The three 2.0 screens the engine now has data for: Openings (where a new plant would pay),
 * Auctions and listings (plants on offer) and Raids (campaigns announced and not yet decided).
 * Each is a [DataGrid] with the same control bar and its own built-in views; every action goes
 * through the action catalogue with the row's fields filled in. Also the Ctrl K finder.
 */

/** Opens an action of the catalogue by its command, with some of its fields filled in. */
val LocalActions = staticCompositionLocalOf<(String, Map<String, String>) -> Unit> { { _, _ -> } }

/** Every screen that keeps views, for the finder: its settings name, its title, its built-in views and where it lives. */
internal val ScreenViews: List<Triple<String, String, Pair<List<ViewState>, Place>>> get() = listOf(
    Triple("goods", "Goods", GoodsViews to Place.Goods),
    Triple("openings", "Openings", OpeningViews to Place.Openings),
    Triple("auctions", "Auctions", AuctionViews to Place.Auctions),
    Triple("league", "League table", LeagueViews to Place.League),
    Triple("raids", "Raids", RaidViews to Place.Raids),
    Triple("divisions", "Divisions", DivisionViews to Place.Overview),
    Triple("lines", "Product lines", LineViews to Place.Overview),
    Triple("results", "Results", ResultViews to Place.Overview),
)

// MARK: Openings

private fun goodName(store: GameStore, id: String): String =
    store.commodities.firstOrNull { it.productId == id }?.name ?: id.replace('_', ' ').replaceFirstChar { it.uppercase() }

private fun regionName(store: GameStore, index: UInt): String = store.boards.firstOrNull { it.index == index }?.name ?: "Region ${index.toInt() + 1}"

/** Where a new plant would pay: the engine's capacity openings, best first. */
@Composable
fun OpeningsScreen(store: GameStore) {
    val rows = rememberEngine(store, store.overview?.tick) { it.capacityOpenings(200u) }.orEmpty()
    val act = LocalActions.current
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = rows.firstOrNull { openingKey(it) == pick } ?: rows.firstOrNull()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Openings", rows, openingColumns(store), key = ::openingKey,
            // "Mine" is where you already run a division.
            views = GridViews("openings", OpeningViews, mine = { o -> store.divisions.any { it.region == o.regionCode } }, scopes = openingScopes(store, rows)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${goodName(store, it.productId)} ${Sectors.name(it.industry)} ${regionName(store, it.region)}" },
            selected = chosen?.let(::openingKey), onSelect = { pick = openingKey(it) },
            breakdown = { o ->
                "${goodName(store, o.productId)} in ${regionName(store, o.region)}: ${Format.count(o.roomLots)} lots a week short, a plant makes ${Format.count(o.blockLots)}; " +
                    "price ${Format.money(o.priceCents)}, contribution ${Format.compact(o.weeklyContributionCents)} a week, build ${Format.compact(o.buildCostCents)}" +
                    (o.paybackWeeks?.let { ", pays back in $it weeks" } ?: ", never pays back at today's prices")
            },
            initialSort = "payback", initialDescending = false,
        )
        DetailPane(chosen?.let { "${goodName(store, it.productId)} · ${regionName(store, it.region)}" } ?: "Opening", Modifier.width(380.dp).fillMaxHeight()) {
            if (chosen == null) {
                Text("No market is short of a good a new plant could make. Openings appear when demand outruns what is built and on its way.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("Short a week", Format.count(chosen.roomLots), Palette.primaryText),
                Triple("One plant", Format.count(chosen.blockLots), Palette.primaryText),
                Triple("Payback", chosen.paybackWeeks?.let { "$it wk" } ?: DASH, Palette.gold),
            )
            Text(
                "Households here go without ${Format.count(chosen.unmetLots)} of the ${Format.count(chosen.wantedLots)} lots they ask for each week. " +
                    "A lot fetches ${Format.money(chosen.priceCents)}, ${changeText(chosen.priceVsReferenceBps)} against its normal price. " +
                    if (chosen.traded) "It trades on the corridor exchange." else "It sells where it is made.",
                color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
            )
            val recipe = store.strategyCatalogue.firstOrNull { it.id == chosen.recipeId }
            GoldButton("Build here") {
                act("BuildFacility", buildMap {
                    put("region", "${chosen.region}")
                    recipe?.let { put("recipe", "${it.index}") }
                })
            }
            Text("Opens the build form with this region and strategy filled in; you choose how many plants.", color = Palette.secondary, fontSize = 10.sp)
        }
    }
}

private fun openingKey(o: CapacityOpeningDto) = o.recipeId + "@" + o.region

private fun openingColumns(store: GameStore): List<GridColumn<CapacityOpeningDto>> = listOf(
    GridColumn("name", "Good", null, minWidth = 170.dp, numeric = false, text = { goodName(store, it.productId) }, bold = true),
    GridColumn("sector", "Sector", 110.dp, numeric = false, text = { Sectors.name(it.industry) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("region", "Region", 120.dp, numeric = false, text = { regionName(store, it.region) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("room", "Short", 70.dp, text = { Format.count(it.roomLots) }, sort = { it.roomLots }, total = true, topic = "Market", tip = "Lots a week the market is short of, after the plants already on their way."),
    GridColumn("block", "One plant", 76.dp, text = { Format.count(it.blockLots) }, sort = { it.blockLots }, topic = "Market", tip = "Lots a week one new plant makes."),
    GridColumn("unmet", "Going without", 96.dp, text = { Format.count(it.unmetLots) }, sort = { it.unmetLots }, total = true, topic = "Market", tip = "Lots households asked for and did not get last week."),
    GridColumn("price", "Price", 80.dp, text = { Format.money(it.priceCents) }, sort = { it.priceCents }, scale = 100.0, topic = "Money"),
    GridColumn("vsnormal", "Vs normal", 76.dp, text = { changeText(it.priceVsReferenceBps) }, sort = { it.priceVsReferenceBps }, tone = { changeTint(it.priceVsReferenceBps) }, scale = 100.0, topic = "Money"),
    GridColumn("contrib", "A week", 84.dp, text = { Format.compact(it.weeklyContributionCents) }, sort = { it.weeklyContributionCents }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Money",
        tip = "What one plant would earn over its running costs each week at today's prices."),
    GridColumn("build", "Build cost", 84.dp, text = { Format.compact(it.buildCostCents) }, sort = { it.buildCostCents }, fmt = { Format.compact(it) }, scale = 100.0, topic = "Money"),
    GridColumn("payback", "Payback", 70.dp, text = { o -> o.paybackWeeks?.let { "$it wk" } ?: DASH }, sort = { o -> o.paybackWeeks ?: Long.MAX_VALUE }, tone = { o -> if ((o.paybackWeeks ?: Long.MAX_VALUE) <= 104) Palette.gold else Palette.primaryText },
        topic = "Money", tip = "Weeks of contribution to pay for the build. A dash means it never pays back at today's prices."),
    GridColumn("traded", "Sold", 76.dp, numeric = false, text = { if (it.traded) "Exchange" else "Locally" }, tone = { Palette.secondary }, topic = "About"),
)

private fun openingScopes(store: GameStore, rows: List<CapacityOpeningDto>): List<ScopeOption<CapacityOpeningDto>> =
    rows.map { it.industry }.distinct().sortedBy { Sectors.order.indexOf(it) }.map { ind -> ScopeOption<CapacityOpeningDto>("sector:$ind", Sectors.name(ind), "Sector", Sectors.color(ind)) { it.industry == ind } } +
        rows.map { it.regionCode to it.region }.distinct().sortedBy { regionName(store, it.second) }.map { (code, idx) -> ScopeOption<CapacityOpeningDto>("region:$code", regionName(store, idx), "Region") { it.regionCode == code } }

internal val OpeningViews = listOf(
    ViewState("Fastest payback", cols("name", "sector", "region", "payback", "contrib", "build", "room"), sort = "payback", descending = false),
    ViewState("By sector", cols("name", "region", "room", "unmet", "price", "contrib", "payback"), sort = "contrib", groupBy = "sector"),
    ViewState("By region", cols("name", "sector", "room", "unmet", "price", "contrib", "payback"), sort = "contrib", groupBy = "region"),
    ViewState("Shortages", cols("name", "sector", "region", "room", "unmet", "block", "vsnormal"), sort = "unmet"),
)

// MARK: Auctions and listings

/** Plants on offer: from failed companies' liquidators and from owners who listed them, yours among them. */
@Composable
fun AuctionsScreen(store: GameStore) {
    val rows = rememberEngine(store, store.overview?.tick) { it.plantLots() }.orEmpty()
    val act = LocalActions.current
    var pick by remember { mutableStateOf<UInt?>(null) }
    val chosen = rows.firstOrNull { it.lot == pick } ?: rows.firstOrNull()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Plants on offer", rows, auctionColumns(), key = { it.lot },
            views = GridViews("auctions", AuctionViews, mine = { it.yours }, scopes = auctionScopes(rows)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.plantName} ${it.sellerName} ${it.regionName}" },
            selected = chosen?.lot, onSelect = { pick = it.lot },
            breakdown = { l ->
                "${l.plantName}, ${l.blocks} plants in ${l.regionName}, from ${l.sellerName}: book ${Format.compact(l.bookCents)}, scrap ${Format.compact(l.reserveCents)}, " +
                    "earned ${Format.compact(l.weeklyCents)} a week, ${l.bids.size} bids, ${l.weeksLeft} weeks left"
            },
            initialSort = "left", initialDescending = false,
        )
        DetailPane(chosen?.plantName ?: "Plant on offer", Modifier.width(380.dp).fillMaxHeight()) {
            GridButton("List one of your sites for sale", false) { act("ListPlant", emptyMap()) }
            if (chosen == null) {
                Text("No plant is on offer. A failed company's plants go to auction for four weeks, and owners can list a site; both show here.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("Plants", "${chosen.blocks}", Palette.primaryText),
                Triple("Book", Format.compact(chosen.bookCents), Palette.primaryText),
                Triple("Scrap", Format.compact(chosen.reserveCents), Palette.primaryText),
                Triple("Weeks left", "${chosen.weeksLeft}", Palette.gold),
            )
            Text(
                "${if (chosen.route == "auction") "Auctioned by the liquidator of" else "Listed by"} ${chosen.sellerName}. " +
                    "It earned ${Format.compact(chosen.weeklyCents)} a week running ${chosen.blocksUsed} of its plants; building the same today costs ${Format.compact(chosen.replacementCents)}.",
                color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
            )
            if (chosen.bids.isNotEmpty()) {
                DataGrid(
                    "Bids", chosen.bids, listOf(
                        GridColumn("who", "Bidder", null, numeric = false, text = { it.bidderName }, bold = true),
                        GridColumn("price", "Bid", 90.dp, text = { Format.compact(it.priceCents) }, sort = { it.priceCents }),
                        GridColumn("weekly", "Their week", 90.dp, text = { Format.compact(it.weeklyCents) }, sort = { it.weeklyCents }),
                    ), key = { it.bidder }, modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(chosen.bids.size)), initialSort = "price",
                )
            }
            if (chosen.yours) {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    GoldButton("Accept the best bid", enabled = chosen.bids.isNotEmpty()) { act("AcceptPlantBid", mapOf("lot" to "${chosen.lot}")) }
                    GridButton("Withdraw", false) { act("WithdrawListing", mapOf("lot" to "${chosen.lot}")) }
                }
            } else {
                GoldButton(if (chosen.playerBidCents > 0) "Change your bid" else "Bid") {
                    act("BidForPlant", buildMap {
                        put("lot", "${chosen.lot}")
                        put("price_cents", "${maxOf(chosen.playerBidCents, chosen.reserveCents)}")
                    })
                }
                if (chosen.playerBidCents > 0) Text("Your bid: ${Format.compact(chosen.playerBidCents)}. A bid of zero withdraws it.", color = Palette.secondary, fontSize = 11.sp)
            }
        }
    }
}

private fun auctionColumns(): List<GridColumn<PlantLotDto>> = listOf(
    GridColumn("name", "Plant", null, minWidth = 170.dp, numeric = false, text = { it.plantName + if (it.yours) " (yours)" else "" }, tone = { if (it.yours) Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("seller", "Seller", 140.dp, numeric = false, text = { it.sellerName }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("route", "Kind", 80.dp, numeric = false, text = { if (it.route == "auction") "Auction" else "Listing" }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("region", "Region", 110.dp, numeric = false, text = { it.regionName }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("blocks", "Plants", 56.dp, text = { "${it.blocks}" }, sort = { it.blocks.toLong() }, total = true, topic = "Plant"),
    GridColumn("book", "Book", 80.dp, text = { Format.compact(it.bookCents) }, sort = { it.bookCents }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Money"),
    GridColumn("reserve", "Scrap", 80.dp, text = { Format.compact(it.reserveCents) }, sort = { it.reserveCents }, fmt = { Format.compact(it) }, scale = 100.0, topic = "Money", tip = "The scrap price: the least an auction sells it for."),
    GridColumn("weekly", "Earned, week", 92.dp, text = { Format.compact(it.weeklyCents) }, sort = { it.weeklyCents }, tone = { if (it.weeklyCents >= 0) Palette.primaryText else Palette.negative }, fmt = { Format.compact(it) }, scale = 100.0, topic = "Money"),
    GridColumn("best", "Best bid", 80.dp, text = { l -> l.bids.maxOfOrNull { it.priceCents }?.let { Format.compact(it) } ?: DASH }, sort = { l -> l.bids.maxOfOrNull { it.priceCents } ?: Long.MIN_VALUE }, scale = 100.0, topic = "Bids"),
    GridColumn("bids", "Bids", 50.dp, text = { "${it.bids.size}" }, sort = { it.bids.size.toLong() }, topic = "Bids"),
    GridColumn("yourbid", "Your bid", 80.dp, text = { if (it.playerBidCents > 0) Format.compact(it.playerBidCents) else "" }, sort = { it.playerBidCents }, tone = { Palette.gold }, scale = 100.0, topic = "Bids"),
    GridColumn("left", "Weeks left", 76.dp, text = { "${it.weeksLeft}" }, sort = { it.weeksLeft.toLong() }, tone = { if (it.weeksLeft <= 1u) Palette.warning else Palette.primaryText }, topic = "Time"),
)

private fun auctionScopes(rows: List<PlantLotDto>): List<ScopeOption<PlantLotDto>> =
    rows.map { it.regionName }.distinct().sorted().map { r -> ScopeOption<PlantLotDto>("region:$r", r, "Region") { it.regionName == r } }

internal val AuctionViews = listOf(
    ViewState("On offer", cols("name", "route", "region", "blocks", "weekly", "reserve", "best", "yourbid", "left"), sort = "left", descending = false),
    ViewState("Closing soon", cols("name", "seller", "blocks", "best", "bids", "left"), sort = "left", descending = false),
    ViewState("Best earners", cols("name", "region", "blocks", "weekly", "book", "reserve", "best"), sort = "weekly"),
    ViewState("My listings", cols("name", "region", "blocks", "bids", "best", "left"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Mine))),
)

// MARK: Raids

/** Raid campaigns announced and not yet decided: against you, by you and between rivals. */
@Composable
fun RaidsScreen(store: GameStore, onRival: (ULong) -> Unit) {
    val rows = rememberEngine(store, store.overview?.tick) { it.raidCampaigns() }.orEmpty()
    val act = LocalActions.current
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = rows.firstOrNull { raidKey(it) == pick } ?: rows.firstOrNull()
    Row(Modifier.fillMaxSize().padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        DataGrid(
            "Raid campaigns", rows, raidColumns(), key = ::raidKey,
            views = GridViews("raids", RaidViews, mine = { it.role != "between_rivals" }, scopes = raidScopes(rows)),
            modifier = Modifier.weight(1f).fillMaxHeight(),
            filterText = { "${it.attackerName} ${it.defenderName} ${it.regionName} ${it.industryName}" },
            selected = chosen?.let(::raidKey), onSelect = { pick = raidKey(it) },
            breakdown = { c ->
                "${c.attackerName} against ${c.defenderName}, ${c.industryName} in ${c.regionName}: budget ${Format.compact(c.budgetCents)}, attack ${Format.compact(c.attackCents)} against defence ${Format.compact(c.defenceCents)}, " +
                    "chance ${"%.1f%%".format(c.chanceBps / 100.0)}, ${c.blocksAtStake} of ${c.divisionBlocks} plants at stake, decided in ${c.weeksLeft} weeks"
            },
            initialSort = "left", initialDescending = false,
        )
        DetailPane(chosen?.let { "${it.attackerName} vs ${it.defenderName}" } ?: "Raid", Modifier.width(380.dp).fillMaxHeight()) {
            GridButton("Raid a rival's market", false) { act("RaidMarket", emptyMap()) }
            if (chosen == null) {
                Text("No raid is under way. A rival must announce a raid four weeks before it is decided; any against you show here first.", color = Palette.secondary, fontSize = 12.sp)
                return@DetailPane
            }
            FigureLine(
                Triple("Chance", "%.1f%%".format(chosen.chanceBps / 100.0), if (chosen.role == "against_player") Palette.negative else Palette.gold),
                Triple("At stake", "${chosen.blocksAtStake} of ${chosen.divisionBlocks}", Palette.primaryText),
                Triple("Decided in", "${chosen.weeksLeft} wk", Palette.primaryText),
            )
            Text(
                "Attack ${Format.compact(chosen.attackCents)} against defence ${Format.compact(chosen.defenceCents)}. The raid is decided against the defence the defender has then, so marketing spent before ${ResultBuckets.weekLabel(chosen.resolvesWeek.toInt())} counts.",
                color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
            )
            when (chosen.role) {
                "against_player" -> {
                    GoldButton("Defend: raise marketing") { act("SetMarketingBudget", mapOf("firm" to "${chosen.defender}")) }
                    Text("Defence is the marketing the business has. Raising the weekly budget before the raid is decided lowers the raider's chance.", color = Palette.secondary, fontSize = 10.sp)
                }
                "by_player" -> Text("Your campaign. The budget is spent whatever happens.", color = Palette.secondary, fontSize = 11.sp)
                else -> GridButton("Open ${chosen.defenderName}", false) { onRival(chosen.defender) }
            }
        }
    }
}

private fun raidKey(c: RaidCampaignDto) = "${c.attacker}>${c.defender}@${c.region}/${c.industry}"

private fun roleLabel(role: String) = when (role) {
    "against_player" -> "Against you"
    "by_player" -> "By you"
    else -> "Between rivals"
}

private fun raidColumns(): List<GridColumn<RaidCampaignDto>> = listOf(
    GridColumn("name", "Raider", null, minWidth = 150.dp, numeric = false, text = { it.attackerName }, tone = { if (it.role == "by_player") Palette.gold else Palette.primaryText }, bold = true),
    GridColumn("role", "Who", 110.dp, numeric = false, text = { roleLabel(it.role) }, tone = { if (it.role == "against_player") Palette.negative else Palette.secondary }, topic = "About"),
    GridColumn("target", "Target", 140.dp, numeric = false, text = { it.defenderName }, tone = { if (it.role == "against_player") Palette.gold else Palette.primaryText }, topic = "About"),
    GridColumn("sector", "Sector", 100.dp, numeric = false, text = { it.industryName }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("region", "Region", 100.dp, numeric = false, text = { it.regionName }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("budget", "Budget", 80.dp, text = { Format.compact(it.budgetCents) }, sort = { it.budgetCents }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Strength"),
    GridColumn("attack", "Attack", 80.dp, text = { Format.compact(it.attackCents) }, sort = { it.attackCents }, scale = 100.0, topic = "Strength"),
    GridColumn("defence", "Defence", 80.dp, text = { Format.compact(it.defenceCents) }, sort = { it.defenceCents }, scale = 100.0, topic = "Strength"),
    GridColumn("chance", "Chance", 70.dp, text = { "%.1f%%".format(it.chanceBps / 100.0) }, sort = { it.chanceBps.toLong() }, bar = { it.chanceBps / 10_000f }, scale = 100.0, topic = "Strength"),
    GridColumn("stake", "At stake", 70.dp, text = { "${it.blocksAtStake}/${it.divisionBlocks}" }, sort = { it.blocksAtStake.toLong() }, total = true, topic = "Strength", tip = "Plants the raid would take, of the plants the division has."),
    GridColumn("left", "Weeks left", 76.dp, text = { "${it.weeksLeft}" }, sort = { it.weeksLeft.toLong() }, tone = { if (it.weeksLeft <= 1u) Palette.warning else Palette.primaryText }, topic = "Time"),
)

private fun raidScopes(rows: List<RaidCampaignDto>): List<ScopeOption<RaidCampaignDto>> =
    rows.map { it.regionName }.distinct().sorted().map { r -> ScopeOption<RaidCampaignDto>("region:$r", r, "Region") { it.regionName == r } }

internal val RaidViews = listOf(
    ViewState("Against you", cols("name", "target", "sector", "region", "chance", "stake", "defence", "left"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Is, "role", "Against you"))),
    ViewState("By you", cols("name", "target", "sector", "region", "budget", "chance", "stake", "left"), sort = "left", descending = false, filters = listOf(Filter(FilterOp.Is, "role", "By you"))),
    ViewState("Between rivals", cols("name", "target", "sector", "region", "budget", "chance", "stake", "left"), sort = "chance", filters = listOf(Filter(FilterOp.Is, "role", "Between rivals"))),
    ViewState("All", cols("name", "role", "target", "sector", "budget", "chance", "stake", "left"), sort = "left", descending = false),
)

// MARK: Find (Ctrl K)

/**
 * Ctrl K: one search over places, goods, companies, divisions, saved views and actions. Enter
 * opens the first result; "All actions" opens the full action list.
 */
@Composable
fun FindPalette(
    store: GameStore, start: String, onPlace: (Place) -> Unit, onGood: (UInt) -> Unit, onRival: (ULong) -> Unit,
    onAction: (String) -> Unit, onAllActions: () -> Unit, onClose: () -> Unit,
) {
    var q by remember { mutableStateOf(start) }
    val prefs: ViewPrefs = LocalAppSettings.current ?: MapViewPrefs()
    val words = q.trim().lowercase().split(' ').filter { it.isNotBlank() }
    fun hit(vararg text: String) = words.isEmpty() || words.all { w -> text.any { it.lowercase().contains(w) } }
    data class Hit(val kind: String, val text: String, val hint: String, val go: () -> Unit)
    val hits = buildList {
        Place.entries.filter { hit(it.title, it.group) }.forEach { p -> add(Hit("Place", p.title, p.group + if (p.key.isNotEmpty()) "  ${p.key}" else "") { onPlace(p) }) }
        if (words.isNotEmpty()) {
            store.commodities.filter { hit(it.name, Sectors.name(it.sector)) }.take(8).forEach { g -> add(Hit("Good", g.name, Format.money(g.priceCents)) { onGood(g.product) }) }
            store.stocks.filter { hit(it.name) }.take(8).forEach { s -> add(Hit("Company", s.name, Format.price(s.priceCents)) { onRival(s.firm) }) }
            store.divisions.filter { hit(Sectors.name(it.industry), it.regionName, "division") }.take(6).forEach { d -> add(Hit("Division", "${Sectors.name(d.industry)} · ${d.regionName}", "${d.blocks} plants") { onPlace(Place.Overview); store.showDivision(d) }) }
        }
        // Saved views: every screen's views, built-in and the player's own, by name.
        ScreenViews.forEach { (screen, title, rest) ->
            val (builtIn, place) = rest
            ViewBook(screen, builtIn, prefs).all.filter { hit(it.name, title, "view") }.forEach { v ->
                add(Hit("View", "$title: ${v.name}", if (builtIn.any { it.name == v.name }) "ready" else "yours") {
                    ViewBook(screen, builtIn, prefs).activate(v.name); GridPrefs.epoch++; onPlace(place)
                })
            }
        }
        if (words.isNotEmpty()) store.actions.filter { hit(it.title, it.keywords, it.category) }.take(8).forEach { a -> add(Hit("Action", a.title, a.category) { onAction(a.command) }) }
    }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(Modifier.fillMaxSize().background(Palette.panel)) {
        Row(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("FIND", color = Palette.gold, fontFamily = Pixel.font, fontSize = 12.sp)
            Spacer(Modifier.width(10.dp))
            GridSearch(q, "A good, a company, a division, a view or an action", 560.dp, {}, { q = it }, { hits.firstOrNull()?.go?.invoke() }, Modifier.weight(1f).focusRequester(focus).focusable())
            Spacer(Modifier.width(10.dp))
            GridButton("All actions", false, 10.sp, onAllActions)
            Spacer(Modifier.width(4.dp))
            GridButton("Close", false, 10.sp, onClose)
        }
        Column(Modifier.fillMaxWidth().weight(1f).verticalScroll(rememberScrollState()).padding(vertical = 4.dp)) {
            var kind = ""
            hits.forEachIndexed { i, h ->
                if (h.kind != kind) { kind = h.kind; MenuHeading(if (kind == "Company") "Companies" else kind + "s") }
                FindLine(h.kind, h.text, h.hint, i == 0 && words.isNotEmpty(), h.go)
            }
            if (hits.isEmpty()) Text("Nothing matches. Try another word, or open All actions.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(12.dp))
        }
        Text("Enter opens the first line. Esc closes.", color = Palette.secondary, fontSize = 10.sp, modifier = Modifier.fillMaxWidth().background(Palette.ink).border(1.dp, Palette.stroke).padding(horizontal = 10.dp, vertical = 4.dp))
    }
}
