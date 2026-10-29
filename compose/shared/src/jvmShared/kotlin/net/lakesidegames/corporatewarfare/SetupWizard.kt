package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors
import kotlin.math.sqrt

/** The plain-language privacy text, shared by the Settings page and the setup wizard. */
object Privacy {
    const val explanation = "Every five weeks, the goods made, sold and priced, and how your company is doing, are sent with a random id. No names or account details."

    /** The in-app privacy summary, the same facts as the published policy. */
    val points = listOf(
        "No account is needed and nothing that names you, contacts you or identifies your device is sent.",
        "The snapshot is used only to balance the game.",
        "Saves and settings stay on this device.",
        "You can switch this off here, or later in Settings under Analytics.",
    )
}

/** What the one-second map test measured. */
sealed interface SetupBenchmark {
    data object Idle : SetupBenchmark
    data object Running : SetupBenchmark
    data class Done(val frameMs: Double, val frames: Int) : SetupBenchmark

    /** The map could not be drawn for the test; the advice then goes by screen size and backend alone. */
    data object Unavailable : SetupBenchmark
}

/** Turns a measured frame time, the screen and the drawing backend into a graphics preset. */
object SetupAdvice {
    const val BENCH_WIDTH = 480
    const val BENCH_HEIGHT = 320

    /** A projected frame time at or under this is comfortable on High. */
    const val HIGH_MS = 50.0

    /** At or under this, Medium; above it, Low. */
    const val MEDIUM_MS = 130.0

    /**
     * The test frame is [BENCH_WIDTH] by [BENCH_HEIGHT]; the real map is bigger. Drawing cost grows slower than
     * pixel count (the geometry is shared), so the frame time is scaled by the square root of the size ratio.
     * Software drawing and very large screens never get High, the same rule a first start already follows.
     * Ultra is never recommended: it uncaps the frame rate, which is a choice, not a default.
     */
    fun recommend(frameMs: Double, screenPixels: Long, software: Boolean): GraphicsPreset {
        val ratio = maxOf(screenPixels, 1L).toDouble() / (BENCH_WIDTH * BENCH_HEIGHT)
        val projected = frameMs * sqrt(maxOf(ratio, 1.0))
        val measured = when {
            projected <= HIGH_MS -> GraphicsPreset.High
            projected <= MEDIUM_MS -> GraphicsPreset.Medium
            else -> GraphicsPreset.Low
        }
        return if (measured == GraphicsPreset.High && (software || screenPixels > SUGGEST_MEDIUM_ABOVE)) GraphicsPreset.Medium else measured
    }

    /** The preset to start on when nothing was measured (Skip, or the test could not run). */
    fun withoutBenchmark(screenPixels: Long, software: Boolean): GraphicsPreset =
        if (software || screenPixels > SUGGEST_MEDIUM_ABOVE) GraphicsPreset.Medium else GraphicsPreset.High

    /** The preset for a finished or missing test. */
    fun recommend(benchmark: SetupBenchmark, screenPixels: Long, software: Boolean): GraphicsPreset =
        (benchmark as? SetupBenchmark.Done)?.let { recommend(it.frameMs, screenPixels, software) } ?: withoutBenchmark(screenPixels, software)
}

/** The choices on the wizard's screens, held until Play or Skip so nothing is saved halfway. */
class SetupDraft(preset: GraphicsPreset, channel: UpdateChannel, analytics: Boolean, tutorial: Boolean) {
    var preset by mutableStateOf(preset)
    var channel by mutableStateOf(channel)
    var analytics by mutableStateOf(analytics)
    var tutorial by mutableStateOf(tutorial)

    /** Write every choice to the existing settings keys and mark setup done. */
    fun commit(settings: AppSettings, includeChannel: Boolean) {
        settings.choosePreset(preset)
        if (includeChannel) settings.chooseUpdateChannel(channel)
        settings.chooseAnalytics(analytics)
        settings.chooseTutorial(settings.tutorialDone, hidden = !tutorial)
        settings.finishSetup()
    }

    companion object {
        /** Today's choices, for setting up again. */
        fun from(settings: AppSettings) = SetupDraft(settings.graphics.preset ?: GraphicsPreset.Medium, settings.updateChannel, settings.analytics, !settings.tutorialHidden)

        /** What Skip applies on a first launch: the app's own defaults, with graphics judged from the screen and backend. */
        fun defaults(screenPixels: Long, software: Boolean) =
            SetupDraft(SetupAdvice.withoutBenchmark(screenPixels, software), UpdateChannel.build, analytics = true, tutorial = true)
    }
}

