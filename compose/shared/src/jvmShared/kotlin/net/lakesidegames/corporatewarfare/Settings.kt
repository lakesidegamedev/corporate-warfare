package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
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

const val DOCK_MIN = 0.2f
const val DOCK_MAX = 0.8f
const val DOCK_DEFAULT = 0.45f

class AppSettings(private val file: File) : ViewPrefs {
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
    /** Desktop: use the 1.x page layout instead of the 2.0 layout (kept for 2.0 only). */
    var classicLayout by mutableStateOf(props.getProperty("classicLayout")?.toBoolean() ?: false); private set
    /** The brief panel at the top of the CEO Office is folded away. */
    var briefCollapsed by mutableStateOf(props.getProperty("briefCollapsed")?.toBoolean() ?: false); private set
    /** Pages with the map docked beside them, and how much of the width the map takes (0.2 to 0.8), per page id. */
    val docks = androidx.compose.runtime.mutableStateMapOf<String, Float>().apply {
        props.stringPropertyNames().filter { it.startsWith("dock.") }.forEach { key ->
            props.getProperty(key)?.toFloatOrNull()?.let { put(key.removePrefix("dock."), it.coerceIn(DOCK_MIN, DOCK_MAX)) }
        }
    }
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
    fun chooseClassicLayout(value: Boolean) { classicLayout = value; store() }
    fun chooseBriefCollapsed(value: Boolean) { briefCollapsed = value; store() }
    /** Dock the map beside a page with the given share of the width, or undock it with null. */
    fun chooseDock(pageId: String, share: Float?) {
        if (share == null) docks.remove(pageId) else docks[pageId] = share.coerceIn(DOCK_MIN, DOCK_MAX)
        store()
    }
    fun chooseUiScale(value: Float) { uiScale = value; store() }
    fun chooseWindowMode(value: WindowMode) { windowMode = value; store() }
    fun chooseResolution(value: Pair<Int, Int>) { resolution = value; store() }

    /** Put one section of Settings back to how the game came. Saves, careers, views and the tutorial are not touched. */
    fun resetSection(section: SettingsSection) {
        when (section) {
            SettingsSection.Game -> { pauseOnWarnings = false; showBriefEachWeek = false }
            SettingsSection.Graphics -> {
                graphics = GraphicsConfig.Default; graphicsChosen = true
                windowMode = WindowMode.Windowed; resolution = 1600 to 900
            }
            SettingsSection.Interface -> { uiScale = 1f; classicLayout = false }
            SettingsSection.Data -> analytics = true
            SettingsSection.Updates -> { autoUpdate = true; updateChannel = UpdateChannel.build }
            SettingsSection.Keys, SettingsSection.About -> Unit
        }
        store()
    }

    fun resetAll() = SettingsSection.entries.forEach(::resetSection)

    /** Saved grid views and the game's timeframe (the 2.0 grids): kept here so they outlast any one career. */
    override fun read(key: String): String? = props.getProperty(key)
    override fun write(key: String, value: String?) {
        if (value == null) props.remove(key) else props.setProperty(key, value)
        store()
    }

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
            props.setProperty("map3d", g.newMap.toString())
            g.directMap?.let { props.setProperty("directMap", it.toString()) }
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
        props.setProperty("classicLayout", classicLayout.toString())
        props.setProperty("briefCollapsed", briefCollapsed.toString())
        props.stringPropertyNames().filter { it.startsWith("dock.") }.forEach { props.remove(it) }
        docks.forEach { (id, share) -> props.setProperty("dock.$id", share.toString()) }
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
            // A new key from 2.0: the 3D map is on unless the player turns it off (the 1.x key defaulted to off).
            newMap = bool("map3d", base.newMap),
            directMap = props.getProperty("directMap")?.toBooleanStrictOrNull() ?: base.directMap,
        )
    }

    private inline fun <reified T : Enum<T>> enumOr(name: String?, fallback: T): T =
        enumValues<T>().firstOrNull { it.name == name } ?: fallback
}

