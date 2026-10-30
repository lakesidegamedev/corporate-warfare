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
import uniffi.cw_ffi.StakeDto
import uniffi.cw_ffi.StakePointDto
import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * Disclosed stakes, the "Likely next move" line and net migration, on a real career's dossier and
 * region dossier with a stake history laid over them. Pictures go to CW_MAP_RENDER_DIR as PNG, and
 * the phone ones as JPEG too.
 */
class RivalPlansRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun render(name: String, width: Int, height: Int, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-plans-renders").let(::File).also { it.mkdirs() }
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
        if (name.endsWith("phone")) File(out, "$name.jpg").writeBytes(image.encodeToData(EncodedImageFormat.JPEG, 90)!!.bytes)
    }

    @Test
    fun stakesNextMoveAndMigrationRender() {
        val dir = kotlin.io.path.createTempDirectory("cwplans").toFile()
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
        val (rival, rivalName, dossier, region) = kotlinx.coroutines.runBlocking {
            store.read { h ->
                val me = h.overview().primaryFirm!!
                var guard = 0
                while (guard++ < 60 && h.overview().tick < 30u) {
                    for (e in h.events().filter { it.mandatory }) {
                        e.choices.firstOrNull()?.let { c -> h.submitJson(CommandJson.command("RespondEvent", "event" to e.id, "choice" to c.id)) }
                    }
                    h.dismissPause(); h.advance(1u)
                }
                val other = h.firms().first { it.handle != me }
                Quad(other.handle, other.name, h.dossier(other.handle)!!, h.regionDossier(0u, me)!!)
            }
        }!!
        // A real rival's dossier with a stake history laid over it: five weeks of buying.
        val history = listOf(520, 560, 610, 690, 760, 840, 930, 1_010).mapIndexed { i, bps -> StakePointDto(40u + i.toUInt(), bps) }
        val stake = StakeDto(
            holder = rival, holderName = rivalName, bps = 1_010, since = 40u, campaign = true, history = history,
        )
        val shown = dossier.copy(
            likelyNext = "Building a stake in $rivalName, now 10 percent and rising. A bid usually follows once it nears a quarter.",
            stakes = listOf(stake, stake.copy(holder = rival + 1uL, holderName = "Lakeshore Press Group", bps = 620, campaign = false, history = emptyList(), since = null)),
        )
        val quiet = dossier.copy(likelyNext = null, stakes = emptyList())
        val moved = region.copy(
            netMigrationYear = 214, netMigrationWeek = 5,
            series = region.series.map {
                if (it.key == "migration") it.copy(weeks = (40u..60u).toList(), values = (0..20).map { i -> (3 + (i * 7) % 9 - 2).toLong() }) else it
            },
        )
        for ((label, width) in listOf("desktop" to 560, "phone" to 412)) {
            render("rival-dossier-$label", width, 900) {
                Box(Modifier.padding(12.dp)) { Panel { Eyebrow("Who runs it"); DossierBody(shown) } }
            }
            render("rival-dossier-quiet-$label", width, 560) {
                Box(Modifier.padding(12.dp)) { Panel { Eyebrow("Who runs it"); DossierBody(quiet) } }
            }
            render("company-stakes-$label", width, 360) {
                Box(Modifier.padding(12.dp)) { Panel { DisclosedStakes(shown.stakes) } }
            }
            render("region-migration-$label", width, 1100) {
                Box(Modifier.padding(12.dp)) { Panel { RegionDossierView(moved) } }
            }
        }
        assertTrue(!shown.likelyNext!!.contains('—'), "plain words")
    }
}

private data class Quad<A, B, C, D>(val a: A, val b: B, val c: C, val d: D)
