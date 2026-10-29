package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
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
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * Board goals, chapters and the Hall of Careers from a real Great Lakes 2000 career: the offer in
 * week 2, a choice through the store, then past the end of the first year. Set CW_MAP_RENDER_DIR
 * to keep the pictures.
 */
class ChaptersRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun open(): GameStore {
        val dir = kotlin.io.path.createTempDirectory("cwchapters").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = true, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical", timeline = "historical",
            ),
        )
        waitFor { store.ready }
        return store
    }

    private fun waitFor(what: () -> Boolean) {
        val deadline = System.currentTimeMillis() + 30_000
        while (!what() && System.currentTimeMillis() < deadline) Thread.sleep(50)
        assertTrue(what(), "settled in time")
    }

    private fun playTo(store: GameStore, week: UInt) {
        kotlinx.coroutines.runBlocking {
            store.read { h ->
                var guard = 0
                while (guard++ < 400 && h.overview().tick < week) {
                    for (e in h.events().filter { it.mandatory }) {
                        e.choices.firstOrNull()?.let { c -> h.submitJson(CommandJson.command("RespondEvent", "event" to e.id, "choice" to c.id)) }
                    }
                    h.dismissPause(); h.advance(1u)
                }
            }
        }
        store.refresh()
        waitFor { (store.overview?.tick ?: 0u) >= week && !store.busy }
        store.refreshHall()
        waitFor { (store.hall?.weeks ?: 0u) >= week }
    }

    private fun render(name: String, width: Int, height: Int, settings: AppSettings, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-chapters-renders").let(::File).also { it.mkdirs() }
        val scene = ImageComposeScene(width, height, Density(1f)) {
            CorporateWarfareTheme {
                androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides settings) {
                    // An opaque ground, so the picture reads on any viewer.
                    Box(Modifier.fillMaxSize().background(Palette.ink)) { content() }
                }
            }
        }
        scene.render(0L)
        Thread.sleep(300)
        scene.render(1_000_000_000L)
        val image = scene.render(3_000_000_000L)
        scene.close()
        File(out, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes)
    }

    @Test
    fun chooseSendsTheRightCommand() {
        assertEquals(
            "{\"ChooseMandate\":{\"firm\":42,\"pick\":1}}",
            CommandJson.command("ChooseMandate", "firm" to 42uL, "pick" to 1),
        )
        val store = open()
        playTo(store, 2u)
        val offer = assertNotNull(store.boardOffer, "the board offers goals in the first weeks")
        assertEquals(3, offer.offered.size)
        assertTrue(offer.offerWeeksLeft > 0u)
        assertTrue(offer.offered.all { it.title.isNotBlank() && it.detail.isNotBlank() })
        store.chooseMandate(offer.firm, 1)
        waitFor { store.activeMandate != null }
        val (chapter, mandate) = store.activeMandate!!
        assertEquals(1u, chapter.number)
        assertEquals(offer.offered[1].key, mandate.key)
        assertEquals(null, store.boardOffer, "a taken offer is no longer on the table")
    }

    @Test
    fun theOfferTheGoalAndTheHallRender() {
        val settings = AppSettings(File(kotlin.io.path.createTempDirectory("cwset").toFile(), "settings.properties"))
        settings.recordCareer(CareerSummary("Lakeshore Holdings", "Great Lakes 2000", 312, 6, 4, 2, 7))
        settings.recordCareer(CareerSummary("Prairie Combine", "Texas 1981", 104, 2, 1, 1, 3))
        val store = open()
        playTo(store, 2u)
        val offer = assertNotNull(store.boardOffer)
        render("offer-desktop", 1280, 560, settings) { Column(Modifier.padding(12.dp)) { BoardOfferCard(store, offer) } }
        render("offer-phone", 412, 1100, settings) { Column(Modifier.padding(12.dp)) { BoardOfferCard(store, offer) } }
        store.chooseMandate(offer.firm, 0)
        waitFor { store.activeMandate != null }
        playTo(store, 20u)
        val (chapter, mandate) = assertNotNull(store.activeMandate)
        render("goal-desktop", 640, 220, settings) { Column(Modifier.padding(12.dp)) { MandateStrip(store, chapter, mandate) } }
        render("goal-phone", 412, 240, settings) { Column(Modifier.padding(12.dp)) { MandateStrip(store, chapter, mandate) } }

        playTo(store, 54u)
        val hall = assertNotNull(store.hall)
        assertTrue(hall.chapters.size >= 2, "chapter 2 has opened")
        val first = hall.chapters.first()
        assertEquals("closed", first.status)
        assertTrue(first.outcome == "met" || first.outcome == "missed", "the chosen goal was settled: ${first.outcome}")
        assertNotNull(store.chapterResult ?: first, "a closed result is available")
        render("result-desktop", 700, 220, settings) { Column(Modifier.padding(12.dp)) { ChapterResultPanel(first, {}) } }
        render("hall-desktop", 1280, 900, settings) { HallScreen(store, settings) }
        render("hall-phone", 412, 1500, settings) { HallScreen(store, settings) }
        render("menu-careers-phone", 412, 700, settings) { CareerListScreen(settings.careers) {} }
        println("chapters: " + hall.chapters.joinToString { "${it.name} ${it.outcome}" })
    }

    @Test
    fun careerSummariesSurviveTheSettingsFile() {
        val file = File(kotlin.io.path.createTempDirectory("cwset").toFile(), "settings.properties")
        val a = AppSettings(file)
        a.recordCareer(CareerSummary("One", "Texas 1981", 60, 1, 1, 0, 2))
        a.recordCareer(CareerSummary("One", "Texas 1981", 120, 2, 1, 1, 3))
        a.recordCareer(CareerSummary("Two", "Great Lakes 2000", 30, 0, 0, 0, 0))
        val b = AppSettings(file)
        assertEquals(listOf("Two", "One"), b.careers.map { it.title })
        assertEquals(120, b.careers.last().weeks, "leaving a career again replaces its line")
    }
}
