package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
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
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The CEO Office as a room: every object routes to its screen, and a real Great Lakes career at
 * week 60 (letters, a mandate, hired executives, achievements) renders at desktop and phone sizes.
 * Pictures go to CW_MAP_RENDER_DIR.
 */
class OfficeRoomTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    @Test
    fun everyObjectInTheRoomRoutesToItsScreen() {
        val expected = mapOf(
            "window" to OfficeRoute.Screen("map"),
            "chart" to OfficeRoute.Screen("stats"),
            "cabinet" to OfficeRoute.Screen("stats"),
            "phone" to OfficeRoute.Screen("inbox"),
            "paper" to OfficeRoute.Screen("paper"),
            "board" to OfficeRoute.Screen("hall"),
            "trophies" to OfficeRoute.Screen("hall"),
            "door" to OfficeRoute.Screen("research"),
            "chief" to OfficeRoute.Screen("actions"),
            "nameplate" to OfficeRoute.Screen("actions"),
            "mandate" to OfficeRoute.Section(OfficeSection.Goal),
            "memo" to OfficeRoute.Section(OfficeSection.Brief),
            "safe" to OfficeRoute.Section(OfficeSection.Figures),
            "exec0" to OfficeRoute.Section(OfficeSection.Executives),
            "exec1" to OfficeRoute.Section(OfficeSection.Executives),
            "exec2" to OfficeRoute.Section(OfficeSection.Executives),
        )
        val objects = OfficeRoomArt.objects
        assertEquals(expected.keys, objects.map { it.id }.toSet(), "the room's objects are the ones that route")
        for (o in objects) {
            assertEquals(expected[o.id], officeRoute(o.id), "${o.id} routes to its screen")
            assertTrue(o.label.isNotBlank(), "${o.id} has a label for screen readers")
            val r = o.rect
            assertTrue(r[0] >= 0 && r[1] >= 0 && r[0] + r[2] <= OfficeRoomArt.width && r[1] + r[3] <= OfficeRoomArt.height, "${o.id} is inside the room")
        }
        // Every screen the room opens is one the shells handle.
        val screens = expected.values.filterIsInstance<OfficeRoute.Screen>().map { it.id }.toSet()
        assertEquals(setOf("map", "stats", "inbox", "paper", "hall", "research", "actions"), screens)
        assertEquals(null, officeRoute("nowhere"))
    }

    @Test
    fun theChiefReadsTheQuartersResults() {
        assertEquals("pleased", chiefMood(1L))
        assertEquals("displeased", chiefMood(-1L))
        assertEquals("neutral", chiefMood(0L))
        assertEquals("neutral", chiefMood(null))
    }

    private fun render(name: String, width: Int, height: Int, density: Float, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-office-renders").let(::File).also { it.mkdirs() }
        val settings = AppSettings(File(kotlin.io.path.createTempDirectory("cwset").toFile(), "settings.properties"))
        val scene = ImageComposeScene(width, height, Density(density)) {
            CorporateWarfareTheme {
                androidx.compose.runtime.CompositionLocalProvider(LocalAppSettings provides settings) {
                    // An opaque ground, so the picture reads on any viewer.
                    Box(Modifier.fillMaxSize().background(Palette.ink)) { content() }
                }
            }
        }
        scene.render(0L)
        Thread.sleep(600)
        scene.render(1_000_000_000L)
        val image = scene.render(3_000_000_000L)
        scene.close()
        File(out, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes)
        if (name.startsWith("office-phone")) File(out, "$name.jpg").writeBytes(image.encodeToData(EncodedImageFormat.JPEG, 88)!!.bytes)
    }

    private fun career(week: UInt): GameStore {
        val dir = kotlin.io.path.createTempDirectory("cwoffice").toFile()
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
                    // Take the board's goal when it is offered, and hire whoever the company can afford.
                    val offer = h.hallOfCareers().chapters.lastOrNull { it.status == "current" && it.chosen == null && it.offered.isNotEmpty() }
                    if (offer != null) h.submitJson(CommandJson.command("ChooseMandate", "firm" to me, "pick" to 0))
                    if (h.overview().tick % 6u == 0u) {
                        val d = h.executives(me)
                        for (post in d.posts.filter { it.holder == null }) {
                            val pick = d.candidates.firstOrNull { it.postCode == post.postCode }?.people?.firstOrNull { it.affordable } ?: continue
                            h.submitJson(CommandJson.command("HireExecutive", "firm" to me, "post" to post.postCode.toInt(), "person" to pick.id.toInt()))
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
        assertTrue((store.overview?.tick ?: 0u) >= finalTick, "the store caught up to week $finalTick")
        // The Office loads these while it is on screen; ask for them the same way.
        store.executivesOpen = true; store.briefOpen = true; store.hallOpen = true
        store.refreshExecutives(); store.refreshBrief(); store.refreshHall()
        store.headline = kotlinx.coroutines.runBlocking { store.read { it.ticker() } }?.firstOrNull()
        val loaded = System.currentTimeMillis() + 30_000
        while ((store.executives == null || store.brief == null || store.hall == null) && System.currentTimeMillis() < loaded) Thread.sleep(100)
        return store
    }

    @Test
    fun aGreatLakesCareerAtWeekSixtyRendersAsARoom() {
        val store = career(60u)
        val facts = assertNotNull(officeFacts(store), "the room has figures")
        println("office facts: unread=${facts.unread} mandate=${facts.mandateTitle} seats=${facts.seats.map { it.portrait }} trophies=${facts.trophiesEarned}/${facts.trophiesTotal} profit=${facts.profitText} chief=${facts.chief} headline=${facts.headline}")
        assertTrue(facts.unread > 0, "there are letters to read")
        assertTrue(facts.trophiesTotal > 0, "the hall is loaded")
        val currency = store.selectedFirmDto?.currency.orEmpty()

        render("office-desktop", 1400, 900, 1f) {
            Row(Modifier.fillMaxSize().padding(10.dp), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(10.dp)) {
                Column(Modifier.weight(1.1f)) { OfficeRoomHost(store) {} }
                Column(Modifier.weight(0.9f).verticalScroll(rememberScrollState()), verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(10.dp)) {
                    BriefPanel(store, currency)
                    ChapterBlock(store)
                    ExecutivesPanel(store, currency)
                }
            }
        }
        render("office-desktop-wide", 1920, 1080, 1f) {
            Row(Modifier.fillMaxSize().padding(10.dp), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(10.dp)) {
                Column(Modifier.weight(1.3f)) { OfficeRoomHost(store) {} }
                Column(Modifier.weight(0.7f)) { BriefPanel(store, currency) }
            }
        }
        render("office-phone", 1030, 3800, 2.5f) { OfficeScreen(store) }
        render("office-phone-room", 1030, 1000, 2.5f) { Column { OfficeRoomHost(store) {} } }
    }
}
