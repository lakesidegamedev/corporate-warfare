package net.lakesidegames.corporatewarfare

import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.cw_ffi.NewGameDto

/**
 * Runs against the shrunk release build: a Great Lakes career starts through the game store, as
 * the new-game screen does, five weeks play, and the overview still has a company and a week.
 * If R8 removed or renamed anything the bindings or JNA need, this fails.
 */
class ShrunkEngineTest {
    private fun waitFor(what: String, timeoutMs: Long = 60_000, done: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (!done()) {
            if (System.currentTimeMillis() > end) throw AssertionError("timed out waiting for $what")
            Thread.sleep(50)
        }
    }

    @Test
    fun aGreatLakesCareerStartsAndPlaysFiveWeeks() {
        val dir = File(InstrumentationRegistry.getInstrumentation().targetContext.cacheDir, "smoke-saves").apply { mkdirs() }
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        store.startCareer(
            NewGameDto(
                seed = 11uL,
                scenarioId = "great-lakes-2000",
                guided = true,
                fiveYear = false,
                difficulty = "standard",
                economy = "historic",
                startingCapitalCents = null,
                pricePath = "",
                timeline = "",
            ),
        )
        waitFor("the career to start") { store.ready || store.error != null }
        assertNull("starting failed: ${store.error}", store.error)
        val start = store.overview
        assertNotNull("overview after start", start)
        repeat(5) {
            val before = store.overview!!.tick
            store.advance()
            waitFor("week ${before + 1u}") { store.error != null || (store.overview?.tick ?: 0u) > before }
            assertNull("advancing failed: ${store.error}", store.error)
        }
        val overview = store.overview!!
        assertEquals(start!!.tick + 5u, overview.tick)
        assertNotNull("a company", overview.primaryFirm)
        assertTrue("company name", overview.primaryName.isNotBlank())
        assertTrue("week label", overview.weekLabel.isNotBlank())
    }
}
