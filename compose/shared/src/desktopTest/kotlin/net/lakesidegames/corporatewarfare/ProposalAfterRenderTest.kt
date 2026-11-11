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
