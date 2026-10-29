package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * The paper and its ticker, from real careers: Texas 1981 at week 60 (its timeline is Historical,
 * so its dated events are in the news), Texas at week 76, two weeks after the notice of the
 * 1982 bank failure, and Great Lakes 2000 at week 30. Set
 * CW_MAP_RENDER_DIR to keep the pictures.
 */
class PaperRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun career(scenario: String, week: UInt): GameStore {
        val dir = kotlin.io.path.createTempDirectory("cwpaper").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = scenario, guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical", timeline = "historical",
            ),
        )
        val deadline = System.currentTimeMillis() + 30_000
        while (!store.ready && System.currentTimeMillis() < deadline) Thread.sleep(100)
        assertTrue(store.ready, "the career opened")
        kotlinx.coroutines.runBlocking {
            store.read { h ->
                var guard = 0
                while (h.overview().tick < week && guard++ < 60) { h.dismissPause(); h.advance(4u) }
            }
        }
        store.refresh()
        val settled = System.currentTimeMillis() + 30_000
        while ((store.busy || (store.overview?.tick ?: 0u) < week) && System.currentTimeMillis() < settled) Thread.sleep(100)
        assertTrue((store.overview?.tick ?: 0u) >= week, "the store caught up to week $week")
        return store
    }

    private fun render(name: String, width: Int, height: Int, store: GameStore, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-paper-renders").let(::File).also { it.mkdirs() }
        val settings = AppSettings(File(kotlin.io.path.createTempDirectory("cwset").toFile(), "settings.properties"))
        val scene = ImageComposeScene(width, height, Density(1f)) {
            CorporateWarfareTheme {
                androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides settings) {
                    // An opaque ground, so the picture reads on any viewer.
                    Box(Modifier.fillMaxSize().background(Palette.ink)) { content() }
                }
            }
        }
        scene.render(0L)
        Thread.sleep(400)
        scene.render(1_000_000_000L)
        val image = scene.render(3_000_000_000L)
        scene.close()
        File(out, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes)
    }

    @Test
    fun thePaperAndTickerRender() {
        for ((scenario, week, label) in listOf(Triple("texas-1981", 60u, "texas-w60"), Triple("texas-1981", 76u, "texas-w76"), Triple("great-lakes-2000", 30u, "great-lakes-w30"))) {
            val store = career(scenario, week)
            val paper = kotlinx.coroutines.runBlocking { store.read { it.paper(it.overview().tick) } }!!
            val ticker = kotlinx.coroutines.runBlocking { store.read { it.ticker() } }!!
            assertTrue(paper.masthead.isNotEmpty() && paper.markets.open, "$label has a masthead and closing prices")
            assertTrue(ticker.size <= 3)
            for (story in listOfNotNull(paper.lead) + paper.briefs) assertTrue(story.headline.length <= 70)
            println("$label ${paper.masthead}: lead=${paper.lead?.headline} briefs=${paper.briefs.map { it.headline }} ticker=$ticker")
            render("paper-$label-desktop", 1400, 900, store) { Column { TickerBar(store) {}; PaperScreen(store) } }
            render("paper-$label-phone", 412, 1500, store) { Column { TickerBar(store) {}; PaperScreen(store) } }
        }
    }
}
