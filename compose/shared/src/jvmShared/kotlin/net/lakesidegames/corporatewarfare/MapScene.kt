package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import uniffi.cw_ffi.LaneDto
import uniffi.cw_ffi.MapDivisionDto
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.acos
import kotlin.math.asin
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * The 3D parts of the map: sunlight, night, raised regions, buildings and everything that moves.
 * The static parts are drawn by [drawWorldMap] and cached as a layer; the moving parts by
 * [drawMapOverlay] each frame, over it. Same rules and colours as the iPhone app.
 */

/** Where the sun is overhead, in radians. */
data class Sun(val lon: Double, val lat: Double) {
    /** Unit vector in world space. */
    val x = cos(lat) * cos(lon)
    val y = cos(lat) * sin(lon)
    val z = sin(lat)

    /** How much a place faces the sun: 1 noon, 0 at dusk, negative at night. */
    fun facing(lon: Double, lat: Double): Double = cos(lat) * cos(lon) * x + cos(lat) * sin(lon) * y + sin(lat) * z

    companion object {
        /** The light used when day and night is off: from the upper left, in front of the globe. */
        fun fixed(p: MapProjection): Sun {
            val n = sqrt(0.50 * 0.50 + 0.55 * 0.55 + 0.67 * 0.67)
            val g = p.fromView(-0.50 / n, 0.55 / n, 0.67 / n)
            return Sun(g[0], g[1])
        }

        /** A low sun from the left for the menu, so a broad crescent of night shows its lights. */
        fun side(p: MapProjection): Sun {
            val n = sqrt(0.80 * 0.80 + 0.30 * 0.30 + 0.30 * 0.30)
            val g = p.fromView(-0.80 / n, 0.30 / n, 0.30 / n)
            return Sun(g[0], g[1])
        }

        /** The moving sun: a whole day every four minutes, stepped by half a degree. */
        fun moving(seconds: Double): Sun {
            val degrees = -90 - 360 * seconds / 240
            val stepped = floor(degrees * 2) / 2
            return Sun(stepped * PI / 180, 18 * PI / 180)
        }
    }
}

internal fun smoothstep(edge0: Double, edge1: Double, x: Double): Double {
    val k = ((x - edge0) / (edge1 - edge0)).coerceIn(0.0, 1.0)
    return k * k * (3 - 2 * k)
}

internal fun Color.shade(k: Float): Color = Color((red * k).coerceIn(0f, 1f), (green * k).coerceIn(0f, 1f), (blue * k).coerceIn(0f, 1f), alpha)

internal fun lerp(a: Color, b: Color, f: Float): Color =
    Color(a.red + (b.red - a.red) * f, a.green + (b.green - a.green) * f, a.blue + (b.blue - a.blue) * f, a.alpha + (b.alpha - a.alpha) * f)

/** Soft round sprites drawn once and stamped many times: far cheaper than a gradient per dot. */
internal object Sprites {
    val glow: ImageBitmap by lazy { radial(64, floatArrayOf(0f, 0.25f, 1f), floatArrayOf(1f, 0.55f, 0f)) }
    val cloud: ImageBitmap by lazy { radial(96, floatArrayOf(0f, 0.6f, 0.82f, 1f), floatArrayOf(0.95f, 0.85f, 0.35f, 0f)) }

    private fun radial(px: Int, stops: FloatArray, alphas: FloatArray): ImageBitmap {
        val image = ImageBitmap(px, px)
        val canvas = androidx.compose.ui.graphics.Canvas(image)
        CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, canvas, Size(px.toFloat(), px.toFloat())) {
            drawCircle(
                Brush.radialGradient(
                    *stops.indices.map { stops[it] to Color.White.copy(alpha = alphas[it]) }.toTypedArray(),
                    center = Offset(px / 2f, px / 2f), radius = px / 2f,
                ),
            )
        }
        return image
    }

    private val filters = HashMap<Color, ColorFilter>()

    /** A cached tint so a frame allocates nothing. */
    fun tint(color: Color): ColorFilter = filters.getOrPut(color) { ColorFilter.tint(color, BlendMode.Modulate) }
}

/** Stamps [image] centred on [at], [radius] pixels, tinted and faded. */
internal fun DrawScope.stamp(
    image: ImageBitmap, at: Offset, radius: Float, color: Color, alpha: Float, blend: BlendMode = BlendMode.SrcOver, aspect: Float = 1f,
) {
    if (radius < 0.4f || alpha <= 0.004f) return
    val h = (radius * 2).toInt().coerceAtLeast(1)
    val w = (radius * 2 * aspect).toInt().coerceAtLeast(1)
    drawImage(
        image, srcOffset = IntOffset.Zero, srcSize = IntSize(image.width, image.height),
        dstOffset = IntOffset((at.x - w / 2f).toInt(), (at.y - radius).toInt()), dstSize = IntSize(w, h),
        alpha = alpha.coerceIn(0f, 1f), colorFilter = Sprites.tint(color), blendMode = blend,
    )
}

