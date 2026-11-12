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
import uniffi.cw_ffi.chiefChoices
import androidx.compose.material3.OutlinedTextField
import androidx.compose.foundation.layout.size
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

/** Which page of the main menu is open. */
internal enum class MenuPage { Main, Load, NewCareer, Scenarios, Hall, Editor }

/**
 * The main menu (2.0): one column of choices on a solid panel. A returning player gets Continue with a strip of
 * the last save (company, game date, net worth, ambition and medal, play time); a new player gets New career,
 * whose first step has Quick start. The globe turns behind it on a desktop; a phone shows the plain panel.
 */
@Composable
internal fun StartScreen(store: GameStore, updater: Updater?, onSettings: () -> Unit, onQuit: (() -> Unit)?, startPage: MenuPage = MenuPage.Main) {
    var page by remember { mutableStateOf(if (store.openNewGame) MenuPage.NewCareer else startPage) }
    LaunchedEffect(Unit) { store.openNewGame = false }
    // A scenario picked on the Scenarios page opens New career on its second step.
    var scenarioPick by remember { mutableStateOf<String?>(null) }
    var customPick by remember { mutableStateOf<uniffi.cw_ffi.CustomScenarioDto?>(null) }
    var editing by remember { mutableStateOf<Pair<String?, uniffi.cw_ffi.CustomScenarioDto?>>(null to null) }
    val settings = LocalAppSettings.current
    when (page) {
        MenuPage.Hall -> { BackAction { page = MenuPage.Main }; CareerListScreen(settings?.careers.orEmpty()) { page = MenuPage.Main }; return }
        MenuPage.Editor -> { ScenarioEditorScreen(store, baseId = editing.first, existing = editing.second, onBack = { page = MenuPage.Scenarios }); return }
        MenuPage.Load -> { LoadGameScreen(store) { page = MenuPage.Main }; return }
        MenuPage.NewCareer -> {
            NewGameScreen(
                store, startStep = if (scenarioPick != null || customPick != null) 1 else 0, startScenario = scenarioPick, startCustom = customPick,
                onBack = { page = MenuPage.Main; scenarioPick = null; customPick = null },
            )
            return
        }
        MenuPage.Scenarios -> {
            ScenariosScreen(
                store, onBack = { page = MenuPage.Main },
                onPlay = { id -> scenarioPick = id; customPick = null; page = MenuPage.NewCareer },
                onPlayCustom = { c -> customPick = c; scenarioPick = null; page = MenuPage.NewCareer },
                onEdit = { base, existing -> editing = base to existing; page = MenuPage.Editor },
            )
            return
        }
        MenuPage.Main -> Unit
    }
    LaunchedEffect(Unit) { StartupTimeline.menuShown() }
    val slots = remember(page) { store.saveSlots() }
    val last = slots.firstOrNull()
    var updates by remember { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val wide = maxWidth >= 900.dp
        if (wide) GlobeBackdrop(Modifier.fillMaxSize(), offsetX = 0.22f, calm = true, focus = last?.info?.scenario?.ifEmpty { null })
        Column(
            Modifier.then(if (wide) Modifier.align(Alignment.CenterStart) else Modifier.fillMaxHeight()).width(if (wide) 520.dp else maxWidth)
                .then(if (wide) Modifier.padding(start = 56.dp, top = 32.dp, bottom = 32.dp).pixelFrame(Palette.ink, Palette.stroke, shadow = 6.dp) else Modifier)
                .windowInsetsPadding(WindowInsets.safeDrawing).verticalScroll(rememberScrollState())
                .padding(horizontal = if (wide) 32.dp else 18.dp, vertical = if (wide) 28.dp else 20.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                LogoArt(Modifier.width(if (wide) 190.dp else 150.dp))
                Spacer(Modifier.width(16.dp))
                if (wide) FlyingBadges(spread = 0.3f, count = 4)
            }
            Spacer(Modifier.height(6.dp))
            if (last != null) ContinueCard(last.info, enabled = !store.busy) { store.continueCareer() }
            MenuButton("New career", primary = last == null, detail = if (last == null && wide) "Quick start on the first step" else null) { page = MenuPage.NewCareer }
            if (slots.isNotEmpty()) MenuButton("Load", detail = "${slots.size} save${if (slots.size == 1) "" else "s"}") { page = MenuPage.Load }
            MenuButton("Scenarios", detail = "${uniffi.cw_ffi.scenarios().count { it.available }} to play, and your own") { page = MenuPage.Scenarios }
            if (settings?.careers?.isNotEmpty() == true) MenuButton("Hall of Careers", detail = "${settings.careers.size} careers") { page = MenuPage.Hall }
            MenuButton("Settings", onClick = onSettings)
            if (updater != null) {
                MenuButton("Updates and version", detail = updateDetail(updater)) { updates = !updates }
                if (updates) Column(Modifier.fillMaxWidth().pixelFrame(Palette.panel, Palette.stroke).padding(12.dp)) { UpdatePanel(updater) }
            }
            // Reserved: the Lakeside account entry (sign in, store) goes here, one row, once accounts exist.
            AccountEntry()
            if (onQuit != null) MenuButton("Quit", onClick = onQuit)
            Spacer(Modifier.height(4.dp))
            if (updater == null && LauncherSession.current.managed) Text(
                "The Lakeside launcher keeps this copy up to date.",
                color = Palette.secondary, fontSize = 11.sp,
            )
            if (updater == null) Text(
                "Version ${BuildInfo.VERSION}" + if (BuildInfo.CHANNEL == "preview") ", tester preview" else "",
                color = Palette.secondary, fontSize = 11.sp,
            )
        }
    }
}

