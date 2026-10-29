package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.io.File
import java.util.Properties

/** How much the map draws. Lower detail is lighter on older devices. */
enum class MapDetail(val title: String, val note: String) {
    Low("Low", "Countries and the scenario only"),
    Medium("Medium", "Adds state and province borders"),
    High("High", "Adds highways, counties and rivers"),
}

/** Where first-run setup stands, from the `setupDone` settings key. */
enum class SetupState { Fresh, Upgrade, Done }

/** How the desktop window sits on screen. */
enum class WindowMode(val title: String) { Windowed("Windowed"), Maximized("Maximized"), Fullscreen("Full screen") }

/** Window sizes offered on desktop, in display points. */
val Resolutions = listOf(1280 to 720, 1366 to 768, 1600 to 900, 1920 to 1080, 2560 to 1440)

/**
 * Player settings that live outside any career, saved to a small file beside the saves. Desktop
 * builds also read the window settings here.
 */
/** Window pixels above which a first start suggests Medium: 2.5 million, a little over 1920 by 1200. */
internal const val SUGGEST_MEDIUM_ABOVE = 2_500_000L

class AppSettings(private val file: File) {
    /** Whether a settings file was already there: a player upgrading from 1.8 has one, a first launch does not. */
    private val existedAtLaunch = file.exists()
    private val props = Properties().apply { if (file.exists()) file.inputStream().use { load(it) } }

    /**
     * The graphics options. A settings file from before presets keeps its map detail and
     * animations and takes High for the rest.
     */
    var graphics by mutableStateOf(loadGraphics()); private set

    /** True once the player (or an older version of the game) has saved a graphics choice; a suggested preset is never saved. */
    private var graphicsChosen = props.getProperty("preset") != null || props.getProperty("mapDetail") != null
    val mapDetail: MapDetail get() = graphics.detail
    val animations: Boolean get() = graphics.animations
    var autoUpdate by mutableStateOf(props.getProperty("autoUpdate")?.toBoolean() ?: true); private set
    /** Full releases or previews; a new install follows the channel its build came from. */
    var updateChannel by mutableStateOf(props.getProperty("updateChannel")?.let(UpdateChannel::of) ?: UpdateChannel.build); private set
    var tutorialDone: Set<String> = props.getProperty("tutorial.done")?.split(',')?.filter { it.isNotEmpty() }?.toSet() ?: emptySet(); private set
    var tutorialHidden: Boolean = props.getProperty("tutorial.hidden")?.toBoolean() ?: false; private set
    /** Currency id the investor's figures are shown in; empty until the player picks one. */
    var reportingCurrency: String = props.getProperty("reportingCurrency").orEmpty(); private set
    /** Anonymous market-balance snapshots every five game weeks; on unless the player switches it off. */
    var analytics by mutableStateOf(props.getProperty("analytics")?.toBoolean() ?: true); private set
    /** Also stop the clock for warnings, not only for decisions that need an answer. */
    var pauseOnWarnings by mutableStateOf(props.getProperty("pauseOnWarnings")?.toBoolean() ?: false); private set
    /** Open the weekly brief by itself when a new week starts with time paused or at 1x. Desktop only. */
    var showBriefEachWeek by mutableStateOf(props.getProperty("showBriefEachWeek")?.toBoolean() ?: false); private set
    /** The brief panel at the top of the CEO Office is folded away. */
    var briefCollapsed by mutableStateOf(props.getProperty("briefCollapsed")?.toBoolean() ?: false); private set
    var uiScale by mutableStateOf(props.getProperty("uiScale")?.toFloatOrNull() ?: 1f); private set
    var windowMode by mutableStateOf(enumOr(props.getProperty("windowMode"), WindowMode.Windowed)); private set
    var resolution by mutableStateOf(
        props.getProperty("resolution")?.split("x")?.mapNotNull { it.toIntOrNull() }?.takeIf { it.size == 2 }?.let { it[0] to it[1] }
            ?: (1600 to 900),
    ); private set
    /** The `setupDone` key: "true" once setup is finished, "false" while a first launch has not finished it, absent in a 1.8 file. */
    private var setupFlag: String? = props.getProperty("setupDone") ?: if (existedAtLaunch) null else "false".also { props.setProperty("setupDone", it) }