/**
 * Soft light drawn at half the view's width and height and stretched over it: night, the globe's
 * shading, the atmosphere, the vignette. These change smoothly across the screen, so a quarter of
 * the pixels looks the same and costs a fraction. It covers a margin round the view too, so a
 * translated drawing (the menu's globe sits off centre) never shows its edge. Redrawn only when
 * [key] changes.
 */
internal class LightLayer {
    private var key: Any? = null
    private var image: ImageBitmap? = null

    fun draw(scope: DrawScope, key: Any, build: DrawScope.(full: Size) -> Unit) = with(scope) {
        val full = size
        val pad = (max(full.width, full.height) * 0.25f).toInt()
        val w = max(1, ((full.width + 2 * pad) / FACTOR).toInt() + 1)
        val h = max(1, ((full.height + 2 * pad) / FACTOR).toInt() + 1)
        var held = image
        if (held == null || held.width != w || held.height != h) {
            held = ImageBitmap(w, h)
            image = held
            this@LightLayer.key = null
        }
        if (key != this@LightLayer.key) {
            val canvas = androidx.compose.ui.graphics.Canvas(held)
            // Start clear: the bitmap is reused between frames.
            canvas.drawRect(0f, 0f, w.toFloat(), h.toFloat(), androidx.compose.ui.graphics.Paint().apply { blendMode = BlendMode.Clear })
            CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, canvas, Size(w.toFloat(), h.toFloat())) {
                scale(1f / FACTOR, 1f / FACTOR, pivot = Offset.Zero) { translate(pad.toFloat(), pad.toFloat()) { build(full) } }
            }
            this@LightLayer.key = key
        }
        drawImage(
            held, srcOffset = IntOffset.Zero, srcSize = IntSize(w, h), dstOffset = IntOffset(-pad, -pad),
            dstSize = IntSize(w * FACTOR, h * FACTOR), filterQuality = androidx.compose.ui.graphics.FilterQuality.Low,
        )
    }

    private companion object {
        const val FACTOR = 2
    }
}

// MARK: Static layer pieces

/** Land colours by climate, so the globe reads as a planet rather than a silhouette. */
private val Biomes = listOf(
    Color(0xFF3B4A3A), // temperate
    Color(0xFF5A5040), // dry
    Color(0xFF2E4733), // tropical
    Color(0xFF8E9AA4), // polar
)

/** Which climate band a country's centre falls in. */
private fun biome(ring: GeoRing): Int {
    val lat = asin(ring.cz.coerceIn(-1.0, 1.0)) * 180 / PI
    val lon = atan2(ring.cy, ring.cx) * 180 / PI
    val a = abs(lat)
    return when {
        a > 62 -> 3
        a < 14 -> 2
        // The great deserts: Sahara and Arabia, central Asia, Australia, the American southwest.
        lat in 14.0..35.0 && lon in -18.0..60.0 -> 1
        lat in 35.0..50.0 && lon in 50.0..110.0 -> 1
        lat in -35.0..-18.0 && lon in 115.0..145.0 -> 1
        lat in 22.0..36.0 && lon in -118.0..-100.0 -> 1
        else -> 0
    }
}

/** Countries grouped by climate, as four paths. */
internal fun biomePaths(map: WorldMap, p: MapProjection): List<Path> {
    val paths = List(Biomes.size) { Path().apply { fillType = PathFillType.EvenOdd } }
    map.countries.forEach { country ->
        val main = country.rings.maxByOrNull { it.size } ?: return@forEach
        p.addPolygons(country.rings, paths[biome(main)])
    }
    return paths
}

internal fun DrawScope.drawBiomes(paths: List<Path>, grey: Float) {
    paths.forEachIndexed { i, path -> drawPath(path, lerp(Biomes[i], Color(0xFF25282B), grey)) }
}

/** A deterministic field of stars in three depths. */
internal class StarField(count: Int, seed: Long) {
    val x = FloatArray(count)
    val y = FloatArray(count)
    val size = FloatArray(count)
    val alpha = FloatArray(count)
    val depth = IntArray(count)
    val phase = FloatArray(count)

    init {
        val rng = java.util.Random(seed)
        for (i in 0 until count) {
            x[i] = rng.nextFloat(); y[i] = rng.nextFloat()
            depth[i] = rng.nextInt(3)
            size[i] = 0.5f + rng.nextFloat() * (0.6f + depth[i] * 0.5f)
            alpha[i] = 0.10f + rng.nextFloat() * (0.25f + depth[i] * 0.12f)
            phase[i] = rng.nextFloat() * 6.28f
        }
    }

