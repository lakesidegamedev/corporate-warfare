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
        // A page not yet converted, inside the new shell, to show nothing is lost.
        ProposalCareer.shoot("after-shell-legacy-finance", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.Finance) }
        // The phone: the same grid in two-line rows with the same sort and filter.
        ProposalCareer.shoot("after-goods-phone", 390, 844, settings, density = 2f) { GoodsScreen(store) }
    }

    @Test
    fun overviewOfALargeCompany() {
        // A company with many plants: the tables scroll and nothing is lost to dead space.
        val (store, settings) = ProposalCareer.openBig()
        assertTrue(store.divisions.isNotEmpty(), "the large company needs divisions")
        println("large company: ${store.divisions.size} divisions, ${store.divisions.sumOf { it.blocks.toInt() }} plants, ${store.productLines.size} product lines")
        ProposalCareer.shoot("after-overview-big", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = Place.Overview) }
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
