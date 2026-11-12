@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.height
import androidx.compose.material3.OutlinedTextField
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * What the load screen shows about a save without opening it: the small description written beside the save
 * file. Fields added in 2.0 have defaults, so the description of an older save still reads (its new columns show
 * a dash). The engine save itself is not changed by any of this.
 */
@Serializable
data class SaveInfo(
    val company: String = "",
    val week: String = "",
    val tick: Int = 0,
    val savedAt: Long = 0,
    val autosave: Boolean = false,
    /** The player's name for the save (Save as or Rename); empty shows the company. */
    val name: String = "",
    val scenario: String = "",
    val netWorthCents: Long = 0,
    val currency: String = "",
    val ambition: String = "",
    val medal: String = "",
    val playSeconds: Long = 0,
    /** The game version that wrote the save; empty before 2.0. */
    val version: String = "",
) {
    val title: String get() = name.ifEmpty { company.ifEmpty { "Saved career" } }

    /** Whether this description was written by 2.0 or later, so net worth, medal and play time are known. */
    val detailed: Boolean get() = version.isNotEmpty()
}

/** One save on disk: the save file and its small description beside it. */
data class SaveSlot(val file: File, val info: SaveInfo)

/** How a desktop picks files for export and import; null where the platform has no file picker (the folder fallback is used). */
object SaveTransfer {
    var chooseExport: ((suggestedName: String) -> File?)? = null
    var chooseImport: (() -> List<File>)? = null
}

/**
 * Save files in the game's folder: `autosave.cwsave`, written every four weeks and on exit, and
 * one `save-<time>.cwsave` per Save now. Each has a `.json` description beside it.
 */
object Saves {
    private val json = Json { ignoreUnknownKeys = true }

    private fun sideOf(file: File) = File(file.parentFile, file.nameWithoutExtension + ".json")

    fun infoOf(file: File): SaveInfo =
        runCatching { json.decodeFromString(SaveInfo.serializer(), sideOf(file).readText()) }.getOrNull()
            ?: SaveInfo(company = "Saved career", savedAt = file.lastModified(), autosave = file.name.startsWith("autosave"))

    fun list(dir: File): List<SaveSlot> =
        (dir.listFiles { f -> f.name.endsWith(".cwsave") } ?: emptyArray())
            .map { file -> SaveSlot(file, infoOf(file)) }
            .sortedByDescending { it.info.savedAt }

    /** Write a save and its description, swapping in a finished file so a crash never leaves half a save. */
    fun write(dir: File, name: String, bytes: ByteArray, info: SaveInfo) {
        dir.mkdirs()
        val temp = File(dir, "$name.tmp")
        temp.writeBytes(bytes)
        val target = File(dir, "$name.cwsave")
        target.delete()
        temp.renameTo(target)
        File(dir, "$name.json").writeText(json.encodeToString(SaveInfo.serializer(), info))
    }

    fun delete(slot: SaveSlot) {
        slot.file.delete()
        sideOf(slot.file).delete()
    }

    /** Give a save a name of its own; the save file is not touched. */
    fun rename(slot: SaveSlot, name: String) {
        sideOf(slot.file).writeText(json.encodeToString(SaveInfo.serializer(), slot.info.copy(name = name.trim().take(60))))
    }

    /** A second copy of a save, kept as a manual save so autosaves never replace it. */
    fun duplicate(slot: SaveSlot): File {
        val dir = slot.file.parentFile
        var stamp = System.currentTimeMillis()
        while (File(dir, "save-$stamp.cwsave").exists()) stamp++
        val target = File(dir, "save-$stamp.cwsave")
        slot.file.copyTo(target)
        val info = slot.info.copy(name = "Copy of ${slot.info.title}".take(60), autosave = false, savedAt = slot.info.savedAt + 1)
        File(dir, "save-$stamp.json").writeText(json.encodeToString(SaveInfo.serializer(), info))
        return target
    }

    /** A file name a player can recognise outside the game. */
    fun exportName(slot: SaveSlot): String =
        slot.info.title.replace(Regex("[^A-Za-z0-9 _-]"), "").trim().replace(' ', '-').ifEmpty { "career" } +
            slot.info.week.replace(' ', '-').let { if (it.isEmpty()) "" else "-$it" } + ".cwsave"