    /** Stars drift left by depth at [drift] pixels a second; every fifth twinkles. */
    fun draw(scope: DrawScope, seconds: Double, drift: Float, twinkle: Boolean, count: Int = x.size) = with(scope) {
        val w = size.width
        val h = size.height
        for (i in 0 until min(count, x.size)) {
            val speed = drift * (1f + depth[i] * 1.5f)
            var sx = (x[i] * w - (seconds * speed).toFloat()) % w
            if (sx < 0) sx += w
            var a = alpha[i]
            if (twinkle && i % 5 == 0) a *= 0.55f + 0.45f * sin(seconds.toFloat() * (1.3f + depth[i]) + phase[i])
            drawCircle(Color.White.copy(alpha = a), this@StarField.size[i], Offset(sx, y[i] * h))
        }
    }
}

internal val SharedStars = StarField(220, 7)

/** The globe's light: atmosphere behind, then the ocean lit from the sun. */
/** The glow of the atmosphere behind the globe. Soft, so it is drawn into a small [LightLayer]. */
internal fun DrawScope.drawAtmosphere(p: MapProjection, sun: Sun, g: GraphicsConfig) {
    val t = p.t
    val center = Offset(p.cx.toFloat(), p.cy.toFloat())
    val r = p.r.toFloat()
    val sv = p.toView(sun.lon, sun.lat)
    val fade = (1 - t).toFloat()
    if (g.lighting > 0 && fade > 0.01f) {
        val outer = r * 1.06f
        val edge = r / outer
        drawCircle(
            Brush.radialGradient(
                0f to Color.Transparent, edge * 0.985f to Color(0xFF7FB0F0).copy(alpha = 0.55f * fade),
                edge + (1 - edge) * 0.25f to Color(0xFF4F80C0).copy(alpha = 0.14f * fade), 1f to Color.Transparent,
                center = center, radius = outer,
            ),
            outer, center,
        )
        if (g.lighting == 2) {
            val warm = center + Offset((sv[0] * 0.06 * r).toFloat(), (-sv[1] * 0.06 * r).toFloat())
            drawCircle(
                Brush.radialGradient(0f to Color.Transparent, 0.93f to Color(0xFFFFD8A0).copy(alpha = 0.10f * fade), 1f to Color.Transparent, center = warm, radius = outer),
                outer, warm,
            )
        }
    }
}

/** The ocean, lit from the sun: a crisp disc that grows to fill the view as the map flattens. */
internal fun DrawScope.drawGlobeBack(p: MapProjection, sun: Sun, g: GraphicsConfig) {
    val t = p.t
    val center = Offset(p.cx.toFloat(), p.cy.toFloat())
    val r = p.r.toFloat()
    val sv = p.toView(sun.lon, sun.lat)
    val oceanRadius = (p.r / max(1 - t, 0.03)).toFloat()
    if (t >= 0.999) {
        drawRect(Color(0xFF0E2840))
    } else if (g.lighting > 0) {
        val hl = center + Offset((0.45 * r * sv[0]).toFloat(), (-0.45 * r * sv[1]).toFloat())
        drawCircle(
            Brush.radialGradient(
                0f to lerp(Color(0xFF1D4E7A), Color(0xFF0E2840), t.toFloat()),
                0.6f to Color(0xFF0E2840),
                1f to lerp(Color(0xFF06141F), Color(0xFF0E2840), t.toFloat()),
                center = hl, radius = max(r * 1.35f, oceanRadius),
            ),
            oceanRadius, center,
        )
    } else {
        drawCircle(Color(0xFF0E2840), radius = oceanRadius, center = center)
    }
}

/** Shading over the globe once land is drawn: limb, rim, sun highlight and glint. */
internal fun DrawScope.drawGlobeFront(p: MapProjection, sun: Sun, g: GraphicsConfig) {
    val fade = (1 - p.t).toFloat()
    if (g.lighting == 0 || fade < 0.01f) return
    val center = Offset(p.cx.toFloat(), p.cy.toFloat())
    val r = p.r.toFloat()
    drawCircle(
        Brush.radialGradient(
            0f to Color.Transparent, 0.78f to Color.Transparent, 0.95f to Color.Black.copy(alpha = 0.30f * fade), 1f to Color.Black.copy(alpha = 0.5f * fade),
            center = center, radius = r,
        ),
        r, center,
    )
    drawCircle(
        Brush.radialGradient(0f to Color.Transparent, 0.90f to Color.Transparent, 1f to Color(0xFF7FB2FF).copy(alpha = 0.22f * fade), center = center, radius = r),
        r, center,
    )
    if (g.lighting == 2) {
        val sv = p.toView(sun.lon, sun.lat)
        if (sv[2] > -0.2) {
            val hl = center + Offset((0.45 * r * sv[0]).toFloat(), (-0.45 * r * sv[1]).toFloat())
            drawCircle(Brush.radialGradient(0f to Color.White.copy(alpha = 0.05f * fade), 1f to Color.Transparent, center = hl, radius = r * 0.9f), r, center)
            val hx = sv[0]; val hy = sv[1]; val hz = sv[2] + 1
            val hn = max(sqrt(hx * hx + hy * hy + hz * hz), 1e-6)
            val glint = center + Offset((r * hx / hn).toFloat(), (-r * hy / hn).toFloat())
            stamp(Sprites.glow, glint, r * 0.22f, Color(0xFFDDEBFF), 0.14f * fade * smoothstep(-0.2, 0.3, sv[2]).toFloat())
        }
    }
}

