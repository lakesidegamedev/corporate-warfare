package net.lakesidegames.corporatewarfare

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
