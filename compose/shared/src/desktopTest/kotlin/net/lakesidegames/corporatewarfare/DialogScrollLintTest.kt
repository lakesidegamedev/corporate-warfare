package net.lakesidegames.corporatewarfare

import java.io.File
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * A new dialog whose body cannot scroll fails here, before it can cut a button off on a short window. A body counts as
 * scrolling when its slot uses dialogScroll, verticalScroll or a lazy list, or hands off to a body that does.
 */
class DialogScrollLintTest {
    private val scrolls = Regex("""dialogScroll|verticalScroll|LazyColumn|GuidanceBody|HireListBody""")

    @Test
    fun everyAlertDialogBodyScrolls() {
        val root = generateSequence(File("").absoluteFile) { it.parentFile }
            .map { File(it, "shared/src/jvmShared/kotlin") }.firstOrNull { it.isDirectory }
            ?: File("src/jvmShared/kotlin")
        assertTrue(root.isDirectory, "source folder not found from ${File("").absolutePath}")
        val bad = mutableListOf<String>()
        var seen = 0
        root.walkTopDown().filter { it.extension == "kt" }.forEach { file ->
            val lines = file.readLines()
            lines.forEachIndexed { i, line ->
                if (!line.contains("AlertDialog(")) return@forEachIndexed
                seen++
                val text = (i + 1 until minOf(i + 60, lines.size)).firstOrNull { lines[it].trim().startsWith("text = ") } ?: return@forEachIndexed
                // The text slot runs until the next slot of the same dialog.
                val end = (text + 1 until minOf(text + 60, lines.size)).firstOrNull { Regex("""^\s*(confirmButton|dismissButton|title|onDismissRequest|containerColor|modifier) = """).containsMatchIn(lines[it]) } ?: minOf(text + 30, lines.size)
                val body = lines.subList(text, end).joinToString("\n")
                if (!scrolls.containsMatchIn(body)) bad += "${file.name}:${i + 1}"
            }
        }
        assertTrue(seen > 15, "expected to find the app's dialogs, found $seen")
        assertTrue(bad.isEmpty(), "dialog bodies that do not scroll (use Modifier.dialogScroll()): $bad")
    }
}
