package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.scenarios
import kotlin.math.PI
import kotlin.math.acos
import kotlin.math.asin
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.sin
import kotlin.math.sqrt

private val landerScenarioNames: Map<String, String> by lazy { runCatching { scenarios().associate { it.id to it.name } }.getOrDefault(emptyMap()) }

internal fun scenarioTint(id: String): Color = when (id) {
    "texas-1981" -> Color(0xFFE5605E)
    "silicon-valley-1995" -> Color(0xFF42A5F5)
    "ruhr-rhine-1991" -> Color(0xFF3FBF8A)
    else -> Palette.gold
}

private val LanderStars = StarField(300, 7)

/**
 * The menu's world: a lit globe turning in space with night and city lights on its far side,
 * trade routes arcing between the great cities, satellites and the sun's glare. Each
 * scenario glows and is named as it turns to face the viewer.
 */
@Composable
fun GlobeBackdrop(modifier: Modifier = Modifier, offsetX: Float = 0.22f) {
    val map = WorldMap.shared ?: return
    val measurer = rememberTextMeasurer(cacheSize = 32)
    val graphics = LocalAppSettings.current?.graphics ?: GraphicsConfig.Default
    // The globe turns two degrees a second, smooth at 30 frames; only Unlimited asks for more.
    val clock = rememberFrameSeconds(graphics.animations, if (graphics.frameRate == 0) 0 else minOf(graphics.frameRate, 30))
    val world = remember { WorldCities() }
    val geometry = remember(map) { MapGeometryCache() }
    Canvas(modifier) {
        val seconds = if (graphics.animations) clock.value else 0.0
        val cinematic = graphics.lighting == 2
        val stars = when { graphics.preset == GraphicsPreset.Ultra -> 300; graphics.lighting == 2 -> 220; else -> 120 }
        LanderStars.draw(this, seconds, if (graphics.animations) 1f else 0f, cinematic, stars)
        val lon = ((-82 + seconds * 2.2 + 540) % 360) - 180
        val lat = 24 + 4 * sin(seconds / 23)
        val camera = MapCamera(lon, lat, 1.25)
        translate(left = size.width * offsetX, top = size.height * 0.08f) {
            val p = MapProjection(camera, size)
            val sun = Sun.side(p)
            if (cinematic) drawGlare(p, sun)
            drawWorldMap(
                map, camera, Palette.gold, emptyMap(), null, emptyList(), measurer, space = false, geometry = geometry,
                graphics = graphics.copy(detail = MapDetail.Low, extrude = false, buildings = 0, traffic = 0), sun = sun,
            )
            if (graphics.cityLights && graphics.lighting > 0) drawHubLights(world, p, sun)
            with(Weather) { drawWeather(p, sun, seconds * 3, graphics.clouds, map.playBounds, 0.0, 1.0, false) }
            drawScenarioGlow(map, p, seconds, measurer)
            if (graphics.lighting > 0) {
                drawTradeArcs(world, p, seconds)
                drawSatellites(p, seconds)
            }
        }
        if (graphics.lighting > 0) {
            drawRect(
                Brush.radialGradient(
                    0f to Color.Transparent, 0.7f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.35f),
                    center = Offset(size.width / 2, size.height / 2), radius = max(size.width, size.height) * 0.72f,
                ),
            )
        }
    }
}

/**
 * The world's great trading cities, longitude, latitude and millions of people, for the menu's
 * trade routes and night lights. The scenario maps only carry their own region's cities.
 */
private val WorldHubs = doubleArrayOf(
    -74.0, 40.7, 19.0, -87.6, 41.9, 9.5, -118.2, 34.1, 13.0, -95.4, 29.8, 7.0, -79.4, 43.7, 6.2, -99.1, 19.4, 21.0,
    -46.6, -23.5, 22.0, -43.2, -22.9, 13.0, -58.4, -34.6, 15.0, -77.0, -12.0, 10.0, -74.1, 4.7, 11.0, -122.4, 37.8, 7.0,
    -0.1, 51.5, 14.0, 2.35, 48.9, 11.0, 13.4, 52.5, 6.0, 4.9, 52.4, 2.5, 6.96, 50.9, 3.5, 12.5, 41.9, 4.3, -3.7, 40.4, 6.7,
    37.6, 55.8, 12.5, 28.97, 41.0, 15.0, 31.2, 30.0, 21.0, 3.4, 6.5, 15.0, 36.8, -1.3, 5.0, 28.0, -26.2, 9.0,
    55.3, 25.3, 3.5, 51.4, 35.7, 9.0, 72.9, 19.1, 21.0, 77.2, 28.6, 30.0, 88.4, 22.6, 15.0, 100.5, 13.8, 11.0,
    103.8, 1.35, 6.0, 106.8, -6.2, 11.0, 114.2, 22.3, 7.5, 121.5, 31.2, 27.0, 116.4, 39.9, 21.0, 127.0, 37.6, 10.0,
    139.7, 35.7, 37.0, 135.5, 34.7, 19.0, 151.2, -33.9, 5.3, 144.9, -37.8, 5.0, 174.8, -36.8, 1.7,
)

