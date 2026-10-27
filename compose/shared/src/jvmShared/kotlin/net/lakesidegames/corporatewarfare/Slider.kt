package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import kotlin.math.roundToLong

/**
 * A fine-grained control for one number: a custom track with tick marks and a filled run from the
 * origin, a gold handle, a live readout of what the value means, and nudge buttons for exact
 * steps. Dragging updates the readout live; [onCommit] fires once when a drag ends, a tap lands
 * or a nudge is pressed, so the engine sees one command per change.
 */
@Composable
fun ValueSlider(
    value: Double,
    onValue: (Double) -> Unit,
    range: ClosedFloatingPointRange<Double>,
    step: Double,
    label: (Double) -> String,
    modifier: Modifier = Modifier,
    origin: Double? = null,
    ticks: Int = 20,
    detail: ((Double) -> String)? = null,
    tint: Color = Palette.gold,
    onCommit: (Double) -> Unit = {},
) {
    var dragging by remember { mutableStateOf(false) }
    val current by rememberUpdatedState(value)
    fun snap(v: Double): Double {
        val clamped = v.coerceIn(range.start, range.endInclusive)
        return (clamped / step).roundToLong() * step
    }
    val span = range.endInclusive - range.start
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(label(value), color = Palette.primaryText, fontSize = 18.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
            Nudge("minus") { val v = snap(current - step); onValue(v); onCommit(v) }
            Spacer(Modifier.width(6.dp))
            Nudge("plus") { val v = snap(current + step); onValue(v); onCommit(v) }
        }
        Canvas(
            Modifier.fillMaxWidth().height(30.dp)
                .pointerInput(range, step) {
                    detectTapGestures { pos ->
                        val inset = 14.dp.toPx()
                        val v = snap(range.start + (pos.x - inset) / (size.width - 2 * inset) * span)
                        onValue(v); onCommit(v)
                    }
                }
                .pointerInput(range, step) {
                    detectDragGestures(
                        onDragStart = { dragging = true },
                        onDragEnd = { dragging = false; onCommit(current) },
                        onDragCancel = { dragging = false; onCommit(current) },
                    ) { change, _ ->
                        val inset = 14.dp.toPx()
                        onValue(snap(range.start + (change.position.x - inset) / (size.width - 2 * inset) * span))
                    }
                },
        ) {
            val mid = size.height / 2
            val trackH = 8.dp.toPx()
            val inset = 14.dp.toPx()
            val width = size.width - 2 * inset
            fun x(v: Double) = (inset + (v - range.start) / span * width).toFloat()
            drawRoundRect(Palette.panelRaised, Offset(inset, mid - trackH / 2), Size(width, trackH), CornerRadius(trackH / 2))
            for (i in 0..ticks) {
                val tx = inset + width * i / ticks
                val h = (if (i % 5 == 0) 16 else 10).dp.toPx()
                drawLine(Palette.secondary.copy(alpha = if (i % 5 == 0) 0.55f else 0.3f), Offset(tx, mid - h / 2), Offset(tx, mid + h / 2), 1f)
            }
            val start = x(origin ?: range.start)
            val end = x(value)
            drawRoundRect(
                Brush.horizontalGradient(listOf(tint.copy(alpha = 0.55f), tint), startX = minOf(start, end), endX = maxOf(start, end) + 1f),
                Offset(minOf(start, end), mid - trackH / 2), Size(abs(end - start), trackH), CornerRadius(trackH / 2),
            )
            val r = (if (dragging) 14 else 12).dp.toPx()
            drawCircle(Color.Black.copy(alpha = 0.35f), r, Offset(end, mid + 1.5f))
            drawCircle(Palette.primaryText, r, Offset(end, mid))
            drawCircle(tint, r - 1.5f, Offset(end, mid), style = Stroke(3f))
        }
        Row(Modifier.fillMaxWidth()) {
            Text(label(range.start), color = Palette.secondary, fontSize = 11.sp, modifier = Modifier.weight(1f))
            Text(label(range.endInclusive), color = Palette.secondary, fontSize = 11.sp)
        }
        detail?.let { Text(it(value), color = Palette.secondary, fontSize = 12.sp) }
    }
}

@Composable
private fun Nudge(symbol: String, onClick: () -> Unit) {
    Box(Modifier.size(30.dp).clip(CircleShape).background(Palette.panelRaised).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        PixelIcon(symbol, size = 16.dp, tint = Palette.primaryText)
    }
}
