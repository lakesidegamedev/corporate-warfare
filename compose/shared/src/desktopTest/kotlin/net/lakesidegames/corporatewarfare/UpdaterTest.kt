package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class UpdaterTest {
    @Test
    fun versionsCompareByMajorMinorPatch() {
        assertTrue(Updater.isNewer("1.6.0", "1.5.0"))
        assertTrue(Updater.isNewer("1.10.0", "1.9.3"))
        assertTrue(Updater.isNewer("2.0.0", "1.99.99"))
        assertTrue(Updater.isNewer("1.5.1", "1.5.0"))
        assertFalse(Updater.isNewer("1.5.0", "1.5.0"))
        assertFalse(Updater.isNewer("1.4.9", "1.5.0"))
        assertFalse(Updater.isNewer("1.5", "1.5.0"))
        assertTrue(Updater.isNewer("1.6.0-preview.3", "1.6.0-preview.2"))
        assertTrue(Updater.isNewer("1.6.0", "1.6.0-preview.9"))
        assertTrue(Updater.isNewer("1.6.0-preview.2", "1.5.0"))
        assertFalse(Updater.isNewer("1.6.0-preview.2", "1.6.0"))
    }
}
