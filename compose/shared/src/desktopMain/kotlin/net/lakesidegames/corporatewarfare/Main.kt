package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
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
    Window(
        onCloseRequest = ::exitApplication,
        title = "Corporate Warfare",
        state = rememberWindowState(size = DpSize(480.dp, 900.dp)),
    ) {
        val scope = rememberCoroutineScope()
        val store = remember { GameStore(saveDirectory(), scope) }
        CorporateWarfareApp(store)
    }
}
