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
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp

/**
 * The History card from a Texas 1981 career at week 76 (a bank failure announced) and the
 * new-game choice for scenario history, on desktop and phone. Set CW_MAP_RENDER_DIR to keep the pictures.
 */
class TimelineRenderTest {
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
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-timeline-renders").let(::File).also { it.mkdirs() }
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
    fun theChoiceMapsToWhatTheEnginePlays() {
        assertEquals(listOf("historical", "off", "shuffled"), ScenarioHistory.entries.map { it.key })
        assertEquals("historical", ScenarioHistory.entries.first().key, "Historical is the default")
        for (choice in ScenarioHistory.entries) {
            val h = uniffi.cw_ffi.SessionHandle.start(
                uniffi.cw_ffi.NewGameDto(
                    seed = 7uL, scenarioId = "texas-1981", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                    startingCapitalCents = null, pricePath = "historical", timeline = choice.key,
                ),
            )
            assertEquals(choice.key, h.timeline().mode)
        }
    }

    @Test
    fun theHistoryCardAndTheNewGameStepRender() {
        val store = career("texas-1981", 76u)
        val view = kotlinx.coroutines.runBlocking {
            store.read { h -> TimelineView(h.timeline(), h.paperArchive().map { it.toInt() }.toSet()) }
        }!!
        println("history: upcoming=${view.timeline.upcoming.map { it.title to it.weeksAway }} past=${view.timeline.past.map { it.title }}")
        assertTrue(view.timeline.upcoming.isNotEmpty(), "an announced event shows at week 76")
        render("history-desktop", 900, 700, store) { Column(Modifier.padding(16.dp)) { Panel { TimelineCard(view) {} } } }
        render("history-phone", 412, 700, store) { Column(Modifier.padding(12.dp)) { Panel { TimelineCard(view) {} } } }
        val off = view.copy(timeline = view.timeline.copy(mode = "off", upcoming = emptyList(), past = emptyList()))
        render("history-off-phone", 412, 260, store) { Column(Modifier.padding(12.dp)) { Panel { TimelineCard(off) {} } } }
        render("newgame-history-desktop", 1400, 1350, store) { NewGameScreen(store, startStep = 1) {} }
        render("newgame-history-phone", 412, 2600, store) { NewGameScreen(store, startStep = 1) {} }
    }
}