    /** Where the first-run setup stands: not yet seen, a 1.8 player who has not seen What's new, or finished. */
    var setupState by mutableStateOf(
        when {
            setupFlag == "true" -> SetupState.Done
            setupFlag == null -> SetupState.Upgrade
            else -> SetupState.Fresh
        },
    ); private set

    /** Finished or left careers, newest first: a few lines each, for the main menu's Hall of Careers. */
    var careers by mutableStateOf(loadCareers()); private set

    /** Keep a career's summary. A career left again replaces its earlier line instead of adding one. */
    fun recordCareer(summary: CareerSummary) {
        careers = (listOf(summary) + careers.filterNot { it.title == summary.title && it.scenario == summary.scenario }).take(MAX_CAREERS)
        store()
    }

    private fun loadCareers(): List<CareerSummary> =
        props.getProperty("careers")?.let { runCatching { CareerSummary.decode(it) }.getOrNull() } ?: emptyList()

    /** A random id for this install, not tied to any account or device identifier. */
    val installId: String = props.getProperty("installId")?.takeIf { it.length == 36 }
        ?: java.util.UUID.randomUUID().toString().also { props.setProperty("installId", it); store() }

    /** Mark setup finished (or What's new dismissed); never touches any other choice. */
    fun finishSetup() { setupFlag = "true"; setupState = SetupState.Done; store() }

    fun chooseGraphics(value: GraphicsConfig) { graphics = value; graphicsChosen = true; store() }

    /**
     * On a first start with no graphics choice saved, a big window or software drawing starts at Medium instead of High,
     * so the first look is smooth. Judged from the window's pixels, not a benchmark. Never replaces a saved choice and is
     * not saved itself, so it is judged again at the next start until the player picks a preset. Returns whether it applied.
     */
    fun suggestGraphics(windowPixels: Long, software: Boolean): Boolean {
        if (graphicsChosen || !(software || windowPixels > SUGGEST_MEDIUM_ABOVE)) return false
        graphics = GraphicsPreset.Medium.config(graphics.animations, graphics.showFps)
        return true
    }
    fun choosePreset(preset: GraphicsPreset) = chooseGraphics(preset.config(graphics.animations, graphics.showFps).copy(newMap = graphics.newMap))
    fun chooseAutoUpdate(value: Boolean) { autoUpdate = value; store() }
    fun chooseUpdateChannel(value: UpdateChannel) { updateChannel = value; store() }
    fun chooseAnalytics(value: Boolean) { analytics = value; store() }
    fun chooseTutorial(done: Set<String>, hidden: Boolean) { tutorialDone = done; tutorialHidden = hidden; store() }
    fun chooseReportingCurrency(value: String) { reportingCurrency = value; store() }
    fun choosePauseOnWarnings(value: Boolean) { pauseOnWarnings = value; store() }
    fun chooseShowBriefEachWeek(value: Boolean) { showBriefEachWeek = value; store() }
    fun chooseBriefCollapsed(value: Boolean) { briefCollapsed = value; store() }
    fun chooseUiScale(value: Float) { uiScale = value; store() }
    fun chooseWindowMode(value: WindowMode) { windowMode = value; store() }
    fun chooseResolution(value: Pair<Int, Int>) { resolution = value; store() }

