package net.lakesidegames.corporatewarfare

import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import java.io.File
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Rebindable keys: defaults, refused clashes, Reset, and the settings file round trip. */
class KeyBindingsTest {
    @AfterTest
    fun restore() = KeyBindings.reset()

    @OptIn(androidx.compose.ui.InternalComposeUiApi::class)
    private fun press(key: Key) = KeyEvent(key, KeyEventType.KeyDown)

    @Test
    fun defaultsAreTheOriginalKeys() {
        KeyBindings.reset()
        assertEquals("Space", KeyBindings.keyOf("PlayPause"))
        assertEquals("N", KeyBindings.keyOf("AdvanceWeek"))
        assertEquals("F2", KeyBindings.keyOf("place.Overview"))
        assertEquals(PlainShortcut.PlayPause, plainShortcutFor(press(Key.Spacebar)))
        assertEquals(PlainShortcut.Speed3, plainShortcutFor(press(Key.Three)))
        assertTrue(KeyBindings.actions.map { it.default }.toSet().size == KeyBindings.actions.size, "defaults do not clash")
    }

    @Test
    @OptIn(androidx.compose.ui.InternalComposeUiApi::class)
    fun aRebindMovesTheShortcutAndFreesTheOldKey() {
        assertNull(KeyBindings.bind("AdvanceWeek", "W"))
        assertEquals(PlainShortcut.AdvanceWeek, plainShortcutFor(press(Key.W)))
        assertNull(plainShortcutFor(press(Key.N)), "the old key no longer runs it")
        assertNull(KeyBindings.bind("Speed1", "N"), "the freed key can be used by another action")
        assertEquals(PlainShortcut.Speed1, plainShortcutFor(press(Key.N)))
    }

    @Test
    fun aKeyInUseIsRefusedWithAPlainMessageAndNothingChanges() {
        val message = assertNotNull(KeyBindings.bind("AdvanceWeek", "Space"))
        assertTrue(message.contains("Space") && message.contains("play or pause time"), message)
        assertEquals("N", KeyBindings.keyOf("AdvanceWeek"))
        assertNotNull(KeyBindings.bind("AdvanceWeek", "F4"), "a page key is taken too")
        assertNotNull(KeyBindings.bind("AdvanceWeek", "Tab"), "a key that cannot be bound")
        assertNull(KeyBindings.bind("AdvanceWeek", "N"), "binding an action to its own key is fine")
    }

    @Test
    fun aPageKeyCanBeMoved() {
        assertNull(KeyBindings.bind("place.Finance", "F11"))
        assertEquals("F11", KeyBindings.placeKey(Place.Finance))
        assertEquals("place.Finance", KeyBindings.actionFor("F11"))
        assertNull(KeyBindings.actionFor("F4"))
    }

    @Test
    fun resetRestoresEveryDefault() {
        KeyBindings.bind("PlayPause", "P")
        KeyBindings.bind("place.Map", "F12")
        KeyBindings.reset()
        assertEquals("Space", KeyBindings.keyOf("PlayPause"))
        assertEquals("F1", KeyBindings.keyOf("place.Map"))
        assertTrue(KeyBindings.changed().isEmpty())
    }

    @Test
    fun bindingsSurviveTheSettingsFileAndResetSectionClearsThem() {
        val file = File(kotlin.io.path.createTempDirectory("cwkeys").toFile(), "settings.properties")
        val settings = AppSettings(file)
        assertNull(settings.bindKey("PlayPause", "P"))
        assertNotNull(settings.bindKey("AdvanceWeek", "P"))
        KeyBindings.reset()
        AppSettings(file)
        assertEquals("P", KeyBindings.keyOf("PlayPause"), "loaded from the file")
        assertEquals("N", KeyBindings.keyOf("AdvanceWeek"), "the refused change was not saved")
        val again = AppSettings(file)
        again.resetSection(SettingsSection.Keys)
        assertEquals("Space", KeyBindings.keyOf("PlayPause"))
        KeyBindings.reset()
        AppSettings(file)
        assertEquals("Space", KeyBindings.keyOf("PlayPause"), "Reset was saved")
    }

    @Test
    fun aSavedClashIsDroppedOnLoad() {
        KeyBindings.load(mapOf("PlayPause" to "N", "Speed1" to "Q", "nonsense" to "Z"))
        // N is held by Next week, so the first is refused; the valid one stays.
        assertEquals("Space", KeyBindings.keyOf("PlayPause"))
        assertEquals("Q", KeyBindings.keyOf("Speed1"))
    }
}