/** Draws a small map offscreen, on any platform. Call off the UI thread. */
internal fun renderSmallMap(
    map: WorldMap, camera: MapCamera, graphics: GraphicsConfig, width: Int, height: Int,
    measurer: TextMeasurer, density: Density, geometry: MapGeometryCache = MapGeometryCache(),
    shading: Map<String, Double> = emptyMap(),
): ImageBitmap {
    val bitmap = ImageBitmap(width, height)
    CanvasDrawScope().draw(density, LayoutDirection.Ltr, Canvas(bitmap), Size(width.toFloat(), height.toFloat())) {
        drawRect(Color(0xFF05090E))
        val sun = Sun.side(MapProjection(camera, size))
        drawWorldMap(map, camera, Palette.gold, shading, null, emptyList(), measurer, geometry = geometry, graphics = graphics, sun = sun, heights = shading)
    }
    return bitmap
}

/**
 * The pictures and the test behind the wizard. Everything draws on one background thread, started only when
 * the wizard needs it, so opening the app costs nothing and the UI thread never draws a map for the wizard.
 */
class SetupVisuals internal constructor(private val measurer: TextMeasurer, private val density: Density, private val dispatcher: CoroutineDispatcher) {
    var globe by mutableStateOf<ImageBitmap?>(null); private set
    var preview by mutableStateOf<ImageBitmap?>(null); private set
    var previewPreset by mutableStateOf<GraphicsPreset?>(null); private set
    var benchmark by mutableStateOf<SetupBenchmark>(SetupBenchmark.Idle); private set

    private val geometry = MapGeometryCache()

    suspend fun renderGlobe() {
        if (globe != null) return
        globe = withContext(dispatcher) {
            val map = WorldMap.shared ?: return@withContext null
            val graphics = GraphicsPreset.Medium.config(animations = false, showFps = false).copy(detail = MapDetail.Low, extrude = false, buildings = 0, traffic = 0)
            runCatching { renderSmallMap(map, MapCamera(-82.0, 24.0, 1.25), graphics, 560, 360, measurer, density, geometry) }.getOrNull()
        }
    }

    /** The scenario map, at the look [preset] gives it. */
    suspend fun renderPreview(preset: GraphicsPreset) {
        if (previewPreset == preset && preview != null) return
        val image = withContext(dispatcher) {
            val map = WorldMap.forScenario("great-lakes-2000") ?: WorldMap.shared ?: return@withContext null
            val size = Size(560f, 360f)
            val camera = MapCamera.scenario(size, map).zoomed(0.9)
            // A spread of values so raised regions and shading show.
            val shading = map.regions.mapIndexed { i, r -> r.code to 0.25 + 0.75 * ((i * 37) % 11) / 10.0 }.toMap()
            runCatching { renderSmallMap(map, camera, preset.config(animations = false, showFps = false), 560, 360, measurer, density, MapGeometryCache(), shading) }.getOrNull()
        }
        if (image != null) { preview = image; previewPreset = preset }
    }

    /**
     * Draw the menu globe offscreen for about a second and time the frames. The first frame builds the
     * geometry and is not counted. The median is kept, so one hiccup does not change the advice.
     */
    suspend fun runBenchmark(budgetMs: Long = 1000) {
        if (benchmark != SetupBenchmark.Idle) return
        benchmark = SetupBenchmark.Running
        benchmark = withContext(dispatcher) {
            val map = WorldMap.shared ?: return@withContext SetupBenchmark.Unavailable
            val graphics = GraphicsPreset.High.config(animations = false, showFps = false).copy(detail = MapDetail.Low, extrude = false, buildings = 0, traffic = 0)
            runCatching {
                val cache = MapGeometryCache()
                fun frame(i: Int) = renderSmallMap(map, MapCamera(-82.0 + i * 2.0, 24.0, 1.25), graphics, SetupAdvice.BENCH_WIDTH, SetupAdvice.BENCH_HEIGHT, measurer, density, cache)
                frame(-1)
                val times = ArrayList<Double>()
                val start = System.nanoTime()
                var i = 0
                while ((times.size < 3 || (System.nanoTime() - start) / 1_000_000 < budgetMs) && times.size < 200) {
                    val t = System.nanoTime()
                    frame(i++)
                    times += (System.nanoTime() - t) / 1e6
                }
                SetupBenchmark.Done(times.sorted()[times.size / 2], times.size)
            }.getOrDefault(SetupBenchmark.Unavailable)
        }
    }
}

@Composable
fun rememberSetupVisuals(): SetupVisuals {
    val density = LocalDensity.current
    val resolver = LocalFontFamilyResolver.current
    val executor = remember { Executors.newSingleThreadExecutor { r -> Thread(r, "setup-visuals").apply { isDaemon = true } } }
    DisposableEffect(executor) { onDispose { executor.shutdown() } }
    return remember(executor) { SetupVisuals(TextMeasurer(resolver, density, LayoutDirection.Ltr, 16), density, executor.asCoroutineDispatcher()) }
}

