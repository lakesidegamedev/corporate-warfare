package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.jsonPrimitive
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * The voxel models drawn as true 3D: each model is a handful of boxes (tools/voxel_models.py)
 * projected through the map's camera every frame, so turning or tilting the map shows every side,
 * and vehicles turn as they follow the road. Faces are cel shaded by a fixed sun in the north-west
 * sky, so the lit sides change as the map turns, with dark ink along each face's edges.
 */
internal object VoxelModels {
    class Box(val x0: Float, val y0: Float, val z0: Float, val x1: Float, val y1: Float, val z1: Float, val color: Color)
    class Model(val w: Float, val d: Float, val h: Float, val boxes: List<Box>)

    @Serializable private class File(val models: Map<String, ModelFile>)
    @Serializable private class ModelFile(val w: Float, val d: Float, val h: Float, val boxes: List<JsonArray>)

    private val models: Map<String, Model> by lazy {
        runCatching {
            val text = VoxelModels::class.java.getResourceAsStream("/voxel-models.json")?.use { it.readBytes().decodeToString() } ?: return@runCatching emptyMap()
            Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), text).models.mapValues { (_, m) ->
                Model(m.w, m.d, m.h, m.boxes.map { b ->
                    fun f(i: Int) = b[i].jsonPrimitive.content.toFloat()
                    Box(f(0), f(1), f(2), f(3), f(4), f(5), Color(("FF" + b[6].jsonPrimitive.content.removePrefix("#")).toLong(16)))
                })
            }
        }.getOrDefault(emptyMap())
    }

    fun has(name: String) = name in models

    /** How tall a model stands on screen above its base at [unit] pixels a voxel, before perspective. */
    fun lift(name: String, unit: Float): Double = (models[name]?.h ?: 0f) * unit * VERTICAL

    /** The model name for an industry or city at a plant count: small, medium or large. */
    fun sized(kind: String, plants: Int) = "$kind/${Voxels.size(plants)}"

    /** Height on screen per voxel against its width on the ground, so cubes read as cubes from the tilted view. */
    private const val VERTICAL = 2.2

    // The sun, in world east, north, up: north-west and high.
    private const val LX = -0.55f
    private const val LY = 0.62f

    /** Scratch for one draw. Per thread: the structures are baked off the UI thread while vehicles draw on it. */
    private class Scratch { val path = Path(); val xs = FloatArray(8); val ys = FloatArray(8) }
    private val scratch = ThreadLocal.withInitial { Scratch() }

    /** The screen rectangle a model covered, filled by [drawModel] when asked. */
    class Bounds {
        var l = Float.MAX_VALUE; var t = Float.MAX_VALUE; var r = -Float.MAX_VALUE; var b = -Float.MAX_VALUE
        val valid: Boolean get() = l <= r && t <= b
        fun reset() { l = Float.MAX_VALUE; t = Float.MAX_VALUE; r = -Float.MAX_VALUE; b = -Float.MAX_VALUE }
    }

    /** The boxes that form a model's main roof: the highest broad tops. Chimneys and cranes are left alone. */
    private val roofs = java.util.concurrent.ConcurrentHashMap<String, Set<Box>>()
    private fun roofBoxes(name: String, model: Model): Set<Box> = roofs.getOrPut(name) {
        val broad = model.boxes.filter { (it.x1 - it.x0) * (it.y1 - it.y0) >= 6f }
        val top = broad.maxOfOrNull { it.z1 } ?: return@getOrPut emptySet()
        broad.filter { it.z1 >= top - 0.01f }.toSet()
    }

    /**
     * Draws model [name] standing at ground offset ([gx], [gy]) from the view centre, [floor]
     * pixels up, [unit] pixels to a voxel, turned [heading] radians anticlockwise from its own
     * east. [tone] recolours it (rivals greyed, closures dark). Returns the top of the model on
     * screen, or null when there is no such model.
     */
    fun DrawScope.drawModel(
        name: String, p: MapProjection, gx: Float, gy: Float, floor: Double, unit: Float, heading: Double = 0.0,
        tone: ((Color) -> Color)? = null, ink: Boolean = unit >= 1.4f, roof: Color? = null, bounds: Bounds? = null,
    ): Float? {
        val model = models[name] ?: return null
        val sc = scratch.get()
        val xs = sc.xs; val ys = sc.ys
        val roofSet = if (roof != null) roofBoxes(name, model) else emptySet()
        val e = p.east
        val n = p.north
        val ch = cos(heading).toFloat()
        val sh = sin(heading).toFloat()
        // A local east and north on the ground for this model's own heading.
        val ax = (e.x * ch + n.x * sh) * unit; val ay = (e.y * ch + n.y * sh) * unit
        val bx = (-e.x * sh + n.x * ch) * unit; val by = (-e.y * sh + n.y * ch) * unit
        fun corner(i: Int, lx: Float, ly: Float, z: Float) {
            val s = p.lift((gx + ax * lx + bx * ly).toDouble(), (gy + ay * lx + by * ly).toDouble(), floor + z * unit * VERTICAL)
            xs[i] = s.x; ys[i] = s.y
        }
        // Far boxes first: the one whose centre stands highest on screen.
        val order = model.boxes.sortedWith(compareBy({ b ->
            val cx = (b.x0 + b.x1) / 2; val cy = (b.y0 + b.y1) / 2
            -(ay * cx + by * cy)
        }, { it.z0 }))
        var top = Float.MAX_VALUE
        // The sun's direction turned into this model's own frame, for the side faces.
        val sunX = LX * ch + LY * sh
        val sunY = -LX * sh + LY * ch
        val sunLen = sqrt(sunX * sunX + sunY * sunY)
        for (b in order) {
            corner(0, b.x0, b.y0, b.z0); corner(1, b.x1, b.y0, b.z0); corner(2, b.x1, b.y1, b.z0); corner(3, b.x0, b.y1, b.z0)
            corner(4, b.x0, b.y0, b.z1); corner(5, b.x1, b.y0, b.z1); corner(6, b.x1, b.y1, b.z1); corner(7, b.x0, b.y1, b.z1)
            val base = tone?.invoke(b.color) ?: b.color
            if (bounds != null) for (i in 0 until 8) {
                if (xs[i] < bounds.l) bounds.l = xs[i]; if (xs[i] > bounds.r) bounds.r = xs[i]
                if (ys[i] < bounds.t) bounds.t = ys[i]; if (ys[i] > bounds.b) bounds.b = ys[i]
            }
            // South, east, north, west sides, then the top: corners listed anticlockwise seen from outside.
            face(sc, 0, 1, 5, 4, side(0f, -1f, sunX, sunY, sunLen), base, ink)
            face(sc, 1, 2, 6, 5, side(1f, 0f, sunX, sunY, sunLen), base, ink)
            face(sc, 2, 3, 7, 6, side(0f, 1f, sunX, sunY, sunLen), base, ink)
            face(sc, 3, 0, 4, 7, side(-1f, 0f, sunX, sunY, sunLen), base, ink)
            if (face(sc, 4, 5, 6, 7, 1f, if (b in roofSet) roof!! else base, ink)) top = min(top, min(min(ys[4], ys[5]), min(ys[6], ys[7])))
        }
        return if (top == Float.MAX_VALUE) null else top
    }

    /** Cel light for a side facing (nx, ny) in the model's frame: three flat steps. */
    private fun side(nx: Float, ny: Float, sx: Float, sy: Float, len: Float): Float {
        val d = (nx * sx + ny * sy) / max(len, 1e-3f)
        return when { d > 0.45f -> 0.84f; d > -0.2f -> 0.68f; else -> 0.52f }
    }

    /** Fills one face if it faces the camera; true when drawn. */
    private fun DrawScope.face(sc: Scratch, a: Int, b: Int, c: Int, d: Int, light: Float, base: Color, ink: Boolean): Boolean {
        val xs = sc.xs; val ys = sc.ys; val path = sc.path
        // Anticlockwise from outside in the world stays anticlockwise on screen: a negative area faces us.
        val area = (xs[a] * ys[b] - xs[b] * ys[a]) + (xs[b] * ys[c] - xs[c] * ys[b]) + (xs[c] * ys[d] - xs[d] * ys[c]) + (xs[d] * ys[a] - xs[a] * ys[d])
        if (area >= -0.5f) return false
        path.reset()
        path.moveTo(xs[a], ys[a]); path.lineTo(xs[b], ys[b]); path.lineTo(xs[c], ys[c]); path.lineTo(xs[d], ys[d]); path.close()
        val color = base.shade(light)
        drawPath(path, color)
        if (ink) drawPath(path, color.shade(0.45f), style = Stroke(1f))
        return true
    }

    /** Rivals: colour drained and darker. */
    val muted: (Color) -> Color = { c -> val g = (c.red + c.green + c.blue) / 3; Color(lerpF(g, c.red, 0.25f) * 0.8f, lerpF(g, c.green, 0.25f) * 0.82f, lerpF(g, c.blue, 0.25f) * 0.88f, c.alpha) }

    /** Mothballed plants and cities in a recession. */
    val dark: (Color) -> Color = { c -> val g = (c.red + c.green + c.blue) / 3; Color(lerpF(g, c.red, 0.15f) * 0.5f, lerpF(g, c.green, 0.15f) * 0.52f, lerpF(g, c.blue, 0.15f) * 0.58f, c.alpha) }

    private fun lerpF(a: Float, b: Float, f: Float) = a + (b - a) * f
}