    private fun store() {
        if (graphicsChosen) graphics.let { g ->
            props.setProperty("mapDetail", g.detail.name)
            props.setProperty("animations", g.animations.toString())
            props.setProperty("preset", g.preset?.name?.lowercase() ?: "custom")
            props.setProperty("tilt", g.tilt.toString())
            props.setProperty("extrude", g.extrude.toString())
            props.setProperty("lighting", g.lighting.toString())
            props.setProperty("dayNight", g.dayNight.toString())
            props.setProperty("cityLights", g.cityLights.toString())
            props.setProperty("earth", g.earth.toString())
            props.setProperty("clouds", g.clouds.toString())
            props.setProperty("traffic", g.traffic.toString())
            props.setProperty("buildings", g.buildings.toString())
            props.setProperty("frameRate", g.frameRate.toString())
            props.setProperty("showFps", g.showFps.toString())
            props.setProperty("countryside", g.countryside.toString())
            props.setProperty("rivals", g.rivals.toString())
            props.setProperty("interstates", g.interstates.toString())
            props.setProperty("seasons", g.seasons.toString())
            props.setProperty("labels", g.labels.toString())
            props.setProperty("newMap", g.newMap.toString())
        }
        setupFlag?.let { props.setProperty("setupDone", it) }
        props.setProperty("careers", CareerSummary.encode(careers))
        props.setProperty("autoUpdate", autoUpdate.toString())
        props.setProperty("updateChannel", updateChannel.id)
        props.setProperty("analytics", analytics.toString())
        props.setProperty("tutorial.done", tutorialDone.joinToString(","))
        props.setProperty("tutorial.hidden", tutorialHidden.toString())
        props.setProperty("reportingCurrency", reportingCurrency)
        props.setProperty("pauseOnWarnings", pauseOnWarnings.toString())
        props.setProperty("showBriefEachWeek", showBriefEachWeek.toString())
        props.setProperty("briefCollapsed", briefCollapsed.toString())
        props.setProperty("uiScale", uiScale.toString())
        props.setProperty("windowMode", windowMode.name)
        props.setProperty("resolution", "${resolution.first}x${resolution.second}")
        runCatching {
            file.parentFile?.mkdirs()
            file.outputStream().use { props.store(it, "Corporate Warfare settings") }
        }
    }

    private fun loadGraphics(): GraphicsConfig {
        val base = GraphicsConfig.Default.copy(
            detail = enumOr(props.getProperty("mapDetail"), MapDetail.High),
            animations = props.getProperty("animations")?.toBoolean() ?: true,
        )
        if (props.getProperty("preset") == null) return base
        fun int(key: String, fallback: Int, range: IntRange) = props.getProperty(key)?.toIntOrNull()?.takeIf { it in range } ?: fallback
        fun bool(key: String, fallback: Boolean) = props.getProperty(key)?.toBooleanStrictOrNull() ?: fallback
        return base.copy(
            tilt = int("tilt", base.tilt, 0..2),
            extrude = bool("extrude", base.extrude),
            lighting = int("lighting", base.lighting, 0..2),
            dayNight = bool("dayNight", base.dayNight),
            cityLights = bool("cityLights", base.cityLights),
            earth = bool("earth", base.earth),
            clouds = int("clouds", base.clouds, 0..3),
            traffic = int("traffic", base.traffic, 0..2),
            buildings = int("buildings", base.buildings, 0..2),
            frameRate = props.getProperty("frameRate")?.toIntOrNull()?.takeIf { it in listOf(0, 30, 60) } ?: base.frameRate,
            showFps = bool("showFps", base.showFps),
            countryside = bool("countryside", base.countryside),
            rivals = bool("rivals", base.rivals),
            interstates = bool("interstates", base.interstates),
            seasons = bool("seasons", base.seasons),
            labels = int("labels", base.labels, 0..2),
            newMap = bool("newMap", base.newMap),
        )
    }

    private inline fun <reified T : Enum<T>> enumOr(name: String?, fallback: T): T =
        enumValues<T>().firstOrNull { it.name == name } ?: fallback
}

/**
 * The full settings page, from the main menu or a running career: game, graphics and, on
 * desktop, the window.
 */
