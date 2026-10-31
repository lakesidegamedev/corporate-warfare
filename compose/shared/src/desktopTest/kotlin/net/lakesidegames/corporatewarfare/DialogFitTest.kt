package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.background
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import java.io.File
import kotlin.test.Test
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * No control out of reach: every dialog is drawn in a short desktop window and a phone-sized one. A tall dialog may fill
 * the window, but its buttons must still show at the bottom; a body that does not scroll pushes them off, which is how the
 * stock dialog once cut off its Buy now button. Pictures go to CW_DIALOG_FIT_DIR.
 */
class DialogFitTest {
    init {
        Images.decode = { bytes -> Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private val out = System.getenv("CW_DIALOG_FIT_DIR")?.let(::File)?.also { it.mkdirs() }
    private val panel = Palette.panel.toArgb()

    /** Draws [content] and returns the problems found: the panel touching an edge, or not drawn at all. */
    private fun fit(name: String, settings: AppSettings, width: Int, height: Int, density: Float, content: @androidx.compose.runtime.Composable () -> Unit): String? {
        val scene = ImageComposeScene(width, height, Density(density)) {
            CorporateWarfareTheme { CompositionLocalProvider(LocalAppSettings provides settings) { Box(Modifier.fillMaxSize().background(Palette.ink)); content() } }
        }
        scene.render(0L)
        var image = scene.render(500_000_000L)
        for (i in 1..8) { Thread.sleep(50); image = scene.render(500_000_000L + i * 50_000_000L) }
        scene.close()
        out?.let { File(it, "$name-${width}x$height.jpg").writeBytes(image.encodeToData(EncodedImageFormat.JPEG, 80)!!.bytes) }
        val bmp = Bitmap.makeFromImage(image)
        var panelRows = 0
        var touchTop = false
        var touchBottom = false
        for (y in 0 until bmp.height) {
            var hit = false
            for (x in 0 until bmp.width step 4) if (bmp.getColor(x, y) == panel) { hit = true; break }
            if (hit) {
                panelRows++
                if (y < 3) touchTop = true
                if (y >= bmp.height - 3) touchBottom = true
            }
        }
        // A tall dialog may fill the window: that is fine while its body scrolls and its buttons stay on screen. The buttons
        // (Close, the gold action) sit in the panel's last rows, so when the panel reaches the bottom edge the gold of a button
        // must show in the bottom band of the window; if it does not, the buttons were pushed off screen.
        val gold = Palette.gold.toArgb()
        fun goldIn(rows: IntRange): Boolean {
            for (y in rows) for (x in 0 until bmp.width step 2) if (bmp.getColor(x, y) == gold) return true
            return false
        }
        val band = (bmp.height - 90).coerceAtLeast(0) until bmp.height
        return when {
            panelRows < 40 -> "$name at ${width}x$height: no dialog panel was drawn"
            touchBottom && !goldIn(band) -> "$name at ${width}x$height: the dialog reaches the bottom edge and no button shows there, so its buttons are out of reach"
            else -> null.also { if (touchTop && !touchBottom) println("$name at ${width}x$height: reaches the top edge only") }
        }
    }

    @Test
    fun everyDialogFitsAShortWindowAndAPhone() {
        val dir = kotlin.io.path.createTempDirectory("cwdialogfit").toFile()
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
        while ((store.stocks.isEmpty() || store.boards.isEmpty() || store.commodities.isEmpty()) && System.currentTimeMillis() < wait) Thread.sleep(100)
        val stock = assertNotNull(store.stocks.maxByOrNull { it.marketCapCents })
        val commodity = assertNotNull(store.commodities.firstOrNull())
        val board = assertNotNull(store.boards.firstOrNull())

        val cases = linkedMapOf<String, @androidx.compose.runtime.Composable () -> Unit>(
            "stock" to { StockDialog(store, stock, startQuantity = "10000") {} },
            "commodity" to { CommodityDialog(store, commodity) {} },
            "build" to { BuildDialog(store, board) {} },
            "region-sheet" to { RegionSheet(store, board, onBuild = {}, onClose = {}) },
            "region-picker" to { RegionPicker(store) {} },
            "rivals-board" to { RivalsBoardDialog(store) {} },
            "company-page" to { CompanyPageDialog(store, stock.firm) {} },
            "guidance" to { GuidanceDialog(stock.earnings, {}) { _, _ -> } },
            "game-menu" to { GameMenuDialog(onResume = {}, onSave = {}, onSettings = {}, onMainMenu = {}, onQuit = {}) },
        )
        store.productLines.firstOrNull()?.let { line -> cases["posture"] = { PostureDialog(store, line, presetBps = -500) {} } }
        store.divisions.firstOrNull()?.let { d -> cases["division"] = { DivisionDetailDialog(store, d) {} } }

        val problems = mutableListOf<String>()
        for ((name, content) in cases) {
            for ((w, h, d) in listOf(Triple(1280, 600, 1f), Triple(393, 700, 1f))) {
                fit(name, settings, w, h, d, content)?.let { problems += it }
            }
        }
        println("dialogs checked: ${cases.keys}")
        assertTrue(problems.isEmpty(), problems.joinToString("\n"))
    }
}
