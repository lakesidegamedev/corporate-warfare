package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
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
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sqrt

/**
 * The classic map's layers as one set. The ground, the clouds and the structures (buildings and labels) are baked
 * together, always for the same camera, and swapped in as a set. The moving layers (traffic and effects) are drawn
 * through the same warp as the baked pictures, at the camera the set was baked for, so while the camera is moving every
 * layer is carried to the new view by one transform and nothing slides against anything else.
 *
 * While the camera moves, a set is baked at a reduced size so a fresh one lands quickly; once the camera has rested for
 * a moment a full size set replaces it. The first frame, and a resized view, draw every layer directly.
 */
internal class MapLayerSet(private val scope: CoroutineScope) {
    enum class Layer { Ground, Traffic, Clouds, Structures, Effects }

    /** One baked layer: everything but the camera that it depends on ([key], compared by equality) and how to paint it. */
    class PartSpec(val key: Any?, val minGapMs: Long, val bake: DrawScope.(MapGeometryCache) -> Unit)

    /** What to draw this frame. The baked parts are painted for the camera they are requested at; the live ones for the camera they are given. */
    class Frame(
        val ground: PartSpec,
        val clouds: PartSpec?,
        val structures: PartSpec,
        val traffic: (DrawScope.(MapCamera, Double) -> Unit)?,
        val effects: (DrawScope.(MapCamera, Double) -> Unit)?,
    ) {
        internal val parts: Array<PartSpec?> get() = arrayOf(ground, clouds, structures)
    }

    /** The camera each layer was drawn for (a baked layer warped to the view counts as the camera it was baked for, with [warped] set). */
    class Report(val target: MapCamera, val drawnFor: Map<Layer, MapCamera>, val warped: Boolean, val reduced: Boolean)

    class Baked(
        val images: Array<ImageBitmap?>, val keys: Array<Any?>, val cam: MapCamera, val pitch: Double, val size: Size, val scale: Float,
    )

    private class Wanted(val cam: MapCamera, val pitch: Double, val size: Size, val density: Density, val parts: Array<PartSpec?>)

    /** Projection cache for the bake thread only. */
    val geometry = MapGeometryCache()

    @Volatile var baked: Baked? = null
        private set
    @Volatile var lastReport: Report? = null
        private set
    private var version by mutableIntStateOf(0)
    private val pools = Array(2) { Array(PARTS) { arrayOfNulls<ImageBitmap>(3) } }
    private val poolNext = Array(2) { IntArray(PARTS) }
    private val lastBaked = LongArray(PARTS)
    private val lock = Any()
    private var job: Job? = null
    private var wanted: Wanted? = null
    private var camChangedAt = 0L
    private var requestedCam: MapCamera? = null
    private var requestedPitch = 0.0
    private var failures = 0
    private var closed = false

    /** The benchmark and tests hold bakes back so a draw only ever shows what is already there. */
    @Volatile var paused = false

    /** Fixes how a set baked for a moving camera is shrunk (a fraction of each side); null picks it from the view size. */
    @Volatile var movingScaleOverride: Float? = null

    /** Told each time a set lands: the fraction of the view it was baked at, and the time it took in milliseconds. The benchmark listens. */
    @Volatile var onBake: ((Float, Double) -> Unit)? = null

    fun close() = synchronized(lock) { closed = true; wanted = null; job?.cancel() }