@Composable
fun SettingsScreen(store: GameStore, settings: AppSettings, desktop: Boolean, updater: Updater? = null, onSetupAgain: (() -> Unit)? = null, onClose: () -> Unit) {
    TabList {
        item { ScreenHeader("Settings", null) { TextButton(onClick = onClose) { Text("Done", color = Palette.gold) } } }
        if (store.ready) {
            item {
                Panel {
                    Eyebrow("Game")
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text("Save", color = Palette.primaryText)
                            Text("The game also saves itself every four weeks and when you leave.", color = Palette.secondary, fontSize = 12.sp)
                        }
                        TextButton(onClick = { store.saveNow() }) { Text("Save now", color = Palette.gold) }
                    }
                    if (store.status.isNotEmpty()) Text(store.status, color = Palette.secondary, fontSize = 12.sp)
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text("Stop for warnings too", color = Palette.primaryText)
                            Text("Time always stops for a decision that needs your answer. Turn this on to also stop when a company runs short of grace, or workers strike.", color = Palette.secondary, fontSize = 12.sp)
                        }
                        Switch(
                            checked = settings.pauseOnWarnings,
                            onCheckedChange = { settings.choosePauseOnWarnings(it) },
                            colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold),
                        )
                    }
                    if (desktop) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text("Show the brief each week", color = Palette.primaryText)
                                Text("Opens the weekly brief when a new week starts while time is paused or at normal speed. It never opens at faster speeds.", color = Palette.secondary, fontSize = 12.sp)
                            }
                            Switch(
                                checked = settings.showBriefEachWeek,
                                onCheckedChange = { settings.chooseShowBriefEachWeek(it) },
                                colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold),
                            )
                        }
                    }
                }
            }
        }
        val fx = store.economy?.fx
        if (store.ready && fx != null) {
            item {
                Panel {
                    Eyebrow("Currency")
                    Text("Show my cash and net worth in", color = Palette.primaryText)
                    CurrencyChoices(store, fx)
                    Text("Each company keeps its own currency. This only changes how your own figures are shown.", color = Palette.secondary, fontSize = 12.sp)
                }
            }
        }
        item {
            Panel {
                Eyebrow("Graphics")
                GraphicsChoices(settings)
                Text("Interface size", color = Palette.primaryText)
                Choices(listOf(0.85f, 1f, 1.15f, 1.3f), settings.uiScale, { "${(it * 100).toInt()}%" }) { settings.chooseUiScale(it) }
            }
        }
        if (desktop) {
            item {
                Panel {
                    Eyebrow("Display")
                    Text("Window", color = Palette.primaryText)
                    Choices(WindowMode.entries, settings.windowMode, { it.title }) { settings.chooseWindowMode(it) }
                    Text("Resolution", color = Palette.primaryText)
                    Choices(Resolutions, settings.resolution, { "${it.first} x ${it.second}" }) {
                        settings.chooseResolution(it)
                        if (settings.windowMode != WindowMode.Windowed) settings.chooseWindowMode(WindowMode.Windowed)
                    }
                    Text("Resolution sets the window size; full screen uses the whole display.", color = Palette.secondary, fontSize = 12.sp)
                }
            }
        }
        if (onSetupAgain != null) {
            item {
                Panel {
                    Eyebrow("Setup")
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text("First-run setup", color = Palette.primaryText)
                            Text("Test your graphics again and review updates, privacy and the tutorial.", color = Palette.secondary, fontSize = 12.sp)
                        }
                        TextButton(onClick = onSetupAgain) { Text("Set up again", color = Palette.gold) }
                    }
                }
            }
        }
        item {
            Panel {
                Eyebrow("Tutorial")
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        if (Tutorial.finished) "Tutorial complete" else "${Tutorial.done.size} of ${Tutorial.steps.size} steps done",
                        color = Palette.primaryText,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = { Tutorial.restart() }) { Text("Start over", color = Palette.gold) }
                }
            }
        }
        item {
            Panel {
                Eyebrow("Analytics")
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Share anonymous market data", color = Palette.primaryText)
                        Text(
                            Privacy.explanation,
                            color = Palette.secondary,
                            fontSize = 12.sp,
                        )
                    }
                    Switch(
                        checked = settings.analytics,
                        onCheckedChange = { settings.chooseAnalytics(it) },
                        colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold),
                    )
                }
            }
        }
        if (updater != null) {
            item { UpdatesSection(settings, updater) }
        }
        item {
            Panel {
                Eyebrow("About")
                val info = uniffi.cw_ffi.engineInfo()
                Text("Corporate Warfare ${BuildInfo.VERSION} · engine ${info.version}", color = Palette.secondary, fontSize = 12.sp)
                Text("Institutions are simplified game rules, not legal, tax or investment advice.", color = Palette.secondary, fontSize = 11.sp)
                Text("Clouds: Solar System Scope, CC BY 4.0, based on NASA Blue Marble imagery.", color = Palette.secondary, fontSize = 11.sp)
            }
        }
    }
}

