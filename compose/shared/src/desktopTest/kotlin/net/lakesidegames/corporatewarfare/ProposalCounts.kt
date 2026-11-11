package net.lakesidegames.corporatewarfare

import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsOwner
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import java.io.File

/**
 * Counts what a picture shows from the code side, with no OCR: every text node that is placed and
 * has area inside the picture (a node scrolled out of its pane or clipped away has none), read from
 * the semantics tree of the scene that was just drawn.
 *
 * texts: visible text nodes. figures: those containing a digit. distinct: unique figure strings.
 * rows: horizontal lines of text (nodes within 5 dp of each other vertically) that hold two or more figures.
 */
object ProposalCounts {
    data class Count(val texts: Int, val figures: Int, val distinct: Int, val rows: Int)

    fun count(scene: ImageComposeScene, width: Int, height: Int, density: Float): Count {
        val owner = semanticsOwner(scene)
        val texts = mutableListOf<Triple<String, Float, Float>>()
        fun walk(n: SemanticsNode) {
            val b = n.boundsInWindow
            val seen = b.width > 0f && b.height > 0f && b.right > 0f && b.bottom > 0f && b.left < width * density && b.top < height * density
            if (seen) {
                n.config.getOrNull(SemanticsProperties.Text)?.joinToString(" ") { it.text }?.takeIf { it.isNotBlank() }
                    ?.let { texts += Triple(it.trim(), b.center.y / density, b.left / density) }
            }
            n.children.forEach(::walk)
        }
        walk(owner.unmergedRootSemanticsNode)
        val figures = texts.filter { (t, _, _) -> t.any { it.isDigit() } }
        val rows = figures.sortedBy { it.second }.fold(mutableListOf<MutableList<Float>>()) { acc, (_, y, _) ->
            if (acc.isNotEmpty() && y - acc.last().last() <= 5f) acc.last().add(y) else acc.add(mutableListOf(y))
            acc
        }.count { it.size >= 2 }
        return Count(texts.size, figures.size, figures.map { it.first }.toSet().size, rows)
    }

    /** The semantics owner of a scene: scene, then its main owner, then that owner's root for tests. */
    private fun semanticsOwner(scene: ImageComposeScene): SemanticsOwner {
        val inner = field(scene, "scene")
        val main = field(inner, "mainOwner")
        val root = field(main, "rootForTest")
        return root.javaClass.getMethod("getSemanticsOwner").also { it.isAccessible = true }.invoke(root) as SemanticsOwner
    }

    private fun field(from: Any, name: String): Any {
        var c: Class<*>? = from.javaClass
        while (c != null) {
            runCatching { c.getDeclaredField(name) }.getOrNull()?.let { f -> f.isAccessible = true; return f.get(from)!! }
            c = c.superclass
        }
        error("no field $name on ${from.javaClass.name}")
    }

    /** Keep [count] for [name] as counts/<name>.tsv in the render folder (name, figures, distinct, rows, texts). */
    fun keep(out: File?, name: String, count: Count) {
        val dir = out?.let { File(it, "counts") }?.also { it.mkdirs() } ?: return
        File(dir, "$name.tsv").writeText("$name\t${count.figures}\t${count.distinct}\t${count.rows}\t${count.texts}\n")
    }
}