/** The sections of Settings, down the side. There is no Sound section: the game has no sound yet. */
enum class SettingsSection(val title: String, val resettable: Boolean) {
    Game("Game", true), Graphics("Graphics", true), Interface("Interface", true), Keys("Controls and keys", false),
    Data("Data and privacy", true), Updates("Updates", true), About("About", false),
}

/** One setting: what it is called, one plain sentence on what it does, extra words search finds it by, and its control. */
internal class SettingRow(
    val section: SettingsSection,
    val title: String,
    val note: String,
    val words: String = "",
    /** The control sits under the words (chips, lists) rather than at the right (a switch, a button). */
    val below: Boolean = false,
    val control: @Composable () -> Unit = {},
)

/** Whether a row matches what was typed in the search box: every word must appear somewhere in it. */
internal fun SettingRow.matches(query: String): Boolean {
    val hay = "$title $note $words ${section.title}".lowercase()
    return query.lowercase().split(' ').filter { it.isNotBlank() }.all { it in hay }
}

/** What this machine can do with the 3D map, in one plain sentence, and whether that is a problem. */
internal fun mapSupport(): Pair<String, Boolean> = when {
    GpuMapHost.opener == null ->
        "The 3D map is not available here, so the flat map is used. It needs the desktop app on a graphics card with Vulkan, Direct3D 12 or Metal." to false
    GpuMapHost.failure != null ->
        "This machine cannot run the 3D map (${GpuMapHost.failure}). The flat map is used instead; the options marked 3D have no effect." to true
    RenderInfo.software ->
        "This machine draws in software, so the 3D map will be slow here. Medium or Low is the better preset." to true
    else -> "This machine can run the 3D map: it needs a graphics card with Vulkan, Direct3D 12 or Metal, which this one has." to false
}

/**
 * Settings (2.0): sections down the side, a search box that finds any setting, one plain sentence under each,
 * presets where they exist, Reset section and Reset all. Every change applies at once; there is no Apply button.
 */
