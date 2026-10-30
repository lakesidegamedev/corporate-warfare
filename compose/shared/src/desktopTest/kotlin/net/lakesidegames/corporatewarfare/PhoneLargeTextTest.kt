package net.lakesidegames.corporatewarfare

import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * Every main phone tab and full screen drawn on a phone-sized screen with the system font scale at 130 percent, the size
 * a large-text user runs. Set CW_PHONE_RENDER_DIR to keep a JPEG of each screen for a human look at clipping and wrapping.
 */
class PhoneLargeTextTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    @Test
    fun everyPhoneScreenRendersAtLargeText() {
        val out = System.getenv("CW_PHONE_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val scale = System.getenv("CW_PHONE_FONT_SCALE")?.toFloatOrNull() ?: 1.3f
        val dir = kotlin.io.path.createTempDirectory("cwphone").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(File(dir, "settings.properties")).apply { finishSetup() }
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical",
            ),
        )
        val screens = listOf("map", "office", "markets", "inbox", "portfolio", "actions", "research", "stats", "paper", "hall")
        // 1080 x 2400 pixels at 2.625 is a 411 x 914 dp phone.
        val scene = ImageComposeScene(1080, 2400, Density(2.625f, scale)) {
            CorporateWarfareTheme { CorporateWarfareApp(store, settings) }
        }
        try {
            var nanos = 0L
            fun frame(n: Int) { repeat(n) { nanos += 50_000_000L; scene.render(nanos) } }
            frame(4)
            for (page in screens) {
                // Back to the tabs, then to the screen asked for.
                Tutorial.requestedPage = "map"; frame(3)
                Tutorial.requestedPage = page; frame(6)
                val image = scene.render(nanos + 1_000_000_000L)
                nanos += 1_000_000_000L
                val jpeg = image.encodeToData(EncodedImageFormat.JPEG, 82)!!.bytes
                assertTrue(jpeg.isNotEmpty(), "$page rendered")
                out?.let { File(it, "phone-$page-font${(scale * 100).toInt()}.jpg").writeBytes(jpeg) }
            }
        } finally {
            scene.close()
        }
    }
}