/** The trading cities as unit vectors. */
private class WorldCities {
    val count = WorldHubs.size / 3
    val lon = DoubleArray(count) { WorldHubs[3 * it] * PI / 180 }
    val lat = DoubleArray(count) { WorldHubs[3 * it + 1] * PI / 180 }
    val people = DoubleArray(count) { WorldHubs[3 * it + 2] }
    val x = DoubleArray(count) { cos(lat[it]) * cos(lon[it]) }
    val y = DoubleArray(count) { cos(lat[it]) * sin(lon[it]) }
    val z = DoubleArray(count) { sin(lat[it]) }
}

/** The trading cities glowing where night has fallen on them. */
private fun DrawScope.drawHubLights(world: WorldCities, p: MapProjection, sun: Sun) {
    for (i in 0 until world.count) {
        val light = smoothstep(0.05, -0.15, sun.facing(world.lon[i], world.lat[i])).toFloat()
        if (light <= 0f) continue
        val at = p.at(world.lon[i], world.lat[i])
        if (!at.isSpecified) continue
        val radius = 3f + 2.2f * kotlin.math.ln(1 + world.people[i]).toFloat()
        stamp(Sprites.glow, at, radius * 1.1f, Color(0xFFFFC872), 0.7f * light, BlendMode.Plus)
        drawCircle(Color(0xFFFFF1D0).copy(alpha = light), 1.6f, at)
    }
}

/** The sun's glow off the globe's upper left, and faint lens ghosts across it. */
private fun DrawScope.drawGlare(p: MapProjection, sun: Sun) {
    val sv = p.toView(sun.lon, sun.lat)
    val n = max(sqrt(sv[0] * sv[0] + sv[1] * sv[1]), 1e-6)
    val r = p.r.toFloat()
    val centre = Offset(p.cx.toFloat(), p.cy.toFloat())
    val at = centre + Offset((sv[0] / n * r * 1.25).toFloat(), (-sv[1] / n * r * 1.25).toFloat())
    // A small hard sun with a tight glow, not a wash of light across the scene.
    drawCircle(
        Brush.radialGradient(
            0f to Color(0xFFFFFBF0), 0.16f to Color(0xFFFFF0D0).copy(alpha = 0.9f), 0.3f to Color(0xFFFFD890).copy(alpha = 0.10f), 1f to Color.Transparent,
            center = at, radius = r * 0.22f,
        ),
        r * 0.22f, at,
    )
    val axis = centre - at
    listOf(0.6f to 0.05f, 1.35f to 0.09f, 1.8f to 0.03f).forEach { (along, radius) ->
        val ghost = at + axis * along
        drawCircle(Color(0xFFFFE2B0).copy(alpha = 0.035f), r * radius, ghost)
    }
}

/** Each scenario's area glows and is named as it turns to face the viewer, with a slow pulse. */
private fun DrawScope.drawScenarioGlow(map: WorldMap, projection: MapProjection, seconds: Double, measurer: androidx.compose.ui.text.TextMeasurer) {
    val pulse = (0.85 + 0.15 * sin(seconds * 2)).toFloat()
    for ((id, rings) in map.scenarioAreas) {
        val facing = rings.maxOfOrNull { projection.facing(it) } ?: continue
        val glow = ((facing - 0.35) / 0.5).coerceIn(0.0, 1.0).toFloat().let { it * it * (3 - 2 * it) } * pulse
        if (glow <= 0f) continue
        val tint = scenarioTint(id)
        val path = projection.polygons(rings)
        drawPath(path, tint.copy(alpha = 0.16f * glow), style = Stroke(6f))
        drawPath(path, tint.copy(alpha = 0.55f * glow))
        drawPath(path, Color.White.copy(alpha = 0.75f * glow), style = Stroke(1.6f))
        val anchor = rings.maxByOrNull { it.size } ?: continue
        val at = projection.project(GeoRing.point(Math.toDegrees(atan2(anchor.cy, anchor.cx)), Math.toDegrees(asin(anchor.cz.coerceIn(-1.0, 1.0)))))
        if (at != null) {
            val name = landerScenarioNames[id] ?: continue
            val label = measurer.measure(name, TextStyle(color = Color.White.copy(alpha = glow.coerceAtMost(1f)), fontSize = 13.sp, fontWeight = FontWeight.Bold))
            drawText(label, topLeft = at + Offset(-label.size.width / 2f, -label.size.height - 6f))
        }
    }
}

private const val ArcSlots = 9
private const val ArcLife = 4.5

/**
 * Trade routes: arcs lifted off the surface between pairs of great cities. Each grows from its
 * origin, lands with a ring, and its tail follows it home.
 */