    /**
     * Draws every layer of [frame] for [cam]. With bakes switched off, or before the first set lands, each layer is drawn
     * directly at [cam]. Otherwise the newest set is drawn, carried to [cam] by one transform.
     */
    fun draw(scope: DrawScope, cam: MapCamera, pitch: Double, frame: Frame, ui: MapGeometryCache, fill: Color): Report = with(scope) {
        // Reading the version redraws this set when a bake lands.
        @Suppress("UNUSED_VARIABLE") val seen = version
        val b = baked
        val usable = MapBake.enabled && failures < 3 && !closed
        if (!usable) return@with direct(cam, pitch, frame, ui)
        val parts = frame.parts
        val satisfied = b != null && b.size == size && b.cam == cam && b.pitch == pitch && b.scale >= 1f &&
            parts.indices.all { i -> parts[i]?.key == b.keys[i] && (parts[i] == null) == (b.images[i] == null) }
        if (!satisfied && !paused) request(Wanted(cam, pitch, size, Density(density, fontScale), parts))
        if (b == null || b.size != size) return@with direct(cam, pitch, frame, ui)
        val drawn = HashMap<Layer, MapCamera>()
        val bc = b.cam
        val bp = b.pitch
        val moved = bc != cam || bp != pitch
        val t = if (moved) AsyncLayer.stalePlacement(bc, bp, cam, pitch, size) else null
        val quality = if (moved || b.scale < 1f) FilterQuality.Low else FilterQuality.None
        fun image(i: Int, layer: Layer) {
            val img = b.images[i] ?: return
            drawImage(img, IntOffset.Zero, IntSize(img.width, img.height), IntOffset.Zero, IntSize(size.width.roundToInt(), size.height.roundToInt()), filterQuality = quality)
            drawn[layer] = bc
        }
        fun layers() {
            image(GROUND, Layer.Ground)
            frame.traffic?.let { it(this, bc, bp); drawn[Layer.Traffic] = bc }
            image(CLOUDS, Layer.Clouds)
            image(STRUCTURES, Layer.Structures)
            frame.effects?.let { it(this, bc, bp); drawn[Layer.Effects] = bc }
        }
        if (t == null) {
            layers()
        } else {
            val (a, d, s, rot) = t
            // The ground beyond the baked picture is the space colour, drawn once under the transform.
            drawRect(fill)
            if (rot == 0.0 && abs(s - 1f) < 0.0005f) {
                val dx = kotlin.math.round(d.x - a.x); val dy = kotlin.math.round(d.y - a.y)
                withTransform({ translate(dx, dy) }) { layers() }
            } else {
                withTransform({
                    translate(d.x, d.y)
                    rotate(rot.toFloat(), Offset.Zero)
                    scale(s, s, Offset.Zero)
                    translate(-a.x, -a.y)
                }) { layers() }
            }
        }
        Report(cam, drawn, moved, b.scale < 1f).also { lastReport = it }
    }

    private fun DrawScope.direct(cam: MapCamera, pitch: Double, frame: Frame, ui: MapGeometryCache): Report {
        val drawn = HashMap<Layer, MapCamera>()
        frame.ground.bake(this, ui); drawn[Layer.Ground] = cam
        frame.traffic?.let { it(this, cam, pitch); drawn[Layer.Traffic] = cam }
        frame.clouds?.let { it.bake(this, ui); drawn[Layer.Clouds] = cam }
        frame.structures.bake(this, ui); drawn[Layer.Structures] = cam
        frame.effects?.let { it(this, cam, pitch); drawn[Layer.Effects] = cam }
        return Report(cam, drawn, false, false).also { lastReport = it }
    }

    private fun request(w: Wanted) {
        synchronized(lock) {
            if (closed) return
            if (requestedCam != w.cam || requestedPitch != w.pitch) camChangedAt = System.nanoTime()
            requestedCam = w.cam; requestedPitch = w.pitch
            wanted = w
            if (job != null) return
            job = scope.launch(MapBake.dispatcher) { run() }
        }
    }