internal enum class SetupStep(val title: String) {
    Welcome("Welcome"), Graphics("Graphics"), Updates("Updates"), Privacy("Privacy"), Start("Start")
}

/**
 * First-run setup: five short steps in the 8-bit panels. Nothing is saved until Play, or Skip on a first
 * launch, which applies the defaults. Set up again from Settings starts from today's choices and Cancel
 * leaves them alone. [saveFolder] is shown on desktop; the Updates step is skipped when [updater] is null
 * (Play Store builds update through the store).
 */
@Composable
fun SetupWizard(
    settings: AppSettings,
    updater: Updater?,
    saveFolder: String?,
    firstRun: Boolean,
    onDone: () -> Unit,
    visuals: SetupVisuals = rememberSetupVisuals(),
    startStep: Int = 0,
) {
    val steps = remember(updater) { SetupStep.entries.filter { it != SetupStep.Updates || updater != null } }
    var index by remember { mutableStateOf(startStep.coerceIn(0, steps.lastIndex)) }
    val draft = remember { SetupDraft.from(settings) }
    var touched by remember { mutableStateOf(false) }
    val step = steps[index]

    fun finish(apply: Boolean) {
        if (apply) draft.commit(settings, includeChannel = updater != null) else if (firstRun) settings.finishSetup()
        if (apply && updater != null) updater.channelChanged()
        Tutorial.load(settings)
        onDone()
    }

    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val density = LocalDensity.current
        val pixels = with(density) { (maxWidth.toPx() * maxHeight.toPx()).toLong() }
        val software = RenderInfo.software
        // The test result replaces the starting preset until the player picks one themselves.
        LaunchedEffect(visuals.benchmark) {
            val result = visuals.benchmark
            if (!touched && result is SetupBenchmark.Done) draft.preset = SetupAdvice.recommend(result, pixels, software)
        }
        LaunchedEffect(step) {
            if (step == SetupStep.Welcome) visuals.renderGlobe()
            if (step == SetupStep.Graphics) visuals.runBenchmark()
        }
        LaunchedEffect(step, draft.preset) { if (step == SetupStep.Graphics) visuals.renderPreview(draft.preset) }

        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing), horizontalAlignment = Alignment.CenterHorizontally) {
            Column(
                Modifier.weight(1f).widthIn(max = 640.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                StepDots(steps.size, index)
                when (step) {
                    SetupStep.Welcome -> WelcomeStep(visuals)
                    SetupStep.Graphics -> GraphicsStep(visuals, draft, pixels, software) { draft.preset = it; touched = true }
                    SetupStep.Updates -> UpdatesStep(draft)
                    SetupStep.Privacy -> PrivacyStep(draft)
                    SetupStep.Start -> StartStep(draft, saveFolder)
                }
            }
            Row(
                Modifier.widthIn(max = 640.dp).fillMaxWidth().background(Palette.ink).padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TextButton(onClick = { index-- }, enabled = index > 0) { Text("Back", color = if (index > 0) Palette.primaryText else Palette.secondary, fontFamily = Pixel.font) }
                TextButton(onClick = { finish(apply = firstRun) }) {
                    Text(if (firstRun) "Skip" else "Cancel", color = Palette.secondary, fontFamily = Pixel.font)
                }
                Spacer(Modifier.weight(1f))
                if (index < steps.lastIndex) GoldButton("Next") { index++ } else GoldButton("Play") { finish(apply = true) }
            }
        }
    }
}

@Composable
private fun StepDots(count: Int, index: Int) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        repeat(count) { i ->
            Box(Modifier.size(if (i == index) 14.dp else 10.dp).background(if (i <= index) Palette.gold else Palette.panelRaised))
        }
        Spacer(Modifier.width(6.dp))
        Text("Step ${index + 1} of $count", color = Palette.secondary, fontSize = 12.sp, fontFamily = Pixel.font)
    }
}

