package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import java.io.File

/**
 * One seeded mid-career game shared by the 2.0 interface renders (guided, so the career starts with a company): a fixed seed played to week 156
 * with a plant built every quarter, so every table has rows in it. CW_PROPOSAL_SAVE keeps the
 * save between runs so the before and after pictures come from exactly the same game.
 */
object ProposalCareer {
    const val WEEK = 156u

    fun init() {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
        ExplainTiming.delayMs = 0
    }

    fun open(): Pair<GameStore, AppSettings> {
        init()
        val dir = kotlin.io.path.createTempDirectory("cwproposal").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(File(dir, "settings.properties")).apply {
            finishSetup(); chooseTutorial(emptySet(), true); chooseGraphics(graphics.copy(animations = false))
        }
        val cached = System.getenv("CW_PROPOSAL_SAVE")?.let(::File)
        if (cached != null && cached.isFile) {
            store.loadSave(cached)
            waitReady(store)
        } else {
            store.startCareer(
                uniffi.cw_ffi.NewGameDto(
                    seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                    startingCapitalCents = null, pricePath = "historical",
                ),
            )
            waitReady(store)
            kotlinx.coroutines.runBlocking {
                store.read { h ->
                    val me = h.overview().primaryFirm!!
                    val industry = h.firm(me)!!.industry
                    val recipes = h.recipes().filter { it.industry == industry }
                    var guard = 0
                    while (h.overview().tick < WEEK && guard++ < 400) {
                        h.dismissPause()
                        // A plant a quarter in the next region along, when the company can afford it twice over.
                        if (h.overview().tick % 13u == 2u) {
                            val cash = h.firm(me)!!.cashCents
                            val boards = h.regionBoards(me)
                            val pick = boards.getOrNull((h.overview().tick / 13u).toInt() % boards.size.coerceAtLeast(1))
                            val recipe = recipes.getOrNull((h.overview().tick / 26u).toInt() % recipes.size.coerceAtLeast(1))
                            if (pick != null && recipe != null && cash > recipe.capexCents * 2) {
                                runCatching { h.buildFacility(me, pick.index, recipe.index, 1u) }
                            }
                        }
                        h.advance(1u)
                    }
                }
            }
            if (cached != null) kotlinx.coroutines.runBlocking { store.read { h -> cached.parentFile?.mkdirs(); cached.writeBytes(h.exportSave()) } }
            store.refresh()
        }
        // Wait for the store to read the week 156 state back before anything is drawn.
        val loaded = System.currentTimeMillis() + 30_000
        while ((store.overview?.tick ?: 0u) < WEEK && System.currentTimeMillis() < loaded) Thread.sleep(100)
        Thread.sleep(1500)
        return store to settings
    }

    private fun waitReady(store: GameStore) {
        val deadline = System.currentTimeMillis() + 60_000
        while (!store.ready && System.currentTimeMillis() < deadline) Thread.sleep(100)
        check(store.ready) { "the proposal career did not start" }
    }

    /** Render [content] at [width] x [height] logical pixels and keep it as [name].png in CW_MAP_RENDER_DIR. */
    fun shoot(name: String, width: Int, height: Int, settings: AppSettings, density: Float = 1f, content: @Composable () -> Unit) {
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val scene = ImageComposeScene((width * density).toInt(), (height * density).toInt(), Density(density)) {
            CorporateWarfareTheme {
                CompositionLocalProvider(LocalAppSettings provides settings) {
                    Box(Modifier.fillMaxSize().background(Palette.ink)) { content() }
                }
            }
        }
        // Screens read the engine on background threads: give them real time to arrive, then draw.
        scene.render(0L)
        Thread.sleep(1200)
        scene.render(1_000_000_000L)
        Thread.sleep(600)
        scene.render(2_000_000_000L)
        val image = scene.render(3_000_000_000L)
        scene.close()
        out?.let { File(it, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
    }
}
