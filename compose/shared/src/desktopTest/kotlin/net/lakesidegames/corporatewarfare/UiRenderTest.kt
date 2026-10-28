package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test

/** The main menu, settings and the shared panels render in the 8-bit style. Set CW_MAP_RENDER_DIR to keep the pictures. */
class UiRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    @Test
    fun screensRender() {
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val dir = kotlin.io.path.createTempDirectory("cwui").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(File(dir, "settings.properties"))
        val screens = mapOf<String, @androidx.compose.runtime.Composable () -> Unit>(
            "ui-menu" to { StartScreen(store, null, {}, {}) },
            "ui-settings" to { Box(Modifier.fillMaxSize().background(Palette.ink)) { SettingsScreen(store, settings, desktop = true) {} } },
            "ui-panels" to {
                Box(Modifier.fillMaxSize().background(Palette.ink).padding(24.dp)) {
                    Column(Modifier.width(520.dp)) {
                        ScreenHeader("CEO Office", "Week 12, 2000") { GoldButton("Next week") {} }
                        StatGroup(Stat("Cash", "$4.2M"), listOf(Stat("Profit", "$310K", Palette.positive), Stat("Debt", "$1.1M"), Stat("Plants", "14")))
                        Panel { Eyebrow("Divisions"); androidx.compose.material3.Text("Manufacturing, Central Ohio", color = Palette.primaryText) }
                    }
                }
            },
        )
        // A live career for the in-game desktop screens.
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical",
            ),
        )
        val deadline = System.currentTimeMillis() + 30_000
        while (!store.ready && System.currentTimeMillis() < deadline) Thread.sleep(100)
        val desktop = if (store.ready) (0 until 8).associate { page ->
            "ui-desktop-$page" to @androidx.compose.runtime.Composable { DesktopShell(store, onSettings = {}, startPage = page) }
        } else emptyMap()
        for ((name, content) in screens + desktop) {
            val scene = ImageComposeScene(1400, 860, Density(1f)) {
                CorporateWarfareTheme { androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides settings) { content() } }
            }
            scene.render(0L)
            val image = scene.render(3_000_000_000L)
            scene.close()
            out?.let { File(it, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
        }
    }
}