    /** Copy a save (and its description) to [target]. */
    fun export(slot: SaveSlot, target: File) {
        target.parentFile?.mkdirs()
        slot.file.copyTo(target, overwrite = true)
        sideOf(slot.file).takeIf { it.exists() }?.copyTo(File(target.parentFile, target.nameWithoutExtension + ".json"), overwrite = true)
    }

    /** Bring saves in from outside: each becomes a manual save. Returns how many came in. */
    fun import(dir: File, files: List<File>): Int {
        var count = 0
        for (file in files.filter { it.isFile && it.name.endsWith(".cwsave") }) {
            var stamp = System.currentTimeMillis()
            while (File(dir, "save-$stamp.cwsave").exists()) stamp++
            file.copyTo(File(dir, "save-$stamp.cwsave"))
            val side = sideOf(file)
            val info = (if (side.exists()) infoOf(file) else SaveInfo(company = file.nameWithoutExtension, savedAt = file.lastModified()))
                .copy(autosave = false)
            File(dir, "save-$stamp.json").writeText(json.encodeToString(SaveInfo.serializer(), info))
            count++
        }
        return count
    }

    /** Keep the newest manual saves only. */
    fun prune(dir: File, keep: Int = 20) {
        list(dir).filter { !it.info.autosave }.drop(keep).forEach(::delete)
    }

    fun playTime(seconds: Long): String = when {
        seconds <= 0 -> "-"
        seconds < 3600 -> "${(seconds / 60).coerceAtLeast(1)} min"
        else -> "${seconds / 3600} h ${(seconds % 3600) / 60} min"
    }
}

/** The columns of the save browser, each with how it sorts. */
internal enum class SaveColumn(val title: String, val width: Int, val numeric: Boolean, val key: (SaveInfo) -> Comparable<*>) {
    Name("Save", 300, false, { it.title.lowercase() }),
    Scenario("Scenario", 190, false, { it.scenario.lowercase() }),
    Date("Game date", 120, true, { it.tick }),
    NetWorth("Net worth", 130, true, { it.netWorthCents }),
    Medal("Medal", 150, false, { medalRank(it.medal) }),
    Played("Play time", 110, true, { it.playSeconds }),
    Version("Version", 90, false, { it.version }),
    Saved("Saved", 170, true, { it.savedAt }),
}

private fun medalRank(m: String) = when (m) { "gold" -> 3; "silver" -> 2; "bronze" -> 1; else -> 0 }

@Suppress("UNCHECKED_CAST")
internal fun sortSaves(slots: List<SaveSlot>, column: SaveColumn, descending: Boolean): List<SaveSlot> {
    val sorted = slots.sortedWith { a, b -> (column.key(a.info) as Comparable<Any>).compareTo(column.key(b.info)) }
    return if (descending) sorted.reversed() else sorted
}

private fun cell(info: SaveInfo, column: SaveColumn, format: SimpleDateFormat): String = when (column) {
    SaveColumn.Name -> info.title
    SaveColumn.Scenario -> info.scenario.ifEmpty { "-" }
    SaveColumn.Date -> info.week.ifEmpty { "-" }
    SaveColumn.NetWorth -> if (info.detailed) Format.compact(info.netWorthCents, info.currency) else "-"
    SaveColumn.Medal -> when {
        !info.detailed -> "-"
        info.ambition.isEmpty() -> "No ambition"
        else -> "${ambitionWords(info.ambition)}: ${medalWords(info.medal).lowercase()}"
    }
    SaveColumn.Played -> if (info.detailed) Saves.playTime(info.playSeconds) else "-"
    SaveColumn.Version -> info.version.ifEmpty { "before 2.0" }
    SaveColumn.Saved -> (if (info.autosave) "Auto, " else "") + format.format(Date(info.savedAt))
}

/**
 * The save browser: every save as a sortable grid (scenario, company, game date, net worth, medal, play time,
 * version, when saved), one selected, and the actions for it in one bar: Load, Rename, Duplicate, Export, Delete,
 * plus Import. On a phone the rows take two lines.
 */
