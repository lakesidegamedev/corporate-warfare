package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.type

/** The plain-key game shortcuts of the desktop shell. */
enum class PlainShortcut { PlayPause, AdvanceWeek, Speed1, Speed2, Speed3 }

/** The shortcut a key press stands for, or null when it is not one (modifier chords are never plain shortcuts). */
fun plainShortcutFor(event: KeyEvent): PlainShortcut? {
    if (event.type != KeyEventType.KeyDown) return null
    if (event.isCtrlPressed || event.isAltPressed || event.isMetaPressed) return null
    return when (event.key) {
        Key.Spacebar -> PlainShortcut.PlayPause
        Key.N -> PlainShortcut.AdvanceWeek
        Key.One -> PlainShortcut.Speed1
        Key.Two -> PlainShortcut.Speed2
        Key.Three -> PlainShortcut.Speed3
        else -> null
    }
}

/**
 * Which text fields hold the keyboard focus. A text field types a key as a separate event after the key press, so a key
 * press alone cannot tell the shell the key is spoken for; every text field the player can type in carries
 * [textEntry], and plain shortcuts stand down while any of them has focus.
 */
object TextEntryFocus {
    private val holders = mutableSetOf<Any>()
    val active: Boolean get() = holders.isNotEmpty()
    internal fun set(holder: Any, focused: Boolean) { if (focused) holders += holder else holders -= holder }
}

/** Marks a text field: while it has focus, the plain-key shortcuts leave every key to it. */
fun Modifier.textEntry(): Modifier = composed {
    val holder = remember { Any() }
    DisposableEffect(holder) { onDispose { TextEntryFocus.set(holder, false) } }
    onFocusChanged { TextEntryFocus.set(holder, it.isFocused) }
}

/**
 * Plain-key shortcuts for everything inside this element. They run on the way back up, after the focused control has had
 * its turn, so a focused button keeps Enter and Space for itself; they also stand down while a [textEntry] field has
 * focus, so typing a space, an N or a digit never plays or steps the game.
 */
fun Modifier.plainShortcuts(enabled: Boolean = true, onShortcut: (PlainShortcut) -> Unit): Modifier =
    onKeyEvent { event ->
        val shortcut = if (enabled && !TextEntryFocus.active) plainShortcutFor(event) else null
        if (shortcut == null) false else { onShortcut(shortcut); true }
    }