/** The quality preset, then every graphics option with its note. */
@Composable
private fun GraphicsChoices(settings: AppSettings) {
    val g = settings.graphics
    Text("Quality", color = Palette.primaryText)
    Choices<GraphicsPreset?>(GraphicsPreset.entries, g.preset, { it?.title.orEmpty() }) { it?.let(settings::choosePreset) }
    Text(
        if (g.preset == null) "Custom: your own mix of the options below." else "Picking a preset sets every option below. Change any of them to make your own mix.",
        color = Palette.secondary, fontSize = 12.sp,
    )
    graphicsOptions.filter { !it.gpuOnly || GpuMapHost.opener != null }.forEach { option ->
        Column(Modifier.padding(top = 6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(option.title, color = Palette.primaryText)
            Choices(option.choices.indices.toList(), option.get(g), { option.choices[it] }) { settings.chooseGraphics(option.set(g, it)) }
            Text(option.note(g), color = Palette.secondary, fontSize = 12.sp)
        }
    }
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
internal fun <T> Choices(options: List<T>, selected: T, label: (T) -> String, onPick: (T) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        options.forEach { option ->
            val on = option == selected
            Text(
                label(option),
                color = if (on) Palette.ink else Palette.primaryText,
                fontSize = 12.sp,
                fontFamily = Pixel.font,
                modifier = Modifier.clip(Pixel.small)
                    .background(if (on) Palette.gold else Palette.panelRaised)
                    .clickable { onPick(option) }
                    .padding(horizontal = 12.dp, vertical = 7.dp),
            )
        }
    }
}

/**
 * The Updates section: check at launch, which channel to follow, the update status and the list
 * of versions the player can move to. [showVersions] opens the list straight away (for tests).
 */
@Composable
fun UpdatesSection(settings: AppSettings, updater: Updater, showVersions: Boolean = false) {
    var open by remember { mutableStateOf(showVersions) }
    Panel {
        Eyebrow("Updates")
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Check for updates at launch", color = Palette.primaryText)
                Text("Reads the public release list on GitHub. Nothing about you or your game is sent.", color = Palette.secondary, fontSize = 12.sp)
            }
            Switch(
                checked = settings.autoUpdate,
                onCheckedChange = { settings.chooseAutoUpdate(it) },
                colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold),
            )
        }
        Text("Get", color = Palette.primaryText)
        Choices(UpdateChannel.entries, settings.updateChannel, { it.title }) {
            settings.chooseUpdateChannel(it)
            updater.channelChanged()
        }
        Text(settings.updateChannel.note, color = Palette.secondary, fontSize = 12.sp)
        if (settings.updateChannel == UpdateChannel.Stable && Versions.isPreview(updater.current)) {
            Text(
                "You are on a preview. You will be offered the newest full release from ${Versions.MINIMUM} on, even if its number " +
                    "is lower than this preview, and your saves are backed up before going back." +
                    if (updater.status is UpdateStatus.Available) "" else " There is none yet, so you stay on this preview until one comes out.",
                color = Palette.warning, fontSize = 12.sp,
            )
        }
        UpdatePanel(updater, compact = true)
        if (open) {
            VersionPicker(updater)
        } else {
            TextButton(onClick = { open = true; updater.loadVersions() }) { Text("Choose a version", color = Palette.gold) }
        }
    }
}

