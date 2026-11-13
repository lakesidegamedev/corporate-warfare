package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** System back: the deepest open screen handles the press, and with nothing open the platform acts. */
class BackStackTest {
    @Test
    fun theDeepestHandlerRunsFirstThenTheOneBelow() {
        val calls = mutableListOf<String>()
        val page = { calls += "page" }
        val sheet = { calls += "sheet" }
        Back.add(page)
        Back.add(sheet)
        try {
            assertTrue(Back.press())
            Back.remove(sheet)
            assertTrue(Back.press())
            assertEquals(listOf("sheet", "page"), calls)
        } finally {
            Back.remove(sheet)
            Back.remove(page)
        }
    }

    @Test
    fun withNothingOpenBackIsLeftToThePlatform() {
        assertFalse(Back.press())
    }
}
