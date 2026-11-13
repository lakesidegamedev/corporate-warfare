package net.lakesidegames.corporatewarfare

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlin.test.Test
import kotlin.test.assertTrue
import uniffi.cw_ffi.CareerOptionsDto
import uniffi.cw_ffi.NewGameDto
import uniffi.cw_ffi.SessionHandle

/** Pictures of the new-game career choices, the Career screen with a standing and the end of career screen, from seeded careers. */
class RollA2RenderTest {
    private fun options(seed: ULong, guided: Boolean, capital: Long? = null) = NewGameDto(
        seed = seed, scenarioId = "great-lakes-2000", guided = guided, fiveYear = false, difficulty = "standard", economy = "historic",
        startingCapitalCents = capital, pricePath = "historical",
    )

    private fun blank(): Pair<GameStore, AppSettings> {
        ProposalCareer.init()
        val dir = kotlin.io.path.createTempDirectory("cwrolla2").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(java.io.File(dir, "settings.properties")).apply {
            finishSetup(); chooseTutorial(emptySet(), true); chooseGraphics(graphics.copy(animations = false))
        }
        return store to settings
    }

    /** A career started through the new career FFI call and played to [weeks]; [play] runs once a week. */
    private fun career(
        helm: Boolean, ambition: String, years: Int, weeks: UInt, capital: Long? = null, guided: Boolean = !helm,
        first: (SessionHandle) -> Unit = {}, play: (SessionHandle) -> Unit = {},
    ): Pair<GameStore, AppSettings> {
        val (store, settings) = blank()
        val o = options(11uL, guided, capital)
        val firm = if (helm) uniffi.cw_ffi.helmOffers(o).first().firm else null
        store.startCareerWith(o, CareerOptionsDto(start = if (helm) "helm" else "founder", helmFirm = firm, ambition = ambition, years = years.toUInt()))
        ProposalCareer.waitReady(store)
        kotlinx.coroutines.runBlocking {
            store.read { h ->
                first(h)
                var guard = 0
                while (h.overview().tick < weeks && guard++ < 700) {
                    h.dismissPause()
                    play(h)
                    h.advance(1u)
                    if (h.careerStanding()?.ended == true) break
                }
            }
        }
        store.refresh()
        Thread.sleep(2500)
        return store to settings
    }

    private fun place(name: String, store: GameStore, settings: AppSettings, where: Place) {
        try { ProposalCareer.shoot("roll-a-$name", 1920, 1080, settings) { TycoonShell(store, onSettings = {}, start = where) } } finally { GridDemo.clear() }
    }

    @Test
    fun newGame() {
        val (store, settings) = blank()
        val t0 = System.nanoTime()
        uniffi.cw_ffi.helmOffers(options(11uL, false))
        val ms = (System.nanoTime() - t0) / 1_000_000
        println("ROLLA2 helmOffers took $ms ms")
        val settle = ms * 2 + 4000
        ProposalCareer.shoot("roll-a-newgame-helm", 1500, 2300, settings, settleMs = settle) { NewGameScreen(store, startStep = 1, startHelm = true, startAmbition = "biggest", onBack = {}) }
        ProposalCareer.shoot("roll-a-newgame-founder", 1500, 1700, settings, settleMs = 1000) { NewGameScreen(store, startStep = 1, startAmbition = "richest", onBack = {}) }
        ProposalCareer.shoot("roll-a-newgame-phone", 390, 2700, settings, density = 2f, settleMs = settle) { NewGameScreen(store, startStep = 1, startHelm = true, startAmbition = "dominant", onBack = {}) }
    }

    @Test
    fun careerMidGame() {
        val (helm, hs) = career(helm = true, ambition = "biggest", years = 10, weeks = 104u)
        val standing = kotlinx.coroutines.runBlocking { helm.read { it.careerStanding() } }
        println("ROLLA2 helm standing: $standing")
        place("career-helm", helm, hs, Place.Career)
        val (founder, fs) = career(helm = false, ambition = "richest", years = 10, weeks = 104u)
        println("ROLLA2 founder standing: ${kotlinx.coroutines.runBlocking { founder.read { it.careerStanding() } }}")
        place("career-founder", founder, fs, Place.Career)
        ProposalCareer.shoot("roll-a-career-helm-phone", 390, 1200, hs, density = 2f) { CareerScreen(helm) }
    }

    @Test
    fun endWithMedal() {
        val (store, settings) = career(helm = true, ambition = "biggest", years = 5, weeks = 300u)
        val summary = kotlinx.coroutines.runBlocking { store.read { it.careerSummary() } }
        println("ROLLA2 medal summary: $summary")
        assertTrue(summary?.ended == true, "the five-year career did not end")
        ProposalCareer.shoot("roll-a-end-medal", 1500, 1500, settings) { EndOfCareerHost(store, {}, {}) }
        ProposalCareer.shoot("roll-a-end-medal-phone", 390, 1900, settings, density = 2f) { EndOfCareerHost(store, {}, {}) }
        place("career-ended", store, settings, Place.Career)
    }

    @Test
    fun endWithLoss() {
        // A founder with less than the fortune floor who never founds a company: the engine ends the career as lost.
        val (store, settings) = career(helm = false, ambition = "survivor", years = 10, weeks = 120u, capital = 20_000_000L, guided = false)
        val summary = kotlinx.coroutines.runBlocking { store.read { it.careerSummary() } }
        println("ROLLA2 loss summary: $summary")
        assertTrue(summary?.ended == true && summary.lost, "the career did not end as a loss")
        ProposalCareer.shoot("roll-a-end-loss", 1500, 1300, settings) { EndOfCareerHost(store, {}, {}) }
    }

    @Test
    fun debtAndBonds() {
        val (store, settings) = ProposalCareer.open()
        kotlinx.coroutines.runBlocking {
            store.read { h ->
                val me = h.overview().primaryFirm!!
                println("ROLLA2 loan: " + h.submitJson(CommandJson.command("AcceptLoanQuote", "firm" to me, "amount_cents" to 200_000_000L, "rate_bps" to 600, "weeks" to 156)).let { "${it.accepted} ${it.errorKey} ${it.errorDetail}" })
                println("ROLLA2 bond: " + h.submitJson(CommandJson.command("IssueBond", "firm" to me, "principal_cents" to 300_000_000L, "coupon_bps" to 550, "weeks" to 260)).let { "${it.accepted} ${it.errorKey} ${it.errorDetail}" })
                println("ROLLA2 meeting: " + h.submitJson(CommandJson.command("CallMeeting", "firm" to me)).let { "${it.accepted} ${it.errorKey} ${it.errorDetail}" })
                for (i in 0 until 6) { h.dismissPause(); h.advance(1u) }
            }
        }
        store.refresh()
        Thread.sleep(2500)
        place("finance-debt", store, settings, Place.Finance)
    }
}
