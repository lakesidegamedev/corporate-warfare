package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
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
 * The Inbox as correspondence, from real careers: Texas 1981 around week 76 (the bank failure
 * foreshadow) and Great Lakes 2000 played until a rival writes about a raid. Set
 * CW_MAP_RENDER_DIR to keep the pictures.
 */
class LettersRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun career(scenario: String, week: UInt, stopWhen: (uniffi.cw_ffi.SessionHandle) -> Boolean = { false }): GameStore {
        val dir = kotlin.io.path.createTempDirectory("cwletters").toFile()
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
        val finalTick = kotlinx.coroutines.runBlocking {
            store.read { h ->
                var guard = 0
                while (guard++ < 200 && h.overview().tick < week && !stopWhen(h)) {
                    // Answer waiting decisions the way a player would, then run on.
                    for (e in h.events().filter { it.mandatory }) {
                        e.choices.firstOrNull()?.let { c -> h.submitJson(CommandJson.command("RespondEvent", "event" to e.id, "choice" to c.id)) }
                    }
                    h.dismissPause(); h.advance(1u)
                }
                h.overview().tick
            }
        }!!
        store.refresh()
        val settled = System.currentTimeMillis() + 30_000
        while (((store.overview?.tick ?: 0u) < finalTick || store.busy) && System.currentTimeMillis() < settled) Thread.sleep(100)
        assertTrue((store.overview?.tick ?: 0u) >= finalTick, "the store caught up to week $finalTick")
        return store
    }

    private fun render(name: String, width: Int, height: Int, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-letters-renders").let(::File).also { it.mkdirs() }
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

    private fun check(label: String, store: GameStore, wantKind: String) {
        val letters = store.letters
        println("$label week ${store.overview?.tick}: " + letters.joinToString(" | ") { "${it.kind}:${it.sender.name}" })
        assertTrue(letters.isNotEmpty(), "$label has letters")
        letters.forEach { assertTrue(it.paragraphs.joinToString(" ").length <= 600, "${it.id} length") }
        val letter = letters.firstOrNull { it.kind == wantKind } ?: letters.first()
        render("inbox-$label-desktop", 1400, 900) { DesktopInbox(store, letter.id) }
        render("letter-$label-desktop", 760, 700) { LetterPaper(store, letter) }
        render("inbox-$label-phone", 412, 900) { LetterStack(store, null) {} }
        render("letter-$label-phone", 412, 900) { LetterPaper(store, letter) }
    }

    @Test
    fun theInboxAndALetterRender() {
        val texas = career("texas-1981", 76u)
        check("texas-w76", texas, "analyst")
        val lakes = career("great-lakes-2000", 150u) { h -> h.letters(1u).any { it.kind == "rival" } }
        check("great-lakes-raid", lakes, "rival")
    }
}
