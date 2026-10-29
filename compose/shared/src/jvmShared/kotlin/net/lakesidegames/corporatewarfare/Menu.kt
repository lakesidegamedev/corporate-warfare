package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.drawText
import uniffi.cw_ffi.NewGameDto
import uniffi.cw_ffi.ScenarioDto
import uniffi.cw_ffi.scenarios

/** Turns bundled PNG bytes into an image; each platform sets it at launch (Skia or Android). */
object Images {
    var decode: (ByteArray) -> ImageBitmap? = { null }
    private val cache = mutableMapOf<String, ImageBitmap?>()

    @Synchronized
    fun load(name: String): ImageBitmap? = cache.getOrPut(name) {
        val bytes = Images::class.java.getResourceAsStream(if ('.' in name) "/$name" else "/$name.png")?.use { it.readBytes() } ?: return@getOrPut null
        runCatching { decode(bytes) }.getOrNull()
    }
}

/** The main menu: a turning globe, the company's way in, and the new-game flow. */
@Composable
internal fun StartScreen(store: GameStore, updater: Updater?, onSettings: () -> Unit, onQuit: (() -> Unit)?) {
    var setup by remember { mutableStateOf(false) }
    var loading by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf(false) }
    if (editing) {
        ScenarioEditorScreen(store, baseId = null, existing = null, onBack = { editing = false })
        return
    }
    if (loading) {
        LoadGameScreen(store) { loading = false }
        return
    }
    if (setup) {
        NewGameScreen(store, onBack = { setup = false })
        return
    }
    LaunchedEffect(Unit) { StartupTimeline.menuShown() }
    BoxWithConstraints(Modifier.fillMaxSize().background(Color(0xFF05090E))) {
        val wide = maxWidth >= 900.dp
        GlobeBackdrop(Modifier.fillMaxSize(), offsetX = if (wide) 0.22f else 0f)
        // Shade the left side so the menu reads over the globe.
        Box(
            Modifier.fillMaxSize().background(
                Brush.horizontalGradient(
                    0f to Color(0xF205090E),
                    (if (wide) 0.45f else 0.9f) to Color(0xB305090E),
                    1f to Color(0x2005090E),
                ),
            ),
        )
        Column(
            Modifier.fillMaxHeight().width(if (wide) 460.dp else maxWidth).windowInsetsPadding(WindowInsets.safeDrawing).verticalScroll(rememberScrollState())
                .padding(horizontal = if (wide) 48.dp else 24.dp, vertical = 40.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp, Alignment.CenterVertically),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            ShiningLogo(Modifier.width(if (wide) 300.dp else 240.dp))
            FlyingBadges(spread = if (wide) 1f else 0.45f)
            Spacer(Modifier.height(10.dp))
            if (store.hasAutosave) {
                MenuButton("Continue", primary = true, enabled = !store.busy) { store.continueCareer() }
            }
            MenuButton("New game", primary = !store.hasAutosave) { setup = true }
            if (store.hasAutosave) MenuButton("Load game") { loading = true }
            MenuButton("Scenario editor") { editing = true }
            MenuButton("Settings", onClick = onSettings)
            if (onQuit != null) MenuButton("Quit", onClick = onQuit)
            Spacer(Modifier.height(6.dp))
            if (updater != null) {
                Column(
                    Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panel.copy(alpha = 0.85f))
                        .border(1.dp, Palette.stroke, Pixel.shape).padding(12.dp),
                ) { UpdatePanel(updater) }
            }
            Text(
                "Version ${BuildInfo.VERSION}" + if (BuildInfo.CHANNEL == "preview") " · tester preview" else "",
                color = Palette.secondary.copy(alpha = 0.7f),
                fontSize = 11.sp,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
        }
    }
}

