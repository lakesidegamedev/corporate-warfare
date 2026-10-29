package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Arrangement
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
 * The cast's portraits in the game: a letter, the paper, a dossier, the hire list and the CEO Office,
 * from a real Great Lakes 2000 career with a raid in it. Set CW_MAP_RENDER_DIR to keep the pictures.
 */
class PortraitsRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun career(scenario: String, week: UInt, raid: Boolean = false): GameStore {
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
                while (guard++ < 200 && h.overview().tick < week) {
                    // Answer waiting decisions the way a player would, then run on.
                    for (e in h.events().filter { it.mandatory }) {
                        e.choices.firstOrNull()?.let { c -> h.submitJson(CommandJson.command("RespondEvent", "event" to e.id, "choice" to c.id)) }
                    }
                    h.dismissPause(); h.advance(1u)
                }
                if (raid) raidARival(h)
                h.overview().tick
            }
        }!!
        store.refresh()
        val settled = System.currentTimeMillis() + 30_000
        while (((store.overview?.tick ?: 0u) < finalTick || store.busy) && System.currentTimeMillis() < settled) Thread.sleep(100)
        assertTrue((store.overview?.tick ?: 0u) >= finalTick, "the store caught up to week $finalTick")
        return store
    }

    /** The player raids the first rival market the rules allow, then a week passes. */
    private fun raidARival(h: uniffi.cw_ffi.SessionHandle) {
        val me = h.overview().primaryFirm ?: return
        val plants = h.mapPlants()
        val mine = plants.filter { it.player }.map { it.region to it.industry }.toSet()
        println("player markets: $mine")
        for (plant in plants.filter { !it.player && (it.region to it.industry) in mine }) {
            val check = h.raidCheck(me, plant.firm, plant.region, plant.industry)
            val option = check.option
            println("raid check ${plant.firmName} ${plant.region} ${plant.industry}: ${check.kind} raidable=${check.raidable} ready=${option?.readyWeek} now=${h.overview().tick}")
            if (option == null || !check.raidable) continue
            val json = CommandJson.command(
                "RaidMarket", "firm" to me, "target" to option.target, "region" to option.regionIndex,
                "industry" to option.industryIndex, "budget_cents" to option.suggestedBudgetCents,
            )
            val receipt = h.submitJson(json)
            println("raid receipt accepted=${receipt.accepted} ${receipt.errorKey} ${receipt.errorDetail}")
            if (receipt.accepted) {
                repeat(3) { h.dismissPause(); h.advance(1u) }
                println("raids: ${h.recentRaids().size}")
                return
            }
        }
    }

    private fun render(name: String, width: Int, height: Int, content: @androidx.compose.runtime.Composable () -> Unit) {
        val out = (System.getenv("CW_MAP_RENDER_DIR") ?: "/tmp/cw-portraits-renders").let(::File).also { it.mkdirs() }
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


    @Test
    fun theCastShowsUpWithMoods() {
        // Every person of the atlas has three moods at 32 and at 16, the chiefs an office portrait.
        assertTrue(CastPortraits.all.size >= 136, "the atlas has the cast and every generated chief")
        assertTrue(CastPortraits.all.values.all { it.expressions.keys == setOf("neutral", "pleased", "displeased") && it.small.size == 3 })
        assertTrue(CastPortraits.office.size >= 16, "office portraits for the named chiefs")

        val store = career("great-lakes-2000", 80u, raid = true)
        val letter = store.letters.firstOrNull { it.kind == "rival" } ?: store.letters.first()
        println("letter from ${letter.sender.name} mood ${letter.sender.mood}")
        render("letter-desktop", 760, 700) { LetterPaper(store, letter) }
        render("letter-phone", 412, 900) { LetterPaper(store, letter) }

        val paper = kotlinx.coroutines.runBlocking { store.read { it.paper(it.overview().tick) } }!!
        val lead = paper.lead!!
        println("paper lead by ${lead.authorName} (${lead.author}) mood ${lead.mood}")
        assertTrue(lead.author.isNotEmpty() && lead.authorName.isNotEmpty(), "the scenario's journalist signs the paper")
        render("paper-desktop", 1400, 900) { Column { TickerBar(store) {}; PaperScreen(store) } }
        render("paper-phone", 412, 1500) { Column { TickerBar(store) {}; PaperScreen(store) } }

        val dossiers = kotlinx.coroutines.runBlocking {
            store.read { h -> store.rivals.mapNotNull { h.dossier(it.tag.firm) }.filter { it.ceo != null }.take(3) }
        }!!
        assertTrue(dossiers.isNotEmpty(), "rival dossiers with a chief")
        println("dossier moods: " + dossiers.joinToString { "${it.ceo!!.name}=${it.ceo!!.mood} office=${it.ceo!!.officePortrait}" })
        val currency = store.firms.firstOrNull()?.currency.orEmpty()
        for ((label, width) in listOf("desktop" to 520, "phone" to 412)) {
            render("dossier-$label", width, 1500) {
                Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    dossiers.forEach { Panel { DossierBody(it, currency) } }
                }
            }
        }

        val me = store.selectedFirmDto?.handle ?: store.firms.first().handle
        val exec = kotlinx.coroutines.runBlocking { store.read { it.executives(me) } }!!
        val people = exec.candidates.flatMap { it.people }
        val named = people.filter { it.named }
        println("hire list named: " + named.joinToString { "${it.name} (${it.hireNote})" })
        assertTrue(named.isNotEmpty() && named.all { it.hireNote.isNotEmpty() }, "the scenario's named people for hire are in the pool")
        val list = named + people.filter { !it.named }.take(2)
        for ((label, width) in listOf("desktop" to 520, "phone" to 412)) {
            render("executives-$label", width, 1000) {
                Box(Modifier.padding(12.dp)) { Panel { Eyebrow("Hire"); HireListBody(list, currency, false) {} } }
            }
        }

        render("ceo-office-desktop", 1400, 1400) { OfficeScreen(store) }
        render("ceo-office-phone", 412, 1800) { OfficeScreen(store) }
    }
}
