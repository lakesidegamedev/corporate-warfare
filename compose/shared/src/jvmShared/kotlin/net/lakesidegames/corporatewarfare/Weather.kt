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
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/**
 * Weather on the map. Clouds come from textures baked by tools/cloud_gen.py (after
 * SatelliteCloudGenerator) and move the way the real atmosphere does: bands drift west on the
 * trade winds and east on the westerlies, mid-latitude lows wind up along the storm tracks, ride
 * east and fade, and hurricanes form over warm seas, track west and curve away poleward. Each
 * layer is one textured mesh drawn through the map's projection, so the clouds curve with the
 * globe, tilt with the map, and cost the graphics chip a few draws.
 */

/** Draws a textured, coloured triangle mesh straight from arrays; each platform sets it at launch. */
object MeshCanvas {
    var draw: ((canvas: Canvas, positions: FloatArray, tex: FloatArray, colors: IntArray, indices: ShortArray, vertices: Int, indexCount: Int, paint: Paint) -> Unit)? = null
}

/** Reusable arrays for one mesh, so a frame allocates nothing per vertex. */
internal class Mesh {
    var positions = FloatArray(0); var tex = FloatArray(0); var colors = IntArray(0); var indices = ShortArray(0)
    var count = 0; var indexCount = 0
    var visible = BooleanArray(0)