private fun updateDetail(updater: Updater): String = when (val s = updater.status) {
    is UpdateStatus.Available -> "${s.release.version} is out"
    UpdateStatus.Current -> "${updater.current}, up to date"
    UpdateStatus.Checking -> "${updater.current}, checking"
    else -> updater.current
}

/** Continue, with the last save in one strip: who, when, how rich, how the ambition stands, how long played. */
@Composable
private fun ContinueCard(info: SaveInfo, enabled: Boolean, onClick: () -> Unit) {
    Column(
        Modifier.fillMaxWidth().padding(end = 3.dp, bottom = 3.dp).pixelFrame(Palette.gold, Color(0xFF8A6420)).clip(Pixel.shape)
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("CONTINUE", color = Palette.ink, fontSize = 18.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
            Text(info.week, color = Palette.ink, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        }
        Text(info.title + if (info.scenario.isNotEmpty()) "  |  ${info.scenario}" else "", color = Palette.ink, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
        if (info.detailed) {
            val ambition = if (info.ambition.isEmpty()) "No ambition" else "${ambitionWords(info.ambition)}: ${medalWords(info.medal).lowercase()}"
            Text(
                "Net worth ${Format.compact(info.netWorthCents, info.currency)}  |  $ambition  |  ${Saves.playTime(info.playSeconds)} played",
                color = Palette.ink, fontSize = 12.sp, maxLines = 1,
            )
        }
    }
}

@Composable
private fun MenuButton(title: String, primary: Boolean = false, enabled: Boolean = true, detail: String? = null, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().alpha(if (enabled) 1f else 0.5f)
            .padding(end = 3.dp, bottom = 3.dp)
            .pixelFrame(if (primary) Palette.gold else Palette.panel, if (primary) Color(0xFF8A6420) else Palette.stroke)
            .clip(Pixel.shape)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title.uppercase(), color = if (primary) Palette.ink else Palette.primaryText, fontSize = 15.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, letterSpacing = 1.sp, modifier = Modifier.weight(1f))
        if (detail != null) Text(detail, color = if (primary) Palette.ink else Palette.secondary, fontSize = 12.sp, maxLines = 1)
    }
}

/**
 * The sector icons fly in from across the screen on curved paths, one after another, and settle into a row.
 */
@Composable
private fun FlyingBadges(spread: Float, count: Int = 8) {
    val sectors = Sectors.order.take(count)
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
            val angle = rng.nextDouble() * Math.PI * 2
            val distance = 420 + rng.nextDouble() * 520
            Triple((620 + kotlin.math.cos(angle) * distance).toFloat(), (kotlin.math.sin(angle) * distance * 0.6).toFloat(), (rng.nextDouble() * 720 - 360).toFloat())
        }
    }
    val density = androidx.compose.ui.platform.LocalDensity.current
    androidx.compose.foundation.layout.FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        sectors.forEachIndexed { index, sector ->
            val p = progress[index].value
            val (sx, sy, spin) = starts[index]
            val inv = 1f - p
            val cx = sx * 0.3f
            val cy = sy - 260f
            val x = inv * inv * sx + 2 * inv * p * cx
            val y = inv * inv * sy + 2 * inv * p * cy
            Box(
                Modifier.graphicsLayer {
                    translationX = with(density) { (x * spread).dp.toPx() }
                    translationY = with(density) { (y * spread).dp.toPx() }
                    rotationZ = spin * inv
                    scaleX = 0.6f + 0.4f * p
                    scaleY = scaleX
                },
            ) { SectorBadge(sector, 30) }
        }
    }
}