/** The logo with a band of light sweeping across it every six seconds. */
@Composable
private fun ShiningLogo(modifier: Modifier) {
    val graphics = LocalAppSettings.current?.graphics ?: GraphicsConfig.Default
    val shine = graphics.animations && graphics.lighting > 0
    val clock = rememberFrameSeconds(shine, graphics.frameRate)
    LogoArt(
        modifier.graphicsLayer { compositingStrategy = androidx.compose.ui.graphics.CompositingStrategy.Offscreen }
            .drawWithContent {
                drawContent()
                if (!shine) return@drawWithContent
                val phase = ((clock.value % 6.0) / 1.3).toFloat()
                if (phase >= 1f) return@drawWithContent
                val band = size.width * 0.3f
                val x = -band + phase * (size.width + band * 2)
                drawRect(
                    Brush.linearGradient(
                        0f to Color.Transparent, 0.5f to Color.White.copy(alpha = 0.55f), 1f to Color.Transparent,
                        start = androidx.compose.ui.geometry.Offset(x, 0f), end = androidx.compose.ui.geometry.Offset(x + band, size.height),
                    ),
                    blendMode = androidx.compose.ui.graphics.BlendMode.SrcAtop,
                )
            },
    )
}

@Composable
private fun MenuButton(title: String, primary: Boolean = false, enabled: Boolean = true, onClick: () -> Unit) {
    val shape = Pixel.shape
    Box(
        Modifier.fillMaxWidth().alpha(if (enabled) 1f else 0.5f)
            .padding(end = 3.dp, bottom = 3.dp)
            .pixelFrame(if (primary) Palette.gold else Palette.panel, if (primary) Color(0xFF8A6420) else Palette.stroke)
            .clip(shape)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(vertical = 15.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(title.uppercase(), color = if (primary) Palette.ink else Palette.primaryText, fontSize = 16.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, letterSpacing = 1.sp)
    }
}

/**
 * The sector icons fly in from across the screen and around the globe on curved paths, one
 * after another, and settle into a row under the logo.
 */
@Composable
private fun FlyingBadges(spread: Float) {
    val sectors = Sectors.order.take(8)
    val animate = LocalAppSettings.current?.animations != false
    val progress = remember { sectors.map { androidx.compose.animation.core.Animatable(if (animate) 0f else 1f) } }
    LaunchedEffect(animate) {
        if (!animate) return@LaunchedEffect
        progress.forEachIndexed { index, value ->
            launch {
                kotlinx.coroutines.delay(150L + index * 110L)
                value.animateTo(1f, androidx.compose.animation.core.tween(1_300, easing = androidx.compose.animation.core.FastOutSlowInEasing))
            }
        }
    }
    val starts = remember {
        val rng = java.util.Random(31)
        sectors.indices.map {
            // Start out over the globe to the right, spread around it, curving in.
            val angle = rng.nextDouble() * Math.PI * 2
            val distance = 420 + rng.nextDouble() * 520
            Triple((620 + kotlin.math.cos(angle) * distance).toFloat(), (kotlin.math.sin(angle) * distance * 0.6).toFloat(), (rng.nextDouble() * 720 - 360).toFloat())
        }
    }
    val density = androidx.compose.ui.platform.LocalDensity.current
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        sectors.forEachIndexed { index, sector ->
            val p = progress[index].value
            val (sx, sy, spin) = starts[index]
            val inv = 1f - p
            // A quadratic curve: out over the globe, swinging up and round into place.
            val cx = sx * 0.3f
            val cy = sy - 260f
            val x = inv * inv * sx + 2 * inv * p * cx
            val y = inv * inv * sy + 2 * inv * p * cy
            Box(
                Modifier.graphicsLayer {
                    translationX = with(density) { (x * spread).dp.toPx() }
                    translationY = with(density) { (y * spread).dp.toPx() }
                    rotationZ = spin * inv
                    scaleX = 0.6f + 0.4f * p + 0.25f * kotlin.math.sin(p * Math.PI).toFloat()
                    scaleY = scaleX
                    alpha = (p * 3).coerceAtMost(1f)
                },
            ) { SectorBadge(sector, 30) }
        }
    }
}


// MARK: New game

private enum class Mode(val title: String, val note: String, val guided: Boolean, val fiveYear: Boolean) {
    Guided("Take over a company", "Start running 60 percent of an established company, with the walkthrough.", true, false),
    Founder("Found your own", "Start with cash and no company. Pick a region and a strategy and build from nothing.", false, false),
    Scored("Five-year score", "Take over a company and be scored at the end of year five. You can play on after.", true, true),
}

