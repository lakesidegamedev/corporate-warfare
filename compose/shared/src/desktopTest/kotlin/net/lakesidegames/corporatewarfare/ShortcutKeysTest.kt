package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.clickable
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Desktop shortcuts and focus: Space, N and 1 to 3 run the game only when nothing under the focus wants the key. A text
 * field keeps every key it types; a focused button keeps Enter and Space for its own click.
 */
class ShortcutKeysTest {
    @OptIn(androidx.compose.ui.InternalComposeUiApi::class)
    private fun key(key: Key, type: KeyEventType = KeyEventType.KeyDown, ctrl: Boolean = false): KeyEvent =
        KeyEvent(key, type, isCtrlPressed = ctrl)

    private class Rig {
        val fired = mutableListOf<PlainShortcut>()
        var clicks = 0
        val text = mutableStateOf("")
        val field = FocusRequester()
        val button = FocusRequester()
        val background = FocusRequester()
    }

    private fun scene(rig: Rig): ImageComposeScene = ImageComposeScene(300, 200, Density(1f)) {
        Column(Modifier.size(300.dp, 200.dp).plainShortcuts { rig.fired += it }) {
            BasicTextField(rig.text.value, { rig.text.value = it }, Modifier.size(100.dp, 30.dp).textEntry().focusRequester(rig.field))
            Box(Modifier.size(100.dp, 30.dp).focusRequester(rig.button).clickable { rig.clicks++ })
            Box(Modifier.size(100.dp, 30.dp).focusRequester(rig.background).focusable())
        }
    }

    private fun ImageComposeScene.press(key: Key) {
        sendKeyEvent(key(key))
        sendKeyEvent(key(key, KeyEventType.KeyUp))
        render()
    }

    @Test
    fun theMapOrGameFocusStillRunsTheShortcuts() {
        val rig = Rig()
        val s = scene(rig)
        s.render()
        rig.background.requestFocus(); s.render()
        listOf(Key.Spacebar, Key.N, Key.One, Key.Two, Key.Three).forEach { s.press(it) }
        assertEquals(
            listOf(PlainShortcut.PlayPause, PlainShortcut.AdvanceWeek, PlainShortcut.Speed1, PlainShortcut.Speed2, PlainShortcut.Speed3),
            rig.fired,
        )
        // A control chord is not a plain shortcut.
        s.sendKeyEvent(key(Key.N, ctrl = true)); s.render()
        assertEquals(5, rig.fired.size)
        s.close()
    }

    @Test
    fun aTextFieldKeepsTheKeysItTypes() {
        val rig = Rig()
        val s = scene(rig)
        s.render()
        rig.field.requestFocus(); s.render()
        listOf(Key.Spacebar, Key.N, Key.One, Key.Two, Key.Three).forEach { s.press(it) }
        assertTrue(rig.fired.isEmpty(), "typing in a text field never runs a shortcut, saw ${rig.fired}")
        s.close()
    }

    @Test
    fun aFocusedButtonKeepsEnterAndSpace() {
        val rig = Rig()
        val s = scene(rig)
        s.render()
        rig.button.requestFocus(); s.render()
        s.press(Key.Enter)
        s.press(Key.Spacebar)
        assertEquals(2, rig.clicks, "Enter and Space click the focused button")
        assertTrue(rig.fired.isEmpty(), "and do not also run the global shortcut, saw ${rig.fired}")
        s.close()
    }
}
