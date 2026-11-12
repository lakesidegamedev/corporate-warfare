package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue

/**
 * Detail panes on a phone. A screen draws a grid with a [DetailPane] beside it; on a phone there is no room beside, so
 * the pane registers here instead of drawing, and the phone shell shows it as a page of its own (with a back arrow)
 * once a row of the grid is tapped.
 */
class PhonePanes {
    val entries = mutableStateListOf<PaneEntry>()
    var open by mutableStateOf(false)

    /** A row was tapped: open the pane, if the screen has one. */
    fun tap() { if (entries.isNotEmpty()) open = true }
}

class PaneEntry {
    var title by mutableStateOf("")
    var content by mutableStateOf<(@Composable ColumnScope.() -> Unit)?>(null)
}

/** Set by the phone shell. Null on a desktop and in the classic layout, where a pane draws where it is put. */
val LocalPhonePanes = compositionLocalOf<PhonePanes?> { null }

/** True inside a layout that already stacks the pane under its grid, so the pane stays where it is. */
val LocalStackedPane = compositionLocalOf { false }

/** The pane's slot on a phone: nothing is drawn here; the title and content wait for the shell to show them. */
@Composable
internal fun PhonePaneEntry(host: PhonePanes, title: String, content: @Composable ColumnScope.() -> Unit) {
    val entry = remember { PaneEntry() }
    SideEffect { entry.title = title; entry.content = content }
    DisposableEffect(host) {
        host.entries.add(entry)
        onDispose {
            host.entries.remove(entry)
            if (host.entries.isEmpty()) host.open = false
        }
    }
}