/** Every version the player can move to, newest first, the installed one marked. */
@Composable
private fun VersionPicker(updater: Updater) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("Versions", color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
        Text(
            "Versions before ${Versions.MINIMUM} are not listed: they cannot open newer saves or come back here by themselves.",
            color = Palette.secondary, fontSize = 11.sp,
        )
        if (updater.downgrade == Downgrade.UninstallFirst) {
            Text(
                "Android will not put an older version over a newer one. Going back means uninstalling first, which deletes the careers saved on this device.",
                color = Palette.secondary, fontSize = 11.sp,
            )
        }
        when (val list = updater.versions) {
            VersionList.Idle, VersionList.Loading -> Text("Loading the release list...", color = Palette.secondary, fontSize = 12.sp)
            is VersionList.Failed -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text(list.reason, color = Palette.warning, fontSize = 12.sp, modifier = Modifier.weight(1f))
                TextButton(onClick = { updater.loadVersions() }) { Text("Retry", color = Palette.gold) }
            }
            is VersionList.Loaded -> {
                if (list.releases.isEmpty()) Text("No versions on this channel yet.", color = Palette.secondary, fontSize = 12.sp)
                list.releases.forEach { release -> VersionRow(updater, release) }
            }
        }
    }
}

@Composable
private fun VersionRow(updater: Updater, release: Release) {
    val installed = release.version == updater.current
    val older = Versions.isDowngrade(release.version, updater.current)
    Row(
        Modifier.fillMaxWidth().clip(Pixel.small).background(if (installed) Palette.panelRaised else Palette.panel).padding(horizontal = 10.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                release.version + (if (release.prerelease) " · preview" else "") + (if (release.date.isNotEmpty()) " · ${release.date}" else ""),
                color = if (installed) Palette.gold else Palette.primaryText, fontSize = 13.sp,
            )
            val note = Versions.shortNote(release.notes)
            if (note.isNotEmpty()) Text(note, color = Palette.secondary, fontSize = 11.sp, maxLines = 2)
        }
        when {
            installed -> Text("Installed", color = Palette.positive, fontSize = 12.sp, modifier = Modifier.padding(start = 8.dp))
            older && updater.downgrade == Downgrade.UninstallFirst ->
                TextButton(onClick = { updater.choose(release) }) { Text("How to go back", color = Palette.secondary) }
            else -> TextButton(onClick = { updater.choose(release) }) {
                Text(if (older) "Go back" else "Install", color = if (older) Palette.secondary else Palette.gold)
            }
        }
    }
}