/**
 * The night side as rings on the sphere, from the soft edge in. Each is the set of places where
 * the sun is below [offsets] of the horizon, closed round the dark pole.
 */
internal fun nightRings(sun: Sun, p: MapProjection): List<GeoRing> =
    if (p.t < 0.5) nightCaps(sun, p) else nightBands(sun, p.lon0)

/**
 * On the globe: each edge of night is a circle on the sphere round the point opposite the sun.
 * The part of it facing the viewer is closed along the globe's rim on the dark side, so the shape
 * is exactly the night the viewer can see.
 */
private fun nightCaps(sun: Sun, p: MapProjection): List<GeoRing> {
    val s = p.toView(sun.lon, sun.lat)
    // Two directions at right angles to the sun.
    var ux = s[1]; var uy = -s[0]
    var n = sqrt(ux * ux + uy * uy)
    if (n < 1e-6) { ux = 1.0; uy = 0.0; n = 1.0 }
    ux /= n; uy /= n
    val vx = -s[2] * uy
    val vy = s[2] * ux
    val vz = s[0] * uy - s[1] * ux
    val samples = 120
    return NightEdges.mapNotNull { c ->
        val k = sqrt(1 - c * c)
        val xs = DoubleArray(samples); val ys = DoubleArray(samples); val zs = DoubleArray(samples)
        for (i in 0 until samples) {
            val a = 2 * PI * i / samples
            xs[i] = c * s[0] + k * (cos(a) * ux + sin(a) * vx)
            ys[i] = c * s[1] + k * (cos(a) * uy + sin(a) * vy)
            zs[i] = c * s[2] + k * sin(a) * vz
        }
        val flat = ArrayList<Int>(400)
        fun add(x: Double, y: Double, z: Double) {
            val g = p.fromView(x, y, z)
            flat += (g[0] * 180 / PI * 10_000).toInt(); flat += (g[1] * 180 / PI * 10_000).toInt()
        }
        val front = (0 until samples).count { zs[it] >= 0 }
        when (front) {
            0 -> return@mapNotNull null
            samples -> for (i in 0 until samples) add(xs[i], ys[i], zs[i])
            else -> {
                // Start just after the circle comes round from behind the globe.
                val first = (0 until samples).first { zs[it] >= 0 && zs[(it - 1 + samples) % samples] < 0 }
                fun crossing(a: Int, b: Int): DoubleArray {
                    val f = zs[a] / (zs[a] - zs[b])
                    val x = xs[a] + (xs[b] - xs[a]) * f
                    val y = ys[a] + (ys[b] - ys[a]) * f
                    val m = max(sqrt(x * x + y * y), 1e-9)
                    return doubleArrayOf(x / m, y / m)
                }
                val start = crossing((first - 1 + samples) % samples, first)
                add(start[0], start[1], 1e-4)
                var i = first
                while (zs[i] >= 0) {
                    add(xs[i], ys[i], zs[i])
                    i = (i + 1) % samples
                }
                val end = crossing((i - 1 + samples) % samples, i)
                add(end[0], end[1], 1e-4)
                // Back along the rim through the dark side, away from the sun.
                val a0 = atan2(end[1], end[0])
                var a1 = atan2(start[1], start[0])
                var sweep = a1 - a0
                if (sweep <= 0) sweep += 2 * PI
                val mid = a0 + sweep / 2
                if (cos(mid) * s[0] + sin(mid) * s[1] > 0) sweep -= 2 * PI
                val steps = max(2, (abs(sweep) / (4 * PI / 180)).toInt())
                for (step in 1 until steps) {
                    val a = a0 + sweep * step / steps
                    add(cos(a), sin(a), 1e-4)
                }
            }
        }
        GeoRing(flat)
    }
}

