package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** A refused action always says why in plain words, never the engine's developer dump or its key. */
class MessagesTest {
    private val keys = listOf(
        "error.invalid_actor", "error.no_authority", "error.stale_revision", "error.invalid_entity", "error.insufficient_cash",
        "error.insufficient_stock", "error.capacity_unavailable", "error.solvency_restriction", "error.ownership_cycle",
        "error.quote_expired", "error.legal_review_required", "error.duplicate_command", "error.arithmetic", "error.wrong_phase",
        "error.malformed_command", "error.resource_limit", "error.internal_fault", "error.something_new",
    )

    @Test
    fun noRejectionShowsADeveloperDump() {
        val dump = "InvalidEntity { kind: Facility, id: 42 }"
        for (key in keys) {
            val text = Messages.rejection(key, dump)
            assertFalse(text.contains("{") || text.contains("InvalidEntity") || text.contains("error."), "$key shows: $text")
            assertTrue(text.endsWith("."), "$key is a sentence: $text")
        }
    }

    @Test
    fun specificRulesStillNameTheirFix() {
        assertTrue(Messages.rejection("error.wrong_phase", "WrongPhase { rule: \"raid_cooldown\" }").contains("thirteen weeks"))
        assertEquals(Messages.rejection("error.wrong_phase", ""), Messages.rejection("error.wrong_phase", "WrongPhase { other }"))
    }
}
