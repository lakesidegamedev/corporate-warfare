package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import java.util.concurrent.Executors
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.roundToInt

/** Where map layers are baked: two daemon threads, so drawing the map never waits behind a bake. */
internal object MapBake {
    /** Layers bake off the UI thread. The desktop app switches it on; the benchmark and tests may too. */
    @Volatile var enabled = false

    val dispatcher = Executors.newFixedThreadPool(2) { r -> Thread(r, "map-bake").apply { isDaemon = true; priority = Thread.NORM_PRIORITY - 1 } }
        .asCoroutineDispatcher()
}

/**
 * One map layer drawn into a bitmap on a background thread. The UI thread draws the newest finished
 * bitmap and nothing else: when the camera has moved on since the bitmap was baked, it is drawn
 * moved, scaled and turned to where the camera is now, and replaced as soon as a fresh one lands
 * (one frame, no pop). The first frame, and a resized view, draw directly so there is never a hole.
 *
 * The bake draws through [geometry], a cache of its own, so it never shares a path with the UI
 * thread. Three bitmaps rotate so the one on screen is never the one being painted.
 */
internal class AsyncLayer(private val scope: CoroutineScope) {
    class Baked(val image: ImageBitmap, val cam: MapCamera, val pitch: Double, val size: Size, val key: Any?)

    private class Wanted(
        val cam: MapCamera, val pitch: Double, val key: Any?, val size: Size, val density: Density, val minGapMs: Long,
        val bake: DrawScope.(MapGeometryCache) -> Unit,
    )

    /** Projection cache for the bake thread only. */
    val geometry = MapGeometryCache()

    @Volatile var baked: Baked? = null
        private set
    private var version by mutableIntStateOf(0)
    private val pool = arrayOfNulls<ImageBitmap>(3)
    private var next = 0
    private val lock = Any()
    private var job: Job? = null
    private var wanted: Wanted? = null
    private var lastDone = 0L
    private var failures = 0
    private var closed = false

    /** The benchmark and tests hold bakes back so a draw only ever shows what is already there. */
    @Volatile var paused = false

    fun close() = synchronized(lock) { closed = true; wanted = null; job?.cancel() }

    /**
     * Draws the layer for [cam]. [key] is everything else the picture depends on, compared by equality; [bake] paints it
     * through the geometry it is given, [direct] paints it on the UI thread when there is nothing to show yet. A change of
     * [key] alone is baked no sooner than [minGapMs] after the last bake, so a slow clock does not keep a core busy.
     * [fill] shows where a moved bitmap does not reach.
     */
    fun draw(
        scope: DrawScope, cam: MapCamera, pitch: Double, key: Any?, minGapMs: Long, fill: Color?,
        bake: DrawScope.(MapGeometryCache) -> Unit, direct: DrawScope.() -> Unit,
    ) = with(scope) {
        // Reading the version redraws this layer when a bake lands.
        @Suppress("UNUSED_VARIABLE") val seen = version
        val b = baked
        if (failures >= 3 || closed) return@with direct()
        val same = b != null && b.size == size && b.cam == cam && b.pitch == pitch
        if (same && b!!.key == key) {
            drawBitmap(b.image)
            return@with
        }
        if (!paused) request(Wanted(cam, pitch, key, size, Density(density, fontScale), minGapMs, bake))
        if (b == null || b.size != size) return@with direct()
        drawMoved(b, cam, pitch, fill)
    }

    internal fun DrawScope.drawBitmap(image: ImageBitmap) {
        drawImage(
            image, IntOffset.Zero, IntSize(image.width, image.height), IntOffset.Zero, IntSize(image.width, image.height),
            filterQuality = FilterQuality.None,
        )
    }

