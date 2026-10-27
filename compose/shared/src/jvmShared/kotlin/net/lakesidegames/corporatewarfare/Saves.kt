package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
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

/** What the load screen shows about a save without opening it. */
@Serializable
data class SaveInfo(
    val company: String = "",
    val week: String = "",
    val tick: Int = 0,
    val savedAt: Long = 0,
    val autosave: Boolean = false,
)

/** One save on disk: the save file and its small description beside it. */
data class SaveSlot(val file: File, val info: SaveInfo)

/**
 * Save files in the game's folder: `autosave.cwsave`, written every four weeks and on exit, and
 * one `save-<time>.cwsave` per Save now. Each has a `.json` description beside it.
 */
object Saves {
    private val json = Json { ignoreUnknownKeys = true }

    fun list(dir: File): List<SaveSlot> =
        (dir.listFiles { f -> f.name.endsWith(".cwsave") } ?: emptyArray())
            .map { file ->
                val side = File(file.parentFile, file.nameWithoutExtension + ".json")
                val info = runCatching { json.decodeFromString(SaveInfo.serializer(), side.readText()) }.getOrNull()
                    ?: SaveInfo(company = "Saved career", savedAt = file.lastModified(), autosave = file.name.startsWith("autosave"))
                SaveSlot(file, info)
            }
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
        File(slot.file.parentFile, slot.file.nameWithoutExtension + ".json").delete()
    }

    /** Keep the newest manual saves only. */
    fun prune(dir: File, keep: Int = 20) {
        list(dir).filter { !it.info.autosave }.drop(keep).forEach(::delete)
    }
}

/** Every save, newest first: open one, or delete it. */
@Composable
fun LoadGameScreen(store: GameStore, onBack: () -> Unit) {
    var slots by remember { mutableStateOf(store.saveSlots()) }
    var confirm by remember { mutableStateOf<SaveSlot?>(null) }
    val format = remember { SimpleDateFormat("MMM d, yyyy h:mm a", Locale.US) }
    Box(Modifier.fillMaxSize().background(Palette.ink), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = 760.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = onBack) { Text("‹ Back", color = Palette.secondary) }
                Spacer(Modifier.width(8.dp))
                Text("Load game", color = Palette.primaryText, fontSize = 22.sp, fontWeight = FontWeight.Bold)
            }
            if (slots.isEmpty()) {
                Callout("No saves yet. Start a new game; it saves every four weeks and when you leave.", Palette.secondary)
            }
            slots.forEach { slot ->
                val shape = RoundedCornerShape(12.dp)
                Row(
                    Modifier.fillMaxWidth().clip(shape).background(Palette.panel).border(1.dp, Palette.stroke, shape)
                        .clickable(enabled = !store.busy) { store.loadSave(slot.file); onBack() }
                        .padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(slot.info.company.ifEmpty { "Saved career" }, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 16.sp)
                        Text(
                            listOfNotNull(
                                slot.info.week.takeIf { it.isNotEmpty() },
                                if (slot.info.autosave) "Autosave" else "Saved",
                                format.format(Date(slot.info.savedAt)),
                            ).joinToString(" · "),
                            color = Palette.secondary,
                            fontSize = 12.sp,
                        )
                    }
                    if (confirm == slot) {
                        TextButton(onClick = { Saves.delete(slot); slots = store.saveSlots(); confirm = null }) { Text("Delete", color = Palette.negative) }
                        TextButton(onClick = { confirm = null }) { Text("Keep", color = Palette.secondary) }
                    } else {
                        TextButton(onClick = { confirm = slot }) { Text("Delete", color = Palette.secondary) }
                        GoldButton("Load", enabled = !store.busy) { store.loadSave(slot.file); onBack() }
                    }
                }
            }
        }
    }
}