/** On the flat map: night as a band from its edge to the dark pole, across every longitude. */
private fun nightBands(sun: Sun, lon0: Double): List<GeoRing> {
    val ds = if (abs(sun.lat) < 2 * PI / 180) (if (sun.lat >= 0) 2.0 else -2.0) * PI / 180 else sun.lat
    val pole = if (ds > 0) -89.9 else 89.9
    val centre = lon0 * 180 / PI
    return NightEdges.map { c ->
        val flat = ArrayList<Int>(250)
        var lon = centre - 180
        while (lon <= centre + 180.001) {
            val dl = (lon * PI / 180) - sun.lon
            val a = cos(ds) * cos(dl)
            val b = sin(ds)
            val rr = sqrt(a * a + b * b)
            // Rising from the dark pole, night ends where the sun's height first passes c.
            val lat = when {
                c >= rr -> -pole
                c <= -rr -> pole
                else -> {
                    val alpha = atan2(b, a)
                    val k = acos(c / rr)
                    ((if (ds > 0) alpha - k else alpha + k) * 180 / PI).coerceIn(-89.9, 89.9)
                }
            }
            flat += (lon * 10_000).toInt(); flat += (lat * 10_000).toInt()
            lon += 3
        }
        lon = centre + 180
        while (lon >= centre - 180.001) {
            flat += (lon * 10_000).toInt(); flat += (pole * 10_000).toInt()
            lon -= 3
        }
        GeoRing(flat)
    }
}

/**
 * The edges of night, from twilight in. Twenty thin steps each darken a little, so dusk fades
 * smoothly instead of showing bands; the deepest night ends about three quarters dark.
 */
private val NightEdges = List(20) { 0.16 - 0.46 * it / 19.0 }

internal fun DrawScope.drawNight(paths: List<Path>) {
    // Dusk first: a warm band along the edge of night, which the darker steps then cover.
    paths.take(6).forEach { drawPath(it, Color(0xFFFF9A4A).copy(alpha = 0.018f)) }
    paths.forEach { drawPath(it, Color(0xFF02060F).copy(alpha = 0.065f)) }
}

/** Cities glowing on the night side. */
internal fun DrawScope.drawCityLights(cities: List<Pair<WorldMapFile.City, GeoRing>>, p: MapProjection, sun: Sun, lift: Double, scale: Float) {
    cities.forEach { (city, point) ->
        val n = sun.facing(point.lon[0], point.lat[0])
        val light = smoothstep(0.05, -0.15, n).toFloat()
        if (light <= 0f) return@forEach
        val at = p.at(point.lon[0], point.lat[0], lift)
        if (!at.isSpecified) return@forEach
        val radius = (2f + 2.5f * log10(max(city.population / 1e5, 1.0)).toFloat()) * scale
        stamp(Sprites.glow, at, radius * 1.3f, Color(0xFFFFC872), 0.6f * light, BlendMode.Plus)
        drawCircle(Color(0xFFFFE6B0).copy(alpha = 0.9f * light), 1f * scale, at)
    }
}

/** A box from [floor] to [h] pixels up: the two sides facing the viewer, then the top. Corners are ground offsets from the view centre. */
internal fun DrawScope.drawBox(
    p: MapProjection, gx: Float, gy: Float, half: Float, floor: Double, h: Double, color: Color, windows: Float, outline: Color? = null,
) {
    fun at(dx: Float, dy: Float, up: Double) = p.lift((gx + dx).toDouble(), (gy + dy).toDouble(), up)
    val b0 = at(-half, half, floor); val b1 = at(half, half, floor)
    val t0 = at(-half, half, h); val t1 = at(half, half, h)
    val tb0 = at(-half, -half, h); val tb1 = at(half, -half, h)
    val front = Path().apply { moveTo(b0.x, b0.y); lineTo(b1.x, b1.y); lineTo(t1.x, t1.y); lineTo(t0.x, t0.y); close() }
    drawPath(front, color.shade(0.58f))
    // The side towards the view centre shows.
    val left = gx > 0
    val sb = if (left) at(-half, -half, floor) else at(half, -half, floor)
    val sideFrontB = if (left) b0 else b1
    val sideFrontT = if (left) t0 else t1
    val sideBackT = if (left) tb0 else tb1
    val side = Path().apply { moveTo(sideFrontB.x, sideFrontB.y); lineTo(sb.x, sb.y); lineTo(sideBackT.x, sideBackT.y); lineTo(sideFrontT.x, sideFrontT.y); close() }
    drawPath(side, color.shade(0.80f))
    val top = Path().apply { moveTo(t0.x, t0.y); lineTo(t1.x, t1.y); lineTo(tb1.x, tb1.y); lineTo(tb0.x, tb0.y); close() }
    drawPath(top, color.shade(1.15f))
    if (outline != null) drawPath(top, outline, style = Stroke(1f))
    if (windows > 0.01f) {
        // Rows of lit windows on the front face.
        val rows = ((b0.y - t0.y) / 5f).toInt()
        val win = Color(0xFFFFD27A).copy(alpha = windows)
        for (row in 1 until rows) {
            val f = row / rows.toFloat()
            val y0 = b0.y + (t0.y - b0.y) * f
            val xa = b0.x + (t0.x - b0.x) * f
            val xb = b1.x + (t1.x - b1.x) * f
            val w = (xb - xa)
            drawRect(win, Offset(xa + w * 0.18f, y0), Size(w * 0.22f, 1.4f))
            drawRect(win, Offset(xa + w * 0.60f, y0), Size(w * 0.22f, 1.4f))
        }
    }
}