    fun reset(vertices: Int) {
        if (positions.size < vertices * 2) {
            positions = FloatArray(vertices * 2); tex = FloatArray(vertices * 2); colors = IntArray(vertices); visible = BooleanArray(vertices)
            indices = ShortArray(vertices * 6)
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
    return -0.12 * exp(-((a - 14) / 12).let { it * it }) + 0.25 * exp(-((a - 47) / 12).let { it * it }) - 0.05 * exp(-((a - 75) / 8).let { it * it })
}

/** A storm at one moment: where it is, how big, how far it has turned, how strong. */
internal class Storm(val lon: Double, val lat: Double, val radius: Double, val spin: Double, val strength: Float, val hurricane: Boolean)

private const val LOW_SLOTS = 10
private const val LOW_LIFE = 150.0
private const val HURRICANE_SLOTS = 3
private const val HURRICANE_LIFE = 170.0

/** Where hurricanes form: west, east, south, north in degrees. */
private val Basins = listOf(
    doubleArrayOf(-60.0, -30.0, 12.0, 18.0), doubleArrayOf(135.0, 160.0, 10.0, 18.0),
    doubleArrayOf(-118.0, -100.0, 12.0, 16.0), doubleArrayOf(60.0, 90.0, -18.0, -12.0),
)

private fun unit(hash: Int, salt: Int): Double = (stableHash("$hash/$salt") % 10_000) / 10_000.0

/** Every storm alive at [seconds]. The same time always gives the same weather. */
internal fun storms(seconds: Double): List<Storm> {
    val out = ArrayList<Storm>(LOW_SLOTS + HURRICANE_SLOTS)
    for (slot in 0 until LOW_SLOTS) {
        val local = seconds + slot * LOW_LIFE / LOW_SLOTS
        val cycle = floor(local / LOW_LIFE).toInt()
        val age = local - cycle * LOW_LIFE
        val h = stableHash("low$slot/$cycle")
        val north = slot % 2 == 0
        val lat0 = 38 + 12 * unit(h, 1)
        val lon = -180 + 360 * unit(h, 2) + 0.24 * age
        val lat = (lat0 + 0.03 * age) * (if (north) 1 else -1)
        val f = age / LOW_LIFE
        val strength = (min(f / 0.15, (1 - f) / 0.25)).coerceIn(0.0, 1.0).toFloat()
        // Lows turn counterclockwise in the north and clockwise in the south.
        val spin = 0.12 * age * (if (north) 1 else -1)
        out += Storm(((lon + 540) % 360) - 180, lat, 16 + 6 * unit(h, 3), spin, strength, false)
    }
    for (slot in 0 until HURRICANE_SLOTS) {
        val local = seconds + slot * HURRICANE_LIFE / HURRICANE_SLOTS
        val cycle = floor(local / HURRICANE_LIFE).toInt()
        val age = local - cycle * HURRICANE_LIFE
        val h = stableHash("hurricane$slot/$cycle")
        val basin = Basins[h % Basins.size]
        val north = basin[2] > 0
        var lon = basin[0] + (basin[1] - basin[0]) * unit(h, 1)
        var lat = basin[2] + (basin[3] - basin[2]) * unit(h, 2)
        val f = age / HURRICANE_LIFE
        // West on the trades, then recurving poleward and east into the westerlies.
        val turn = 0.55
        val westward = min(f, turn) * HURRICANE_LIFE
        val recurve = max(0.0, f - turn) * HURRICANE_LIFE
        lon += -0.15 * westward + 0.10 * recurve
        lat += (0.02 * westward + 0.12 * recurve) * (if (north) 1 else -1)
        val strength = (min(f / 0.12, (1 - f) / 0.2)).coerceIn(0.0, 1.0).toFloat()
        val spin = 0.6 * age * (if (north) 1 else -1)
        out += Storm(((lon + 540) % 360) - 180, lat, 6.0 + 2.0 * unit(h, 3), spin, strength, true)
    }
    return out
}

/** Cloud cover from 0 to 1 at a place, read from the same textures and storms that are drawn. */
internal class CloudCover(world: ImageBitmap, low: ImageBitmap, hurricane: ImageBitmap) {
    private val w = 256
    private val h = 128
    private val worldAlpha = sample(world, w, h)
    private val lowAlpha = sample(low, 64, 64)
    private val hurricaneAlpha = sample(hurricane, 64, 64)

    private fun sample(image: ImageBitmap, sw: Int, sh: Int): FloatArray {
        val pixels = IntArray(image.width * image.height)
        image.readPixels(pixels)
        return FloatArray(sw * sh) { i ->
            val x = (i % sw) * image.width / sw
            val y = (i / sw) * image.height / sh
            (pixels[y * image.width + x] ushr 24) / 255f
        }
    }

    /** The world texture's cover at a place, both flow layers blended as they are drawn. */
    fun at(lonDeg: Double, latDeg: Double, seconds: Double, storms: List<Storm>): Float {
        var cover = 0f
        for (layer in 0..1) {
            val (offset, weight) = flowPhase(seconds, layer)
            val u = (((lonDeg - windAt(latDeg) * offset + layer * 97.0 + 180) / 360 * w) % w + w) % w
            val v = ((90 - latDeg) / 180 * h).coerceIn(0.0, h - 1.001)
            val x0 = u.toInt() % w; val y0 = v.toInt()
            val x1 = (x0 + 1) % w; val y1 = min(y0 + 1, h - 1)
            val fx = (u - floor(u)).toFloat(); val fy = (v - y0).toFloat()
            val top = worldAlpha[y0 * w + x0] * (1 - fx) + worldAlpha[y0 * w + x1] * fx
            val bottom = worldAlpha[y1 * w + x0] * (1 - fx) + worldAlpha[y1 * w + x1] * fx
            cover += weight * (top * (1 - fy) + bottom * fy)
        }
        cover *= 0.55f
        for (s in storms) {
            val dx = (lonDeg - s.lon) * cos(s.lat * PI / 180)
            val dy = latDeg - s.lat
            if (abs(dx) > s.radius || abs(dy) > s.radius) continue
            val (tu, tv) = stormTex(dx, dy, s, 64.0)
            val a = (if (s.hurricane) hurricaneAlpha else lowAlpha)[tv.toInt().coerceIn(0, 63) * 64 + tu.toInt().coerceIn(0, 63)]
            cover = max(cover, a * s.strength)
        }
        return cover
    }
}

private const val FLOW_PERIOD = 40.0

/**
 * Flow-map phases: each of two layers slides with the wind for one period then starts over,
 * half a period apart, and fades out as it restarts, so the shear between wind bands never
 * stretches the clouds far.
 */
internal fun flowPhase(seconds: Double, layer: Int): Pair<Double, Float> {
    val phase = ((seconds / FLOW_PERIOD + layer * 0.5) % 1.0)
    return phase * FLOW_PERIOD to (1 - abs(1 - 2 * phase)).toFloat()
}

/** Texture position on a storm sprite of [size] texels for an offset in degrees from its centre. */
private fun stormTex(dx: Double, dy: Double, s: Storm, size: Double): Pair<Double, Double> {
    val c = cos(-s.spin)
    val sn = sin(-s.spin)
    val x = (dx * c - dy * sn) / s.radius
    var y = (dx * sn + dy * c) / s.radius
    // Southern storms are the northern ones mirrored, so they wind the other way.
    if (s.lat < 0) y = -y
    return (0.5 + 0.5 * x) * size to (0.5 - 0.5 * y) * size
}

internal object Weather {
    private val paints = HashMap<ImageBitmap, Paint>()
    private val world = Mesh()
    private val stormMesh = Mesh()
    private val local = Mesh()
    private var cover: CloudCover? = null

    private fun paint(image: ImageBitmap, repeat: Boolean): Paint = paints.getOrPut(image) {
        Paint().apply {
            shader = ImageShader(image, if (repeat) TileMode.Repeated else TileMode.Clamp, if (repeat) TileMode.Repeated else TileMode.Clamp)
            filterQuality = FilterQuality.Low
        }
    }

    private fun light(sun: Sun?, lon: Double, lat: Double): Float =
        if (sun == null) 1f else (0.22 + 0.78 * smoothstep(-0.15, 0.1, sun.facing(lon, lat))).toFloat()

    private fun argb(r: Float, a: Float) = Color(r, r, (r * 1.02f).coerceAtMost(1f), a.coerceIn(0f, 1f)).toArgb()

    /**
     * Fills [mesh] with a grid of [nx] by [ny] points from [place] (longitude and latitude in
     * degrees, texture u and v, alpha). Cells with a hidden or torn corner are left out.
     */
    private inline fun grid(
        mesh: Mesh, p: MapProjection, nx: Int, ny: Int, up: Double, shift: Offset, sun: Sun?, dark: Boolean,
        place: (i: Int, j: Int, out: DoubleArray) -> Unit,
    ) {
        mesh.reset(nx * ny)
        val v = DoubleArray(5)
        val jump = (p.r * 0.5).toFloat()
        for (j in 0 until ny) for (i in 0 until nx) {
            val k = j * nx + i
            place(i, j, v)
            val lon = v[0] * PI / 180
            val lat = v[1] * PI / 180
            mesh.tex[2 * k] = v[2].toFloat(); mesh.tex[2 * k + 1] = v[3].toFloat()
            val g = p.ground(lon, lat)
            if (!g.isSpecified || v[4] < 0) { mesh.visible[k] = false; mesh.colors[k] = 0; continue }
            var a = v[4].toFloat()
            if (p.t < 0.5) a *= smoothstep(0.0, 0.3, p.toView(lon, lat)[2]).toFloat()
            val at = p.lift(g.x.toDouble(), g.y.toDouble(), up) + shift
            mesh.positions[2 * k] = at.x; mesh.positions[2 * k + 1] = at.y
            mesh.visible[k] = true
            val l = light(sun, lon, lat)
            mesh.colors[k] = if (dark) Color.Black.copy(alpha = (0.32f * a * l).coerceIn(0f, 1f)).toArgb() else argb(l, a)
        }
        for (j in 0 until ny - 1) for (i in 0 until nx - 1) {
            val k = j * nx + i
            val c1 = k + 1; val c2 = k + nx; val c3 = k + nx + 1
            if (!mesh.visible[k] || !mesh.visible[c1] || !mesh.visible[c2] || !mesh.visible[c3]) continue
            if ((mesh.colors[k] or mesh.colors[c1] or mesh.colors[c2] or mesh.colors[c3]) ushr 24 == 0) continue
            val x0 = mesh.positions[2 * k]; val y0 = mesh.positions[2 * k + 1]
            if (abs(mesh.positions[2 * c3] - x0) + abs(mesh.positions[2 * c3 + 1] - y0) > jump) continue
            if (abs(mesh.positions[2 * c1] - x0) + abs(mesh.positions[2 * c2 + 1] - y0) > jump) continue
            val n = mesh.indexCount
            mesh.indices[n] = k.toShort(); mesh.indices[n + 1] = c1.toShort(); mesh.indices[n + 2] = c2.toShort()
            mesh.indices[n + 3] = c1.toShort(); mesh.indices[n + 4] = c3.toShort(); mesh.indices[n + 5] = c2.toShort()
            mesh.indexCount = n + 6
        }
    }

    /**
     * The weather for one frame. [amount] is the clouds setting (0 off, 1 light, 2 heavy).
     * [bounds] frames the scenario for the close-up clouds; [altitude] lifts them off a tilted map
     * so their shadows fall apart from them.
     */
    fun DrawScope.drawWeather(p: MapProjection, sun: Sun?, seconds: Double, amount: Int, bounds: DoubleArray, altitude: Double, zoom: Double, shadows: Boolean) {
        if (amount <= 0) return
        val worldImage = Images.load("clouds-world") ?: return
        val lowImage = Images.load("clouds-low") ?: return
        val hurricaneImage = Images.load("clouds-hurricane") ?: return
        val localImage = Images.load("clouds-local") ?: return
        val strength = if (amount >= 2) 1f else 0.7f
        val t = p.t
        val globe = (1 - smoothstep(0.3, 0.7, t)).toFloat()
        val close = smoothstep(0.3, 0.7, t).toFloat()
        val alive = storms(seconds)
        drawIntoCanvas { canvas ->
            if (globe > 0.01f) {
                // The background weather: two flow layers sheared by the prevailing winds.
                val tw = worldImage.width / 360.0
                val th = worldImage.height / 180.0
                for (layer in 0..1) {
                    val (offset, weight) = flowPhase(seconds, layer)
                    grid(world, p, 61, 29, 0.0, Offset.Zero, sun, false) { i, j, out ->
                        val lon = -180.0 + i * 6
                        val lat = 84.0 - j * 6
                        out[0] = lon; out[1] = lat
                        out[2] = (lon + 180 - windAt(lat) * offset + layer * 97.0) * tw
                        out[3] = (90 - lat) * th
                        // Thin out towards the poles so the layer has no edge there.
                        out[4] = 0.6 * strength * globe * weight * smoothstep(84.0, 66.0, abs(lat))
                    }
                    world.draw(canvas, paint(worldImage, true))
                }
                // Storms over it.
                for (s in alive) {
                    if (s.strength <= 0.01f) continue
                    val image = if (s.hurricane) hurricaneImage else lowImage
                    val size = image.width.toDouble()
                    val cosLat = max(cos(s.lat * PI / 180), 0.2)
                    grid(stormMesh, p, 11, 11, 0.0, Offset.Zero, sun, false) { i, j, out ->
                        val dx = (i / 5.0 - 1) * s.radius
                        val dy = (1 - j / 5.0) * s.radius
                        out[0] = s.lon + dx / cosLat; out[1] = s.lat + dy
                        val (u, v) = stormTex(dx, dy, s, size)
                        out[2] = u; out[3] = v
                        out[4] = (strength * globe * s.strength).toDouble()
                    }
                    stormMesh.draw(canvas, paint(image, false))
                }
            }
            if (close > 0.01f) {
                val c = cover ?: CloudCover(worldImage, lowImage, hurricaneImage).also { cover = it }
                // Close up: the fine tiling texture over the scenario, as thick as the weather
                // passing over it, drifting with the wind at the scenario's latitude.
                val pad = 6.0
                val west = bounds[0] - pad; val south = bounds[1] - pad; val east = bounds[2] + pad; val north = bounds[3] + pad
                val step = 0.6
                val nx = ((east - west) / step).toInt() + 1
                val ny = ((north - south) / step).toInt() + 1
                val midLat = (south + north) / 2
                val per = localImage.width / 8.0
                val perLat = per * cos(midLat * PI / 180)
                val drift = windAt(midLat) * seconds * 0.6
                val fade = (1.4 - zoom / 30).coerceIn(0.3, 1.0)
                val base = strength * close * fade
                for (pass in if (shadows && altitude > 0) 0..1 else 1..1) {
                    val shadowPass = pass == 0
                    grid(
                        local, p, nx, ny, if (shadowPass) 0.0 else altitude,
                        if (shadowPass) Offset((altitude * 0.45).toFloat(), (altitude * 0.30).toFloat()) else Offset.Zero, sun, shadowPass,
                    ) { i, j, out ->
                        val lon = west + i * step
                        val lat = north - j * step
                        out[0] = lon; out[1] = lat
                        out[2] = (lon - west - drift) * per; out[3] = (north - lat) * perLat
                        val edge = (min(min(i, nx - 1 - i), min(j, ny - 1 - j)) / 4.0).coerceAtMost(1.0)
                        // Clear sky where the weather is clear; thick where a front or storm passes.
                        val weather = smoothstep(0.12, 0.55, c.at(lon, lat, seconds, alive).toDouble())
                        out[4] = base * edge * weather
                    }
                    local.draw(canvas, paint(localImage, true))
                }
            }
        }
    }
}
