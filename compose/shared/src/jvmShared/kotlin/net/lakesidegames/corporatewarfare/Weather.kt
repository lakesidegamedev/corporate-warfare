package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ImageShader
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.VertexMode
import androidx.compose.ui.graphics.Vertices
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.toArgb
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min

/**
 * Clouds from a real satellite cloud map of the whole Earth ("clouds.webp", from Solar System
 * Scope's 8K Earth clouds, CC BY 4.0, based on NASA Blue Marble imagery; see the About panel).
 * The map holds real weather: fronts, cyclones, tropical clusters, marine cloud sheets. It moves
 * the way the atmosphere does, west on the trade winds, east on the westerlies, slowly west near
 * the poles, using two flow layers so the shear between wind bands never stretches it far. It is
 * drawn as one textured mesh through the map's projection, so it curves with the globe and tilts
 * with the map, and its shadow falls on the ground below.
 */

/** Draws a textured, coloured triangle mesh straight from arrays; each platform sets it at launch. */
object MeshCanvas {
    /** A smoothly sampled shader for a texture, where the platform's default would not be. */
    var shader: ((ImageBitmap, repeatX: Boolean) -> androidx.compose.ui.graphics.Shader)? = null
    /** Runs the clouds' flow layers as one shader pass for a texture, where the platform can; null keeps the mesh per layer. */
    var cloudFlow: ((ImageBitmap) -> CloudFlow?)? = null
    var draw: ((canvas: Canvas, positions: FloatArray, tex: FloatArray, colors: IntArray, indices: ShortArray, vertices: Int, indexCount: Int, paint: Paint) -> Unit)? = null
}

/** Reusable arrays for one mesh, so a frame allocates nothing per vertex. */
internal class Mesh {
    var positions = FloatArray(0); var tex = FloatArray(0); var tex2 = FloatArray(0); var colors = IntArray(0); var indices = ShortArray(0)
    var visible = BooleanArray(0)
    var count = 0; var indexCount = 0

    fun reset(vertices: Int) {
        if (positions.size < vertices * 2) {
            positions = FloatArray(vertices * 2); tex = FloatArray(vertices * 2); tex2 = FloatArray(vertices * 2); colors = IntArray(vertices)
            visible = BooleanArray(vertices); indices = ShortArray(vertices * 6)
            require(vertices <= Short.MAX_VALUE) { "a mesh this big needs splitting" }
        }
        count = vertices; indexCount = 0
    }

    fun draw(canvas: Canvas, paint: Paint) {
        if (indexCount == 0) return
        val fast = MeshCanvas.draw
        if (fast != null) return fast(canvas, positions, tex, colors, indices, count, indexCount, paint)
        // Portable path: the Compose vertices type, which copies into lists.
        val pos = List(count) { Offset(positions[2 * it], positions[2 * it + 1]) }
        val uv = List(count) { Offset(tex[2 * it], tex[2 * it + 1]) }
        val col = List(count) { Color(colors[it]) }
        val idx = List(indexCount) { indices[it].toInt() }
        canvas.drawVertices(Vertices(VertexMode.Triangles, pos, uv, col, idx), BlendMode.Modulate, paint)
    }
}

/** The prevailing wind in degrees of longitude a second: west in the tropics and near the poles, east in between. */
internal fun windAt(latDegrees: Double): Double {
    val a = abs(latDegrees)
    fun bump(centre: Double, width: Double) = exp(-((a - centre) / width) * ((a - centre) / width))
    return -0.05 * bump(14.0, 12.0) + 0.10 * bump(47.0, 13.0) - 0.02 * bump(75.0, 8.0)
}

private const val FLOW_PERIOD = 90.0

/**
 * Flow-map phases: each of two layers slides with the wind for one period then starts over,
 * half a period apart, and is invisible as it restarts. Returns how far it has slid in seconds
 * of wind and its weight.
 */
internal fun flowPhase(seconds: Double, layer: Int): Pair<Double, Float> {
    val phase = (seconds / FLOW_PERIOD + layer * 0.5) % 1.0
    return phase * FLOW_PERIOD to (1 - abs(1 - 2 * phase)).toFloat()
}

/** A scenario's close-up cloud texture and the area it covers, in degrees. */
internal class CloudRegion(val west: Double, val south: Double, val east: Double, val north: Double)

/** Must match REGIONS in tools/cloud_tex.py. */
internal val CloudRegions = mapOf(
    "great-lakes-2000" to CloudRegion(-122.0, 18.0, -42.0, 66.0),
    "texas-1981" to CloudRegion(-137.0, 8.0, -57.0, 56.0),
    "silicon-valley-1995" to CloudRegion(-160.0, 13.0, -80.0, 61.0),
    "ruhr-rhine-1991" to CloudRegion(-32.0, 28.0, 48.0, 76.0),
)