/** A small stable hash for seeding per-city and per-division detail. */
internal fun stableHash(text: String): Int {
    var h = 0x811C9DC5.toInt()
    text.forEach { h = (h xor it.code) * 0x01000193 }
    return h and 0x7FFFFFFF
}

/** Heavy industry smokes. */
internal fun smokes(industry: String): Boolean = industry in setOf("extraction", "energy", "chemical_industries", "manufacturing", "construction")

// MARK: Moving overlay

/** What the overlay needs from the static layer for the current camera, built once per camera. */
internal class OverlayGeometry(
    val p: MapProjection,
    /** Each lane's ends as ground offsets, with its mode and weight. */
    val lanes: List<LaneTrack>,
    /** Highway polylines on screen with cumulative lengths. */
    val roads: List<Polyline>,
    val buildings: List<BuildingSpot>,
    val selectedTop: Path?,
    val scale: Float,
    val plinth: Double,
    /** Ground points of the biggest cities, where planes fly between. */
    val airports: List<Offset> = emptyList(),
)

internal class LaneTrack(val a: Offset, val b: Offset, val mode: String, val weight: Float, val length: Float, val seed: Float, val km: Float)

/** Which moving things to draw: traffic (under the buildings), effects (over them), or both. */
internal enum class OverlayPart { Traffic, Effects, All }
/** A road on screen; [cum] is the distance along it on the ground, in degrees, so traffic keeps its pace whatever the camera does. */
internal class Polyline(val xs: FloatArray, val ys: FloatArray, val cum: FloatArray) { val length get() = cum.last() }
internal class BuildingSpot(val base: Offset, val top: Offset, val color: Color, val smoke: Boolean, val seed: Float)

internal fun buildPolylines(p: MapProjection, rings: List<GeoRing>, lift: Double, viewSize: Size): List<Polyline> {
    val out = ArrayList<Polyline>()
    val jump = (p.r * 0.5).toFloat()
    for (ring in rings) {
        if (ring.size < 2) continue
        val xs = FloatArray(ring.size); val ys = FloatArray(ring.size); val cum = FloatArray(ring.size)
        var n = 0
        var onScreen = false
        var screen = 0f
        var lastLon = 0.0
        var lastLat = 0.0
        for (i in 0 until ring.size) {
            val pt = p.at(ring.lon[i], ring.lat[i], lift)
            if (!pt.isSpecified) continue
            if (n > 0 && abs(pt.x - xs[n - 1]) + abs(pt.y - ys[n - 1]) > jump) continue
            if (pt.x in -50f..viewSize.width + 50 && pt.y in -50f..viewSize.height + 50) onScreen = true
            xs[n] = pt.x; ys[n] = pt.y
            if (n == 0) cum[n] = 0f else {
                val dLon = (ring.lon[i] - lastLon) * cos(ring.lat[i])
                val dLat = ring.lat[i] - lastLat
                cum[n] = cum[n - 1] + (sqrt(dLon * dLon + dLat * dLat) * 180 / PI).toFloat()
                screen += sqrt((xs[n] - xs[n - 1]) * (xs[n] - xs[n - 1]) + (ys[n] - ys[n - 1]) * (ys[n] - ys[n - 1]))
            }
            lastLon = ring.lon[i]; lastLat = ring.lat[i]
            n++
        }
        if (n >= 2 && onScreen && screen > 20f) out += Polyline(xs.copyOf(n), ys.copyOf(n), cum.copyOf(n))
    }
    return out
}

private fun Polyline.pointAt(d: Float): Offset {
    var lo = 0
    var hi = cum.size - 1
    while (hi - lo > 1) {
        val mid = (lo + hi) / 2
        if (cum[mid] <= d) lo = mid else hi = mid
    }
    val span = (cum[hi] - cum[lo]).coerceAtLeast(1e-3f)
    val f = ((d - cum[lo]) / span).coerceIn(0f, 1f)
    return Offset(xs[lo] + (xs[hi] - xs[lo]) * f, ys[lo] + (ys[hi] - ys[lo]) * f)
}

private val ModeColors = mapOf("road" to Color(0xFFE0A040), "rail" to Color(0xFF7FD3FF), "sea" to Color(0xFF9AF0E0))
private val ModeSpeed = mapOf("road" to 40f, "rail" to 55f, "sea" to 22f)