    private suspend fun run() {
        try {
            while (true) {
                // Looking and giving up the job are one step, so a request made as the loop ends starts a new one.
                val w = synchronized(lock) { wanted ?: run { job = null; null } } ?: return
                val cur = baked
                val sameView = cur != null && cur.size == w.size && cur.cam == w.cam && cur.pitch == w.pitch
                val todo = BooleanArray(PARTS)
                var scale = 1f
                if (!sameView) {
                    // A moving camera gets a small, quick set; the first set for a view is full size, nothing is on screen yet.
                    if (cur != null && cur.size == w.size) scale = movingScaleOverride ?: movingScaleFor(w.size.width.toDouble() * w.size.height)
                    for (i in 0 until PARTS) todo[i] = w.parts[i] != null
                } else {
                    cur!!
                    val now = System.nanoTime()
                    var wait = Long.MAX_VALUE
                    var due = false
                    for (i in 0 until PARTS) {
                        val p = w.parts[i]
                        val stale = if (p == null) cur.images[i] != null else cur.images[i] == null || cur.keys[i] != p.key
                        if (!stale) continue
                        val left = (p?.minGapMs ?: 0L) - (now - lastBaked[i]) / 1_000_000
                        if (left <= 0) { todo[i] = true; due = true } else wait = min(wait, left)
                    }
                    if (cur.scale < 1f) {
                        // The camera has stopped: replace the small set with a full one, once it has stayed put for a moment.
                        val left = SETTLE_MS - (now - camChangedAt) / 1_000_000
                        if (left > 0) { delay(left); continue }
                        for (i in 0 until PARTS) todo[i] = w.parts[i] != null
                        due = true
                    }
                    if (!due) {
                        if (wait == Long.MAX_VALUE) synchronized(lock) { if (wanted === w) wanted = null } else delay(wait)
                        continue
                    }
                }
                val take = synchronized(lock) { if (wanted === w) { wanted = null; true } else false }
                if (!take) continue
                val started = System.nanoTime()
                val result = runCatching { bakeOnce(w, todo, scale, cur.takeIf { sameView && scale >= 1f && it!!.scale >= 1f }) }
                result.onSuccess { baked = it; failures = 0; version++; onBake?.invoke(scale, (System.nanoTime() - started) / 1e6) }
                result.onFailure { failures++ }
                // A failed bake is not retried until a draw asks again.
                if (result.isFailure) synchronized(lock) { if (wanted == null) { job = null; return } }
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            synchronized(lock) { job = null }
            throw e
        } catch (e: Throwable) {
            failures++
            synchronized(lock) { job = null }
        }
    }

    /** Paints a full size set now, on the calling thread. The benchmark and tests use it directly. */
    fun bakeNow(frame: Frame, cam: MapCamera, pitch: Double, size: Size, density: Density, scale: Float = 1f) {
        val parts = frame.parts
        baked = bakeOnce(Wanted(cam, pitch, size, density, parts), BooleanArray(PARTS) { parts[it] != null }, scale, null)
    }

    private fun bakeOnce(w: Wanted, todo: BooleanArray, scale: Float, carry: Baked?): Baked {
        val width = w.size.width.roundToInt().coerceAtLeast(1)
        val height = w.size.height.roundToInt().coerceAtLeast(1)
        val kind = if (scale >= 1f) 0 else 1
        val iw = (width * scale).roundToInt().coerceAtLeast(1)
        val ih = (height * scale).roundToInt().coerceAtLeast(1)
        val images = arrayOfNulls<ImageBitmap>(PARTS)
        val keys = arrayOfNulls<Any?>(PARTS)
        for (i in 0 until PARTS) {
            val p = w.parts[i]
            if (p == null) continue
            if (!todo[i] && carry != null) { images[i] = carry.images[i]; keys[i] = carry.keys[i]; continue }
            val pool = pools[kind][i]
            var image = pool[poolNext[kind][i]]
            if (image == null || image.width != iw || image.height != ih) {
                image = ImageBitmap(iw, ih)
                pool[poolNext[kind][i]] = image
            }
            poolNext[kind][i] = (poolNext[kind][i] + 1) % pool.size
            val canvas = androidx.compose.ui.graphics.Canvas(image)
            canvas.drawRect(0f, 0f, iw.toFloat(), ih.toFloat(), Paint().apply { blendMode = BlendMode.Clear })
            canvas.save()
            if (scale < 1f) canvas.scale(iw / w.size.width, ih / w.size.height)
            CanvasDrawScope().draw(w.density, LayoutDirection.Ltr, canvas, w.size) { p.bake(this, geometry) }
            canvas.restore()
            images[i] = image
            keys[i] = p.key
            lastBaked[i] = System.nanoTime()
        }
        return Baked(images, keys, w.cam, w.pitch, w.size, scale)
    }

    companion object {
        const val GROUND = 0
        const val CLOUDS = 1
        const val STRUCTURES = 2
        const val PARTS = 3

        /** How long the camera must rest before the small set is replaced by a full one. */
        const val SETTLE_MS = 90L

        /** The small size for a view of [pixels], aiming for about a million baked pixels while the camera moves. */
        fun movingScaleFor(pixels: Double): Float = min(1.0, sqrt(1_000_000.0 / pixels.coerceAtLeast(1.0))).toFloat().coerceAtLeast(0.25f)
    }
}
