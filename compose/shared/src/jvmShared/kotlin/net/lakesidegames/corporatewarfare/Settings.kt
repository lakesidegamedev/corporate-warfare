package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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

/** How the desktop window sits on screen. */
enum class WindowMode(val title: String) { Windowed("Windowed"), Maximized("Maximized"), Fullscreen("Full screen") }

/** Window sizes offered on desktop, in display points. */
val Resolutions = listOf(1280 to 720, 1366 to 768, 1600 to 900, 1920 to 1080, 2560 to 1440)

/**
 * Player settings that live outside any career, saved to a small file beside the saves. Desktop
 * builds also read the window settings here.
 */
class AppSettings(private val file: File) {
    private val props = Properties().apply { if (file.exists()) file.inputStream().use { load(it) } }

    var mapDetail by mutableStateOf(enumOr(props.getProperty("mapDetail"), MapDetail.High)); private set
    var animations by mutableStateOf(props.getProperty("animations")?.toBoolean() ?: true); private set
    var autoUpdate by mutableStateOf(props.getProperty("autoUpdate")?.toBoolean() ?: true); private set
    var tutorialDone: Set<String> = props.getProperty("tutorial.done")?.split(',')?.filter { it.isNotEmpty() }?.toSet() ?: emptySet(); private set
    var tutorialHidden: Boolean = props.getProperty("tutorial.hidden")?.toBoolean() ?: false; private set
    /** Currency id the investor's figures are shown in; empty until the player picks one. */
    var reportingCurrency: String = props.getProperty("reportingCurrency").orEmpty(); private set
    var uiScale by mutableStateOf(props.getProperty("uiScale")?.toFloatOrNull() ?: 1f); private set
    var windowMode by mutableStateOf(enumOr(props.getProperty("windowMode"), WindowMode.Windowed)); private set
    var resolution by mutableStateOf(
        props.getProperty("resolution")?.split("x")?.mapNotNull { it.toIntOrNull() }?.takeIf { it.size == 2 }?.let { it[0] to it[1] }
            ?: (1600 to 900),
    ); private set

    fun chooseMapDetail(value: MapDetail) { mapDetail = value; store() }
    fun chooseAnimations(value: Boolean) { animations = value; store() }
    fun chooseAutoUpdate(value: Boolean) { autoUpdate = value; store() }
    fun chooseTutorial(done: Set<String>, hidden: Boolean) { tutorialDone = done; tutorialHidden = hidden; store() }
    fun chooseReportingCurrency(value: String) { reportingCurrency = value; store() }
    fun chooseUiScale(value: Float) { uiScale = value; store() }
    fun chooseWindowMode(value: WindowMode) { windowMode = value; store() }
    fun chooseResolution(value: Pair<Int, Int>) { resolution = value; store() }

    private fun store() {
        props.setProperty("mapDetail", mapDetail.name)
        props.setProperty("animations", animations.toString())
        props.setProperty("autoUpdate", autoUpdate.toString())
        props.setProperty("tutorial.done", tutorialDone.joinToString(","))
        props.setProperty("tutorial.hidden", tutorialHidden.toString())
        props.setProperty("reportingCurrency", reportingCurrency)
        props.setProperty("uiScale", uiScale.toString())
        props.setProperty("windowMode", windowMode.name)
        props.setProperty("resolution", "${resolution.first}x${resolution.second}")
        runCatching {
            file.parentFile?.mkdirs()
            file.outputStream().use { props.store(it, "Corporate Warfare settings") }
        }
    }

    private inline fun <reified T : Enum<T>> enumOr(name: String?, fallback: T): T =
        enumValues<T>().firstOrNull { it.name == name } ?: fallback
}

/**
 * The full settings page, from the main menu or a running career: game, graphics and, on
 * desktop, the window.
 */
@Composable
fun SettingsScreen(store: GameStore, settings: AppSettings, desktop: Boolean, updater: Updater? = null, onClose: () -> Unit) {
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
                Text("Map detail", color = Palette.primaryText)
                Choices(MapDetail.entries, settings.mapDetail, { it.title }) { settings.chooseMapDetail(it) }
                Text(settings.mapDetail.note, color = Palette.secondary, fontSize = 12.sp)
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text("Animations", color = Palette.primaryText)
                        Text("Map flights between the globe and the scenario.", color = Palette.secondary, fontSize = 12.sp)
                    }
                    Switch(
                        checked = settings.animations,
                        onCheckedChange = { settings.chooseAnimations(it) },
                        colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold),
                    )
                }
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
        if (updater != null) {
            item {
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
                    UpdatePanel(updater, compact = true)
                }
            }
        }
        item {
            Panel {
                Eyebrow("About")
                val info = uniffi.cw_ffi.engineInfo()
                Text("Corporate Warfare ${BuildInfo.VERSION} · engine ${info.version}", color = Palette.secondary, fontSize = 12.sp)
                Text("Institutions are simplified game rules, not legal, tax or investment advice.", color = Palette.secondary, fontSize = 11.sp)
            }
        }
    }
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun <T> Choices(options: List<T>, selected: T, label: (T) -> String, onPick: (T) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        options.forEach { option ->
            val on = option == selected
            Text(
                label(option),
                color = if (on) Palette.ink else Palette.primaryText,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(CircleShape)
                    .background(if (on) Palette.gold else Palette.panelRaised)
                    .clickable { onPick(option) }
                    .padding(horizontal = 12.dp, vertical = 7.dp),
            )
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
                Text("Version ${status.release.version} is out (you have ${updater.current})", color = Palette.gold, fontWeight = FontWeight.SemiBold)
                if (!compact) {
                    val notes = status.release.notes.lines().filter { it.startsWith("- ") }.take(4)
                    notes.forEach { Text(it.removePrefix("- ").take(120), color = Palette.secondary, fontSize = 12.sp, maxLines = 2) }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    GoldButton(if (status.release.installer != null) "Update now" else "Get the update") { updater.update(status.release) }
                    TextButton(onClick = { updater.openPage(status.release) }) { Text("What's new", color = Palette.secondary) }
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
