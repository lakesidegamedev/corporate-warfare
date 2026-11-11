package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * The 2.0 interface proposal's after pictures: the new shell with the company overview, the goods
 * grid and the league table with a dossier, from the same week 156 game as the before atlas, and
 * the goods grid on a phone. Set CW_MAP_RENDER_DIR to keep the pictures.
 */
class ProposalAfterRenderTest {
    @Test
    fun afterScreens() {
        val (store, settings) = ProposalCareer.open()
        assertTrue(store.commodities.isNotEmpty(), "the goods grid needs goods")
        ProposalCareer.shoot("after-overview", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.Overview) }
        ProposalCareer.shoot("after-goods", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.Goods) }
        val me = store.selectedFirm
        val rival = store.stocks.filter { it.firm != me && !it.controlledByYou }.maxByOrNull { it.marketCapCents }?.firm
        ProposalCareer.shoot("after-league", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.League, startPick = rival) }
        // The control bar at work on the goods grid, each from the same game.
        val sector = store.commodities.filter { it.sector != "other" }.groupBy { it.sector }.maxByOrNull { it.value.size }!!.key
        demo(store, settings, "views-open", "goods", menu = "view")
        demo(store, settings, "columns-chooser", "goods", menu = "columns")
        demo(store, settings, "filters-active", "goods", search = "sold>0", shape = { v ->
            v.withFilter(Filter(FilterOp.Is, "sector", Sectors.name(sector))).withFilter(Filter(FilterOp.Above, "price", number = 1.0))
        })
        demo(store, settings, "grouped", "goods", shape = { v -> v.copy(groupBy = "sector") })
        demo(store, settings, "league-finance", "league", shape = { LeagueViews[1] }, place = Place.League, pick = rival)
        // A page not yet converted, inside the new shell, to show nothing is lost.
        ProposalCareer.shoot("after-shell-legacy-finance", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.Finance) }
        // The watchlist: two goods and the largest rival starred, the rail on its Watch tab; starred goods lead the grid.
        Watchlist.load(settings)
        val starred = listOf("good:" + store.commodities.first { it.sector == sector }.productId, "good:" + store.commodities.maxBy { it.priceCents }.productId) +
            listOfNotNull(store.stocks.firstOrNull { it.firm == rival }?.let { "firm:" + it.name })
        starred.forEach { if (!Watchlist.has(it)) Watchlist.toggle(it) }
        GridDemo.railTab = "Watch"
        try { ProposalCareer.shoot("watch-rail", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.Goods) } } finally { GridDemo.clear() }
        starred.forEach { if (Watchlist.has(it)) Watchlist.toggle(it) }
        // The phone: the same grid in two-line rows with the same sort and filter.
        ProposalCareer.shoot("after-goods-phone", 390, 844, settings, density = 2f) { GoodsScreen(store) }
        GridDemo.screen = "goods"; GridDemo.menu = "view"
        try { ProposalCareer.shoot("phone-view-sheet", 390, 844, settings, density = 2f) { GoodsScreen(store) } } finally { GridDemo.clear() }
    }

    /** One shell picture with a grid's menu open, a search typed or its view reshaped, as a player would leave it. */
    private fun demo(s: GameStore, settings: AppSettings, name: String, screen: String, menu: String? = null, search: String = "", shape: ((ViewState) -> ViewState)? = null, place: Place = Place.Goods, pick: ULong? = null) {
        GridDemo.screen = screen; GridDemo.menu = menu; GridDemo.search = search; GridDemo.shape = shape
        try {
            ProposalCareer.shoot(name, 1920, 1080, settings) { TycoonShell(s, onSettings = {}, start = place, startPick = pick) }
        } finally {
            GridDemo.clear()
        }
    }

    /** Round 2: history on every grid, compare, focus, folded rail, alerts, drill-down, side panes, Ctrl K, chart these, heading menu and the three new screens. */
    @Test
    fun roundTwoScreens() {
        val (store, settings) = ProposalCareer.open()
        fun shot(name: String, place: Place = Place.Goods, setup: () -> Unit = {}) {
            setup()
            try { ProposalCareer.shoot(name, 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = place) } } finally { GridDemo.clear(); RailState.tab = "Needs you" }
        }
        // Five years of weekly supply on the goods grid, a year back.
        shot("goods-supply-1y") { GridDemo.screen = "goods"; GridDemo.shape = { GoodsViews[2].copy(timeframe = Timeframe.Y1) } }
        // Compare: the last 13 weeks against the 13 before, on the Prices view.
        shot("goods-compare") { GridDemo.screen = "goods"; GridDemo.shape = { GoodsViews[1].copy(compare = Compare.Previous) } }
        // A right-click heading menu on the price column.
        shot("heading-menu") { GridDemo.screen = "goods"; GridDemo.head = "price" }
        // A figure's drill-down with its own period buttons.
        shot("drill-down") { GridDemo.screen = "goods"; GridDemo.shape = { GoodsViews[1] }; GridDemo.drill = "price" }
        // Chart these: the first rows of the Supply view as lines.
        shot("chart-these") { GridDemo.screen = "goods"; GridDemo.shape = { GoodsViews[2].copy(timeframe = Timeframe.Y1) }; GridDemo.chart = true }
        // Two side panes, the top one pinned: a good's price beside a company's share price.
        val good = store.commodities.maxBy { it.soldLots }
        val rival = store.stocks.filter { !it.controlledByYou }.maxBy { it.marketCapCents }
        SidePanes.items.clear()
        SidePanes.open(DrillItem("${good.name}: Price", good.historyCents, 1, { Format.money(it) }))
        SidePanes.pin(SidePanes.items.first())
        SidePanes.open(DrillItem("${rival.name}: Share", rival.historyCents, 1, { Format.price(it) }))
        try { shot("side-panes") } finally { SidePanes.items.clear() }
        // The pinned focus: one sector, followed by goods and the divisions grid.
        val sector = store.commodities.filter { it.sector != "other" }.groupBy { it.sector }.maxByOrNull { it.value.size }!!.key
        GridPrefs.load(settings)
        GridPrefs.chooseFocus("sector:$sector", Sectors.name(sector), settings)
        try { shot("focus-pinned") } finally { GridPrefs.chooseFocus(null, "", settings) }
        // The rail folded to its marks and counts.
        GridPrefs.foldRail(true, settings)
        try { shot("rail-folded", Place.League) } finally { GridPrefs.foldRail(false, settings) }
        // A starred good with an alert, the editor open on it.
        Watchlist.load(settings)
        val watched = "good:" + good.productId
        if (!Watchlist.has(watched)) Watchlist.toggle(watched)
        Watchlist.setAlert(watched, WatchAlert(above = true, level = good.priceCents - 1))
        try { shot("watch-alert") { GridDemo.railTab = "Watch"; GridDemo.alertEdit = watched } } finally { Watchlist.toggle(watched) }
        // Ctrl K finding a good, a company and saved views.
        shot("find") { GridDemo.find = "st" }
        // The three new screens and the overview's product lines and results with their bars.
        shot("openings", Place.Openings)
        shot("openings-by-region", Place.Openings) { GridDemo.screen = "openings"; GridDemo.shape = { OpeningViews[2] } }
        shot("overview-lines-results", Place.Overview) { GridDemo.screen = "lines"; GridDemo.shape = { LineViews[1] } }
        ProposalCareer.shoot("openings-phone", 390, 844, settings, density = 2f) { OpeningsScreen(store) }
        // Week 156 has no plant on offer and no raid announced: show the empty screens, then play on
        // (at most a year, the player doing nothing) until the engine has both, and show them again.
        shot("auctions-empty", Place.Auctions)
        shot("raids-empty", Place.Raids)
        val played = kotlinx.coroutines.runBlocking {
            store.read { h ->
                var n = 0
                // A decision the week waits for (a tender offer) gets its first answer, as a player who refuses would give.
                while (n < 52 && (h.plantLots().isEmpty() || h.raidCampaigns().isEmpty())) {
                    h.events().filter { it.mandatory }.forEach { e -> e.choices.firstOrNull()?.let { c -> h.submitJson("{\"RespondEvent\":{\"event\":${e.id},\"choice\":\"${c.id}\"}}") } }
                    h.dismissPause()
                    val before = h.overview().tick
                    h.advance(1u)
                    if (h.overview().tick == before) break
                    n++
                }
                n
            }
        } ?: 0
        store.refresh()
        val until = System.currentTimeMillis() + 30_000
        while ((store.overview?.tick ?: 0u) < (156 + played).toUInt() && System.currentTimeMillis() < until) Thread.sleep(100)
        Thread.sleep(1500)
        System.getenv("CW_MAP_RENDER_DIR")?.let { java.io.File(it, "round2-played.txt").writeText("weeks played after week 156: $played, now ${store.overview?.weekLabel}, lots ${kotlinx.coroutines.runBlocking { store.read { it.plantLots().size } }}, campaigns ${kotlinx.coroutines.runBlocking { store.read { it.raidCampaigns().size } }}\n") }
        shot("auctions", Place.Auctions)
        shot("raids", Place.Raids) { GridDemo.screen = "raids"; GridDemo.shape = { RaidViews[3] } }
    }

    @Test
    fun overviewOfALargeCompany() {
        // The test career stays small, so a large company is made from the real rows: 14 divisions and 18
        // product lines copied under new keys, to show the tables scroll and give room to the results table.
        val (store, settings) = ProposalCareer.open()
        val regions = listOf("A", "B", "C", "D", "E", "F", "G")
        val divisions = (0 until 14).map { i -> store.divisions[i % store.divisions.size].copy(region = regions[i % 7] + i, regionName = "Region ${i + 1}") }
        val lines = (0 until 18).map { i -> store.productLines[i % store.productLines.size].copy(product = (1000 + i).toUInt(), name = "Product ${i + 1}") }
        ProposalCareer.shoot("after-overview-large", 1920, 1080, settings) { CompanyOverviewScreen(store, divisions, lines) }
    }

    @Test
    fun gridSortsAndFilters() {
        // The grid's sort and filter are plain list operations on the engine's rows; check the column keys line up.
        val cols = listOf(
            GridColumn<Pair<String, Long>>("name", "Name", null, numeric = false, text = { it.first }),
            GridColumn<Pair<String, Long>>("v", "Value", text = { "${it.second}" }, sort = { it.second }),
        )
        val rows = listOf("b" to 2L, "a" to 3L, "c" to 1L)
        assertTrue(rows.sortedBy { cols[1].sort!!(it) }.map { it.first } == listOf("c", "b", "a"))
        assertTrue(rows.filter { cols[0].text(it).contains("a") }.size == 1)
    }
}
