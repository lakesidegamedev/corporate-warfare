package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.roundToInt

/**
 * Vehicles as ready-made pictures. A voxel model costs a few dozen filled faces every time it is
 * drawn; a lane can hold hundreds of vehicles, so up close the same car or truck is painted once
 * for each of [HEADINGS] directions at the current zoom and camera angle, in the background, and
 * every vehicle after that is one blit. While a picture is not ready the vehicle is drawn the slow
 * way, so a turning camera never shows a hole, it just pays the old cost until the new set lands.
 * Planes (high above the ground, few of them) are always drawn as models.
 */
internal object VehicleSprites {
    const val HEADINGS = 32

    private data class Id(val model: String, val tone: Any?, val heading: Int, val bearing: Int, val pitch: Int, val unit: Int)
    class Sprite(val image: ImageBitmap, /** Where the model's ground anchor falls inside [image]. */ val ax: Float, val ay: Float)

    private val ready = ConcurrentHashMap<Id, Sprite>()
    private val pending = ConcurrentHashMap.newKeySet<Id>()
    private val worker = Executors.newSingleThreadExecutor { r -> Thread(r, "vehicle-sprites").apply { isDaemon = true; priority = Thread.NORM_PRIORITY - 1 } }

    /** Whether the finished pictures can be used on this platform; tests that compare drawing paths turn it off. */
    @Volatile var enabled = true

    /** Waits until every requested picture is painted. For benchmarks and tests. */
    fun settle(maxMillis: Long = 10_000) {
        val end = System.currentTimeMillis() + maxMillis
        while (pending.isNotEmpty() && System.currentTimeMillis() < end) Thread.sleep(2)
    }

    fun clear() { ready.clear() }

    /**
     * Draws [name] at ground offset ([gx], [gy]) as one picture when one is ready for this camera, heading and zoom.
     * Returns false when it is not, and the caller draws the model; the picture is then painted for next time.
     */
    fun DrawScope.drawSprite(name: String, p: MapProjection, gx: Float, gy: Float, floor: Double, unit: Float, heading: Double, tone: ((Color) -> Color)?): Boolean {
        if (!enabled) return false
        val bucket = (floor(((heading / (2 * PI)) % 1.0 + 1.0) % 1.0 * HEADINGS + 0.5).toInt()) % HEADINGS
        val bearing = (atan2(p.east.y.toDouble(), p.east.x.toDouble()) * 180 / PI).roundToInt()
        val pitch = (p.pitch * 180 / PI).roundToInt()
        val id = Id(name, tone, bucket, bearing, pitch, (unit * 20f).roundToInt())
        val sprite = ready[id]
        if (sprite == null) {
            if (ready.size > 4000) ready.clear()
            if (pending.add(id)) {
                // Painted on the camera of this frame, snapped to the same buckets the picture is filed under.
                worker.execute { try { ready[id] = bake(id, p, unit, tone) } finally { pending.remove(id) } }
            }
            return false
        }
        // Perspective makes things nearer the viewer a little bigger: the model's own scale here, from one pixel of ground.
        val at = p.lift(gx.toDouble(), gy.toDouble(), floor)
        val s = p.lift(gx + 1.0, gy.toDouble(), floor).x - at.x
        val k = if (s > 0.97f && s < 1.03f) 1f else s
        val w = sprite.image.width; val h = sprite.image.height
        drawImage(
            sprite.image, srcOffset = IntOffset.Zero, srcSize = IntSize(w, h),
            dstOffset = IntOffset((at.x - sprite.ax * k).roundToInt(), (at.y - sprite.ay * k).roundToInt()),
            dstSize = IntSize(if (k == 1f) w else (w * k).roundToInt().coerceAtLeast(1), if (k == 1f) h else (h * k).roundToInt().coerceAtLeast(1)),
            filterQuality = if (k == 1f) FilterQuality.None else FilterQuality.Low,
        )
        return true
    }

    private fun bake(id: Id, p: MapProjection, unit: Float, tone: ((Color) -> Color)?): Sprite {
        val heading = id.heading * 2 * PI / HEADINGS
        val reach = ceil(VoxelModels.reach(id.model, unit)).toInt() + 4
        val side = reach * 2
        val big = ImageBitmap(side, side)
        val size = Size(side.toFloat(), side.toFloat())
        val bounds = VoxelModels.Bounds()
        CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, Canvas(big), size) {
            // The ground point (0, 0) is the view centre, where there is no perspective: move it to the middle of the scratch bitmap.
            withTransform({ translate(reach - p.cx.toFloat(), reach - p.cy.toFloat()) }) {
                with(VoxelModels) { drawModel(id.model, p, 0f, 0f, 0.0, unit, heading, tone = tone, bounds = bounds) }
            }
        }
        if (!bounds.valid) return Sprite(big, reach.toFloat(), reach.toFloat())
        val l = floor(bounds.l).toInt().coerceIn(0, side - 1) - 1
        val t = floor(bounds.t).toInt().coerceIn(0, side - 1) - 1
        val r = ceil(bounds.r).toInt().coerceIn(1, side) + 1
        val b = ceil(bounds.b).toInt().coerceIn(1, side) + 1
        val x0 = l.coerceAtLeast(0); val y0 = t.coerceAtLeast(0); val x1 = r.coerceAtMost(side); val y1 = b.coerceAtMost(side)
        val tight = ImageBitmap(x1 - x0, y1 - y0)
        Canvas(tight).drawImageRect(big, IntOffset(x0, y0), IntSize(x1 - x0, y1 - y0), IntOffset.Zero, IntSize(x1 - x0, y1 - y0), androidx.compose.ui.graphics.Paint())
        return Sprite(tight, reach - x0.toFloat(), reach - y0.toFloat())
    }
}
