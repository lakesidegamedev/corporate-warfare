package net.lakesidegames.corporatewarfare

import kotlin.math.roundToInt
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A raised dark panel: hard bevelled edges and a hard shadow, the 8-bit look shared with iPhone. */
@Composable
fun Panel(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier
            .fillMaxWidth()
            .padding(end = 3.dp, bottom = 3.dp)
            .pixelFrame(Palette.panel)
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        content = content,
    )
}

/** A small uppercase label over a figure or section. */
@Composable
fun Eyebrow(text: String) {
    Text(text.uppercase(), color = Palette.secondary, fontSize = 12.sp, fontFamily = Pixel.font, letterSpacing = 0.6.sp)
}

/** Title and context line at the top of each tab. */
@Composable
fun ScreenHeader(title: String, subtitle: String?, trailing: @Composable () -> Unit = {}) {
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title.uppercase(), color = Palette.primaryText, fontSize = 24.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
            if (!subtitle.isNullOrEmpty()) Text(subtitle, color = Palette.secondary, fontSize = 12.sp)
        }
        trailing()
    }
}

data class Stat(val label: String, val value: String, val tone: Color = Palette.primaryText, val note: String? = null)

/** A headline figure over compact supporting figures, three to a row. */
@Composable
fun StatGroup(headline: Stat?, stats: List<Stat>) {
    Panel {
        if (headline != null) {
            Text(headline.value, color = headline.tone, fontSize = 28.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(headline.label, color = Palette.secondary, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                headline.note?.let { Text(it, color = Palette.secondary, fontSize = 12.sp) }
            }
        }
        stats.chunked(3).forEach { row ->
            Row(Modifier.fillMaxWidth()) {
                row.forEach { stat ->
                    Column(Modifier.weight(1f)) {
                        Text(stat.label, color = Palette.secondary, fontSize = 11.sp, maxLines = 1)
                        Text(stat.value, color = stat.tone, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
                        stat.note?.let { Text(it, color = Palette.secondary, fontSize = 11.sp, maxLines = 1) }
                    }
                }
                repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** A short coloured status label. */
@Composable
fun SignalPill(text: String, tint: Color) {
    Text(
        text,
        color = tint,
        fontSize = 11.sp,
        fontWeight = FontWeight.Bold,
        modifier = Modifier.clip(Pixel.small).background(tint.copy(alpha = 0.14f)).padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

/** A one-line callout: the next action, a warning, a quiet week. */
@Composable
fun Callout(text: String, tint: Color = Palette.gold) {
    Row(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(tint.copy(alpha = 0.10f))
            .border(1.dp, tint.copy(alpha = 0.25f), Pixel.shape).padding(horizontal = 12.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(8.dp).clip(Pixel.small).background(tint))
        Spacer(Modifier.width(10.dp))
        Text(text, color = Palette.primaryText, fontSize = 14.sp)
    }
}

/** A sector's 8-bit icon on a dark tile with a rim in the sector's colour. */
@Composable
fun SectorBadge(sector: String, size: Int = 26) {
    val icon = SectorIcons.all[sector]
    androidx.compose.foundation.Canvas(
        Modifier.size(size.dp).clip(androidx.compose.foundation.shape.CutCornerShape((size * 0.12).dp)).background(Palette.ink)
            .border(maxOf(1, size / 18).dp, Sectors.color(sector), androidx.compose.foundation.shape.CutCornerShape((size * 0.12).dp)),
    ) {
        if (icon == null) return@Canvas
        val inset = this.size.width * 0.12f
        val cell = (this.size.width - inset * 2) / 12f
        val art = PixelBitmaps.of(icon, icon.rows) { character -> icon.palette[character.toString()]?.let { SectorIcons.color(it) } }
        val side = (cell * 12).roundToInt()
        drawPixelBitmap(art, androidx.compose.ui.unit.IntOffset(inset.roundToInt(), inset.roundToInt()), side, side)
    }
}

/** Revenue against each cost line on one scale. */
@Composable
fun BarRow(name: String, cents: Long, scale: Double, tint: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Text(name, color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.width(112.dp), maxLines = 1, overflow = TextOverflow.Ellipsis)
        Box(Modifier.weight(1f).height(8.dp)) {
            val share = (kotlin.math.abs(cents.toDouble()) / scale).coerceIn(0.0, 1.0).toFloat()
            Box(Modifier.fillMaxWidth(share.coerceAtLeast(0.01f)).height(8.dp).clip(Pixel.small).background(tint))
        }
        Text(Format.compact(cents), color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.width(68.dp).padding(start = 6.dp))
    }
}
