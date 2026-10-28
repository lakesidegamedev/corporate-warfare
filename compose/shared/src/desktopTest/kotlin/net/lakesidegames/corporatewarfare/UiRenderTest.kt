package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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

/** The main menu, settings and the shared panels render in the 8-bit style. Set CW_MAP_RENDER_DIR to keep the pictures. */
class UiRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    @Test
    fun screensRender() {
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val dir = kotlin.io.path.createTempDirectory("cwui").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(File(dir, "settings.properties"))
        // The menu's badge flight and shine run on real-time delays and an endless frame loop that
        // can deadlock ImageComposeScene. The player's own Animations switch stops both and shows
        // the finished menu, so the menu renders with it off.
        val still = AppSettings(File(dir, "still.properties")).apply { chooseGraphics(graphics.copy(animations = false)) }
        val screens = mapOf<String, @androidx.compose.runtime.Composable () -> Unit>(
            "ui-menu" to { StartScreen(store, null, {}, {}) },
            "ui-settings" to { Box(Modifier.fillMaxSize().background(Palette.ink)) { SettingsScreen(store, settings, desktop = true) {} } },
            "ui-panels" to {
                Box(Modifier.fillMaxSize().background(Palette.ink).padding(24.dp)) {
                    Column(Modifier.width(520.dp)) {
                        ScreenHeader("CEO Office", "Week 12, 2000") { GoldButton("Next week") {} }
                        StatGroup(Stat("Cash", "$4.2M"), listOf(Stat("Profit", "$310K", Palette.positive), Stat("Debt", "$1.1M"), Stat("Plants", "14")))
                        Panel { Eyebrow("Divisions"); androidx.compose.material3.Text("Manufacturing, Central Ohio", color = Palette.primaryText) }
                    }
                }
            },
        )
        // A live career for the in-game desktop screens.
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical",
            ),
        )
        val deadline = System.currentTimeMillis() + 30_000
        while (!store.ready && System.currentTimeMillis() < deadline) Thread.sleep(100)
        val desktop = if (store.ready) (0 until 8).associate { page ->
            "ui-desktop-$page" to @androidx.compose.runtime.Composable { DesktopShell(store, onSettings = {}, startPage = page) }
        } else emptyMap()
        // Play two quarters on the engine thread, then render the financial tabs and charts.
        val finance = if (store.ready) kotlinx.coroutines.runBlocking {
            store.read { h ->
                var guard = 0
                while (h.overview().tick < 30u && guard++ < 40) { h.dismissPause(); h.advance(26u) }
                val me = h.overview().primaryFirm!!
                loadFinancials(h, me, h.overview().primaryName, h.firm(me)!!.industry) to h.goodPriceHistory(0u)
            }
        } else null
        val financeScreens = finance?.let { (data, good) ->
            check(data.rivals.isNotEmpty()) { "the test career should have a visible rival in the sector" }
            // Scrolling columns as in the game, so no chart is squeezed to fit the picture.
            fun page(width: Int, columns: Int, tab: FinanceTab) = @androidx.compose.runtime.Composable {
                Box(Modifier.fillMaxSize().background(Palette.ink).padding(24.dp)) {
                    Column(Modifier.width(width.dp).verticalScroll(rememberScrollState())) { CompanyFinancials(data, columns = columns, initialTab = tab) }
                }
            }
            FinanceTab.entries.associate { tab -> "ui-finance-${tab.name.lowercase()}" to page(760, 5, tab) } +
                ("ui-finance-overview-phone" to page(360, 3, FinanceTab.Overview)) + ("ui-chart" to @androidx.compose.runtime.Composable {
                Box(Modifier.fillMaxSize().background(Palette.ink).padding(24.dp)) {
                    Column(Modifier.width(640.dp)) {
                        Panel {
                            Chart(
                                listOf(
                                    ChartSeries("Good 0", Palette.gold, good.map { ChartPoint(it.week.toInt(), it.cents) }),
                                    ChartSeries("Share", Palette.positive, data.share.map { ChartPoint(it.week.toInt(), it.cents) }),
                                ),
                                title = "Closing prices", initialKind = ChartKind.Area, format = { Format.money(it) },
                            )
                        }
                    }
                }
            })
        }.orEmpty()
        for ((name, content) in screens + desktop + financeScreens) {
            val tall = name.startsWith("ui-finance")
            val scene = ImageComposeScene(1400, if (tall) 1700 else 860, Density(1f)) {
                CorporateWarfareTheme { androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides if (name == "ui-menu") still else settings) { content() } }
            }
            scene.render(0L)
            val image = scene.render(3_000_000_000L)
            scene.close()
            out?.let { File(it, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
        }
    }
}
