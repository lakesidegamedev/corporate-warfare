package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import uniffi.cw_ffi.SessionHandle
import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * Where the desktop app spends its time in play, in milliseconds, printed as a table. Run with
 * CW_PERF_REPORT=<file> (the table is also written there); without it the test does nothing.
 * Software Skia and a shared host, so the absolute numbers are higher and noisier than on a player's
 * machine: compare two runs of the same machine. It asserts only generous ceilings, never a ratio.
 *
 *  - "fetch" is what one weekly reload asks the engine for, call by call, on a settled company.
 *  - "week" is a whole week through the store (advance, reload, state published) and its parts.
 *  - "page" is the UI thread: first draw of each page, a still frame, and the frame after a week lands.
 *  - "memory" is the heap after a collection at the start and after the weeks played.
 */
class DesktopPerfHarness {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        // As the app runs: map layers are baked off the UI thread and the UI thread draws what has landed.
        MapBake.enabled = true
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private val report = StringBuilder()

    private fun line(text: String) { report.append(text).append('\n'); println(text) }

    /** Samples the calling thread's stack every millisecond: where the UI thread's time goes, by function. */
    private class Sampler(private val target: Thread) {
        val inclusive = HashMap<String, Int>()
        val self = HashMap<String, Int>()
        var samples = 0
        @Volatile private var live = true
        /** Samples are kept only while this is set, so a profile can cover just the frames of interest. */
        @Volatile var active = true
        private val thread = Thread {
            while (live) {
                val st = target.stackTrace
                if (active && st.isNotEmpty()) {
                    samples++
                    val top = st[0]; self.merge("${top.className.substringAfterLast('.')}.${top.methodName}", 1, Int::plus)
                    st.filter { it.className.startsWith("net.lakesidegames") || it.className.startsWith("androidx.compose.foundation") || it.className.startsWith("androidx.compose.material") }.map { "${it.className.substringAfterLast('.')}.${it.methodName}" }.distinct().forEach { inclusive.merge(it, 1, Int::plus) }
                }
                Thread.sleep(1)
            }
        }.apply { isDaemon = true; start() }
        fun stop() { live = false; thread.join() }
    }

    /** The share of sampled time spent replaying recorded drawing in native Skia (software here, the graphics card in the app). */
    private fun nativeShare(sampler: Sampler): Double =
        (sampler.self.entries.filter { it.key.startsWith("RenderNodeKt.RenderNode_nDrawInto") || it.key.startsWith("CanvasKt._nClear") }.sumOf { it.value }) / sampler.samples.coerceAtLeast(1).toDouble()

    private fun printProfile(label: String, sampler: Sampler) {
        sampler.stop()
        line("  profile $label: ${sampler.samples} samples")
        sampler.inclusive.entries.filter { !it.key.startsWith("Thread.") && !it.key.startsWith("Sampler") }.sortedByDescending { it.value }.take(40)
            .forEach { line("    incl %5d  %s".format(it.value, it.key)) }
        sampler.self.entries.sortedByDescending { it.value }.take(12).forEach { line("    self %5d  %s".format(it.value, it.key)) }
    }

    private fun median(values: List<Double>) = values.sorted().let { it[it.size / 2] }

    private fun heapMb(): Double {
        repeat(3) { System.gc(); Thread.sleep(50) }
        val rt = Runtime.getRuntime()
        return (rt.totalMemory() - rt.freeMemory()) / 1048576.0
    }

    private fun newGame() = uniffi.cw_ffi.NewGameDto(
        seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
        startingCapitalCents = null, pricePath = "historical",
        chiefName = "Ada Marsh", chiefPortrait = "", chiefTemperament = "raider",
    )

    private fun playWeeks(h: SessionHandle, n: Int) { repeat(n) { h.dismissPause(); h.advance(1u) } }

