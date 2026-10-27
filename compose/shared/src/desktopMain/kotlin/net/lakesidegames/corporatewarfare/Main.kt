package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPlacement
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import java.io.File

/** Where the desktop game keeps its saves: the per-user application data folder of each OS. */
private fun saveDirectory(): File {
    val os = System.getProperty("os.name").lowercase()
    val home = System.getProperty("user.home")
    return when {
        os.contains("win") -> File(System.getenv("APPDATA") ?: "$home\\AppData\\Roaming", "Corporate Warfare")
        os.contains("mac") -> File(home, "Library/Application Support/Corporate Warfare")
        else -> File(System.getenv("XDG_DATA_HOME") ?: "$home/.local/share", "corporate-warfare")
    }
}

fun main() = application {
    val settings = remember { AppSettings(File(saveDirectory(), "settings.properties")) }
    val state = rememberWindowState(
        size = DpSize(settings.resolution.first.dp, settings.resolution.second.dp),
        placement = settings.windowMode.placement(),
    )
    // Window settings apply as soon as they change on the settings page.
    LaunchedEffect(settings.windowMode, settings.resolution) {
        state.placement = settings.windowMode.placement()
        if (settings.windowMode == WindowMode.Windowed) {
            state.size = DpSize(settings.resolution.first.dp, settings.resolution.second.dp)
        }
    }
    Window(
        onCloseRequest = ::exitApplication,
        title = "Corporate Warfare",
        state = state,
    ) {
        val scope = rememberCoroutineScope()
        val store = remember { GameStore(saveDirectory(), scope) }
        CorporateWarfareApp(store, settings, onQuit = ::exitApplication)
    }
}

private fun WindowMode.placement(): WindowPlacement = when (this) {
    WindowMode.Windowed -> WindowPlacement.Floating
    WindowMode.Maximized -> WindowPlacement.Maximized
    WindowMode.Fullscreen -> WindowPlacement.Fullscreen
}
