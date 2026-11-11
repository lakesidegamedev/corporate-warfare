package net.lakesidegames.corporatewarfare

import kotlin.test.Test

/** Pictures of the company, portfolio and career screens in the 2.0 shell, from the seeded week 156 game. */
class RollARenderTest {
    private fun shot(name: String, place: Place, setup: () -> Unit = {}) {
        val (store, settings) = ProposalCareer.open()
        setup()
        try { ProposalCareer.shoot("roll-a-$name", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = place) } } finally { GridDemo.clear() }
    }

    @Test
    fun finance() {
        shot("finance", Place.Finance)
        shot("finance-balance", Place.Finance) { GridDemo.screen = "finance"; GridDemo.shape = { FinanceViews[1] } }
    }

    @Test
    fun company() {
        shot("people", Place.People)
        shot("plants", Place.Plants)
        shot("plants-idle", Place.Plants) { GridDemo.screen = "plants"; GridDemo.shape = { PlantViews[2] } }
        shot("research", Place.Research)
    }

    @Test
    fun you() {
        shot("portfolio", Place.Portfolio)
        shot("portfolio-performance", Place.Portfolio) { GridDemo.screen = "holdings"; GridDemo.shape = { HoldingViews[1] } }
        shot("career", Place.Career)
        shot("hall", Place.Hall)
    }

    @Test
    fun phones() {
        val (store, settings) = ProposalCareer.open()
        ProposalCareer.shoot("roll-a-finance-phone", 390, 844, settings, density = 2f) { FinanceScreen(store) }
        ProposalCareer.shoot("roll-a-plants-phone", 390, 844, settings, density = 2f) { PlantsScreen(store) }
        ProposalCareer.shoot("roll-a-portfolio-phone", 390, 844, settings, density = 2f) { PortfolioScreen(store) }
        ProposalCareer.shoot("roll-a-career-phone", 390, 844, settings, density = 2f) { CareerScreen(store) }
        ProposalCareer.shoot("roll-a-research-phone", 390, 844, settings, density = 2f) { ResearchGridScreen(store) }
        ProposalCareer.shoot("roll-a-people-phone", 390, 844, settings, density = 2f) { PeopleScreen(store) }
    }
}