/**
 * The most cloud in each 16 by 16 block of a texture, read once, so mesh cells over clear sky can
 * be left out and never cost a pixel.
 */
internal class CloudMask(image: ImageBitmap) {
    private val block = 16
    val w = (image.width + block - 1) / block
    val h = (image.height + block - 1) / block
    private val width = image.width
    private val height = image.height
    private val most = IntArray(w * h)

    init {
        val pixels = IntArray(image.width * image.height)
        image.readPixels(pixels)
        for (y in 0 until image.height) for (x in 0 until image.width) {
            val i = (y / block) * w + x / block
            most[i] = max(most[i], pixels[y * image.width + x] ushr 24)
        }
    }

    /** Whether any texel in the texture rectangle holds cloud. [repeatX] wraps east to west; otherwise edges mirror. */
    fun anyCloud(u0: Float, v0: Float, u1: Float, v1: Float, repeatX: Boolean): Boolean {
        fun place(u: Float, size: Int, repeat: Boolean): Float {
            if (repeat) return ((u % size) + size) % size
            val period = 2f * size
            val m = ((u % period) + period) % period
            return if (m < size) m else period - m
        }
        val a = place(u0, width, repeatX); val b = place(u1, width, repeatX)
        // A cell that crosses a wrap or a mirror edge is kept rather than worked out.
        if (b < a || b - a > (u1 - u0) + 1f) return true
        val c = place(v0, height, false); val d = place(v1, height, false)
        if (d < c || d - c > (v1 - v0) + 1f) return true
        val bx0 = (a / block).toInt().coerceIn(0, w - 1); val bx1 = (b / block).toInt().coerceIn(0, w - 1)
        val by0 = (c / block).toInt().coerceIn(0, h - 1); val by1 = (d / block).toInt().coerceIn(0, h - 1)
        for (y in by0..by1) for (x in bx0..bx1) if (most[y * w + x] > 5) return true
        return false
    }
}

/** How opaque the clouds are at each setting: thin, light, heavy. */
internal fun cloudStrength(amount: Int): Float = when (amount) { 1 -> 0.4f; 2 -> 0.72f; else -> 1f }

internal object Weather {
    // Clouds draw on a bake thread and, before the first bake lands, on the UI thread: a scratch mesh each.
    private val meshes = ThreadLocal.withInitial { Mesh() }
    private val mesh: Mesh get() = meshes.get()
    private val masks = HashMap<ImageBitmap, CloudMask>()

    // A flow's shader is replaced for each pass, so each thread keeps its own.
    private val flows = ThreadLocal.withInitial { HashMap<ImageBitmap, CloudFlow?>() }

    private fun flow(image: ImageBitmap): CloudFlow? {
        val make = MeshCanvas.cloudFlow ?: return null
        val held = flows.get()
        return if (image in held) held[image] else make(image).also { held[image] = it }
    }

    @Synchronized
    private fun mask(image: ImageBitmap) = masks.getOrPut(image) { CloudMask(image) }
    private val paints = HashMap<ImageBitmap, Paint>()

    @Synchronized
    private fun paint(image: ImageBitmap, repeatX: Boolean): Paint = paints.getOrPut(image) {
        Paint().apply {
            shader = MeshCanvas.shader?.invoke(image, repeatX)
                ?: ImageShader(image, if (repeatX) TileMode.Repeated else TileMode.Mirror, if (repeatX) TileMode.Clamp else TileMode.Mirror)
            filterQuality = FilterQuality.Low
        }
    }