@Composable
private fun Heading(text: String) {
    Text(text.uppercase(), color = Palette.primaryText, fontSize = 22.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
}

@Composable
private fun Body(text: String, tint: Color = Palette.secondary) {
    Text(text, color = tint, fontSize = 14.sp)
}

@Composable
private fun WelcomeStep(visuals: SetupVisuals) {
    Panel {
        Heading("Welcome")
        LogoArt(Modifier.fillMaxWidth(0.7f).align(Alignment.CenterHorizontally))
        visuals.globe?.let { Image(it, null, Modifier.fillMaxWidth().aspectRatio(560f / 360f), contentScale = ContentScale.Fit) }
            ?: Spacer(Modifier.fillMaxWidth().aspectRatio(560f / 360f))
        Body("Build companies, run them through real booms and busts, and out-earn your rivals. A few quick questions first, so the game suits this device. Skip any time to take the defaults.", Palette.primaryText)
    }
}

@Composable
private fun GraphicsStep(visuals: SetupVisuals, draft: SetupDraft, pixels: Long, software: Boolean, onPick: (GraphicsPreset) -> Unit) {
    Panel {
        Heading("Graphics")
        when (val result = visuals.benchmark) {
            SetupBenchmark.Idle, SetupBenchmark.Running -> Body("Testing how fast this device draws the map. It takes about a second.", Palette.gold)
            is SetupBenchmark.Done -> Body(
                "The test drew ${result.frames} map frames at ${"%.0f".format(result.frameMs)} ms each. Recommended: ${SetupAdvice.recommend(result, pixels, software).title}.",
                Palette.primaryText,
            )
            SetupBenchmark.Unavailable -> Body("The test could not run here, so the choice below goes by your screen size.")
        }
        RenderInfo.label()?.let { backend ->
            Text("Drawing with: $backend", color = if (software) Palette.warning else Palette.primaryText, fontSize = 14.sp)
        }
        if (software) {
            Callout(
                "This computer is drawing the game in software, not on a graphics card, so the map will be slow. Low or Medium is the safest choice. " +
                    "Updating your graphics driver usually fixes it.",
                Palette.warning,
            )
        }
        Choices(GraphicsPreset.entries, draft.preset, { it.title }, onPick)
        Body("You can change every graphics option later in Settings.")
        val image = visuals.preview
        Box(Modifier.fillMaxWidth().aspectRatio(560f / 360f).clip(Pixel.shape).background(Color(0xFF05090E))) {
            if (image != null) Image(image, null, Modifier.fillMaxSize(), contentScale = ContentScale.Fit)
        }
        Text(
            if (visuals.previewPreset == draft.preset) "Preview at ${draft.preset.title}" else "Drawing the preview...",
            color = Palette.secondary, fontSize = 12.sp, fontFamily = Pixel.font,
        )
    }
}

@Composable
private fun UpdatesStep(draft: SetupDraft) {
    Panel {
        Heading("Updates")
        Body("Which versions should the game offer you?", Palette.primaryText)
        Choices(UpdateChannel.entries, draft.channel, { it.title }) { draft.channel = it }
        Body(draft.channel.note)
        Body("The game reads the public release list on GitHub when it starts. Nothing about you or your game is sent.")
    }
}

@Composable
private fun PrivacyStep(draft: SetupDraft) {
    Panel {
        Heading("Privacy")
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Share anonymous market data", color = Palette.primaryText)
                Body(Privacy.explanation)
            }
            Switch(checked = draft.analytics, onCheckedChange = { draft.analytics = it }, colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold))
        }
        Eyebrow("What that means")
        Privacy.points.forEach { Body("- $it") }
    }
}

@Composable
private fun StartStep(draft: SetupDraft, saveFolder: String?) {
    Panel {
        Heading("Start")
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Guided tutorial", color = Palette.primaryText)
                Body("Eight short steps that tick off as you play. Turn it off if you know the game; it is under Settings if you change your mind.")
            }
            Switch(checked = draft.tutorial, onCheckedChange = { draft.tutorial = it }, colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold))
        }
        if (saveFolder != null) {
            Eyebrow("Where your saves are kept")
            Text(saveFolder, color = Palette.primaryText, fontSize = 13.sp)
            Body("Back this folder up to keep your careers safe.")
        }
        Spacer(Modifier.height(4.dp))
        Body("Graphics: ${draft.preset.title}. Anonymous data: ${if (draft.analytics) "on" else "off"}. Tutorial: ${if (draft.tutorial) "on" else "off"}.", Palette.primaryText)
    }
}

/** The one screen an upgrading player sees instead of the full wizard. */
@Composable
fun WhatsNewScreen(onRunSetup: () -> Unit, onContinue: () -> Unit) {
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink).windowInsetsPadding(WindowInsets.safeDrawing), contentAlignment = Alignment.Center) {
        Column(Modifier.widthIn(max = 640.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Panel {
                Heading("What's new in 1.9")
                listOf(
                    "The map is smoother on Windows and Linux, most of all on big screens.",
                    "Every scenario has dated events from its own period that play out in your career.",
                    "The frame counter in Settings now shows how the game is drawing, and warns when it is in slow software mode.",
                    "A first-run setup tests your graphics and sets updates, privacy and the tutorial. Your current settings are kept unless you run it.",
                    "Careers you save now will not open in older versions of the game. Your existing saves open as normal.",
                ).forEach { Body("- $it", Palette.primaryText) }
            }
            GoldButton("Run setup", Modifier.fillMaxWidth(), onClick = onRunSetup)
            TextButton(onClick = onContinue, modifier = Modifier.fillMaxWidth()) { Text("Continue to the game", color = Palette.gold, fontFamily = Pixel.font) }
        }
    }
}
