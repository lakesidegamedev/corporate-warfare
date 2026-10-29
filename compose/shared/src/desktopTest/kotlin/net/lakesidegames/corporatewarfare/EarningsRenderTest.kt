package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import uniffi.cw_ffi.EarningsDto
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Earnings season screens: the section on a company page before results with guidance given, after
 * a beat and after a miss, the guidance dialog's words, and a real career's stock board data at
 * week 40. Set CW_MAP_RENDER_DIR to keep the pictures.
 */
class EarningsRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun sample(
        hasLast: Boolean = false, guided: Boolean = false, beat: Boolean = true, weeks: UInt = 5u, rating: String = "hold",
    ) = EarningsDto(
        covered = true, rating = rating, hasEstimate = true, estimateCents = 4_200_000_00L, weeksToResults = weeks,
        reportsThisWeek = weeks <= 1u, hasGuidance = guided, guideLowCents = 4_000_000_00L, guideHighCents = 4_500_000_00L,
        credibilityBps = if (guided) 4_500 else 3_000, canGuide = !guided, hasLast = hasLast, lastWeek = 26u,
        lastActualCents = if (beat) 4_600_000_00L else 3_300_000_00L, lastEstimateCents = 4_000_000_00L,
        lastMoveBps = if (beat) 420 else -610, lastGuidanceMet = if (hasLast && guided) beat else null, streak = if (beat) 2 else -1,
    )

    private fun render(name: String, width: Int, height: Int, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-earnings-renders").let(::File).also { it.mkdirs() }
        val settings = AppSettings(File(kotlin.io.path.createTempDirectory("cwset").toFile(), "settings.properties"))
        val scene = ImageComposeScene(width, height, Density(1f)) {
            CorporateWarfareTheme {
                androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides settings) {
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
    fun theSectionAndTheGuidanceFormRender() {
        val cases = mapOf(
            "before-results" to sample(rating = "buy"),
            "before-results-guided" to sample(guided = true, weeks = 1u),
            "after-a-beat" to sample(hasLast = true, guided = true, beat = true, rating = "hold"),
            "after-a-miss" to sample(hasLast = true, guided = true, beat = false, rating = "sell"),
        )
        for ((name, dto) in cases) {
            for ((suffix, width) in listOf("desktop" to 640, "phone" to 380)) {
                render("earnings-$name-$suffix", width, 360) {
                    Box(Modifier.padding(12.dp)) { Panel { EarningsSection(dto, if (dto.canGuide) ({}) else null) } }
                }
            }
        }
        render("earnings-guidance-form", 520, 460) {
            var low by remember { mutableStateOf("3990000") }
            var high by remember { mutableStateOf("4410000") }
            Box(Modifier.padding(12.dp)) { Panel { GuidanceBody(sample(), low, { low = it }, high, { high = it }) } }
        }
        assertEquals("Reports this week", resultsWhen(sample(weeks = 1u)))
        assertEquals("Reports in 5 weeks", resultsWhen(sample()))
        assertEquals("Sell", ratingText("sell"))
        assertEquals("Low", credibilityText(3_000))
        assertEquals("High", credibilityText(9_000))
    }

    @Test
    fun aRealCareersStockBoardCarriesTheSeason() {
        val dir = kotlin.io.path.createTempDirectory("cwearn").toFile()
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
                var guard = 0
                while (guard++ < 300 && h.overview().tick < 40u) {
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
        val covered = store.stocks.filter { it.earnings.covered }
        assertTrue(covered.size >= 5, "analysts cover the listed companies")
        val line = covered.firstOrNull { it.earnings.hasLast && it.earnings.hasEstimate }
        assertTrue(line != null, "a company has reported and has a new estimate")
        println("stock ${line!!.name}: ${line.earnings.rating}, expects ${line.earnings.estimateCents}, last ${line.earnings.lastActualCents} against ${line.earnings.lastEstimateCents}")
        render("earnings-real-career-desktop", 640, 360) { Box(Modifier.padding(12.dp)) { Panel { EarningsSection(line, null) } } }
        render("earnings-real-career-phone", 380, 420) { Box(Modifier.padding(12.dp)) { Panel { EarningsSection(line, null) } } }
    }
}
