package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlin.test.Test

/**
 * The 2.0 interface proposal's before atlas: every desktop page and its main sub-screens at
 * 1920 x 1080 and the five phone tabs at 390 x 844, from one seeded game at week 156.
 * Set CW_MAP_RENDER_DIR to keep the pictures and CW_PROPOSAL_SAVE to reuse the game.
 */
class ProposalBeforeRenderTest {
    @Test
    fun beforeAtlas() {
        val (store, settings) = ProposalCareer.open()
        val pages = listOf("map", "office", "research", "stats", "markets", "actions", "inbox", "paper", "portfolio", "hall")
        pages.forEachIndexed { i, name ->
            ProposalCareer.shoot("before-$name", 1920, 1080, settings) { DesktopShell(store, onSettings = {}, startPage = i) }
        }
        val me = store.selectedFirm!!
        // Dialogs open over the page the player opens them from.
        store.divisions.firstOrNull()?.let { d ->
            store.showDivision(d)
            ProposalCareer.shoot("before-division", 1920, 1080, settings) { DesktopShell(store, onSettings = {}, startPage = 1) }
            store.showDivision(null)
        }
        ProposalCareer.shoot("before-rivalry", 1920, 1080, settings) {
            DesktopShell(store, onSettings = {}, startPage = 1)
            RivalsBoardDialog(store) {}
        }
        val rival = store.stocks.filter { it.firm != me && !it.controlledByYou }.maxByOrNull { it.marketCapCents }
        rival?.let { r ->
            ProposalCareer.shoot("before-company", 1920, 1080, settings) {
                DesktopShell(store, onSettings = {}, startPage = 4)
                CompanyPageDialog(store, r.firm) {}
            }
        }
        // Sections that live inside cards or side panes, drawn at the width the game gives them.
        fun pane(width: Int, content: @androidx.compose.runtime.Composable () -> Unit) = @androidx.compose.runtime.Composable {
            Box(Modifier.fillMaxSize().padding(16.dp)) {
                Column(
                    Modifier.width(width.dp).fillMaxHeight().background(Palette.panel).border(1.dp, Palette.stroke)
                        .padding(12.dp).verticalScroll(rememberScrollState()),
                ) { content() }
            }
        }
        val firm = store.selectedFirmDto!!
        val finance = kotlinx.coroutines.runBlocking { store.read { h -> loadFinancials(h, me, firm.name, firm.industry) } }!!
        ProposalCareer.shoot("before-financials", 1920, 1080, settings, content = pane(940) { CompanyFinancials(finance, columns = 5, initialTab = FinanceTab.Income) })
        ProposalCareer.shoot("before-freight", 1920, 1080, settings, content = pane(360) { FreightPanel(store, null, null, {}) })
        ProposalCareer.shoot("before-economy", 1920, 1080, settings, content = pane(430) { EconomyCard(store, collapsible = false) })
        ProposalCareer.shoot("before-systems-logistics", 1920, 1080, settings, content = pane(940) { LogisticsStatsSection(store) })
        val good = store.commodities.maxByOrNull { it.soldLots }
        good?.let { g -> ProposalCareer.shoot("before-systems-good", 1920, 1080, settings, content = pane(940) { GoodDepthSection(store, g.product) }) }
        val board = store.boards.firstOrNull { b -> store.divisions.any { it.region == b.code } } ?: store.boards.first()
        ProposalCareer.shoot("before-region", 1920, 1080, settings, content = pane(940) { RegionDossierSection(store, board.index) })
        // The phone: each of the five tabs.
        listOf("map", "office", "markets", "inbox", "portfolio").forEachIndexed { i, name ->
            PhoneStartTab = i
            ProposalCareer.shoot("before-phone-$name", 390, 844, settings, density = 2f) { CorporateWarfareApp(store, settings) }
        }
        PhoneStartTab = 0
        println("proposal career: week ${store.overview?.tick} divisions ${store.divisions.size} stocks ${store.stocks.size} goods ${store.commodities.size}")
    }
}