    /** The old bitmap carried to the new camera: shifted so the new centre lands where the projection puts it, scaled by the zoom, turned by the bearing. */
    internal fun DrawScope.drawMoved(b: Baked, cam: MapCamera, pitch: Double, fill: Color?) {
        val t = stalePlacement(b.cam, b.pitch, cam, pitch, size)
        if (t == null) { drawBitmap(b.image); return }
        val (a, d, s, rot) = t
        // Where the view's corners fall in the old bitmap decides whether any of the view is uncovered.
        if (fill != null) {
            val rad = -rot * PI / 180
            val c = kotlin.math.cos(rad); val sn = kotlin.math.sin(rad)
            var covered = true
            for ((vx, vy) in listOf(0f to 0f, size.width to 0f, 0f to size.height, size.width to size.height)) {
                val dx = (vx - d.x) / s; val dy = (vy - d.y) / s
                val sx = a.x + dx * c - dy * sn; val sy = a.y + dx * sn + dy * c
                if (sx < 0 || sy < 0 || sx > b.image.width || sy > b.image.height) covered = false
            }
            if (!covered) drawRect(fill)
        }
        withTransform({
            translate(d.x, d.y)
            rotate(rot.toFloat(), Offset.Zero)
            scale(s, s, Offset.Zero)
            translate(-a.x, -a.y)
        }) {
            drawImage(
                b.image, IntOffset.Zero, IntSize(b.image.width, b.image.height), IntOffset.Zero, IntSize(b.image.width, b.image.height),
                filterQuality = FilterQuality.Low,
            )
        }
    }

    private fun request(w: Wanted) {
        synchronized(lock) {
            if (closed) return
            wanted = w
            if (job != null) return
            job = scope.launch(MapBake.dispatcher) {
                try {
                    while (true) {
                        val first = synchronized(lock) { wanted } ?: break
                        val b = baked
                        val urgent = b == null || b.cam != first.cam || b.pitch != first.pitch || b.size != first.size
                        if (!urgent) {
                            val wait = first.minGapMs - (System.nanoTime() - lastDone) / 1_000_000
                            if (wait > 0) delay(wait)
                        }
                        // Bake what is wanted now, not what was wanted before the wait.
                        val w2 = synchronized(lock) { val x = wanted; wanted = null; x } ?: break
                        val now = baked
                        if (now != null && now.cam == w2.cam && now.pitch == w2.pitch && now.size == w2.size && now.key == w2.key) continue
                        val result = runCatching { bakeOnce(w2) }
                        result.onSuccess { baked = it; failures = 0; lastDone = System.nanoTime(); version++ }
                        result.onFailure { failures++ }
                    }
                } finally {
                    synchronized(lock) { job = null }
                }
            }
        }
    }

    /** Paints one bitmap now, on the calling thread. The benchmark and tests use it directly. */
    fun bakeNow(cam: MapCamera, pitch: Double, key: Any?, size: Size, density: Density, bake: DrawScope.(MapGeometryCache) -> Unit) {
        baked = bakeOnce(Wanted(cam, pitch, key, size, density, 0, bake))
    }

    private fun bakeOnce(w: Wanted): Baked {
        val width = w.size.width.roundToInt().coerceAtLeast(1)
        val height = w.size.height.roundToInt().coerceAtLeast(1)
        var image = pool[next]
        if (image == null || image.width != width || image.height != height) {
            image = ImageBitmap(width, height)
            pool[next] = image
        }
        next = (next + 1) % pool.size
        val canvas = androidx.compose.ui.graphics.Canvas(image)
        canvas.drawRect(0f, 0f, width.toFloat(), height.toFloat(), Paint().apply { blendMode = BlendMode.Clear })
        CanvasDrawScope().draw(w.density, LayoutDirection.Ltr, canvas, Size(width.toFloat(), height.toFloat())) { w.bake(this, geometry) }
        return Baked(image, w.cam, w.pitch, w.size, w.key)
    }

    companion object {
        /** Where a bitmap baked for [from] lands for [to]: its own anchor point, the view anchor, the scale and the turn in degrees. */
        fun stalePlacement(from: MapCamera, fromPitch: Double, to: MapCamera, toPitch: Double, size: Size): Placement? {
            val p0 = MapProjection(from, size, fromPitch)
            val p1 = MapProjection(to, size, toPitch)
            val lon = to.lon * PI / 180; val lat = to.lat * PI / 180
            val a = p0.at(lon, lat); val d = p1.at(lon, lat)
            if (!a.isSpecified || !d.isSpecified) return null
            var rot = to.bearing - from.bearing
            if (rot > 180) rot -= 360
            if (rot < -180) rot += 360
            return Placement(a, d, (to.zoom / from.zoom).toFloat(), rot)
        }
    }

    data class Placement(val from: Offset, val to: Offset, val scale: Float, val turn: Double)
}
