package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp
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
 * The weekly brief from real careers: Great Lakes 2000 at week 30, and a company that sells below
 * cost until it loses money. Pictures go to CW_MAP_RENDER_DIR.
 */
class BriefRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun career(week: UInt, losing: Boolean): GameStore {
        val dir = kotlin.io.path.createTempDirectory("cwbrief").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical", timeline = "historical",
            ),
        )
        val deadline = System.currentTimeMillis() + 30_000
        while (!store.ready && System.currentTimeMillis() < deadline) Thread.sleep(100)
        assertTrue(store.ready, "the career opened")
        val finalTick = kotlinx.coroutines.runBlocking {
            store.read { h ->
                val me = h.overview().primaryFirm!!
                var guard = 0
                while (guard++ < 300 && h.overview().tick < week) {
                    for (e in h.events().filter { it.mandatory }) {
                        e.choices.firstOrNull()?.let { c -> h.submitJson(CommandJson.command("RespondEvent", "event" to e.id, "choice" to c.id)) }
                    }
                    if (losing && h.overview().tick == week - 10u) {
                        for (line in h.productLines(me)) {
                            h.submitJson(CommandJson.command("SetPricePosture", "firm" to me, "product" to line.product, "mode" to 1, "bps" to -5000))
                        }
                    }
                    h.dismissPause(); h.advance(1u)
                }
                h.overview().tick
            }
        }!!
        store.refresh()
        val settled = System.currentTimeMillis() + 30_000
        while (((store.overview?.tick ?: 0u) < finalTick || store.busy) && System.currentTimeMillis() < settled) Thread.sleep(100)
        store.briefOpen = true
        store.refreshBrief()
        val until = System.currentTimeMillis() + 15_000
        while (store.brief == null && System.currentTimeMillis() < until) Thread.sleep(100)
        return store
    }

    private fun render(name: String, width: Int, height: Int, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-brief-renders").let(::File).also { it.mkdirs() }
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

    private fun check(label: String, store: GameStore) {
        val brief = store.brief
        assertTrue(brief != null, "$label has a brief")
        println("$label week ${brief!!.week}: changes=${brief.changes.map { it.text }} due=${brief.due.map { it.text }} moves=${brief.moves.map { it.key }}")
        assertTrue(brief.changes.size <= 3 && brief.moves.size <= 3, "$label is within its limits")
        val text = (brief.changes.map { it.text } + brief.due.map { it.text } + brief.moves.map { it.detail }).joinToString(" ")
        assertTrue(!text.contains("bps") && !text.contains('\u2013') && !text.contains('\u2014'), "$label uses plain words")
        val currency = store.selectedFirmDto?.currency.orEmpty()
        render("brief-$label-desktop", 760, 900) { Box(Modifier.padding(12.dp)) { Panel { Eyebrow("This week's brief"); BriefBody(brief, currency) {} } } }
        render("brief-$label-phone", 412, 900) { Box(Modifier.padding(12.dp)) { Panel { Eyebrow("This week's brief"); BriefBody(brief, currency) {} } } }
    }

    @Test
    fun theBriefRendersForAThrivingAndALosingCompany() {
        check("great-lakes-week-30", career(30u, losing = false))
        check("losing", career(40u, losing = true))
    }
}