/** The first sentence of a piece of content, for a two-line description. */
internal fun firstSentence(text: String): String {
    val first = text.indexOf(". ").let { if (it < 0) text else text.substring(0, it + 1) }
    if (first.length >= 60 || first.length == text.length) return first
    // A short first sentence takes the next one too, so every scenario gets two lines.
    val second = text.indexOf(". ", first.length + 1).let { if (it < 0) text else text.substring(0, it + 1) }
    return second
}

/**
 * Scenarios: the four scenarios with their start year, region, currency and size, what makes each different
 * in two lines from content, and the player's own scenarios. Each has New career here and Customize.
 */
@Composable
internal fun ScenariosScreen(
    store: GameStore,
    onBack: () -> Unit,
    onPlay: (String) -> Unit,
    onPlayCustom: (uniffi.cw_ffi.CustomScenarioDto) -> Unit,
    onEdit: (String?, uniffi.cw_ffi.CustomScenarioDto?) -> Unit,
) {
    BackAction(onBack = onBack)
    val catalogue = remember { scenarios() }
    val customs = remember { uniffi.cw_ffi.customScenarioList(store.customDir) }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink).windowInsetsPadding(WindowInsets.safeDrawing)) {
        val wide = maxWidth >= 900.dp
        Column(Modifier.fillMaxSize()) {
            MenuTopBar("Scenarios", onBack)
            Column(
                Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(if (wide) 24.dp else 12.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                val columns = if (wide) 2 else 1
                catalogue.chunked(columns).forEach { row ->
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        row.forEach { s -> ScenarioCard(s, false, wide, Modifier.weight(1f), onPick = null, onPlay = { onPlay(s.id) }, onCustomize = { onEdit(s.id, null) }) }
                        repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
                Eyebrow("Your scenarios")
                if (customs.isEmpty()) {
                    Text("A custom scenario starts from one of the four and changes its difficulty, economy, capital, rivals and laws. Customize on any scenario makes one.", color = Palette.secondary, fontSize = 13.sp)
                }
                customs.forEach { c -> CustomRow(c, catalogue, false, wide, onPick = null, onPlay = { onPlayCustom(c) }, onEdit = { onEdit(c.baseScenarioId, c) }) }
            }
        }
    }
}

@Composable
private fun CustomRow(
    custom: uniffi.cw_ffi.CustomScenarioDto, catalogue: List<ScenarioDto>, selected: Boolean, wide: Boolean,
    onPick: (() -> Unit)?, onPlay: (() -> Unit)?, onEdit: () -> Unit,
) {
    val base = catalogue.firstOrNull { it.id == custom.baseScenarioId }
    Row(
        Modifier.fillMaxWidth().padding(end = 3.dp, bottom = 3.dp).pixelFrame(if (selected) Color(0xFF2A2A1C) else Palette.panel, if (selected) Palette.gold else Palette.stroke)
            .clip(Pixel.shape).then(if (onPick != null) Modifier.clickable(onClick = onPick) else Modifier).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(custom.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
            Text("Based on ${base?.name ?: custom.baseScenarioId}" + (base?.let { ", starts ${it.startYear}" } ?: "") + ", ${custom.difficulty}", color = Palette.secondary, fontSize = 12.sp)
        }
        MenuBarButton("Edit", onClick = onEdit)
        if (onPlay != null) GoldButton(if (wide) "New career here" else "Play", onClick = onPlay)
    }
}

/** One scenario: its art on a solid frame, the facts, and two lines on what makes it different. */
@Composable
private fun ScenarioCard(
    scenario: ScenarioDto, selected: Boolean, wide: Boolean, modifier: Modifier,
    onPick: (() -> Unit)?, onPlay: (() -> Unit)?, onCustomize: (() -> Unit)?,
) {
    Column(
        modifier.padding(end = 3.dp, bottom = 3.dp).pixelFrame(if (selected) Color(0xFF2A2A1C) else Palette.panel, if (selected) Palette.gold else Palette.stroke, cut = 4.dp)
            .clip(Pixel.shape).then(if (onPick != null && scenario.available) Modifier.clickable(onClick = onPick) else Modifier),
    ) {
        Row(Modifier.padding(12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Images.load(scenario.image)?.let {
                Image(
                    it, null, Modifier.width(if (wide) 200.dp else 110.dp).aspectRatio(1200f / 640f).clip(Pixel.small).alpha(if (scenario.available) 1f else 0.45f),
                    contentScale = ContentScale.Crop,
                )
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Text(scenario.name.uppercase(), color = if (selected) Palette.gold else Palette.primaryText, fontSize = if (wide) 16.sp else 14.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                Text("Starts ${scenario.startYear}  |  ${scenario.region}", color = Palette.secondary, fontSize = 12.sp, maxLines = 2)
                Text("${scenario.currency}" + if (scenario.available) "  |  ${scenario.regionCount} regions" else "  |  not yet playable", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        Column(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(scenario.tagline, color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            Text(firstSentence(scenario.summary), color = Palette.secondary, fontSize = 12.sp, maxLines = 2)
            if (scenario.available && (onPlay != null || onCustomize != null)) {
                Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    onPlay?.let { GoldButton("New career here", onClick = it) }
                    onCustomize?.let { MenuBarButton("Customize", onClick = it) }
                }
            }
        }
    }
}


// MARK: New career

private enum class Mode(val title: String, val note: String, val guided: Boolean, val fiveYear: Boolean) {
    Guided("Take over a company", "You run 60 percent of an established company from day one, with the walkthrough.", true, false),
    Founder("Found your own", "Cash and no company. Pick a region and a strategy and build from nothing.", false, false),
    Scored("Five-year score", "Take over a company and be scored at the end of year five. You can play on after.", true, true),
    Helm("At the helm", "Buy control of one of three established companies, shown below with their price.", false, false),
}

private enum class Level(val key: String, val title: String, val rivals: String) {
    Easy("easy", "Easy", "Rivals defend but never attack your markets."),
    Standard("standard", "Standard", "Cost cutters, consolidators and vultures fight for share."),
    Hard("hard", "Hard", "Every rival undercuts, invades and raids."),
}

private enum class Economy(val key: String, val title: String, val note: String) {
    Historic("historic", "As it was", "A late boom on stretched credit, rates at 6 percent: a bust is likely within a few years."),
    Calm("calm", "Calm", "On trend, inflation at target, little debt built up."),
    Boom("boom", "Boom", "Running hot with credit piling up: strong demand now, a hard landing later."),
    Downturn("downturn", "Downturn", "Already in recession with cheap money: weak demand now, a recovery ahead."),
}

private enum class PricePath(val key: String, val title: String, val note: String) {
    Historical("historical", "Historical", "Oil, steel, grain and the rest follow the real path of the period, on the real dates."),
    Variance("historical_variance", "With variance", "The same shape as history, with the timing and depth of each swing shifted by your game's seed."),
    Random("random", "Random", "Neutral prices that drift around where they began. Busts come only from the business cycle."),
}

/** The steps of New career. A custom scenario carries its own world and chief, so it stops after How you start. */
private enum class CareerStep(val title: String) { Scenario("Scenario"), Start("How you start"), Ambition("Ambition"), World("World"), Chief("Chief") }

/**
 * New career (2.0): Scenario, How you start, Ambition and length, World, Chief. Every step is one clear choice
 * with its consequences in figures from the engine. A summary line sits at the bottom of every step, with Back,
 * Quick start and Next always in the same place; Quick start begins at once with every choice not yet made left
 * at its default. The less used world options (economy, capital) sit behind one disclosure.
 */
@Composable
internal fun NewGameScreen(
    store: GameStore,
    startStep: Int = 0,
    startHelm: Boolean = false,
    startAmbition: String = "",
    startScenario: String? = null,
    startCustom: uniffi.cw_ffi.CustomScenarioDto? = null,
    onBack: () -> Unit,
) {
    val catalogue = remember { scenarios() }
    var chosen by remember { mutableStateOf(catalogue.firstOrNull { it.id == startScenario && it.available } ?: catalogue.firstOrNull { it.available }) }
    var customs by remember { mutableStateOf(uniffi.cw_ffi.customScenarioList(store.customDir)) }
    var chosenCustom by remember { mutableStateOf(startCustom) }
    var customizing by remember { mutableStateOf<Pair<String, uniffi.cw_ffi.CustomScenarioDto?>?>(null) }
    customizing?.let { (base, existing) ->
        ScenarioEditorScreen(store, baseId = base, existing = existing, onBack = {
            customs = uniffi.cw_ffi.customScenarioList(store.customDir)
            customizing = null
        })
        return
    }
    val steps = if (chosenCustom != null) listOf(CareerStep.Scenario, CareerStep.Start) else CareerStep.entries
    var step by remember { mutableStateOf(startStep) }
    val at = steps[step.coerceIn(0, steps.lastIndex)]
    BackAction { if (step == 0) onBack() else step-- }
    var mode by remember { mutableStateOf(if (startHelm) Mode.Helm else Mode.Guided) }
    val difficulties = remember { uniffi.cw_ffi.difficulties() }
    fun capitalOf(level: Level) = (difficulties.firstOrNull { it.key == level.key }?.startingCapitalCents ?: 0L) / 100.0
    var level by remember { mutableStateOf(Level.Standard) }
    var economy by remember { mutableStateOf(Economy.Historic) }
    // Null means the scenario's own default.
    var pricePathPick by remember { mutableStateOf<PricePath?>(null) }
    var history by remember { mutableStateOf(ScenarioHistory.Historical) }
    var capital by remember { mutableStateOf(capitalOf(Level.Standard)) }
    var advanced by remember { mutableStateOf(false) }
    val chiefs = remember { chiefChoices() }
    var chiefPick by remember { mutableStateOf(0) }
    var chiefName by remember { mutableStateOf("") }
    // One seed for the whole visit, so the companies offered at the helm are the ones the career starts with.
    val seed = remember { (System.nanoTime() % 9_999_999).toULong() }
    val career = remember { CareerChoice().also { it.ambition = startAmbition } }
    fun gameOptions(pick: ScenarioDto) = NewGameDto(
        seed = seed,
        scenarioId = pick.id,
        guided = mode.guided,
        fiveYear = mode.fiveYear,
        difficulty = level.key,
        economy = economy.key,
        startingCapitalCents = (capital * 100).toLong(),
        pricePath = pricePathPick?.key ?: "",
        timeline = history.key,
        chiefName = chiefName.trim(),
        chiefPortrait = chiefs.getOrNull(chiefPick)?.id ?: "",
        chiefTemperament = chiefs.getOrNull(chiefPick)?.temperament ?: "",
    )
    val helmScenario = if (chosenCustom == null && mode == Mode.Helm) chosen else null
    // The offers are worked out off the main thread, a moment after the last change, and again when a choice that changes them changes.
    val loaded by androidx.compose.runtime.produceState<Result<List<uniffi.cw_ffi.HelmOfferDto>>?>(null, helmScenario?.id, level, economy, capital, pricePathPick, history) {
        value = null
        val pick = helmScenario ?: return@produceState
        kotlinx.coroutines.delay(250)
        value = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) { runCatching { uniffi.cw_ffi.helmOffers(gameOptions(pick)) } }
    }
    val offers = loaded?.getOrNull()
    val offersError = if (loaded?.isFailure == true) "The companies on offer could not be worked out here. Found your own instead." else null
    LaunchedEffect(offers) {
        val list = offers ?: return@LaunchedEffect
        if (career.helmFirm == null || list.none { it.firm == career.helmFirm }) career.helmFirm = list.firstOrNull()?.firm
    }
    val scenario = chosen
    val fallbackPath = PricePath.entries.firstOrNull { it.key == scenario?.defaultPricePath } ?: PricePath.Historical
    val canStart = (scenario != null || chosenCustom != null) && !store.busy && (chosenCustom != null || mode != Mode.Helm || career.helmFirm != null)
    fun start() {
        chosenCustom?.let { store.startCustom(it, mode.guided, mode.fiveYear); return }
        val pick = scenario ?: return
        if (career.isDefault(mode == Mode.Helm)) store.startCareer(gameOptions(pick))
        else store.startCareerWith(gameOptions(pick), career.options(mode == Mode.Helm))
    }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val wide = maxWidth >= 900.dp
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            MenuTopBar("New career", { if (step == 0) onBack() else step-- }) {
                if (wide) {
                    steps.forEachIndexed { i, s ->
                        StepTab(i + 1, s.title, i == step) { step = i }
                        if (i < steps.lastIndex) Spacer(Modifier.width(10.dp))
                    }
                } else {
                    Text("${step + 1} OF ${steps.size}", color = Palette.gold, fontSize = 12.sp, fontFamily = Pixel.font)
                }
            }
            Column(
                Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(if (wide) 28.dp else 14.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                StepTitle(at.title.uppercase() + if (!wide) "" else "", when (at) {
                    CareerStep.Scenario -> "Where and when your career happens."
                    CareerStep.Start -> "What you hold on the first day."
                    CareerStep.Ambition -> "What the career is scored on, what each medal takes, and how long it runs."
                    CareerStep.World -> "How hard your rivals fight, and how the world's prices and history move."
                    CareerStep.Chief -> "Who runs your company. Their temperament is flavour; it does not change the rules."
                })
                when (at) {
                    CareerStep.Scenario -> {
                        val columns = if (wide) 2 else 1
                        catalogue.chunked(columns).forEach { row ->
                            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                                row.forEach { s ->
                                    ScenarioCard(
                                        s, selected = chosenCustom == null && chosen?.id == s.id, wide = wide, modifier = Modifier.weight(1f),
                                        onPick = { chosen = s; chosenCustom = null; pricePathPick = null }, onPlay = null, onCustomize = null,
                                    )
                                }
                                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                            }
                        }
                        if (customs.isNotEmpty()) {
                            Eyebrow("Your scenarios")
                            customs.forEach { c ->
                                CustomRow(c, catalogue, chosenCustom?.id == c.id, wide, onPick = { chosenCustom = c }, onPlay = null, onEdit = { customizing = c.baseScenarioId to c })
                            }
                        }
                    }
                    CareerStep.Start -> {
                        chosenCustom?.let {
                            Text("Difficulty, economy, capital, rivals and laws come from ${it.name}. Edit the scenario to change them.", color = Palette.secondary, fontSize = 13.sp)
                        }
                        Options(Mode.entries.filter { chosenCustom == null || it != Mode.Helm }, mode, { it.title }, { it.note }, wide) { mode = it }
                        if (mode == Mode.Helm && chosenCustom == null) {
                            Eyebrow("The company you take over")
                            HelmOfferGrid(offers, offersError, career.helmFirm, wide) { career.helmFirm = it }
                        }
                    }
                    CareerStep.Ambition -> AmbitionPicker(mode == Mode.Helm, career, wide)
                    CareerStep.World -> {
                        Section("Difficulty") {
                            Options(Level.entries, level, { it.title }, { "${Format.compact((capitalOf(it) * 100).toLong())} to start. ${it.rivals}" }, wide) {
                                level = it
                                capital = capitalOf(it)
                            }
                        }
                        Section("World commodity prices") {
                            Options(PricePath.entries, pricePathPick ?: fallbackPath, { it.title + if (it == fallbackPath) " (scenario default)" else "" }, { it.note }, wide) { pricePathPick = it }
                        }
                        Section("Timeline of real events") {
                            Options(ScenarioHistory.entries, history, { it.title }, { it.note }, wide) { history = it }
                        }
                        Row(
                            Modifier.clip(Pixel.small).clickable { advanced = !advanced }.padding(vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text((if (advanced) "- " else "+ ") + "MORE WORLD OPTIONS", color = Palette.gold, fontSize = 13.sp, fontFamily = Pixel.font)
                            Spacer(Modifier.width(10.dp))
                            Text("Economy on day one: ${economy.title}. Starting capital: ${Format.compact((capital * 100).toLong())}.", color = Palette.secondary, fontSize = 12.sp)
                        }
                        if (advanced) {
                            Section("The economy on day one") {
                                Options(Economy.entries, economy, { it.title }, { it.note }, wide) { economy = it }
                            }
                            Section("Starting capital") {
                                ValueSlider(
                                    capital, { capital = it }, 1_000_000.0..20_000_000.0, 250_000.0,
                                    modifier = Modifier.widthIn(max = 760.dp),
                                    label = { Format.compact((it * 100).toLong()) },
                                    detail = {
                                        if (mode.guided) "Your own cash. Taking over the company uses some of it; the rest is yours to invest."
                                        else "Your own cash to found and fund companies. ${level.title} gives ${Format.compact((capitalOf(level) * 100).toLong())}."
                                    },
                                    ticks = 19,
                                )
                            }
                        }
                    }
                    CareerStep.Chief -> ChiefPicker(chiefs, chiefPick, chiefName, { chiefPick = it }, { chiefName = it.take(40) })
                }
            }
            Box(Modifier.fillMaxWidth().height(2.dp).background(Palette.stroke))
            // The summary and the buttons, the same on every step.
            Column(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = if (wide) 20.dp else 12.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val summary = chosenCustom?.let { listOf(it.name, mode.title) } ?: listOfNotNull(
                    scenario?.let { "${it.name}" },
                    mode.title + if (mode == Mode.Helm) offers?.firstOrNull { it.firm == career.helmFirm }?.let { ": ${it.name}" }.orEmpty() else "",
                    if (career.ambition.isEmpty()) "No ambition" else "${ambitionWords(career.ambition)}, ${if (career.years == 0) uniffi.cw_ffi.careerLengths().defaultYears.toInt() else career.years} years",
                    "${level.title}, ${Format.compact((capital * 100).toLong())}",
                    "Prices ${(pricePathPick ?: fallbackPath).title.lowercase()}",
                    "Timeline ${history.title.lowercase()}",
                )
                Text(summary.joinToString("  |  "), color = Palette.primaryText, fontSize = 13.sp, maxLines = 2)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    MenuBarButton("Back") { if (step == 0) onBack() else step-- }
                    Spacer(Modifier.weight(1f))
                    if (step < steps.lastIndex) MenuBarButton("Quick start", enabled = canStart) { start() }
                    if (step < steps.lastIndex) GoldButton("Next") { step++ }
                    else GoldButton("Start career", enabled = canStart) { start() }
                }
            }
        }
    }
}

@Composable
private fun StepTab(number: Int, label: String, active: Boolean, onClick: () -> Unit) {
    Row(Modifier.clip(Pixel.small).clickable(onClick = onClick).padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
            Modifier.width(22.dp).height(22.dp).clip(Pixel.small).background(if (active) Palette.gold else Palette.panelRaised),
            contentAlignment = Alignment.Center,
        ) { Text("$number", color = if (active) Palette.ink else Palette.secondary, fontSize = 12.sp, fontWeight = FontWeight.Bold) }
        Spacer(Modifier.width(6.dp))
        Text(label, color = if (active) Palette.primaryText else Palette.secondary, fontSize = 13.sp)
    }
}

@Composable
private fun StepTitle(title: String, note: String) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(title, color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
        Text(note, color = Palette.secondary, fontSize = 13.sp)
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
            Column(
                (if (wide) Modifier.width(270.dp) else Modifier.fillMaxWidth()).padding(end = 3.dp, bottom = 3.dp)
                    .pixelFrame(if (on) Color(0xFF2A2A1C) else Palette.panel, if (on) Palette.gold else Palette.stroke)
                    .clip(Pixel.shape)
                    .clickable { onPick(item) }.padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(title(item), color = if (on) Palette.gold else Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                Text(note(item), color = Palette.secondary, fontSize = 12.sp)
            }
        }
    }
}

/** The new-game screen's chief executive: a few portraits from the generated kit, one for each
 *  temperament, an optional name, and the temperament as a line of flavour. */
@Composable
internal fun ChiefPicker(
    choices: List<uniffi.cw_ffi.PersonDto>,
    picked: Int,
    name: String,
    onPick: (Int) -> Unit,
    onName: (String) -> Unit,
) {
    if (choices.isEmpty()) return
    val current = choices[picked.coerceIn(0, choices.lastIndex)]
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        choices.forEachIndexed { i, c ->
            Box(
                Modifier.clip(Pixel.small)
                    .border(2.dp, if (i == picked) Palette.gold else Palette.panelRaised, Pixel.small)
                    .clickable { onPick(i) }
                    .padding(3.dp),
            ) { PixelPortrait(c.id, size = 52.dp) }
        }
    }
    Text("${temperamentLabel(current.temperament)}: ${temperamentNote(current.temperament)}", color = Palette.secondary, fontSize = 12.sp)
    OutlinedTextField(
        name, onName,
        label = { Text("Name (blank keeps ${current.name})") },
        singleLine = true,
        modifier = Modifier.textEntry().widthIn(max = 360.dp),
    )
}