    @Test
    fun harness() {
        val out = System.getenv("CW_PERF_REPORT") ?: return
        line("HOST load ${File("/proc/loadavg").takeIf { it.exists() }?.readText()?.trim() ?: "n/a"} cpus ${Runtime.getRuntime().availableProcessors()}")
        // The launcher's three loads, cold and one at a time: the app now runs them side by side, so its wait is about the longest.
        fun timed(label: String, block: () -> Unit): Double { val s = System.nanoTime(); block(); return ((System.nanoTime() - s) / 1e6).also { line("startup %-26s %8.1f ms".format(label, it)) } }
        val startup = listOf(
            timed("engine library") { runCatching { uniffi.cw_ffi.engineInfo() } },
            timed("world map") { WorldMap.shared },
            timed("icons and logo") { SectorIcons.all; PixelLogo.file },
        )
        line("startup %-26s %8.1f ms one after another, %.1f ms side by side (the longest)".format("launcher total", startup.sum(), startup.max()))
        val heapStart = heapMb()

        // Opening a career, engine side.
        val t0 = System.nanoTime()
        val raw = SessionHandle.start(newGame())
        line("open career (engine)        %8.1f ms".format((System.nanoTime() - t0) / 1e6))
        playWeeks(raw, 40)
        val firm = raw.overview().primaryFirm!!

        // One weekly reload, call by call.
        val calls = linkedMapOf<String, () -> Any?>(
            "overview" to { raw.overview() }, "firms" to { raw.firms() }, "events" to { raw.events() },
            "regionBoards" to { raw.regionBoards(firm) }, "finance" to { raw.finance(firm) }, "productLines" to { raw.productLines(firm) },
            "facilities" to { raw.facilities(firm) }, "divisions" to { raw.divisions(firm) }, "stockBoard" to { raw.stockBoard() },
            "commodityBoard" to { raw.commodityBoard() }, "mapDivisions" to { raw.mapDivisions() }, "history" to { raw.history(firm) },
            "ownership" to { raw.ownership(firm) }, "recentRaids" to { raw.recentRaids() }, "economy" to { raw.economy() },
            "research" to { raw.research(firm) }, "heldStock" to { raw.heldStock(firm) }, "storageSummary" to { raw.storageSummary(firm) },
            "lanes" to { raw.lanes() }, "laneContracts" to { raw.laneContracts() }, "freightSummary" to { raw.freightSummary() },
            "structureView" to { raw.structureView() }, "rivals" to { raw.rivals(firm) }, "rivalFeed" to { raw.rivalFeed(12u) },
            "mapPlants" to { raw.mapPlants() }, "cfoAdvice" to { raw.cfoAdvice(firm) }, "letters" to { raw.letters(30u) },
            "markets (all products)" to { raw.products().forEach { p -> raw.market(0u, p.index) } },
        )
        line("--- fetch: ms per call, median of 9 (week ${raw.overview().tick})")
        var sum = 0.0
        for ((name, call) in calls) {
            repeat(2) { call() }
            val ms = median(List(9) { val s = System.nanoTime(); call(); (System.nanoTime() - s) / 1e6 })
            sum += ms
            line("fetch %-24s %8.3f".format(name, ms))
        }
        line("fetch %-24s %8.3f".format("TOTAL", sum))
        val advance = median(List(9) { val s = System.nanoTime(); raw.dismissPause(); raw.advance(1u); (System.nanoTime() - s) / 1e6 })
        line("engine advance(1)           %8.3f ms (the week itself)".format(advance))

        // A whole week through the store.
        val dir = kotlin.io.path.createTempDirectory("cwperf").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val tOpen = System.nanoTime()
        store.startCareer(newGame())
        waitUntil { store.ready }
        line("open career (store, to ready) %6.1f ms".format((System.nanoTime() - tOpen) / 1e6))
        kotlinx.coroutines.runBlocking { store.read { h -> playWeeks(h, 40) } }
        store.refresh(); waitUntil { !store.busy }
        fun weeks(label: String): Double {
            val ms = List(12) { val s = System.nanoTime(); oneWeek(store); (System.nanoTime() - s) / 1e6 }
            val mid = median(ms)
            line("week %-26s %8.1f ms median, mean %.1f, max %.1f (engine advance %.1f, so reload and publish cost %.1f)".format(label, mid, ms.average(), ms.max(), advance, mid - advance))
            return mid
        }
        val weekMs = weeks("through the store")
        val manual = median(List(5) { val s = System.nanoTime(); store.refresh(); waitUntil { !store.busy }; (System.nanoTime() - s) / 1e6 })
        line("store.refresh() (reload)    %8.1f ms".format(manual))

        // The UI thread, one page at a time.
        val settings = AppSettings(File(dir, "settings.properties"))
        val base = settings.graphics
        val pages = listOf(
            Triple("Map", 0, base), Triple("Map again", 0, base), Triple("Office", 1, base), Triple("Research", 2, base), Triple("Stats", 3, base),
            Triple("Markets", 4, base), Triple("Inbox", 6, base), Triple("Portfolio", 8, base), Triple("Paper", 7, base), Triple("Hall", 9, base),
            // The classic map with one thing changed, to see what its resting frame is made of.
            Triple("Map [Medium]", 0, base.copy(detail = MapDetail.Medium)), Triple("Map [no motion]", 0, base.copy(animations = false)),
            Triple("Map [no clouds]", 0, base.copy(clouds = 0)), Triple("Map [no earth]", 0, base.copy(earth = false)),
        )
        line("--- page: UI thread ms at 1600x900 (first draw / still frame median / frame as a week lands median)")
        for ((name, index, graphics) in pages) {
            settings.chooseGraphics(graphics)
            val scene = ImageComposeScene(1600, 900, Density(1f)) {
                CorporateWarfareTheme { CompositionLocalProvider(LocalAppSettings provides settings) { DesktopShell(store, onSettings = {}, startPage = index) } }
            }
            var clock = 0L
            fun frame(): Double { clock += 16_666_667L; val s = System.nanoTime(); scene.render(clock); return (System.nanoTime() - s) / 1e6 }
            val profile = System.getenv("CW_PERF_PROFILE")?.split(',')?.contains(name.substringBefore(" again")) == true
            val sampler = if (profile) Sampler(Thread.currentThread()) else null
            val first = frame()
            sampler?.let { printProfile("$name first draw", it) }
            repeat(25) { frame(); Thread.sleep(80) }
            val stillSampler = Sampler(Thread.currentThread())
            val still = median(List(30) { frame() })
            if (profile) printProfile("$name still frames", stillSampler) else stillSampler.stop()
            val kotlinSide = still * (1 - nativeShare(stillSampler))
            val landSampler = if (profile) Sampler(Thread.currentThread()).also { it.active = false } else null
            val landing = List(5) {
                oneWeek(store)
                landSampler?.active = true
                frame().also { landSampler?.active = false }
            }
            landSampler?.let { printProfile("$name week landing", it) }
            line("page %-16s %8.1f %8.1f %8.1f   (still frame: %.1f ms is Kotlin side, the rest is Skia replaying the picture)".format(name, first, still, median(landing), kotlinSide))
            scene.close()
        }

        val heapEnd = heapMb()
        line("--- memory: heap after GC, start %.0f MB, after the run %.0f MB".format(heapStart, heapEnd))
        line("canonical hash after the run: ${raw.canonicalHash()}")
        File(out).takeIf { it.parentFile?.isDirectory == true }?.writeText(report.toString())
        assertTrue(weekMs < 5_000, "a week through the store takes under five seconds")
        assertTrue(heapEnd < 3_000, "the heap stays under 3 GB")
    }

    /** One week through the store, returning when the week and everything read for it is published. */
    private fun oneWeek(store: GameStore) {
        kotlinx.coroutines.runBlocking { store.read { h -> h.dismissPause() } }
        val before = store.overview?.tick ?: 0u
        store.advance()
        waitUntil { (store.overview?.tick ?: 0u) > before }
        waitUntil { !store.busy }
    }

    private fun waitUntil(limitMs: Long = 60_000, test: () -> Boolean) {
        val end = System.currentTimeMillis() + limitMs
        while (!test() && System.currentTimeMillis() < end) Thread.sleep(2)
    }
}
