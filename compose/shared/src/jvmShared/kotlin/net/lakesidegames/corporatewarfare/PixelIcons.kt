package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.math.roundToInt

/** The 8-bit UI icon set shared with the Apple apps (pixel-icons.json), 16 by 16 per icon. */
object PixelIcons {
    @Serializable data class Icon(val palette: Map<String, String>, val rows: List<String>)
    @Serializable private data class File(val size: Int = 16, val icons: Map<String, Icon>)

    private val file: File? by lazy {
        val stream = PixelIcons::class.java.getResourceAsStream("/pixel-icons.json") ?: return@lazy null
        Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), stream.bufferedReader().readText())
    }

    val size: Int get() = file?.size ?: 16
    val all: Map<String, Icon> get() = file?.icons ?: emptyMap()

    /** Mono alpha per palette letter: lighter pixels stay solid, shade pixels recede. */
    fun monoAlpha(icon: Icon): Map<String, Float> {
        val lum = icon.palette.mapValues { luminance(SectorIcons.color(it.value)) }
        val top = lum.values.maxOrNull()?.takeIf { it > 0f } ?: 1f
        return lum.mapValues { (it.value / top).coerceIn(0.5f, 1f) }
    }

    private fun luminance(c: Color): Float = 0.299f * c.red + 0.587f * c.green + 0.114f * c.blue
}

/** Draws [icon] into this scope as exact pixels. A non-null [tint] repaints it in one colour, keeping its shading. */
fun DrawScope.drawPixelIcon(icon: PixelIcons.Icon, tint: Color?) {
    val n = PixelIcons.size
    val cell = minOf(size.width, size.height) / n
    val alpha = if (tint != null) PixelIcons.monoAlpha(icon) else emptyMap()
    val colors = icon.palette.mapValues { SectorIcons.color(it.value) }
    icon.rows.forEachIndexed { y, row ->
        row.forEachIndexed { x, ch ->
            if (ch == '.') return@forEachIndexed
            val base = colors[ch.toString()] ?: return@forEachIndexed
            val color = if (tint != null) tint.copy(alpha = tint.alpha * (alpha[ch.toString()] ?: 1f)) else base
            // Round both edges so neighbouring pixels meet without seams or blur.
            val left = (x * cell).roundToInt().toFloat()
            val top = (y * cell).roundToInt().toFloat()
            val right = ((x + 1) * cell).roundToInt().toFloat()
            val bottom = ((y + 1) * cell).roundToInt().toFloat()
            drawRect(color, Offset(left, top), Size(right - left, bottom - top))
        }
    }
}

/** A pixel icon by name. Pass [tint] for one-colour use in menus, buttons and bars; omit it for the full palette. */
@Composable
fun PixelIcon(name: String, modifier: Modifier = Modifier, size: Dp = 18.dp, tint: Color? = null) {
    val icon = PixelIcons.all[name]
    Canvas(modifier.size(size)) { if (icon != null) drawPixelIcon(icon, tint) }
}

/** The content of a Back style text button: pixel arrow then the label. */
@Composable
fun BackLabel(text: String, color: Color) {
    androidx.compose.foundation.layout.Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
        PixelIcon("back", size = 14.dp, tint = color)
        androidx.compose.foundation.layout.Spacer(Modifier.then(Modifier.size(6.dp)))
        androidx.compose.material3.Text(text, color = color)
    }
}
