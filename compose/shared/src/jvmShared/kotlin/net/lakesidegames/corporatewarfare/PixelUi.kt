package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.shape.CutCornerShape
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The 8-bit look: stepped corners, hard two-pixel edges lit from the upper left, hard drop
 * shadows, no blur and no translucency, and a pixel font (Silkscreen, SIL Open Font License) for
 * headings, labels and buttons. Body text and figures stay in the system font so they read
 * easily. The iPhone app follows the same rules.
 */
object Pixel {
    /** Panels, buttons and dialogs: corners cut by a pixel step. */
    val shape = CutCornerShape(4.dp)
    /** Chips, pills and small badges. */
    val small = CutCornerShape(2.dp)
    /** The pixel font, set by each platform at launch; the system font until then and in tests. */
    var font: FontFamily = FontFamily.Default
}

/**
 * A solid frame for a panel or button: a hard shadow down and right, the fill, then a bevel,
 * light along the top and left edges and dark along the bottom and right, inside a hard outline.
 * [cut] matches the corner step of the shape the content is clipped to.
 */
fun Modifier.pixelFrame(fill: Color, outline: Color = Palette.stroke, shadow: Dp = 3.dp, cut: Dp = 4.dp, raised: Boolean = true): Modifier = drawBehind {
    val c = cut.toPx()
    val w = size.width
    val h = size.height
    fun stepped(dx: Float, dy: Float, sw: Float, sh: Float) = Path().apply {
        moveTo(dx + c, dy); lineTo(dx + sw - c, dy); lineTo(dx + sw, dy + c); lineTo(dx + sw, dy + sh - c)
        lineTo(dx + sw - c, dy + sh); lineTo(dx + c, dy + sh); lineTo(dx, dy + sh - c); lineTo(dx, dy + c); close()
    }
    val s = shadow.toPx()
    if (s > 0f) drawPath(stepped(s, s, w, h), Color.Black.copy(alpha = 0.55f))
    drawPath(stepped(0f, 0f, w, h), fill)
    val px = 2.dp.toPx()
    val light = Color.White.copy(alpha = if (raised) 0.16f else 0.04f)
    val dark = Color.Black.copy(alpha = if (raised) 0.40f else 0.15f)
    // Top and left catch the light, bottom and right fall into shade.
    drawRect(light, Offset(c, px), Size(w - 2 * c, px))
    drawRect(light, Offset(px, c), Size(px, h - 2 * c))
    drawRect(dark, Offset(c, h - 2 * px), Size(w - 2 * c, px))
    drawRect(dark, Offset(w - 2 * px, c), Size(px, h - 2 * c))
    drawPath(stepped(0f, 0f, w, h), outline, style = androidx.compose.ui.graphics.drawscope.Stroke(px))
}