private enum class Level(val key: String, val title: String, val note: String) {
    Easy("easy", "Easy", "\$8 million to start. Rivals defend but never attack your markets."),
    Standard("standard", "Standard", "\$5 million to start. Cost cutters, consolidators and vultures fight for share."),
    Hard("hard", "Hard", "\$3 million to start. Every rival undercuts, invades and raids."),
}

private enum class Economy(val key: String, val title: String, val note: String) {
    Historic("historic", "As it was", "A late boom on stretched credit, rates at 6 percent: a bust is likely within a few years."),
    Calm("calm", "Calm", "On trend, inflation at target, little debt built up."),
    Boom("boom", "Boom", "Running hot with credit piling up: strong demand now, a hard landing later."),
    Downturn("downturn", "Downturn", "Already in recession with cheap money: weak demand now, a recovery ahead."),
}

private enum class PricePath(val key: String, val title: String, val note: String) {
    Historical("historical", "Historical", "Oil, steel, grain and the rest follow the real path of the period, on the real dates."),
    Variance("historical_variance", "Historical with variance", "The same shape as history, with the timing and depth of each swing shifted by your game's seed."),
    Random("random", "Random", "Neutral prices that drift around where they began. Busts come only from the business cycle."),
}

private fun capitalOf(level: Level) = when (level) {
    Level.Easy -> 8_000_000.0
    Level.Standard -> 5_000_000.0
    Level.Hard -> 3_000_000.0
}