/** Everything that moves on the map, for one frame. */
internal fun DrawScope.drawMapOverlay(
    o: OverlayGeometry, g: GraphicsConfig, seconds: Double, freightLayer: Boolean, sun: Sun?, bounds: DoubleArray, zoom: Double,
    rippleAge: Double, detailFade: Float, scenario: String? = null, part: OverlayPart = OverlayPart.All, wallSeconds: Double = seconds,
) {
    val p = o.p
    val time = seconds.toFloat()
    val traffic = part != OverlayPart.Effects
    val effects = part != OverlayPart.Traffic
    // Freight along the lanes, both ways.
    if (traffic && (g.traffic > 0 || freightLayer)) {
        val per = if (g.traffic >= 2) 9 else 4
        val boost = if (freightLayer) 2f else 1f
        var drawn = 0
        o.lanes.forEach { lane ->
            val count = 1 + (lane.weight * per).toInt()
            val color = ModeColors[lane.mode] ?: Color(0xFFE8E8E8)
            // Kilometres a second of game time, so freight keeps its pace at any zoom.
            val speed = ModeSpeed[lane.mode] ?: 40f
            for (k in 0 until count) {
                for (dir in 0..1) {
                    if (drawn >= 600) return@forEach
                    var f = (time * speed / lane.km + lane.seed + k / count.toFloat() + dir * 0.37f) % 1f
                    if (dir == 1) f = 1f - f
                    val gx = lane.a.x + (lane.b.x - lane.a.x) * f
                    val gy = lane.a.y + (lane.b.y - lane.a.y) * f
                    val at = p.lift(gx.toDouble(), gy.toDouble(), o.plinth + 2)
                    stamp(Sprites.glow, at, 4f * boost, color, 0.35f * boost.coerceAtMost(1.5f), BlendMode.Plus)
                    drawCircle(color, 1.6f * boost, at)
                    drawn++
                }
            }
        }
    }
    // Cars on the highways once close.
    if (traffic && g.traffic > 0 && g.detail == MapDetail.High && zoom > 9 && o.roads.isNotEmpty()) {
        // Spacing and speed are on the ground, in degrees, so cars hold their pace and places
        // while the camera moves; closer in they simply look further apart and faster.
        val spacing = if (g.traffic >= 2) 0.035f else 0.07f
        val pace = 0.012f
        val pxPerDegree = (p.r * PI / 180).toFloat().coerceAtLeast(1e-3f)
        var cars = 0
        val fade = (detailFade * ((zoom - 9) / 3).coerceIn(0.0, 1.0).toFloat())
        val head = Color(0xFFFFF4D8).copy(alpha = 0.95f * fade)
        val tail = Color(0xFFFF4A3A).copy(alpha = 0.85f * fade)
        for (road in o.roads) {
            val n = (road.length / spacing).toInt().coerceAtLeast(1)
            for (k in 0 until n) {
                if (cars >= 400) break
                val forward = k % 2 == 0
                val base = (k * spacing + (if (forward) 1 else -1) * time * pace) % road.length
                val d = if (base < 0) base + road.length else base
                val at = road.pointAt(d)
                val step = 3f / pxPerDegree
                val ahead = road.pointAt((d + if (forward) step else -step).coerceIn(0f, road.length))
                val dx = ahead.x - at.x; val dy = ahead.y - at.y
                val len = sqrt(dx * dx + dy * dy).coerceAtLeast(1e-3f)
                // Keep the two directions in their own lanes.
                val side = Offset(-dy / len * 1.6f, dx / len * 1.6f) * (if (forward) 1f else -1f)
                drawCircle(head, 1.2f, ahead + side)
                drawCircle(tail, 1.1f, at + side)
                cars++
            }
        }
    }
    if (!effects) return
    // Building beacons, smoke and the week's ripple.
    if (g.buildings > 0) {
        o.buildings.forEach { b ->
            val phase = ((time / 2.4f) + b.seed) % 1f
            drawCircle(b.color.copy(alpha = 0.45f * (1 - phase)), (6f + 20f * phase) * o.scale, b.base, style = Stroke(1.4f))
            // An aircraft warning light blinks on every rooftop.
            if (sin(time * 3f + b.seed * 20f) > 0.55f) {
                stamp(Sprites.glow, b.top, 5f, Color(0xFFFF4A3A), 0.7f, BlendMode.Plus)
                drawCircle(Color(0xFFFF6A5A), 1.3f, b.top)
            }
            if (b.smoke && (g.traffic >= 2 || g.preset == GraphicsPreset.Ultra)) {
                for (k in 0 until 3) {
                    val f = ((time / 3f) + k / 3f + b.seed) % 1f
                    val at = b.top + Offset(4f * sin(f * 5f + b.seed * 9f), -30f * f * o.scale)
                    stamp(Sprites.cloud, at, (2f + 5f * f) * o.scale, Color(0xFF9AA3AB), 0.55f * (1 - f))
                }
            }
            if (rippleAge in 0.0..1.2) {
                val f = (rippleAge / 1.2).toFloat()
                drawCircle(Palette.gold.copy(alpha = 0.6f * (1 - f)), 60f * f * o.scale, b.base, style = Stroke(2f))
            }
        }
    }
    // Planes between the big cities, high over the map, trailing contrails with shadows below.
    if (g.traffic > 0 && o.airports.size >= 2) {
        val planes = if (g.traffic >= 2) 10 else 5
        val n = o.airports.size
        for (k in 0 until planes) {
            val life = 14f
            val local = time + k * 3.1f
            val cycle = floor(local / life).toInt()
            val f = (local - cycle * life) / life
            val hash = stableHash("plane$k/$cycle")
            val a = o.airports[hash % n]
            val b = o.airports[(hash % n + 1 + (hash / n) % (n - 1)) % n]
            val fade = (min(f, 1 - f) * 8).coerceAtMost(1f)
            for (trail in 12 downTo 0) {
                val ft = (f - trail * 0.006f).coerceIn(0f, 1f)
                val gx = (a.x + (b.x - a.x) * ft).toDouble()
                val gy = (a.y + (b.y - a.y) * ft).toDouble()
                val up = (40 + 50 * sin(PI * ft)) * o.scale
                val at = p.lift(gx, gy, up)
                if (trail == 0) {
                    drawCircle(Color.Black.copy(alpha = 0.25f * fade), 1.8f, p.lift(gx, gy, o.plinth) + Offset(3f, 2f))
                    drawCircle(Color.White.copy(alpha = fade), 1.7f, at)
                    if ((time * 2 + k) % 1f < 0.15f) drawCircle(Color(0xFFFF5040).copy(alpha = fade), 1.2f, at + Offset(0f, 2.5f))
                } else {
                    drawCircle(Color.White.copy(alpha = 0.22f * fade * (1 - trail / 13f)), 1.1f + trail * 0.08f, at)
                }
            }
        }
    }
    // The selected region breathes.
    o.selectedTop?.let { top ->
        val pulse = 0.5f + 0.5f * sin(2 * PI.toFloat() * wallSeconds.toFloat() / 1.6f)
        drawPath(top, Palette.gold.copy(alpha = 0.18f), style = Stroke(8f))
        drawPath(top, Palette.gold.copy(alpha = 0.95f), style = Stroke(2f + 1.5f * pulse))
    }
}