    /**
     * The clouds for one frame. [amount] is the setting (0 off, 1 thin, 2 light, 3 heavy). On the flat
     * map only the area round the scenario ([bounds]) is meshed, finely enough that the tilt's
     * perspective does not bend the texture; on the globe the whole world, coarsely.
     */
    fun DrawScope.drawWeather(
        p: MapProjection, sun: Sun?, seconds: Double, amount: Int, bounds: DoubleArray, altitude: Double, zoom: Double, shadows: Boolean,
        scenario: String? = null,
    ) {
        if (amount <= 0) return
        // Close up, the scenario's own full-resolution texture; it fades in as the map flattens.
        val region = scenario?.let { CloudRegions[it] }
        val regionImage = region?.let { Images.load("clouds-$scenario.webp") }
        val closeUp = if (region != null && regionImage != null) smoothstep(0.4, 0.8, p.t).toFloat() else 0f
        if (closeUp > 0f) drawRegion(p, sun, seconds, amount, region!!, regionImage!!, altitude, zoom, shadows, closeUp)
        if (closeUp >= 0.999f) return
        val image = Images.load("clouds.webp") ?: return
        val strength = cloudStrength(amount) * (1 - closeUp)
        // Up close the clouds thin so they never hide the map.
        val near = (1 - smoothstep(9.0, 22.0, zoom)).toFloat()
        val flat = p.t > 0.5
        val west: Double; val east: Double; val south: Double; val north: Double; val step: Double
        if (flat) {
            west = floor(bounds[0] - 30); east = west + max(60.0, bounds[2] - bounds[0] + 60)
            south = max(-84.0, floor(bounds[1] - 20)); north = min(84.0, bounds[3] + 20); step = 1.0
        } else {
            west = -180.0; east = 180.0; south = -84.0; north = 84.0; step = 4.0
        }
        val tw = image.width / 360.0
        val th = image.height / 180.0
        val flow = flow(image)
        if (flow != null) {
            val (slide0, w0) = flowPhase(seconds, 0)
            val (slide1, w1) = flowPhase(seconds, 1)
            drawIntoCanvas { canvas ->
                for (shadow in if (shadows && amount >= 3 && altitude > 0) listOf(true, false) else listOf(false)) {
                    flow.set(strength * near, w0, w1, (slide1 - slide0).toFloat(), tw.toFloat(), th.toFloat(), shadow)
                    fill(p, west, east, south, north, step, if (shadow) 0.0 else altitude,
                        if (shadow) Offset((altitude * 0.5).toFloat(), (altitude * 0.32).toFloat()) else Offset.Zero, sun, shadow, mask(image), true, dual = true) { lon, lat, out ->
                        out[0] = (lon + 180 - windAt(lat) * slide0) * tw
                        out[1] = (90 - lat) * th
                        // The shader weighs the layers and fades the poles; the mesh only carries the globe's limb.
                        out[2] = 1.0
                        out[3] = (lon + 180 - windAt(lat) * slide1 + 133.0) * tw
                        out[4] = out[1]
                    }
                    mesh.draw(canvas, flow.paint)
                }
            }
            return
        }
        val canvasPaint = paint(image, true)
        drawIntoCanvas { canvas ->
            for (layer in 0..1) {
                val (slide, weight) = flowPhase(seconds, layer)
                if (weight < 0.01f) continue
                // Shadows double the cost, so only heavy clouds cast them.
                val passes = if (shadows && amount >= 3 && altitude > 0) listOf(true, false) else listOf(false)
                for (shadow in passes) {
                    fill(p, west, east, south, north, step, if (shadow) 0.0 else altitude,
                        if (shadow) Offset((altitude * 0.5).toFloat(), (altitude * 0.32).toFloat()) else Offset.Zero, sun, shadow, mask(image), true) { lon, lat, out ->
                        // Each layer starts from its own place in the map so a restart never shows.
                        out[0] = (lon + 180 - windAt(lat) * slide + layer * 133.0) * tw
                        out[1] = (90 - lat) * th
                        // Thin out towards the poles so the layer has no edge there.
                        out[2] = strength * near * weight * smoothstep(84.0, 70.0, abs(lat))
                    }
                    mesh.draw(canvas, canvasPaint)
                }
            }
        }
    }

    /**
     * The close-up layer: one texture slid east or west with the wind at the scenario's latitude.
     * Its edges mirror, so however far it drifts it never shows a seam.
     */
    private fun DrawScope.drawRegion(
        p: MapProjection, sun: Sun?, seconds: Double, amount: Int, r: CloudRegion, image: ImageBitmap, altitude: Double, zoom: Double, shadows: Boolean,
        weight: Float,
    ) {
        // Clouds clear as you zoom in, so a close look at the map is never through haze.
        val strength = cloudStrength(amount) * 0.95f * weight * (1 - smoothstep(9.0, 22.0, zoom)).toFloat()
        val tw = image.width / (r.east - r.west)
        val th = image.height / (r.north - r.south)
        val drift = windAt((r.south + r.north) / 2) * seconds
        val canvasPaint = paint(image, false)
        drawIntoCanvas { canvas ->
            for (shadow in if (shadows && amount >= 3 && altitude > 0) listOf(true, false) else listOf(false)) {
                fill(p, r.west, r.east, r.south, r.north, 1.0, if (shadow) 0.0 else altitude,
                    if (shadow) Offset((altitude * 0.5).toFloat(), (altitude * 0.32).toFloat()) else Offset.Zero, sun, shadow, mask(image), false) { lon, lat, out ->
                    out[0] = (lon - r.west - drift) * tw
                    out[1] = (r.north - lat) * th
                    // Fade out over the outer five degrees so the layer has no edge.
                    val edge = minOf(lon - r.west, r.east - lon, lat - r.south, r.north - lat)
                    out[2] = strength * smoothstep(0.0, 5.0, edge)
                }
                mesh.draw(canvas, canvasPaint)
            }
        }
    }