/** Scenario, then the career's terms, then start. */
@Composable
internal fun NewGameScreen(store: GameStore, startStep: Int = 0, onBack: () -> Unit) {
    val catalogue = remember { scenarios() }
    var chosen by remember { mutableStateOf(catalogue.firstOrNull { it.available }) }
    var customs by remember { mutableStateOf(uniffi.cw_ffi.customScenarioList(store.customDir)) }
    var chosenCustom by remember { mutableStateOf<uniffi.cw_ffi.CustomScenarioDto?>(null) }
    var customizing by remember { mutableStateOf<Pair<String, uniffi.cw_ffi.CustomScenarioDto?>?>(null) }
    customizing?.let { (base, existing) ->
        ScenarioEditorScreen(store, baseId = base, existing = existing, onBack = {
            customs = uniffi.cw_ffi.customScenarioList(store.customDir)
            customizing = null
        })
        return
    }
    var step by remember { mutableStateOf(startStep) }
    BackAction { if (step == 0) onBack() else step = 0 }
    var mode by remember { mutableStateOf(Mode.Guided) }
    var level by remember { mutableStateOf(Level.Standard) }
    var economy by remember { mutableStateOf(Economy.Historic) }
    // Null means the scenario's own default.
    var pricePathPick by remember { mutableStateOf<PricePath?>(null) }
    var history by remember { mutableStateOf(ScenarioHistory.Historical) }
    var capital by remember { mutableStateOf(capitalOf(Level.Standard)) }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val wide = maxWidth >= 900.dp
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Row(
                Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 20.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = { if (step == 0) onBack() else step = 0 }) { BackLabel("Back", Palette.secondary) }
                Spacer(Modifier.width(8.dp))
                Text("New game", color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
                StepDot(1, "Scenario", step == 0)
                Spacer(Modifier.width(14.dp))
                StepDot(2, "Setup", step == 1)
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                Column(
                    Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(if (wide) 28.dp else 16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    if (step == 0) {
                        Text("Choose a scenario", color = Palette.primaryText, fontSize = 22.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                        val columns = if (wide) 2 else 1
                        catalogue.chunked(columns).forEach { row ->
                            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                                row.forEach { scenario ->
                                    ScenarioTile(
                                        scenario, selected = chosenCustom == null && chosen?.id == scenario.id, modifier = Modifier.weight(1f),
                                        onCustomize = { customizing = scenario.id to null },
                                    ) {
                                        if (scenario.available) {
                                            chosen = scenario
                                            chosenCustom = null
                                            pricePathPick = null
                                        }
                                    }
                                }
                                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                            }
                        }
                        if (customs.isNotEmpty()) {
                            Spacer(Modifier.height(4.dp))
                            Text("Your scenarios", color = Palette.primaryText, fontSize = 22.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                            FlowRow(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                customs.forEach { custom ->
                                    val on = chosenCustom?.id == custom.id
                                    val shape = Pixel.shape
                                    Column(
                                        Modifier.width(if (wide) 380.dp else 320.dp).clip(shape)
                                            .background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.panel)
                                            .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke, shape)
                                            .clickable { chosenCustom = custom }.padding(14.dp),
                                        verticalArrangement = Arrangement.spacedBy(4.dp),
                                    ) {
                                        Text(custom.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                                        val base = catalogue.firstOrNull { it.id == custom.baseScenarioId }?.name ?: custom.baseScenarioId
                                        Text("Custom · based on $base · ${custom.difficulty}", color = Palette.secondary, fontSize = 12.sp)
                                        TextButton(onClick = { customizing = custom.baseScenarioId to custom }) {
                                            Text("Edit", color = Palette.gold)
                                        }
                                    }
                                }
                            }
                        }
                    } else if (chosenCustom != null) {
                        val custom = chosenCustom!!
                        Text(custom.name, color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                        Text(
                            "Difficulty, economy, capital, rivals and laws come from this scenario. Open the editor to change them.",
                            color = Palette.secondary, fontSize = 13.sp,
                        )
                        Section("How you start") {
                            Options(Mode.entries, mode, { it.title }, { it.note }, wide) { mode = it }
                        }
                    } else {
                        val scenario = chosen
                        if (scenario != null) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Images.load(scenario.image)?.let {
                                    Image(it, null, Modifier.width(160.dp).aspectRatio(1200f / 640f).clip(Pixel.shape), contentScale = ContentScale.Crop)
                                    Spacer(Modifier.width(14.dp))
                                }
                                Column {
                                    Text(scenario.name, color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                                    Text("${scenario.region} · starts ${scenario.startYear} · ${scenario.currency}", color = Palette.secondary, fontSize = 13.sp)
                                }
                            }
                        }
                        Section("How you start") {
                            Options(Mode.entries, mode, { it.title }, { it.note }, wide) { mode = it }
                        }
                        Section("Difficulty") {
                            Options(Level.entries, level, { it.title }, { it.note }, wide) {
                                level = it
                                capital = capitalOf(it)
                            }
                        }
                        Section("The economy on day one") {
                            Options(Economy.entries, economy, { it.title }, { it.note }, wide) { economy = it }
                        }
                        Section("World commodity prices") {
                            val fallback = PricePath.entries.firstOrNull { it.key == scenario?.defaultPricePath } ?: PricePath.Historical
                            val picked = pricePathPick ?: fallback
                            Options(PricePath.entries, picked, { it.title + if (it == fallback) " (scenario default)" else "" }, { it.note }, wide) { pricePathPick = it }
                        }
                        Section("Scenario history") {
                            Options(ScenarioHistory.entries, history, { it.title }, { it.note }, wide) { history = it }
                        }
                        Section("Starting capital") {
                            ValueSlider(
                                capital, { capital = it }, 1_000_000.0..20_000_000.0, 250_000.0,
                                modifier = Modifier.widthIn(max = 760.dp),
                                label = { Format.compact((it * 100).toLong()) },
                                detail = {
                                    if (mode.guided) "Your own cash. Taking over the company uses some of it; the rest is yours to invest."
                                    else "Your own cash to found and fund companies."
                                },
                                ticks = 19,
                            )
                        }
                    }
                }
            }
            Row(
                Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 20.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                val scenario = chosen
                Text(
                    if (step == 0) chosenCustom?.let { "${it.name}: your scenario" } ?: scenario?.let { "${it.name}: ${it.tagline}" } ?: "Pick a scenario"
                    else if (chosenCustom != null) "${mode.title} · ${chosenCustom!!.name}"
                    else "${mode.title} · ${level.title} · ${economy.title} economy · ${Format.compact((capital * 100).toLong())}",
                    color = Palette.secondary,
                    fontSize = 13.sp,
                    modifier = Modifier.weight(1f),
                    maxLines = 2,
                )
                if (step == 0) {
                    GoldButton("Next: setup", enabled = scenario != null || chosenCustom != null) { step = 1 }
                } else {
                    GoldButton("Start career", enabled = (scenario != null || chosenCustom != null) && !store.busy) {
                        chosenCustom?.let { store.startCustom(it, mode.guided, mode.fiveYear); return@GoldButton }
                        val pick = scenario ?: return@GoldButton
                        store.startCareer(
                            NewGameDto(
                                seed = (System.nanoTime() % 9_999_999).toULong(),
                                scenarioId = pick.id,
                                guided = mode.guided,
                                fiveYear = mode.fiveYear,
                                difficulty = level.key,
                                economy = economy.key,
                                startingCapitalCents = (capital * 100).toLong(),
                                pricePath = pricePathPick?.key ?: "",
                                timeline = history.key,
                            ),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun StepDot(number: Int, label: String, active: Boolean) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.width(22.dp).height(22.dp).clip(Pixel.small).background(if (active) Palette.gold else Palette.panelRaised),
            contentAlignment = Alignment.Center,
        ) { Text("$number", color = if (active) Palette.ink else Palette.secondary, fontSize = 12.sp, fontWeight = FontWeight.Bold) }
        Spacer(Modifier.width(6.dp))
        Text(label, color = if (active) Palette.primaryText else Palette.secondary, fontSize = 13.sp)
    }
}

@Composable
private fun Section(title: String, content: @Composable ColumnScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Eyebrow(title)
        content()
    }
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun <T> Options(items: List<T>, selected: T, title: (T) -> String, note: (T) -> String, wide: Boolean, onPick: (T) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        items.forEach { item ->
            val on = item == selected
            val shape = Pixel.shape
            Column(
                Modifier.width(if (wide) 250.dp else 320.dp).clip(shape)
                    .background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.panel)
                    .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke, shape)
                    .clickable { onPick(item) }.padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(title(item), color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                Text(note(item), color = Palette.secondary, fontSize = 12.sp)
            }
        }
    }
}

/** One scenario: its art, name, region, year and currency, or a coming-soon card. */
@Composable
private fun ScenarioTile(scenario: ScenarioDto, selected: Boolean, modifier: Modifier, onCustomize: (() -> Unit)? = null, onClick: () -> Unit) {
    val shape = Pixel.shape
    Column(
        modifier.clip(shape).background(Palette.panel)
            .border(if (selected) 3.dp else 1.dp, if (selected) Palette.gold else Palette.stroke, shape)
            .clickable(enabled = scenario.available, onClick = onClick),
    ) {
        Box(Modifier.fillMaxWidth().aspectRatio(1200f / 640f)) {
            val art = Images.load(scenario.image)
            if (art != null) {
                Image(
                    art, null, Modifier.fillMaxSize().alpha(if (scenario.available) 1f else 0.45f),
                    contentScale = ContentScale.Crop,
                )
            } else {
                Box(Modifier.fillMaxSize().background(Palette.panelRaised))
            }
            Box(
                Modifier.fillMaxSize().background(
                    Brush.verticalGradient(0.55f to Color.Transparent, 1f to Color(0xE60B1622)),
                ),
            )
            Column(Modifier.align(Alignment.BottomStart).padding(14.dp)) {
                Text(scenario.name, color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                Text("${scenario.region} · ${scenario.startYear}", color = Palette.secondary, fontSize = 12.sp)
            }
            if (scenario.available && onCustomize != null) {
                Text(
                    "Customize",
                    color = Palette.primaryText,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.align(Alignment.TopStart).padding(12.dp).clip(Pixel.small)
                        .background(Palette.panelRaised.copy(alpha = 0.9f))
                        .clickable(onClick = onCustomize)
                        .padding(horizontal = 10.dp, vertical = 4.dp),
                )
            }
            Text(
                if (scenario.available) "Play" else "Coming soon",
                color = if (scenario.available) Palette.ink else Palette.primaryText,
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.align(Alignment.TopEnd).padding(12.dp).clip(Pixel.small)
                    .background(if (scenario.available) Palette.gold else Palette.panelRaised.copy(alpha = 0.9f))
                    .padding(horizontal = 10.dp, vertical = 4.dp),
            )
        }
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(scenario.tagline, color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            Text(scenario.summary, color = Palette.secondary, fontSize = 12.sp)
            Text(
                buildString {
                    append(scenario.country)
                    append(" · ")
                    append(scenario.currency)
                    if (scenario.available) append(" · ${scenario.regionCount} regions")
                },
                color = Palette.secondary.copy(alpha = 0.8f),
                fontSize = 11.sp,
            )
        }
    }
}