/** Seeds per lane so particles never line up between lanes. */
internal fun laneTracks(map: WorldMap, p: MapProjection, lanes: List<LaneDto>, count: Int): List<LaneTrack> {
    val busiest = lanes.sortedByDescending { it.capacityLots + it.outsideLots }.take(count)
    val top = busiest.maxOfOrNull { it.capacityLots + it.outsideLots }?.coerceAtLeast(1) ?: return emptyList()
    return busiest.mapNotNull { lane ->
        val a = map.anchor(lane.a)?.let { p.ground(it.lon[0], it.lat[0]) }?.takeIf { it.isSpecified } ?: return@mapNotNull null
        val b = map.anchor(lane.b)?.let { p.ground(it.lon[0], it.lat[0]) }?.takeIf { it.isSpecified } ?: return@mapNotNull null
        val length = (b - a).getDistance().coerceAtLeast(8f)
        LaneTrack(a, b, lane.modeId, (lane.capacityLots + lane.outsideLots).toFloat() / top, length, (stableHash(lane.a + lane.b) % 1000) / 1000f, lane.km.toFloat().coerceAtLeast(20f))
    }
}

/** Plant markers grouped as the static layer draws them, with where their buildings stand. */
internal fun divisionSlots(divisions: List<MapDivisionDto>): List<Triple<String, String, Int>> {
    val order = Sectors.order
    return divisions.groupBy { it.region to it.industry }
        .map { (key, items) -> Triple(key.first, key.second, items.sumOf { it.blocks.toInt() }) }
        .sortedWith(compareBy({ it.first }, { order.indexOf(it.second).let { i -> if (i < 0) 99 else i } }))
}

/** Rolling frames-a-second for the optional counter. */
internal class FpsMeter {
    private val stamps = LongArray(30)
    private var index = 0
    private var filled = 0

    fun tick(now: Long): Float {
        stamps[index] = now
        index = (index + 1) % stamps.size
        filled = min(filled + 1, stamps.size)
        if (filled < 2) return 0f
        val oldest = stamps[(index - filled + stamps.size) % stamps.size]
        val span = (now - oldest) / 1e9
        return if (span > 0) ((filled - 1) / span).toFloat() else 0f
    }
}
