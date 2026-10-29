package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
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
import kotlin.test.assertTrue

/**
 * The Executives section and the hire list from a real career: one post filled, two empty.
 * Pictures go to CW_MAP_RENDER_DIR.
 */
class ExecutivesRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun render(name: String, width: Int, height: Int, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-exec-renders").let(::File).also { it.mkdirs() }
        val settings = AppSettings(File(kotlin.io.path.createTempDirectory("cwset").toFile(), "settings.properties"))
        val scene = ImageComposeScene(width, height, Density(1f)) {
            CorporateWarfareTheme {
                androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides settings) {
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
    fun theExecutivesSectionAndTheHireListRender() {
        val dir = kotlin.io.path.createTempDirectory("cwexec").toFile()
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
        val data = kotlinx.coroutines.runBlocking {
            store.read { h ->
                val me = h.overview().primaryFirm!!
                var guard = 0
                while (guard++ < 40 && h.overview().tick < 8u) {
                    for (e in h.events().filter { it.mandatory }) {
                        e.choices.firstOrNull()?.let { c -> h.submitJson(CommandJson.command("RespondEvent", "event" to e.id, "choice" to c.id)) }
                    }
                    h.dismissPause(); h.advance(1u)
                }
                var d = h.executives(me)
                val cfo = d.candidates.first { it.postCode == 0u }.people.firstOrNull { it.affordable }
                if (cfo != null) h.submitJson(CommandJson.command("HireExecutive", "firm" to me, "post" to 0, "person" to cfo.id.toInt()))
                d = h.executives(me)
                d
            }
        }!!
        val currency = store.firms.firstOrNull()?.currency.orEmpty()
        assertTrue(data.posts.size == 3, "three posts")
        assertTrue(data.posts.any { it.holder != null }, "a chief financial officer was hired")
        val text = data.posts.joinToString(" ") { it.blurb + (it.holder?.effect ?: "") } + data.candidates.flatMap { it.people }.joinToString(" ") { it.effect }
        assertTrue(!text.contains("bps") && !text.contains('\u2013') && !text.contains('\u2014'), "plain words")
        val empty = data.posts.first { it.holder == null }
        val list = data.candidates.first { it.postCode == empty.postCode }.people.take(3)
        assertTrue(list.isNotEmpty(), "candidates to hire")
        for ((label, width) in listOf("desktop" to 520, "phone" to 412)) {
            render("executives-section-$label", width, 560) {
                Box(Modifier.padding(12.dp)) { Panel { Eyebrow("Executives"); ExecutivesBody(data, currency, false, {}, {}) } }
            }
            render("executives-hire-list-$label", width, 760) {
                Box(Modifier.padding(12.dp)) { Panel { Eyebrow("Hire a ${empty.title.lowercase()}"); HireListBody(list, currency, false) {} } }
            }
        }
    }
}