    private inline fun fill(
        p: MapProjection, west: Double, east: Double, south: Double, north: Double, step: Double, up: Double, shift: Offset, sun: Sun?, shadow: Boolean,
        clouds: CloudMask, repeatX: Boolean, dual: Boolean = false, place: (lon: Double, lat: Double, out: DoubleArray) -> Unit,
    ) {
        val out = DoubleArray(5)
        val nx = ((east - west) / step).toInt() + 1
        val ny = ((north - south) / step).toInt() + 1
        mesh.reset(nx * ny)
        val jump = (p.r * 0.5).toFloat()
        for (j in 0 until ny) for (i in 0 until nx) {
            val k = j * nx + i
            val lonDeg = west + i * step
            val latDeg = north - j * step
            place(lonDeg, latDeg, out)
            mesh.tex[2 * k] = out[0].toFloat(); mesh.tex[2 * k + 1] = out[1].toFloat()
            if (dual) { mesh.tex2[2 * k] = out[3].toFloat(); mesh.tex2[2 * k + 1] = out[4].toFloat() }
            val a0 = out[2].toFloat()
            val lon = lonDeg * PI / 180
            val lat = latDeg * PI / 180
            val g = p.ground(lon, lat)
            if (!g.isSpecified) { mesh.visible[k] = false; mesh.colors[k] = 0; continue }
            var a = a0
            if (p.t < 0.5) a *= smoothstep(0.0, 0.25, p.toView(lon, lat)[2]).toFloat()
            val at = p.lift(g.x.toDouble(), g.y.toDouble(), up) + shift
            mesh.positions[2 * k] = at.x; mesh.positions[2 * k + 1] = at.y
            mesh.visible[k] = true
            // Clouds on the night side are lit only faintly; their shadows only fall by day.
            val light = if (sun == null) 1f else (0.18 + 0.82 * smoothstep(-0.15, 0.1, sun.facing(lon, lat))).toFloat()
            // In one pass the shader already holds the shadow's 0.14.
            mesh.colors[k] = if (shadow) Color.Black.copy(alpha = ((if (dual) 1f else 0.14f) * a * light).coerceIn(0f, 1f)).toArgb()
            else Color(light, light, (light * 1.03f).coerceAtMost(1f), a.coerceIn(0f, 1f)).toArgb()
        }
        for (j in 0 until ny - 1) for (i in 0 until nx - 1) {
            val k = j * nx + i
            val c1 = k + 1; val c2 = k + nx; val c3 = k + nx + 1
            if (!mesh.visible[k] || !mesh.visible[c1] || !mesh.visible[c2] || !mesh.visible[c3]) continue
            if ((mesh.colors[k] or mesh.colors[c1] or mesh.colors[c2] or mesh.colors[c3]) ushr 24 == 0) continue
            // Skip cells torn across the date line or the globe's edge.
            val x0 = mesh.positions[2 * k]; val y0 = mesh.positions[2 * k + 1]
            if (abs(mesh.positions[2 * c3] - x0) + abs(mesh.positions[2 * c3 + 1] - y0) > jump) continue
            if (abs(mesh.positions[2 * c1] - x0) + abs(mesh.positions[2 * c2 + 1] - y0) > jump) continue
            // Clear sky costs nothing: leave the cell out when its patch of texture holds no cloud.
            val t = mesh.tex
            val u0 = minOf(t[2 * k], t[2 * c1], t[2 * c2], t[2 * c3]); val u1 = maxOf(t[2 * k], t[2 * c1], t[2 * c2], t[2 * c3])
            val v0 = minOf(t[2 * k + 1], t[2 * c1 + 1], t[2 * c2 + 1], t[2 * c3 + 1]); val v1 = maxOf(t[2 * k + 1], t[2 * c1 + 1], t[2 * c2 + 1], t[2 * c3 + 1])
            var any = clouds.anyCloud(u0, v0, u1, v1, repeatX)
            if (!any && dual) {
                val t2 = mesh.tex2
                val s0 = minOf(t2[2 * k], t2[2 * c1], t2[2 * c2], t2[2 * c3]); val s1 = maxOf(t2[2 * k], t2[2 * c1], t2[2 * c2], t2[2 * c3])
                val r0 = minOf(t2[2 * k + 1], t2[2 * c1 + 1], t2[2 * c2 + 1], t2[2 * c3 + 1]); val r1 = maxOf(t2[2 * k + 1], t2[2 * c1 + 1], t2[2 * c2 + 1], t2[2 * c3 + 1])
                any = clouds.anyCloud(s0, r0, s1, r1, repeatX)
            }
            if (!any) continue
            val n = mesh.indexCount
            mesh.indices[n] = k.toShort(); mesh.indices[n + 1] = c1.toShort(); mesh.indices[n + 2] = c2.toShort()
            mesh.indices[n + 3] = c1.toShort(); mesh.indices[n + 4] = c3.toShort(); mesh.indices[n + 5] = c2.toShort()
            mesh.indexCount = n + 6
        }
    }
}
