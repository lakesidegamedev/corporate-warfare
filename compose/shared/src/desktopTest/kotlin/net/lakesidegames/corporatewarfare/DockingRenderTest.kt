package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The map docked beside a page, a hover explanation, and the player's chief in a letter and on the
 * company page, from a real career. Pictures go to CW_MAP_RENDER_DIR as PNG plus a phone-width JPEG.
 */
class DockingRenderTest {
    init {
        Images.decode = { bytes -> Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }

    private fun save(name: String, content: @androidx.compose.runtime.Composable () -> Unit, width: Int, height: Int, hover: Offset? = null, settings: AppSettings) {
        val scene = ImageComposeScene(width, height, Density(1f)) {
            CorporateWarfareTheme { CompositionLocalProvider(LocalAppSettings provides settings) { content() } }
        }
        scene.render(0L)
        if (hover != null) {
            scene.sendPointerEvent(PointerEventType.Enter, Offset(5f, 5f))
            scene.render(100_000_000L)
            scene.sendPointerEvent(PointerEventType.Move, hover)
            scene.render(200_000_000L)
            scene.sendPointerEvent(PointerEventType.Move, hover + Offset(1f, 1f))
            scene.render(400_000_000L)
            scene.render(800_000_000L)
        }
        val image = scene.render(if (hover != null) 1_500_000_000L else 3_000_000_000L)
        scene.close()
        out?.let {
            File(it, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes)
            // A phone-width JPEG for quick looks.
            val small = org.jetbrains.skia.Surface.makeRasterN32Premul(412, (412L * height / width).toInt())
            small.canvas.drawImageRect(image, org.jetbrains.skia.Rect.makeWH(412f, small.height.toFloat()))
            File(it, "$name-phone.jpg").writeBytes(small.makeImageSnapshot().encodeToData(EncodedImageFormat.JPEG, 85)!!.bytes)
        }
    }

    @Test
    fun dockedMapTooltipAndChief() {
        ExplainTiming.delayMs = 0
        val dir = kotlin.io.path.createTempDirectory("cwdock").toFile()
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
            store.read { h -> var g = 0; while (h.overview().tick < 40u && g++ < 60) { h.dismissPause(); h.advance(1u) } }
        }
        store.refresh()
        val chiefBy = System.currentTimeMillis() + 15_000
        while (store.playerChief == null && System.currentTimeMillis() < chiefBy) Thread.sleep(100)
        val chief = assertNotNull(store.playerChief)
        assertEquals("Ada Marsh", chief.name)

        // The split: the map beside the Markets page, kept per page.
        settings.chooseDock("markets", 0.45f)
        assertEquals(0.45f, settings.docks["markets"])
        settings.chooseDock("office", 0.4f)
        save("dock-markets", { DesktopShell(store, onSettings = {}, startPage = 4) }, 1500, 860, settings = settings)
        save("dock-office", { DesktopShell(store, onSettings = {}, startPage = 1) }, 1500, 860, settings = settings)

        // A short window: the sidebar's pages scroll and Settings, Game menu and Main menu stay in view; and the Escape menu.
        save("sidebar-short-window", { DesktopShell(store, onSettings = {}, startPage = 1) }, 1300, 560, settings = settings)
        save("game-menu", { GameMenuDialog(onResume = {}, onSave = {}, onSettings = {}, onMainMenu = {}, onQuit = {}) }, 900, 640, settings = settings)

        // A tooltip: rest the pointer on the Company cash figure in the status bar.
        save("tooltip-status-bar", { DesktopShell(store, onSettings = {}, startPage = 1) }, 1500, 860, hover = Offset(420f, 40f), settings = AppSettings(File(dir, "plain.properties")))

        // The chief in a letter (any letter to the company) and on the company page.
        val letters = store.letters
        val letter = letters.firstOrNull { it.salutation.contains(chief.name) } ?: letters.firstOrNull()
        if (letter != null) {
            println("letter salutation: ${letter.salutation} (names the chief: ${letter.salutation.contains(chief.name)})")
            save("chief-letter", { Box(Modifier.fillMaxSize().background(Palette.ink).padding(16.dp)) { LetterPaper(store, letter, Modifier.width(620.dp)) } }, 700, 700, settings = settings)
        }
        // Font scaling at 130 percent on the phone layout: nothing should clip.
        val phone = AppSettings(File(dir, "phone.properties")).apply { finishSetup(); chooseTutorial(emptySet(), true) }
        for (tab in listOf("app")) {
            val scene = ImageComposeScene(824, 1800, Density(2.0f, fontScale = 1.3f)) {
                CorporateWarfareTheme { CompositionLocalProvider(LocalAppSettings provides phone) { CorporateWarfareApp(store, phone) } }
            }
            scene.render(0L)
            val image = scene.render(3_000_000_000L)
            scene.close()
            out?.let { File(it, "phone-font130-$tab.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
        }
        var page: uniffi.cw_ffi.CompanyPageDto? = null
        store.companyPage(store.selectedFirm!!) { page = it }
        val wait = System.currentTimeMillis() + 10_000
        while (page == null && System.currentTimeMillis() < wait) Thread.sleep(50)
        val p = assertNotNull(page)
        save("chief-company-page", { Box(Modifier.fillMaxSize().background(Palette.ink).padding(16.dp)) { Box(Modifier.width(520.dp)) { CompanyPageBody(store, p, {}, null) } } }, 600, 900, settings = settings)
    }

    @Test
    fun explanationsArePlainEnglish() {
        val banned = listOf("bps", "basis point", "ppm", "dto")
        for ((label, text) in Words.all()) {
            assertTrue(banned.none { text.lowercase().contains(it) }, "jargon in $label: $text")
            assertTrue(!text.contains('–') && !text.contains('—'), "dash in $label")
            assertTrue(text.length in 15..260, "length of $label")
        }
        assertNotNull(Words.about("Company cash"))
        assertNotNull(Words.about("Profit, 5 wk"))
    }

    @Test
    fun chartSummariesSayLastValueAndTrend() {
        assertTrue(chartSummary("Revenue", listOf(100, 120, 150)).contains("rising"))
        assertTrue(chartSummary("Revenue", listOf(150, 120, 100)).contains("falling"))
        assertTrue(chartSummary("Cash", listOf(1000, 1005, 1000)).contains("flat"))
        assertTrue(chartSummary("Cash", emptyList()).contains("no data"))
    }
}