private fun DrawScope.drawTradeArcs(world: WorldCities, p: MapProjection, seconds: Double) {
    val count = world.count
    val r = p.r
    val gold = Color(0xFFE8B44C)
    for (slot in 0 until ArcSlots) {
        val local = seconds + slot * 0.5
        val cycle = floor(local / ArcLife).toLong()
        val u = local - cycle * ArcLife
        // Routes start from cities facing the viewer when they set off, so they are seen.
        val camAtStart = MapCamera(((-82 + (cycle * ArcLife - slot * 0.5) * 2.2 + 540) % 360) - 180, p.lat0 * 180 / PI, 1.0)
        val start = MapProjection(camAtStart, androidx.compose.ui.geometry.Size(2f, 2f))
        val facing = (0 until count).filter { start.toView(world.lon[it], world.lat[it])[2] > 0.25 }
        if (facing.size < 2) continue
        val hash = stableHash("arc$cycle/$slot")
        val a = facing[hash % facing.size]
        val b = facing[(facing.indexOf(a) + 1 + (hash / facing.size) % (facing.size - 1)) % facing.size]
        val dot = (world.x[a] * world.x[b] + world.y[a] * world.y[b] + world.z[a] * world.z[b]).coerceIn(-1.0, 1.0)
        val angle = acos(dot)
        if (angle < 0.05) continue
        val head = (u / 2.5).coerceIn(0.0, 1.0)
        val tail = ((u - 1.0) / 3.5).coerceIn(0.0, 1.0)
        if (head - tail < 0.01) continue
        val height = 0.16 * (angle / 1.2).coerceIn(0.35, 1.0)
        val path = Path()
        var drawing = false
        var last = Offset.Unspecified
        val steps = 32
        for (i in 0..steps) {
            val s = tail + (head - tail) * i / steps
            val at = arcPoint(world, a, b, angle, s, height, p, r)
            if (at == null) { drawing = false; continue }
            if (drawing) path.lineTo(at.x, at.y) else { path.moveTo(at.x, at.y); drawing = true }
            last = at
        }
        val fade = (1 - ((u - 3.6) / 0.9).coerceIn(0.0, 1.0)).toFloat()
        drawPath(path, gold.copy(alpha = 0.22f * fade), style = Stroke(6f))
        drawPath(path, gold.copy(alpha = 0.85f * fade), style = Stroke(2f))
        if (u < 2.5 && last != Offset.Unspecified) {
            stamp(Sprites.glow, last, 9f, gold, 0.9f, BlendMode.Plus)
            drawCircle(Color(0xFFFFF1CC), 2.2f, last)
        }
        if (u in 2.5..3.5) {
            arcPoint(world, a, b, angle, 1.0, height, p, r)?.let { end ->
                val f = ((u - 2.5) / 1.0).toFloat()
                drawCircle(gold.copy(alpha = 0.6f * (1 - f)), 3f + 14f * f, end, style = Stroke(1.5f))
            }
        }
    }
}

/** A point along an arc on screen, or null when the globe hides it. */
private fun arcPoint(world: WorldCities, a: Int, b: Int, angle: Double, s: Double, height: Double, p: MapProjection, r: Double): Offset? {
    val sa = sin((1 - s) * angle) / sin(angle)
    val sb = sin(s * angle) / sin(angle)
    val x = sa * world.x[a] + sb * world.x[b]
    val y = sa * world.y[a] + sb * world.y[b]
    val z = sa * world.z[a] + sb * world.z[b]
    val v = p.toView(atan2(y, x), asin(z.coerceIn(-1.0, 1.0)))
    val lift = 1 + height * sin(PI * s)
    val vx = v[0] * lift
    val vy = v[1] * lift
    if (v[2] < 0 && sqrt(vx * vx + vy * vy) < 1) return null
    return Offset((p.cx + r * vx).toFloat(), (p.cy - r * vy).toFloat())
}

/** Three satellites on tilted orbits with fading trails, hidden as they pass behind the globe. */
private fun DrawScope.drawSatellites(p: MapProjection, seconds: Double) {
    val orbits = listOf(Triple(19.0, 0.45, 0.2), Triple(27.0, -0.8, 1.9), Triple(41.0, 1.2, 4.1))
    val r = p.r * 1.12
    orbits.forEach { (period, tilt, phase) ->
        for (k in 0 until 24) {
            val angle = 2 * PI * (seconds - k * 0.05) / period + phase
            val x0 = cos(angle)
            val z0 = sin(angle)
            // Tilt the orbit about the x axis, then lean it a little.
            val y = -z0 * sin(tilt)
            val z = z0 * cos(tilt)
            val x = x0 * cos(tilt * 0.3) - y * sin(tilt * 0.3)
            val yy = x0 * sin(tilt * 0.3) + y * cos(tilt * 0.3)
            if (z < 0 && sqrt(x * x + yy * yy) * 1.12 < 1) continue
            val at = Offset((p.cx + r * x).toFloat(), (p.cy - r * yy).toFloat())
            val alpha = if (k == 0) 0.95f else 0.22f * (1 - k / 24f)
            drawCircle(Color(0xFFDDE8F4).copy(alpha = alpha), if (k == 0) 1.8f else 1f, at)
        }
    }
}