/** Version, update status and the one action that applies to it. */
@Composable
fun UpdatePanel(updater: Updater, compact: Boolean = false) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        when (val status = updater.status) {
            UpdateStatus.Idle -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Version ${updater.current}", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f))
                TextButton(onClick = { updater.check() }) { Text("Check for updates", color = Palette.gold) }
            }
            UpdateStatus.Checking -> Text("Version ${updater.current} · checking for updates…", color = Palette.secondary, fontSize = 12.sp)
            UpdateStatus.Current -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Version ${updater.current} · up to date", color = Palette.positive, fontSize = 12.sp, modifier = Modifier.weight(1f))
                if (compact) TextButton(onClick = { updater.check() }) { Text("Check again", color = Palette.secondary) }
            }
            is UpdateStatus.Failed -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Version ${updater.current} · ${status.reason}", color = Palette.warning, fontSize = 12.sp, modifier = Modifier.weight(1f))
                TextButton(onClick = { updater.check() }) { Text("Retry", color = Palette.gold) }
            }
            is UpdateStatus.Available -> {
                if (Versions.isDowngrade(status.release.version, updater.current)) {
                    Text("${status.release.version} is the newest full release (you have preview ${updater.current})", color = Palette.gold, fontWeight = FontWeight.SemiBold)
                    Text("Its number is lower than your preview, so this goes back to it.", color = Palette.secondary, fontSize = 12.sp)
                } else {
                    Text("Version ${status.release.version} is out (you have ${updater.current})", color = Palette.gold, fontWeight = FontWeight.SemiBold)
                }
                if (!compact) {
                    val notes = status.release.notes.lines().filter { it.startsWith("- ") }.take(4)
                    notes.forEach { Text(it.removePrefix("- ").take(120), color = Palette.secondary, fontSize = 12.sp, maxLines = 2) }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    val label = when {
                        Versions.isDowngrade(status.release.version, updater.current) -> "Go back to ${status.release.version}"
                        status.release.installer != null -> "Update now"
                        else -> "Get the update"
                    }
                    GoldButton(label) { updater.choose(status.release) }
                    TextButton(onClick = { updater.openPage(status.release) }) { Text("What's new", color = Palette.secondary) }
                }
            }
            is UpdateStatus.ConfirmDowngrade -> {
                Text("Go back to ${status.release.version}?", color = Palette.warning, fontWeight = FontWeight.SemiBold)
                Text(
                    "Careers saved by this version (${updater.current}) may not open in ${status.release.version}. " +
                        "Before installing, your saves are copied to:",
                    color = Palette.primaryText, fontSize = 12.sp,
                )
                status.backup?.let { Text(it.path, color = Palette.gold, fontSize = 12.sp) }
                Text("To play those careers again, update back to ${updater.current} or later.", color = Palette.secondary, fontSize = 12.sp)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    GoldButton("Back up and install ${status.release.version}") { updater.confirmDowngrade(status) }
                    TextButton(onClick = { updater.cancel() }) { Text("Cancel", color = Palette.secondary) }
                }
            }
            is UpdateStatus.BackedUp -> {
                Text("Your saves are copied to:", color = Palette.primaryText, fontSize = 12.sp)
                Text(status.backup.path, color = Palette.gold, fontSize = 12.sp)
                Text(
                    "The page for ${status.release.version} is open. Install its package from there; on Linux: " +
                        "sudo apt install --allow-downgrades ./<the .deb file>",
                    color = Palette.secondary, fontSize = 12.sp,
                )
                TextButton(onClick = { updater.cancel() }) { Text("Done", color = Palette.gold) }
            }
            is UpdateStatus.NeedsUninstall -> {
                Text("Going back to ${status.release.version} needs an uninstall", color = Palette.warning, fontWeight = FontWeight.SemiBold)
                Text(
                    "Android will not install an older version over a newer one. Uninstalling Corporate Warfare deletes every career " +
                        "saved on this device, and this version has no way to copy them off it. Only do this if you are happy to start again.",
                    color = Palette.primaryText, fontSize = 12.sp,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { updater.openPage(status.release) }) { Text("Open its page", color = Palette.secondary) }
                    TextButton(onClick = { updater.cancel() }) { Text("Cancel", color = Palette.gold) }
                }
            }
            is UpdateStatus.Downloading -> {
                Text("Downloading ${status.release.version}… ${(status.fraction * 100).toInt()}%", color = Palette.primaryText, fontSize = 12.sp)
                androidx.compose.material3.LinearProgressIndicator(
                    progress = { status.fraction },
                    modifier = Modifier.fillMaxWidth(),
                    color = Palette.gold,
                    trackColor = Palette.panelRaised,
                )
            }
        }
    }
}

/** One chip per currency of the scenario; picking one re-quotes the investor's figures. */
@Composable
fun CurrencyChoices(store: GameStore, fx: uniffi.cw_ffi.FxDto) {
    Choices(fx.currencies.map { it.id }, fx.reporting, { id -> fx.currencies.firstOrNull { it.id == id }?.let { "${it.symbol} ${it.name}" } ?: id }) {
        store.chooseReportingCurrency(it)
    }
}
