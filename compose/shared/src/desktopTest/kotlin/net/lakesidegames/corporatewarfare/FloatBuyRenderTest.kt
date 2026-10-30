package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** The buy dialog with a live quote, from a real career; pictures go to CW_FLOAT_RENDER_DIR. */
class FloatBuyRenderTest {
    init {
        Images.decode = { bytes -> Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private val out = System.getenv("CW_FLOAT_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }

    private fun shot(name: String, settings: AppSettings, width: Int, height: Int, density: Float, content: @androidx.compose.runtime.Composable () -> Unit) {
        val scene = ImageComposeScene(width, height, Density(density)) {
            CorporateWarfareTheme { CompositionLocalProvider(LocalAppSettings provides settings) { content() } }
        }
        scene.render(0L)
        // The quote comes back from the engine thread; give it a few frames.
        var image = scene.render(500_000_000L)
        for (i in 1..20) { Thread.sleep(100); image = scene.render(500_000_000L + i * 100_000_000L) }
        scene.close()
        out?.let { File(it, "$name.jpg").writeBytes(image.encodeToData(EncodedImageFormat.JPEG, 88)!!.bytes) }
    }

    @Test
    fun buyDialogShowsALiveQuote() {
        val dir = kotlin.io.path.createTempDirectory("cwfloat").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(File(dir, "settings.properties"))
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical",
                chiefName = "Ada Marsh", chiefPortrait = "", chiefTemperament = "raider",
            ),
        )
        val deadline = System.currentTimeMillis() + 30_000
        while (!store.ready && System.currentTimeMillis() < deadline) Thread.sleep(100)
        assertTrue(store.ready)
        kotlinx.coroutines.runBlocking {
            store.read { h -> var g = 0; while (h.overview().tick < 20u && g++ < 40) { h.dismissPause(); h.advance(1u) } }
        }
        store.refresh()
        val wait = System.currentTimeMillis() + 15_000
        while (store.stocks.isEmpty() && System.currentTimeMillis() < wait) Thread.sleep(100)
        val line = assertNotNull(store.stocks.filter { it.floatBps.toLong() > 500L }.maxByOrNull { it.marketCapCents })
        // The quote is the engine's; ask it directly and check the shape.
        var quote: uniffi.cw_ffi.ShareQuoteDto? = null
        store.shareQuote(line.firm, true, 10_000, 0) { quote = it }
        val by = System.currentTimeMillis() + 10_000
        while (quote == null && System.currentTimeMillis() < by) Thread.sleep(50)
        val q = assertNotNull(quote)
        assertTrue(q.qty > 0 && q.finalCents >= q.markCents && q.averageCents >= q.markCents)
        assertTrue(kotlin.math.abs(q.totalCents.toLong() - q.averageCents.toLong() * q.qty.toLong()) <= q.qty.toLong(), "the total is the average times the shares, within a cent a share")
        println("quote: " + quoteText(q, true))
        shot("buy-dialog-desktop", settings, 1100, 900, 1f) { StockDialog(store, line, startQuantity = "10000") {} }
        shot("buy-dialog-phone", settings, 824, 1800, 2f) { StockDialog(store, line, startQuantity = "10000") {} }
    }
}
