package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.ImageShader
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.toArgb
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.max
import kotlin.math.min

/**
 * The real Earth under the map: NASA's Blue Marble (public domain), baked by tools/earth_tex.py
 * into a world image for the globe and a sharp crop per scenario for close up, drawn as a
 * textured mesh through the map's projection like the clouds. The seasons follow the game's
 * calendar: a snow line that creeps south in winter (drawn through a land mask, so never on the
 * sea), amber in the autumn and a browner winter in the mid-latitudes, all half a year apart in
 * the south.
 */
internal object Earth {
    private val mesh = Mesh()
    private val paints = HashMap<ImageBitmap, Paint>()

    private fun paint(image: ImageBitmap, repeatX: Boolean): Paint = paints.getOrPut(image) {
        Paint().apply {
            shader = MeshCanvas.shader?.invoke(image, repeatX)
                ?: ImageShader(image, if (repeatX) TileMode.Repeated else TileMode.Clamp, TileMode.Clamp)
            filterQuality = FilterQuality.Low
        }
    }

    /** Week of the year 1..52 from a label such as "2000 W10"; midsummer when there is none. */
    fun weekOf(label: String?): Int = label?.let { Regex("W(\\d+)").find(it)?.groupValues?.get(1)?.toIntOrNull() } ?: 28

    /** How deep in winter a latitude is in [week]: 1 at midwinter, 0 from spring to autumn. */
    private fun winter(week: Int, latDeg: Double): Double {
        val shifted = if (latDeg >= 0) week else (week + 26) % 52
        return max(0.0, cos(2 * PI * (shifted - 3) / 52))
    }

    /** How much autumn colour a latitude shows in [week], 0 to 1. */
    private fun autumn(week: Int, latDeg: Double): Double {
        val shifted = if (latDeg >= 0) week else (week + 26) % 52
        val d = (shifted - 42).toDouble()
        val band = smoothstep(28.0, 36.0, abs(latDeg)) * (1 - smoothstep(58.0, 66.0, abs(latDeg)))
        return exp(-(d / 5) * (d / 5)) * band
    }

    /** Snow cover 0 to 1: the line sits near 66 degrees in summer and reaches 40 in midwinter. */
    private fun snow(week: Int, latDeg: Double): Double {
        val line = 68 - 28 * winter(week, latDeg)
        return smoothstep(line - 3, line + 5, abs(latDeg)) * 0.6
    }

    /**
     * The Earth for the current view. [bounds] frames the scenario for its sharp close-up crop,
     * which fades in as the map flattens; [scenario] picks the crop.
     */
    fun DrawScope.drawEarth(p: MapProjection, week: Int, bounds: DoubleArray, scenario: String?) {
        val world = Images.load("earth.jpg") ?: return
        val land = Images.load("earth-land.png")
        val crop = scenario?.let { Images.load("earth-$it.jpg") }
        val region = scenario?.let { CloudRegions[it] }
        val close = if (crop != null && region != null) smoothstep(0.4, 0.8, p.t).toFloat() else 0f
        drawIntoCanvas { canvas ->
            if (close < 0.999f) {
                // The whole world, coarse, for the globe and the unrolling.
                fill(p, -180.0, 180.0, -88.0, 88.0, if (p.t > 0.5) 2.0 else 4.0, week, 1f) { lon, lat, out ->
                    out[0] = (lon + 180) / 360 * world.width; out[1] = (90 - lat) / 180 * world.height
                }
                mesh.draw(canvas, paint(world, true))
            }
            if (close > 0.001f) {
                val r = region!!
                val img = crop!!
                fill(p, r.west, r.east, r.south, r.north, 1.0, week, close) { lon, lat, out ->
                    out[0] = (lon - r.west) / (r.east - r.west) * img.width; out[1] = (r.north - lat) / (r.north - r.south) * img.height
                }
                mesh.draw(canvas, paint(img, false))
            }
            // Snow on land only: the land mask, tinted white by how far into winter each place is.
            if (land != null) {
                val west = if (close > 0.5f) region!!.west else -180.0
                val east = if (close > 0.5f) region!!.east else 180.0
                val south = if (close > 0.5f) region!!.south else -88.0
                val north = if (close > 0.5f) region!!.north else 88.0
                // Close up only the view's own patch, finely enough that coasts stay crisp.
                val b = bounds
                val w2 = if (close > 0.5f) b[0] - 6 else west
                val e2 = if (close > 0.5f) b[2] + 6 else east
                val s2 = if (close > 0.5f) b[1] - 6 else south
                val n2 = if (close > 0.5f) b[3] + 6 else north
                val sharp = if (close > 0.5f) scenario?.let { Images.load("earth-land-$it.png") } else null
                fill(p, w2, e2, s2, n2, if (close > 0.5f) 0.25 else 4.0, week, 1f, snowPass = true) { lon, lat, out ->
                    if (sharp != null) {
                        val r = region!!
                        out[0] = (lon - r.west) / (r.east - r.west) * sharp.width; out[1] = (r.north - lat) / (r.north - r.south) * sharp.height
                    } else {
                        out[0] = (lon + 180) / 360 * land.width; out[1] = (90 - lat) / 180 * land.height
                    }
                }
                mesh.draw(canvas, paint(sharp ?: land, sharp == null))
            }
        }
    }

    private inline fun fill(
        p: MapProjection, west: Double, east: Double, south: Double, north: Double, step: Double, week: Int, alpha: Float,
        snowPass: Boolean = false, place: (lon: Double, lat: Double, out: DoubleArray) -> Unit,
    ) {
        val nx = ((east - west) / step).toInt() + 1
        val ny = ((north - south) / step).toInt() + 1
        mesh.reset(nx * ny)
        val out = DoubleArray(2)
        val jump = (p.r * 0.5).toFloat()
        for (j in 0 until ny) for (i in 0 until nx) {
            val k = j * nx + i
            val lonDeg = west + i * step
            val latDeg = north - j * step
            place(lonDeg, latDeg, out)
            mesh.tex[2 * k] = out[0].toFloat(); mesh.tex[2 * k + 1] = out[1].toFloat()
            val at = p.at(lonDeg * PI / 180, latDeg * PI / 180)
            if (!at.isSpecified) { mesh.visible[k] = false; mesh.colors[k] = 0; continue }
            mesh.positions[2 * k] = at.x; mesh.positions[2 * k + 1] = at.y
            mesh.visible[k] = true
            mesh.colors[k] = if (snowPass) {
                Color(1f, 1f, 1f, snow(week, latDeg).toFloat()).toArgb()
            } else {
                // Autumn warms the leaves, winter browns the mid-latitudes; the far edge of a
                // crop fades so it melts into the world image.
                val a = autumn(week, latDeg).toFloat()
                val w = (winter(week, latDeg) * smoothstep(30.0, 45.0, abs(latDeg))).toFloat()
                val edge = if (alpha < 1f || east - west < 359) {
                    min(min(lonDeg - west, east - lonDeg), min(latDeg - south, north - latDeg)).let { smoothstep(0.0, 4.0, it).toFloat() }
                } else 1f
                Color(
                    (1f - 0.06f * w).coerceIn(0f, 1f), (1f - 0.12f * a - 0.12f * w).coerceIn(0f, 1f),
                    (1f - 0.32f * a - 0.10f * w).coerceIn(0f, 1f), (alpha * edge).coerceIn(0f, 1f),
                ).toArgb()
            }
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
}
