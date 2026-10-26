package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import uniffi.cw_ffi.SessionHandle
import uniffi.cw_ffi.importSave

/** The Kotlin bindings drive the real engine: a career opens, weeks settle, saves round-trip. */
class EngineSmokeTest {
    @Test
    fun aGuidedCareerPlaysAndSavesThroughTheBindings() {
        val session = SessionHandle(7u, true, false)
        val start = session.overview()
        assertTrue(start.primaryFirm != null, "the guided career has a company")
        repeat(3) { session.advance(1u) }
        val later = session.overview()
        assertEquals(start.tick + 3u, later.tick)
        val firm = later.primaryFirm!!
        assertTrue(session.productLines(firm).isNotEmpty(), "the company makes something")
        assertTrue(session.finance(firm)!!.recentWeeks.size == 3)

        // A posture command goes through the same JSON path as the app.
        val line = session.productLines(firm).first()
        val receipt = session.submitJson(CommandJson.command("SetPricePosture", "firm" to firm, "product" to line.product, "mode" to 1, "bps" to -1000))
        assertTrue(receipt.accepted, "posture accepted: ${receipt.errorKey} ${receipt.errorDetail}")
        assertEquals("market", session.productLines(firm).first().mode)

        val restored = importSave(session.exportSave())
        assertEquals(session.canonicalHash(), restored.canonicalHash(), "a save restores the same world")
    }
}