@Composable
fun LoadGameScreen(store: GameStore, onBack: () -> Unit) {
    BackAction(onBack = onBack)
    var slots by remember { mutableStateOf(store.saveSlots()) }
    var sort by remember { mutableStateOf(SaveColumn.Saved) }
    var descending by remember { mutableStateOf(true) }
    var picked by remember { mutableStateOf(slots.firstOrNull()?.file) }
    var confirm by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<String?>(null) }
    var note by remember { mutableStateOf("") }
    val format = remember { SimpleDateFormat("MMM d yyyy, HH:mm", Locale.US) }
    fun refresh() { slots = store.saveSlots(); if (slots.none { it.file == picked }) picked = slots.firstOrNull()?.file }
    val shown = sortSaves(slots, sort, descending)
    val slot = slots.firstOrNull { it.file == picked }
    fun load(s: SaveSlot) { store.loadSave(s.file); onBack() }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink).windowInsetsPadding(WindowInsets.safeDrawing)) {
        val wide = maxWidth >= 900.dp
        Column(Modifier.fillMaxSize()) {
            MenuTopBar("Load a career", onBack) {
                Text("${slots.size} saves", color = Palette.secondary, fontSize = 13.sp)
            }
            Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(if (wide) 24.dp else 12.dp)) {
                if (slots.isEmpty()) {
                    Callout("No saves yet. A career saves itself every four weeks and when you leave it. Import brings in a save from elsewhere.", Palette.secondary)
                } else if (wide) {
                    Row(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 12.dp, vertical = 8.dp)) {
                        SaveColumn.entries.forEach { c ->
                            val on = c == sort
                            Text(
                                c.title.uppercase() + if (on) (if (descending) " v" else " ^") else "",
                                color = if (on) Palette.gold else Palette.secondary, fontSize = 11.sp, fontFamily = Pixel.font,
                                textAlign = if (c.numeric) TextAlign.End else TextAlign.Start,
                                modifier = Modifier.width(c.width.dp).clickable { if (on) descending = !descending else { sort = c; descending = c.numeric } }.padding(end = 12.dp),
                            )
                        }
                    }
                    shown.forEach { s ->
                        val on = s.file == picked
                        Row(
                            Modifier.fillMaxWidth().background(if (on) Palette.gold.copy(alpha = 0.16f) else Palette.panel)
                                .border(if (on) 2.dp else 0.dp, if (on) Palette.gold else Palette.panel)
                                .clickable { picked = s.file; confirm = false; renaming = null }
                                .padding(horizontal = 12.dp, vertical = 9.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            SaveColumn.entries.forEach { c ->
                                val text = cell(s.info, c, format)
                                val color = when {
                                    c == SaveColumn.Name -> Palette.primaryText
                                    c == SaveColumn.Medal && s.info.medal in setOf("bronze", "silver", "gold") -> medalColor(s.info.medal)
                                    else -> Palette.secondary
                                }
                                Text(
                                    text, color = color, fontSize = 13.sp, maxLines = 1,
                                    fontWeight = if (c == SaveColumn.Name) FontWeight.SemiBold else FontWeight.Normal,
                                    textAlign = if (c.numeric) TextAlign.End else TextAlign.Start,
                                    modifier = Modifier.width(c.width.dp).padding(end = 12.dp),
                                )
                            }
                        }
                        Box(Modifier.fillMaxWidth().height(1.dp).background(Palette.stroke))
                    }
                } else {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text("SORT BY", color = Palette.secondary, fontSize = 11.sp, fontFamily = Pixel.font)
                        Choices(listOf(SaveColumn.Saved, SaveColumn.Name, SaveColumn.NetWorth, SaveColumn.Played), sort, { it.title }) {
                            if (it == sort) descending = !descending else { sort = it; descending = it.numeric }
                        }
                    }
                    Spacer(Modifier.height(8.dp))
                    shown.forEach { s ->
                        val on = s.file == picked
                        Column(
                            Modifier.fillMaxWidth().padding(bottom = 6.dp).background(if (on) Palette.gold.copy(alpha = 0.16f) else Palette.panel)
                                .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke)
                                .clickable { picked = s.file; confirm = false; renaming = null }.padding(10.dp),
                        ) {
                            Row {
                                Text(s.info.title, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, modifier = Modifier.weight(1f), maxLines = 1)
                                Text(cell(s.info, SaveColumn.NetWorth, format), color = Palette.primaryText, fontSize = 14.sp)
                            }
                            Text(
                                listOf(cell(s.info, SaveColumn.Scenario, format), cell(s.info, SaveColumn.Date, format), cell(s.info, SaveColumn.Medal, format), cell(s.info, SaveColumn.Played, format)).joinToString("  |  "),
                                color = Palette.secondary, fontSize = 12.sp, maxLines = 1,
                            )
                            Text(cell(s.info, SaveColumn.Saved, format) + "  |  " + cell(s.info, SaveColumn.Version, format), color = Palette.secondary, fontSize = 11.sp)
                        }
                    }
                }
                if (note.isNotEmpty()) Text(note, color = Palette.gold, fontSize = 12.sp, modifier = Modifier.padding(top = 10.dp))
            }
            // One bar for every action, always in the same place.
            Column(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = if (wide) 24.dp else 12.dp, vertical = 10.dp)) {
                renaming?.let { text ->
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(text, { renaming = it.take(60) }, singleLine = true, label = { Text("New name") }, modifier = Modifier.textEntry().weight(1f))
                        GoldButton("Rename") { slot?.let { Saves.rename(it, text) }; renaming = null; refresh() }
                        MenuBarButton("Cancel") { renaming = null }
                    }
                    Spacer(Modifier.height(8.dp))
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                    GoldButton("Load", enabled = slot != null && !store.busy) { slot?.let(::load) }
                    MenuBarButton("Rename", enabled = slot != null) { renaming = slot?.info?.title.orEmpty() }
                    MenuBarButton("Duplicate", enabled = slot != null) { slot?.let { picked = Saves.duplicate(it); note = "Copied as a new save." }; slots = store.saveSlots() }
                    MenuBarButton("Export", enabled = slot != null) {
                        val s = slot ?: return@MenuBarButton
                        val chooser = SaveTransfer.chooseExport
                        val target = if (chooser != null) chooser(Saves.exportName(s)) else File(File(store.saveFolder, "exported"), Saves.exportName(s))
                        if (target != null) runCatching { Saves.export(s, target) }
                            .onSuccess { note = "Exported to ${target.path}" }.onFailure { note = "Could not export: ${it.message}" }
                    }
                    if (confirm) {
                        MenuBarButton("Delete for good", danger = true) { slot?.let(Saves::delete); confirm = false; refresh() }
                        MenuBarButton("Keep") { confirm = false }
                    } else {
                        MenuBarButton("Delete", enabled = slot != null) { confirm = true }
                    }
                    MenuBarButton("Import") {
                        val chooser = SaveTransfer.chooseImport
                        val files = chooser?.invoke() ?: File(store.saveFolder, "import").also { it.mkdirs() }.listFiles()?.toList().orEmpty()
                        val count = runCatching { Saves.import(store.saveFolder, files) }.getOrDefault(0)
                        note = when {
                            count > 0 -> "Imported $count save${if (count == 1) "" else "s"}."
                            chooser == null -> "Put .cwsave files in ${File(store.saveFolder, "import").path}, then press Import."
                            else -> "No save was imported."
                        }
                        refresh()
                    }
                }
            }
        }
    }
}

/** A plain bar button in the 8-bit style: the pixel font, a stepped frame. */
@Composable
internal fun MenuBarButton(text: String, enabled: Boolean = true, danger: Boolean = false, onClick: () -> Unit) {
    Box(
        Modifier.alpha(if (enabled) 1f else 0.45f).padding(end = 3.dp, bottom = 3.dp)
            .pixelFrame(Palette.panelRaised, if (danger) Palette.negative else Palette.stroke)
            .clip(Pixel.shape).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 14.dp, vertical = 9.dp),
    ) { Text(text.uppercase(), color = if (danger) Palette.negative else Palette.primaryText, fontSize = 12.sp, fontFamily = Pixel.font) }
}

/** The bar on top of every menu screen: Back on the left, the title, and anything else on the right. */
@Composable
internal fun MenuTopBar(title: String, onBack: () -> Unit, trailing: @Composable () -> Unit = {}) {
    Row(
        Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(onClick = onBack) { BackLabel("Back", Palette.secondary) }
        Spacer(Modifier.width(8.dp))
        Text(title.uppercase(), color = Palette.primaryText, fontSize = 18.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f), maxLines = 1)
        trailing()
    }
    Box(Modifier.fillMaxWidth().height(2.dp).background(Palette.stroke))
}
