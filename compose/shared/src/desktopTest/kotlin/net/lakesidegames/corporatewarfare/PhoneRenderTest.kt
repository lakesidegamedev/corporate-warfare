package net.lakesidegames.corporatewarfare

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The 2.0 layout on a 390 x 844 phone, from the week 156 game: the bottom bar's lists, every place, the pull-down,
 * the alert sheet and Find. Set CW_MAP_RENDER_DIR to keep the pictures (phone-*.png).
 */
class PhoneRenderTest {
    private fun shoot(name: String, store: GameStore, settings: AppSettings, nav: PhoneNav, tap: Boolean = false) {
        GridDemo.clear()
        try {
            ProposalCareer.shoot("phone-$name", 390, 844, settings, density = 2f) {
                PhoneShell(store, nav, onSettings = {}, onMenu = {})
                // A tap on a row opens the place's detail pane as a page.
                if (tap) androidx.compose.runtime.LaunchedEffect(nav.panes.entries.size) { nav.panes.tap() }
            }
        } finally { GridDemo.clear() }
    }

    @Test
    fun lists() {
        val (store, settings) = ProposalCareer.open()
        for (t in PhoneTab.entries) shoot("tab-" + t.name.lowercase(), store, settings, PhoneNav().apply { tab = t })
        shoot("pulldown", store, settings, PhoneNav().apply { tab = PhoneTab.Company; place = Place.Overview; pulled = true })
        shoot("alerts", store, settings, PhoneNav().apply { alerts = true })
        shoot("find", store, settings, PhoneNav().apply { find = true })
    }

    @Test
    fun places() {
        val (store, settings) = ProposalCareer.open()
        for (p in Place.entries.filter { System.getenv("CW_PHONE_ONLY")?.split(',')?.contains(it.name) ?: true }) {
            val nav = PhoneNav()
            nav.open(p)
            shoot("place-" + p.name.lowercase(), store, settings, nav)
        }
    }

    @Test
    fun panes() {
        val (store, settings) = ProposalCareer.open()
        for (p in listOf(Place.Overview, Place.Goods, Place.Openings, Place.Regions, Place.Freight, Place.Auctions, Place.League, Place.Raids, Place.Activity, Place.Deals, Place.Economy, Place.Politics, Place.Timeline, Place.Inbox)) {
            val nav = PhoneNav()
            nav.open(p)
            shoot("pane-" + p.name.lowercase(), store, settings, nav, tap = true)
        }
    }

    /** The new-game steps and the end of career screen on a phone, from a five year helm career. */
    @Test
    fun careerScreens() {
        ProposalCareer.init()
        val dir = kotlin.io.path.createTempDirectory("cwphone").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(java.io.File(dir, "settings.properties")).apply {
            finishSetup(); chooseTutorial(emptySet(), true); chooseGraphics(graphics.copy(animations = false))
        }
        val o = uniffi.cw_ffi.NewGameDto(
            seed = 11uL, scenarioId = "great-lakes-2000", guided = false, fiveYear = false, difficulty = "standard", economy = "historic",
            startingCapitalCents = null, pricePath = "historical",
        )
        val t0 = System.nanoTime()
        val firm = uniffi.cw_ffi.helmOffers(o).first().firm
        val settle = (System.nanoTime() - t0) / 1_000_000 * 2 + 4000
        ProposalCareer.shoot("phone-newgame-scenario", 390, 844, settings, density = 2f) { NewGameScreen(store, startStep = 0, onBack = {}) }
        ProposalCareer.shoot("phone-newgame-setup", 390, 844, settings, density = 2f, settleMs = settle) { NewGameScreen(store, startStep = 1, startHelm = true, startAmbition = "biggest", onBack = {}) }
        ProposalCareer.shoot("phone-newgame-setup-long", 390, 2700, settings, density = 2f, settleMs = settle) { NewGameScreen(store, startStep = 1, startHelm = true, startAmbition = "dominant", onBack = {}) }
        store.startCareerWith(o, uniffi.cw_ffi.CareerOptionsDto(start = "helm", helmFirm = firm, ambition = "biggest", years = 5u))
        ProposalCareer.waitReady(store)
        kotlinx.coroutines.runBlocking {
            store.read { h ->
                var guard = 0
                while (h.overview().tick < 300u && guard++ < 700) {
                    h.dismissPause()
                    h.advance(1u)
                    if (h.careerStanding()?.ended == true) break
                }
            }
        }
        store.refresh()
        Thread.sleep(2500)
        assertTrue(kotlinx.coroutines.runBlocking { store.read { it.careerSummary() } }?.ended == true, "the five-year career did not end")
        ProposalCareer.shoot("phone-end-of-career", 390, 844, settings, density = 2f) { EndOfCareerHost(store, {}, {}) }
        ProposalCareer.shoot("phone-end-of-career-long", 390, 1900, settings, density = 2f) { EndOfCareerHost(store, {}, {}) }
    }

    @Test
    fun navigation() {
        val nav = PhoneNav()
        nav.open(Place.Goods)
        assertEquals(PhoneTab.Markets, nav.tab)
        nav.open(Place.Economy)
        assertEquals(PhoneTab.More, nav.tab)
        nav.open(Place.Career)
        assertEquals(PhoneTab.More, nav.tab)
        nav.open(Place.Map)
        assertEquals(PhoneTab.Map, nav.tab)
        assertTrue(nav.place == null)
        // Every place except the map lives under exactly one bottom bar entry.
        for (p in Place.entries) assertEquals(1, PhoneTab.entries.count { p.group in it.groups }, p.name)
    }
}
