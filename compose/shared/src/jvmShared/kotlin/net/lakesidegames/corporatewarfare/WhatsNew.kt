package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * The "What's new" panel shown once on the main menu after an update: the top changes of this version, read from the
 * changelog that was bundled at build time. Nothing is shown on a first install, and never twice for one version.
 */
object WhatsNew {
    /** The version without a preview suffix: a preview of 2.0.0 shows the 2.0.0 changes. */
    private fun base(version: String) = version.substringBefore('-')

    /** The changelog section for a version, or the Unreleased one when the version has no heading yet. */
    fun section(changelog: String, version: String): List<String> {
        val lines = changelog.lines()
        val start = lines.indexOfFirst { it.startsWith("## [${base(version)}]") }.takeIf { it >= 0 }
            ?: lines.indexOfFirst { it.startsWith("## [Unreleased]") }.takeIf { it >= 0 }
            ?: return emptyList()
        val end = (start + 1 until lines.size).firstOrNull { lines[it].startsWith("## [") } ?: lines.size
        return lines.subList(start + 1, end)
    }

    /** The first sentence of a bullet, without Markdown marks and without the closing full stop. */
    fun firstSentence(bullet: String): String {
        val plain = bullet.removePrefix("- ").replace("**", "").replace("`", "").trim()
        val cut = Regex("""[.!?](\s|$)""").find(plain)?.range?.first ?: plain.lastIndex
        return plain.substring(0, cut + 1).trimEnd('.', ' ')
    }

    /** The first [count] Added lines of the version, one short sentence each. Empty when the version has none. */
    fun topChanges(changelog: String, version: String, count: Int = 5): List<String> {
        var inAdded = false
        val out = mutableListOf<String>()
        for (line in section(changelog, version)) {
            when {
                line.startsWith("### ") -> inAdded = line.trim() == "### Added"
                inAdded && line.startsWith("- ") -> firstSentence(line).takeIf { it.isNotBlank() }?.let(out::add)
            }
        }
        return out.take(count)
    }

    /** The panel is due when this version has not been seen yet. */
    fun due(seen: String?, current: String): Boolean = seen != current
}

@Composable
internal fun WhatsNewPanel(version: String, changes: List<String>, onClose: () -> Unit, onFull: () -> Unit) {
    Box(
        Modifier.fillMaxSize().background(Color(0xCC000000)).clickable(enabled = true, onClick = {}).windowInsetsPadding(WindowInsets.safeDrawing).padding(16.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier.widthIn(max = 560.dp).fillMaxWidth().pixelFrame(Palette.ink, Palette.gold, shadow = 6.dp).padding(horizontal = 22.dp, vertical = 18.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Text("WHAT'S NEW IN ${version.uppercase()}", color = Palette.gold, fontSize = 16.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
            changes.forEach { Text("+  $it", color = Palette.primaryText, fontSize = 13.sp) }
            Spacer(Modifier.height(4.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                MenuBarButton("Full changelog") { onFull() }
                Spacer(Modifier.weight(1f))
                GoldButton("Close") { onClose() }
            }
        }
    }
}

/** The whole changelog as text, in a scrolling list. */
@Composable
internal fun ChangelogScreen(text: String, onBack: () -> Unit) {
    val lines = androidx.compose.runtime.remember(text) { text.lines() }
    Column(Modifier.fillMaxSize().background(Palette.ink).windowInsetsPadding(WindowInsets.safeDrawing)) {
        MenuTopBar("Changelog", onBack)
        LazyColumn(Modifier.fillMaxSize().padding(horizontal = 18.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 12.dp)) {
            itemsIndexed(lines) { _, line ->
                val heading = line.startsWith("#")
                Text(
                    line.ifEmpty { " " }, color = if (heading) Palette.gold else Palette.primaryText,
                    fontSize = if (heading) 15.sp else 12.sp, fontWeight = if (heading) FontWeight.Bold else FontWeight.Normal,
                    modifier = Modifier.padding(top = if (heading) 8.dp else 0.dp),
                )
            }
        }
    }
}
