package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
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

/**
 * Pixel art rasterised once into a small bitmap and drawn scaled with no smoothing. Drawing the art
 * as one rectangle per pixel on every frame cost hundreds of draw calls per icon.
 */
internal object PixelBitmaps {
    private val cache = java.util.concurrent.ConcurrentHashMap<Any, ImageBitmap>()

    /** The bitmap for [key], built from [rows] with [colorOf] per letter on first use. */
    fun of(key: Any, rows: List<String>, colorOf: (Char) -> Color?): ImageBitmap = cache.getOrPut(key) {
        val width = rows.maxOfOrNull { it.length } ?: 1
        val bitmap = ImageBitmap(width.coerceAtLeast(1), rows.size.coerceAtLeast(1))
        val canvas = Canvas(bitmap)
        val paint = Paint()
        rows.forEachIndexed { y, row ->
            row.forEachIndexed { x, ch ->
                val color = if (ch == '.') null else colorOf(ch)
                if (color != null) {
                    paint.color = color
                    canvas.drawRect(x.toFloat(), y.toFloat(), x + 1f, y + 1f, paint)
                }
            }
        }
        bitmap
    }
}

/** Draws [bitmap] to fill a [side] square at [at], pixels hard-edged. */
internal fun DrawScope.drawPixelBitmap(bitmap: ImageBitmap, at: IntOffset = IntOffset.Zero, width: Int, height: Int) {
    drawImage(
        bitmap,
        srcOffset = IntOffset.Zero,
        srcSize = IntSize(bitmap.width, bitmap.height),
        dstOffset = at,
        dstSize = IntSize(width, height),
        filterQuality = FilterQuality.None,
    )
}

private data class IconKey(val icon: PixelIcons.Icon, val tint: Color?)

/** The icon as a bitmap, repainted in one colour when [tint] is set, keeping its shading. */
internal fun PixelIcons.Icon.bitmap(tint: Color?): ImageBitmap = PixelBitmaps.of(IconKey(this, tint), rows) { ch ->
    val base = palette[ch.toString()]?.let { SectorIcons.color(it) } ?: return@of null
    if (tint == null) base else tint.copy(alpha = tint.alpha * (PixelIcons.monoAlpha(this)[ch.toString()] ?: 1f))
}

/** Draws [icon] into this scope as exact pixels. A non-null [tint] repaints it in one colour, keeping its shading. */
fun DrawScope.drawPixelIcon(icon: PixelIcons.Icon, tint: Color?) {
    val side = minOf(size.width, size.height).roundToInt()
    drawPixelBitmap(icon.bitmap(tint), width = side, height = side)
}

/** A pixel icon by name. Pass [tint] for one-colour use in menus, buttons and bars; omit it for the full palette. */
@Composable
fun PixelIcon(name: String, modifier: Modifier = Modifier, size: Dp = 18.dp, tint: Color? = null, description: String? = null) {
    val icon = PixelIcons.all[name]
    // An icon that stands alone as a button says what it does; one beside a label stays silent.
    Canvas((if (description != null) modifier.described(description, androidx.compose.ui.semantics.Role.Image) else modifier).size(size)) { if (icon != null) drawPixelIcon(icon, tint) }
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
