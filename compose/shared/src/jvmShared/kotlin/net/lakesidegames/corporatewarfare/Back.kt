package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.rememberUpdatedState

/**
 * System back, one level at a time. A screen, step or overlay that back should close registers
 * with [BackAction] while it is shown; the deepest one shown handles the press. Android routes
 * its back gesture here, and with nothing registered back leaves the app as usual. Dialogs close
 * on back by themselves.
 */
object Back {
    private val handlers = mutableStateListOf<() -> Unit>()

    /** Handles a back press; false when nothing is open and the platform should act. */
    fun press(): Boolean {
        val top = handlers.lastOrNull() ?: return false
        top()
        return true
    }

    internal fun add(handler: () -> Unit) { handlers += handler }
    internal fun remove(handler: () -> Unit) { handlers.remove(handler) }
}

/** While this is in the composition and [enabled], back runs [onBack]. */
@Composable
fun BackAction(enabled: Boolean = true, onBack: () -> Unit) {
    val current by rememberUpdatedState(onBack)
    DisposableEffect(enabled) {
        val handler = { current() }
        if (enabled) Back.add(handler)
        onDispose { Back.remove(handler) }
    }
}