@Composable
fun SettingsScreen(store: GameStore, settings: AppSettings, desktop: Boolean, updater: Updater? = null, onSetupAgain: (() -> Unit)? = null, startSection: SettingsSection = SettingsSection.Game, startQuery: String = "", onClose: () -> Unit) {
    var section by remember { mutableStateOf(startSection) }
    var query by remember { mutableStateOf(startQuery) }
    var confirmAll by remember { mutableStateOf(false) }
    val rows = settingRows(store, settings, desktop, updater, onSetupAgain)
    val sections = SettingsSection.entries.filter { s -> rows.any { it.section == s } }
    val shown = if (query.isBlank()) rows.filter { it.section == section } else rows.filter { it.matches(query) }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val wide = maxWidth >= 900.dp
        Column(Modifier.fillMaxSize()) {
            Row(
                Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                TextButton(onClick = onClose) { BackLabel("Done", Palette.gold) }
                Text("SETTINGS", color = Palette.primaryText, fontSize = 18.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, modifier = if (wide) Modifier.width(200.dp) else Modifier.weight(1f))
                if (wide) SettingsSearch(query, Modifier.widthIn(max = 520.dp).weight(1f)) { query = it }
            }
            Box(Modifier.fillMaxWidth().height(2.dp).background(Palette.stroke))
            if (!wide) {
                Column(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    SettingsSearch(query, Modifier.fillMaxWidth()) { query = it }
                    Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        sections.forEach { s -> SectionChip(s.title, s == section && query.isBlank()) { section = s; query = "" } }
                    }
                }
            }
            Row(Modifier.weight(1f).fillMaxWidth()) {
                if (wide) {
                    Column(Modifier.width(260.dp).fillMaxHeight().background(Palette.panel).padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        sections.forEach { s ->
                            val on = s == section && query.isBlank()
                            Text(
                                s.title.uppercase(), color = if (on) Palette.ink else Palette.primaryText, fontSize = 13.sp, fontFamily = Pixel.font,
                                modifier = Modifier.fillMaxWidth().clip(Pixel.small).background(if (on) Palette.gold else Palette.panel)
                                    .clickable { section = s; query = "" }.padding(horizontal = 12.dp, vertical = 10.dp),
                            )
                        }
                        Spacer(Modifier.weight(1f))
                        // Reserved: the Lakeside account row goes here, above Reset all, once accounts exist.
                        AccountEntry()
                        if (confirmAll) {
                            Text("Put every setting back to how the game came? Saves and careers are not touched.", color = Palette.warning, fontSize = 12.sp)
                            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                                MenuBarButton("Reset all", danger = true) { settings.resetAll(); updater?.channelChanged(); confirmAll = false }
                                MenuBarButton("Keep") { confirmAll = false }
                            }
                        } else {
                            MenuBarButton("Reset all") { confirmAll = true }
                        }
                    }
                    Box(Modifier.width(2.dp).fillMaxHeight().background(Palette.stroke))
                }
                Column(
                    Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()).padding(if (wide) 24.dp else 12.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Column(Modifier.widthIn(max = 980.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                if (query.isBlank()) section.title.uppercase() else "${shown.size} SETTING${if (shown.size == 1) "" else "S"} FOUND",
                                color = Palette.primaryText, fontSize = 16.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f),
                            )
                            if (query.isBlank() && section.resettable) MenuBarButton("Reset section") { settings.resetSection(section); if (section == SettingsSection.Updates) updater?.channelChanged() }
                        }
                        if (query.isBlank() && section == SettingsSection.Keys) {
                            Text("Keys cannot be changed yet; this is the full list.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp))
                        }
                        Spacer(Modifier.height(10.dp))
                        if (shown.isEmpty()) Text("No setting matches \"$query\".", color = Palette.secondary, fontSize = 13.sp)
                        var lastSection: SettingsSection? = null
                        shown.forEach { row ->
                            if (query.isNotBlank() && row.section != lastSection) {
                                Spacer(Modifier.height(8.dp))
                                Eyebrow(row.section.title)
                                lastSection = row.section
                            }
                            SettingLine(row, wide)
                        }
                        if (!wide && query.isBlank() && section == SettingsSection.About) {
                            Spacer(Modifier.height(16.dp))
                            MenuBarButton("Reset all settings", danger = confirmAll) { if (confirmAll) { settings.resetAll(); confirmAll = false } else confirmAll = true }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SettingsSearch(query: String, modifier: Modifier, onQuery: (String) -> Unit) {
    OutlinedTextField(
        query, { onQuery(it.take(40)) }, singleLine = true,
        placeholder = { Text("Find a setting", color = Palette.secondary, fontSize = 13.sp) },
        textStyle = androidx.compose.ui.text.TextStyle(color = Palette.primaryText, fontSize = 14.sp),
        modifier = modifier.textEntry(),
    )
}

@Composable
private fun SectionChip(text: String, on: Boolean, onClick: () -> Unit) {
    Text(
        text.uppercase(), color = if (on) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontFamily = Pixel.font,
        modifier = Modifier.clip(Pixel.small).background(if (on) Palette.gold else Palette.panelRaised).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 7.dp),
    )
}

/** One setting: the name and its sentence, the control at the right or underneath. */
@Composable
private fun SettingLine(row: SettingRow, wide: Boolean) {
    Column(Modifier.fillMaxWidth().padding(vertical = 8.dp)) {
        if (row.below || !wide) {
            Text(row.title, color = Palette.primaryText, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
            if (row.note.isNotEmpty()) Text(row.note, color = Palette.secondary, fontSize = 12.sp)
            Spacer(Modifier.height(6.dp))
            row.control()
        } else {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f).padding(end = 16.dp)) {
                    Text(row.title, color = Palette.primaryText, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                    if (row.note.isNotEmpty()) Text(row.note, color = Palette.secondary, fontSize = 12.sp)
                }
                row.control()
            }
        }
        Spacer(Modifier.height(8.dp))
        Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
    }
}

@Composable
private fun OnOff(checked: Boolean, onChange: (Boolean) -> Unit) {
    Switch(
        checked = checked, onCheckedChange = onChange,
        colors = SwitchDefaults.colors(
            checkedTrackColor = Palette.gold, checkedThumbColor = Palette.ink,
            uncheckedTrackColor = Palette.panelRaised, uncheckedThumbColor = Palette.secondary, uncheckedBorderColor = Palette.secondary,
        ),
    )
}

/** Every setting the game has, in section order. Rows that do not apply here (no career, no updater, a phone) are left out. */
@Composable
private fun settingRows(store: GameStore, settings: AppSettings, desktop: Boolean, updater: Updater?, onSetupAgain: (() -> Unit)?): List<SettingRow> {
    val rows = mutableListOf<SettingRow>()
    val g = settings.graphics
    if (store.ready) rows += SettingRow(SettingsSection.Game, "Save now", "Writes this career to disk. The game also saves itself every four weeks and when you leave." + if (store.status.isNotEmpty()) " ${store.status}" else "", "save") {
        MenuBarButton("Save now") { store.saveNow() }
    }
    rows += SettingRow(SettingsSection.Game, "Stop for warnings too", "Time always stops for a decision that needs your answer. This also stops it when a company runs short of grace or workers strike.", "pause") {
        OnOff(settings.pauseOnWarnings) { settings.choosePauseOnWarnings(it) }
    }
    if (desktop) rows += SettingRow(SettingsSection.Game, "Show the brief each week", "Opens the weekly brief when a new week starts while time is paused or at normal speed, never at faster speeds.") {
        OnOff(settings.showBriefEachWeek) { settings.chooseShowBriefEachWeek(it) }
    }
    store.economy?.fx?.takeIf { store.ready }?.let { fx ->
        rows += SettingRow(SettingsSection.Game, "Show my figures in", "Your own cash and net worth in another of the scenario's currencies. Each company keeps its own.", "currency money", below = true) { CurrencyChoices(store, fx) }
    }
    rows += SettingRow(
        SettingsSection.Game, "Tutorial",
        if (Tutorial.finished) "Finished. Start over to see its eight steps again." else "${Tutorial.active.count { it.id in Tutorial.done }} of ${Tutorial.active.size} steps done. Start over to see them from the first.",
        "guide help",
    ) { MenuBarButton("Start over") { Tutorial.restart() } }
    if (onSetupAgain != null) rows += SettingRow(SettingsSection.Game, "First-run setup", "Test the graphics again and go through updates, data and the tutorial.", "wizard welcome") {
        MenuBarButton("Set up again", onClick = onSetupAgain)
    }

    val (support, problem) = mapSupport()
    rows += SettingRow(SettingsSection.Graphics, "3D map on this machine", support, "gpu vulkan direct3d metal card", below = problem) {
        if (problem) Text("Check the graphics driver, or pick Low.", color = Palette.warning, fontSize = 12.sp)
    }
    rows += SettingRow(
        SettingsSection.Graphics, "Quality preset",
        if (g.preset == null) "Custom: your own mix of the options below. A preset sets every one of them." else "Sets every option below at once. Change any of them to make your own mix.",
        "low medium high ultra", below = true,
    ) { Choices<GraphicsPreset?>(GraphicsPreset.entries, g.preset, { it?.title.orEmpty() }) { it?.let(settings::choosePreset) } }
    graphicsOptions.filter { (!it.gpuOnly || GpuMapHost.opener != null) && (!it.surfaceOnly || GpuMapHost.surfaceHost != null) }.forEach { option ->
        rows += SettingRow(SettingsSection.Graphics, option.title, option.note(g), option.choices.joinToString(" "), below = true) {
            Choices(option.choices.indices.toList(), option.get(g), { option.choices[it] }) { settings.chooseGraphics(option.set(g, it)) }
        }
    }
    if (desktop) {
        rows += SettingRow(SettingsSection.Graphics, "Window", "Windowed, filling the screen with the title bar, or full screen.", "fullscreen maximized display", below = true) {
            Choices(WindowMode.entries, settings.windowMode, { it.title }) { settings.chooseWindowMode(it) }
        }
        rows += SettingRow(SettingsSection.Graphics, "Resolution", "The window's size when windowed. Picking one switches back to windowed.", "size display", below = true) {
            Choices(Resolutions, settings.resolution, { "${it.first} x ${it.second}" }) {
                settings.chooseResolution(it)
                if (settings.windowMode != WindowMode.Windowed) settings.chooseWindowMode(WindowMode.Windowed)
            }
        }
    }

    rows += SettingRow(SettingsSection.Interface, "Interface size", "Makes every panel, figure and button bigger or smaller.", "scale zoom text font", below = true) {
        Choices(listOf(0.85f, 1f, 1.15f, 1.3f), settings.uiScale, { "${(it * 100).toInt()}%" }) { settings.chooseUiScale(it) }
    }
    rows += SettingRow(
        SettingsSection.Interface, "Classic layout",
        if (desktop) "The page layout from version 1. Kept for this release only, then removed." else "The five tabs from version 1. Kept for this release only, then removed.",
        "old 1.x",
    ) { OnOff(settings.classicLayout) { settings.chooseClassicLayout(it) } }

    val keys = listOf(
        "Space" to "Play or pause time", "N" to "Next week", "1, 2, 3" to "Time at normal, fast or fastest speed",
        "Esc" to "Close the open screen, or open the game menu", "Ctrl K" to "Find a place, a good, a company, a view or an action",
        "Ctrl M" to "Dock the map beside the screen, or undock it",
    ) + Place.entries.filter { it.key.isNotEmpty() }.sortedBy { it.key.drop(1).toIntOrNull() ?: 99 }.map { it.key to "Open ${it.title}" }
    keys.forEach { (key, what) ->
        rows += SettingRow(SettingsSection.Keys, what, "", "key shortcut $key") {
            Text(key, color = Palette.gold, fontSize = 13.sp, fontFamily = Pixel.font, modifier = Modifier.clip(Pixel.small).background(Palette.panelRaised).padding(horizontal = 10.dp, vertical = 5.dp))
        }
    }

    rows += SettingRow(SettingsSection.Data, "Share anonymous market data", Privacy.explanation + " " + Privacy.points.take(3).joinToString(" "), "analytics telemetry privacy snapshot") {
        OnOff(settings.analytics) { settings.chooseAnalytics(it) }
    }
    rows += SettingRow(SettingsSection.Data, "Where your saves are kept", store.saveFolder.path + ". Back this folder up to keep your careers safe. Load can export and import single saves.", "folder backup files")

    if (updater != null) rows += SettingRow(SettingsSection.Updates, "Updates", "", "version release preview channel check install", below = true) {
        UpdatesSection(settings, updater)
    }

    val info = remember { uniffi.cw_ffi.engineInfo() }
    rows += SettingRow(SettingsSection.About, "Corporate Warfare ${BuildInfo.VERSION}", "Engine ${info.version}." + if (BuildInfo.CHANNEL == "preview") " Tester preview." else "", "version")
    rows += SettingRow(SettingsSection.About, "Rules, not advice", "Institutions are simplified game rules, not legal, tax or investment advice.")
    rows += SettingRow(SettingsSection.About, "Clouds", "Solar System Scope, CC BY 4.0, based on NASA Blue Marble imagery.", "credits")
    return rows
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


