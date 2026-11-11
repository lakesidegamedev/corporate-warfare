package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertTrue

/** Pictures of the markets, rivals and world screens in the 2.0 shell, from the week 156 game. Set CW_MAP_RENDER_DIR to keep them. */
class RollBRenderTest {
    @Test
    fun screens() {
        val (store, settings) = ProposalCareer.open()
        assertTrue(store.boards.isNotEmpty(), "regions need boards")
        fun shot(name: String, place: Place, screen: String? = null, shape: ((ViewState) -> ViewState)? = null, pick: ULong? = null) {
            GridDemo.screen = screen; GridDemo.shape = shape
            try { ProposalCareer.shoot(name, 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = place, startPick = pick) } } finally { GridDemo.clear() }
        }
        val rival = store.stocks.filter { it.firm != store.selectedFirm && !it.controlledByYou }.maxByOrNull { it.marketCapCents }?.firm
        shot("roll-b-regions", Place.Regions)
        shot("roll-b-regions-households", Place.Regions, "regions", shape = { RegionViews[3] })
        shot("roll-b-freight", Place.Freight)
        shot("roll-b-freight-mine", Place.Freight, "lanes", shape = { LaneViews[3] })
        shot("roll-b-stocks", Place.League, "league", { LeagueViews.first { v -> v.name == "Exchange" } }, rival)
        shot("roll-b-activity", Place.Activity)
        shot("roll-b-deals", Place.Deals)
        shot("roll-b-deals-held", Place.Deals, "deals", shape = { DealViews[1] })
        shot("roll-b-economy", Place.Economy)
        shot("roll-b-politics", Place.Politics)
        shot("roll-b-timeline", Place.Timeline)
        shot("roll-b-letters", Place.Inbox)
        shot("roll-b-paper", Place.Paper)
        ProposalCareer.shoot("roll-b-regions-phone", 390, 844, settings, density = 2f) { RegionsScreen(store) }
        ProposalCareer.shoot("roll-b-economy-phone", 390, 844, settings, density = 2f) { EconomyScreen(store) }
        ProposalCareer.shoot("roll-b-letters-phone", 390, 844, settings, density = 2f) { LettersScreen(store) }
    }
}
