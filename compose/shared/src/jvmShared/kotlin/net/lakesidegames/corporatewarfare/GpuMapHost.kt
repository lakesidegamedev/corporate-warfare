package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.drawText
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.max

/**
 * The new GPU map, as the desktop shell offers it. The shared map code only talks to these
 * interfaces; the desktop build installs [GpuMapHost.opener] (Android never does, so the option is
 * hidden there and the classic map is always used).
 */
data class GpuCamera(val lon: Double, val lat: Double, val widthKm: Double, val tiltDeg: Double, val headingDeg: Double, val fovDeg: Double = 28.0)

/**
 * One building of the map: where it stands and whose it is. The renderer keeps the footprint; the screen size comes from
 * [GpuMapSession.buildingRects]. A plant building also says which division it stands for (region code, industry, firm).
 */
class GpuBuilding(
    val lon: Double, val lat: Double, val owner: String, val kind: String, val colour: Int,
    val region: String? = null, val industry: String? = null, val firm: Long? = null, val plants: Int = 0,
) {
    val scenery get() = owner == "scenery"
    val player get() = owner == "player"
}

/**
 * A finished frame. It was drawn for [camera] (the requested view widened by an overscan margin of [margin] view pixels on every
 * side, so a pan or a small zoom out reveals real map), at [factor] image pixels per view pixel; [view] is the camera of the
 * view that was asked for, [viewWidth] by [viewHeight] pixels.
 */
class GpuShown(
    val image: ImageBitmap, val camera: GpuCamera, val view: GpuCamera,
    val viewWidth: Int = image.width, val viewHeight: Int = image.height, val margin: Int = 0, val factor: Float = 1f,
)

interface GpuMapSession {
    /** The graphics adapter's name, and whether it is a software one. */
    val adapter: String
    val software: Boolean

    /** Milliseconds of the last frame: drawing on the card, and copying the pixels out (including the Skia image). */
    val renderMs: Double
    val copyMs: Double

    /** Changes with every finished frame, so a canvas that reads it redraws. */
    val version: Long

    /** The newest finished frame, with the size of the view it was drawn for. */
    val frame: ImageBitmap?

    /** The camera [frame] was drawn for. Labels and overlays use this one, never the live gesture camera, so every layer shows the same view. */
    val frameCamera: GpuCamera?

    /** The newest frame and its camera as one read, so the two can never come from different frames. */
    val shown: GpuShown?

    /** Sends game state as JSON (plants, holdings, map mode, night, clouds). */
    fun update(json: String)

    /** Asks the render thread for a frame of the view; only the newest request is drawn. [moving] tells it the camera is in motion (a smaller, quicker frame). */
    fun request(camera: GpuCamera, width: Int, height: Int, moving: Boolean)

    /** The placed buildings, in the order [buildingRects] answers for; changes only with [update]. */
    fun buildings(): List<GpuBuilding>

    /** The screen rectangle (left, top, right, bottom) each building fills for the camera, NaN when it is behind the camera. */
    fun buildingRects(camera: GpuCamera, width: Int, height: Int): FloatArray

    /** Screen points of (lon, lat) pairs; NaN for points behind the camera. */
    fun project(camera: GpuCamera, width: Int, height: Int, lonLat: DoubleArray): FloatArray

    /** The region code under a screen point. */
    fun pick(camera: GpuCamera, width: Int, height: Int, x: Float, y: Float): String?
    fun close()
}

object GpuMapHost {
    /** Opens the map for a scenario; null on Android. Throws with the reason when the card or the files fail. */
    @Volatile var opener: ((scenario: String) -> GpuMapSession)? = null

    /** Why the last attempt fell back to the classic map, for the frame counter. */
    @Volatile var failure: String? = null
}

/**
 * The open GPU map for the scenario while the player has the option on, else null. Building the
 * scene takes a few seconds, so the classic map keeps drawing until it is ready; a failure (no
 * adapter, a driver fault, missing files) leaves it null for good and says why in [GpuMapHost.failure].
 */
@Composable
internal fun rememberGpuMap(enabled: Boolean, scenario: String?): GpuMapSession? {
    val open = GpuMapHost.opener
    if (!enabled || open == null || scenario == null) return null
    val session by produceState<GpuMapSession?>(null, scenario) {
        value = withContext(Dispatchers.Default) {
            try {
                GpuMapHost.failure = null
                open(scenario)
            } catch (e: Throwable) {
                GpuMapHost.failure = (e.message ?: e.javaClass.simpleName).take(80)
                null
            }
        }
    }
    val held = session
    DisposableEffect(held) { onDispose { held?.close() } }
    return held
}

/** The frame counter line for the map: what is drawing it and what it costs. */
internal fun gpuMapLine(enabled: Boolean, session: GpuMapSession?): String? = when {
    !enabled || GpuMapHost.opener == null -> null
    session != null -> {
        val soft = if (session.software) " (software)" else ""
        val size = session.frame?.let { " ${it.width}x${it.height}" } ?: ""
        "map: GPU ${session.adapter}$soft,$size ${"%.1f".format(session.renderMs)} ms render + ${"%.1f".format(session.copyMs)} ms copy"
    }
    GpuMapHost.failure != null -> "map: classic, no GPU"
    else -> "map: classic, GPU map loading"
}

/** The classic map's camera as the GPU camera: the same centre, width on the ground, turn and tilt. */
internal fun gpuCamera(cam: MapCamera, width: Float, height: Float, pitch: Double): GpuCamera {
    val radius = cam.radius(androidx.compose.ui.geometry.Size(width, height))
    // The flat map shows [width] pixels across [radius] pixels per radian of ground at the equator-scaled east axis.
    val km = width * 6371.0 / radius
    return GpuCamera(cam.lon, cam.lat, km, Math.toDegrees(pitch), cam.bearing)
}

internal fun MapLayer.gpuRamp(): String = when (this) {
    MapLayer.Competition -> "brick"
    MapLayer.Room -> "forest"
    MapLayer.Demand -> "urban"
    MapLayer.Wages, MapLayer.Skill -> "water"
    MapLayer.Infrastructure, MapLayer.Land -> "soil"
    MapLayer.Population -> "signal"
    else -> "gold"
}

/** Game state as the GPU map's update JSON. */
internal fun gpuUpdateJson(
    plants: List<uniffi.cw_ffi.MapPlantDto>, divisions: List<uniffi.cw_ffi.MapDivisionDto>, layer: MapLayer,
    shaded: Map<String, Double>, night: Boolean, clouds: Int, week: Int = 28,
): String {
    fun q(s: String) = "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
    val plantJson = plants.joinToString(",", "[", "]") {
        """{"region":${q(it.region)},"industry":${q(it.industry)},"firm":${it.firm},"blocks":${it.blocks},"player":${it.player},"status":${q(it.status.lowercase())}}"""
    }
    val holdings = divisions.map { it.region }.distinct().joinToString(",", "[", "]") { q(it) }
    val heat = if (layer == MapLayer.Presence || layer == MapLayer.Freight) "{}"
    else shaded.entries.joinToString(",", "{", "}") { "${q(it.key)}:${(it.value * 3.999).toInt().coerceIn(0, 3)}" }
    return """{"plants":$plantJson,"holdings":$holdings,"heat":$heat,"heat_ramp":${q(layer.gpuRamp())},"night":$night,"clouds":$clouds,"week":$week}"""
}

/** The sea of the palette (water ramp, step 2): the ground under the picture, so nothing of the view is ever a void. */
internal val GpuSea = Color(0xFF1E4772)

/** The overscan margin on each side, as a share of the view. */
internal const val GPU_OVERSCAN = 0.2

/** The camera of the widened frame: the same viewpoint and focal length, seen through a window [mx] and [my] pixels larger on each side. */
internal fun overscanCamera(c: GpuCamera, w: Int, h: Int, mx: Int, my: Int): GpuCamera {
    if (mx == 0 && my == 0) return c
    val tanHalf = Math.tan(Math.toRadians(c.fovDeg / 2))
    val bigW = w + 2 * mx; val bigH = h + 2 * my
    return c.copy(widthKm = c.widthKm * bigW / w, fovDeg = 2 * Math.toDegrees(Math.atan(tanHalf * bigH / h)))
}

/**
 * The homography (row-major, 8 numbers with the last fixed to 1) that carries the ground plane as the frame camera drew it onto the
 * ground plane as the live camera sees it. Pan, zoom, turn and tilt of a flat ground are all one such mapping, so applying it to the
 * finished frame puts every point of the ground where the live camera projects it. Null when a corner is behind a camera.
 */
internal fun groundHomography(
    project: (GpuCamera, Int, Int, DoubleArray) -> FloatArray,
    frame: GpuCamera, frameW: Int, frameH: Int, live: GpuCamera, w: Int, h: Int,
): DoubleArray? {
    val kmLon = 111.32 * Math.cos(Math.toRadians(live.lat)).coerceAtLeast(0.05)
    val d = live.widthKm * 0.3
    val dx = d / kmLon
    val dy = d * (h.toDouble() / w) / 111.0
    val pts = doubleArrayOf(
        live.lon - dx, live.lat - dy, live.lon + dx, live.lat - dy, live.lon + dx, live.lat + dy, live.lon - dx, live.lat + dy,
    )
    val src = project(frame, frameW, frameH, pts)
    val dst = project(live, w, h, pts)
    if (src.any { it.isNaN() } || dst.any { it.isNaN() }) return null
    // Eight equations for the eight unknowns: dst = (h0 x + h1 y + h2) / (h6 x + h7 y + 1), likewise for y.
    val m = Array(8) { DoubleArray(9) }
    for (k in 0 until 4) {
        val x = src[2 * k].toDouble(); val y = src[2 * k + 1].toDouble()
        val u = dst[2 * k].toDouble(); val v = dst[2 * k + 1].toDouble()
        m[2 * k] = doubleArrayOf(x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u)
        m[2 * k + 1] = doubleArrayOf(0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v)
    }
    for (c in 0 until 8) {
        var best = c
        for (r in c + 1 until 8) if (Math.abs(m[r][c]) > Math.abs(m[best][c])) best = r
        if (Math.abs(m[best][c]) < 1e-9) return null
        val t = m[c]; m[c] = m[best]; m[best] = t
        for (r in 0 until 8) {
            if (r == c) continue
            val f = m[r][c] / m[c][c]
            if (f != 0.0) for (k in c until 9) m[r][k] -= f * m[c][k]
        }
    }
    return DoubleArray(8) { m[it][8] / m[it][it] }
}

/** Where a frame point lands in the view under [h] from [groundHomography]. */
internal fun applyHomography(h: DoubleArray, x: Double, y: Double): Pair<Double, Double> {
    val w = h[6] * x + h[7] * y + 1.0
    return Pair((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w)
}

/**
 * Draws the newest finished frame carried to the live camera, so the picture follows a drag or a zoom at once and never waits for
 * the next frame to land: the sea is drawn first, then the frame warped by the ground homography between the camera it was drawn
 * for and [live]. Returns the camera every other layer of this draw (names, badges, building boxes) must use: [live] when the
 * picture was carried to it, the frame's own view camera if it could only be placed unchanged. Null when no frame exists yet.
 */
internal fun DrawScope.drawGpuFrame(session: GpuMapSession, live: GpuCamera): GpuCamera? {
    session.version
    val w = size.width.toInt(); val h = size.height.toInt()
    drawRect(GpuSea)
    val shown = session.shown ?: return null
    val image = shown.image
    val hom = groundHomography(session::project, shown.camera, image.width, image.height, live, w, h)
    if (hom == null) {
        // Cannot be carried (a corner behind the camera): show the frame's view part unchanged, with its own camera.
        val f = shown.factor
        drawImage(
            image, srcOffset = IntOffset((shown.margin * f).toInt(), (shown.margin * f).toInt()),
            srcSize = IntSize((shown.viewWidth * f).toInt(), (shown.viewHeight * f).toInt()),
            dstOffset = IntOffset.Zero, dstSize = IntSize(w, h), filterQuality = FilterQuality.Medium,
        )
        return shown.view
    }
    // Same size, no turn, whole-pixel shift: copy the pixels exactly, no resampling blur while the view is still.
    val still = Math.abs(hom[0] - 1) < 1e-3 && Math.abs(hom[4] - 1) < 1e-3 && Math.abs(hom[1]) < 1e-3 && Math.abs(hom[3]) < 1e-3 &&
        Math.abs(hom[6]) < 1e-6 && Math.abs(hom[7]) < 1e-6
    if (still) {
        drawImage(image, dstOffset = IntOffset(Math.round(hom[2]).toInt(), Math.round(hom[5]).toInt()), dstSize = IntSize(image.width, image.height), filterQuality = FilterQuality.None)
        return live
    }
    val matrix = androidx.compose.ui.graphics.Matrix()
    // Compose stores the matrix column by column: the translation sits at [3, 0] and [3, 1], the perspective terms at [0, 3] and [1, 3].
    matrix[0, 0] = hom[0].toFloat(); matrix[1, 0] = hom[1].toFloat(); matrix[3, 0] = hom[2].toFloat()
    matrix[0, 1] = hom[3].toFloat(); matrix[1, 1] = hom[4].toFloat(); matrix[3, 1] = hom[5].toFloat()
    matrix[0, 3] = hom[6].toFloat(); matrix[1, 3] = hom[7].toFloat()
    withTransform({ transform(matrix) }) {
        drawImage(image, filterQuality = FilterQuality.Medium)
    }
    return live
}

/**
 * The plant building under a screen point for the camera of the picture shown: a building whose box (with a few pixels of slop, so
 * a small one can still be hit) holds the point, the nearest by distance from its centre. A building always wins over the region
 * under it. Scenery is not a plant and is never returned.
 */
internal fun gpuBuildingAt(session: GpuMapSession, camera: GpuCamera, width: Int, height: Int, x: Float, y: Float): GpuBuilding? {
    val buildings = session.buildings()
    if (buildings.isEmpty()) return null
    val rects = session.buildingRects(camera, width, height)
    var best: GpuBuilding? = null
    var bestD = Float.MAX_VALUE
    for (i in buildings.indices) {
        if (buildings[i].scenery) continue
        val l = rects[4 * i]; val t = rects[4 * i + 1]; val r = rects[4 * i + 2]; val b = rects[4 * i + 3]
        if (l.isNaN()) continue
        val slop = 5f
        if (x < l - slop || x > r + slop || y < t - slop || y > b + slop) continue
        val d = Math.hypot((x - (l + r) / 2f).toDouble(), (y - (t + b) / 2f).toDouble()).toFloat()
        if (d < bestD) { bestD = d; best = buildings[i] }
    }
    return best
}

/** The player's division a plant building stands for, or null for a rival's plant (or a plant of a firm the player is not looking at). */
internal fun gpuPlantDivision(b: GpuBuilding, divisions: List<uniffi.cw_ffi.DivisionDto>): uniffi.cw_ffi.DivisionDto? {
    val industry = b.industry ?: return null
    if (!b.player) return null
    return divisions.firstOrNull { it.region == b.region && it.industry == industry }
}

/** Below this many pixels across a building is a speck, so a badge marks it instead (the badge is a marker, the building stays the same size on the ground). */
internal const val BUILDING_MIN_PX = 6f

/** Size of a building badge, in pixels. */
private const val BUILDING_BADGE_PX = 11f

/**
 * City names, region names and division badges over the GPU frame, placed by the same label
 * planner as the classic map. Positions come from the renderer's own camera, so a name sits on
 * the place the frame shows. Buildings come first: each one's footprint on screen is reserved so no
 * name lands on it, and a building too small to read (under [BUILDING_MIN_PX]) is marked by a badge
 * at its position, the player's in gold and the rivals' in their sector's colour.
 */
internal fun DrawScope.drawGpuLabels(
    session: GpuMapSession, camera: GpuCamera, map: WorldMap, plants: List<uniffi.cw_ffi.MapPlantDto>,
    divisions: List<uniffi.cw_ffi.MapDivisionDto>, graphics: GraphicsConfig, widthKm: Double, measurer: TextMeasurer, planner: LabelPlanner,
) {
    if (graphics.labels == 0) return
    val w = size.width.toInt(); val h = size.height.toInt()
    val buildings = session.buildings()
    val rects = session.buildingRects(camera, w, h)
    val markers = if (buildings.isEmpty()) FloatArray(0) else session.project(camera, w, h, DoubleArray(buildings.size * 2) { if (it % 2 == 0) buildings[it / 2].lon else buildings[it / 2].lat })
    // Cities (every one in the scenario) first, then regions; one batch call for all of them.
    val cities = map.cities.filter { (c, _) -> widthKm < 900 || c.population >= 500_000 }.take(80)
    val regions = if (graphics.labels >= 2 && widthKm < 1400) map.regions.mapNotNull { r -> map.anchor(r.code)?.let { r to it } } else emptyList()
    val pts = DoubleArray((cities.size + regions.size + divisions.size) * 2)
    var i = 0
    cities.forEach { (c, _) -> pts[i++] = c.x / 10_000.0; pts[i++] = c.y / 10_000.0 }
    regions.forEach { (_, a) -> pts[i++] = Math.toDegrees(a.lon[0]); pts[i++] = Math.toDegrees(a.lat[0]) }
    val badgeRegions = divisions.groupBy { it.region }
    val badgeList = badgeRegions.entries.toList()
    val badgePts = DoubleArray(badgeList.size * 2)
    badgeList.forEachIndexed { k, (code, _) ->
        val a = map.anchor(code)
        badgePts[2 * k] = a?.let { Math.toDegrees(it.lon[0]) } ?: 0.0
        badgePts[2 * k + 1] = a?.let { Math.toDegrees(it.lat[0]) } ?: 0.0
    }
    val all = session.project(camera, w, h, pts.copyOf(i) + badgePts)
    planner.frame {
        // Buildings first: their footprints are kept clear, and the specks get a badge (player's last, on top).
        val order = buildings.indices.filter { !buildings[it].scenery }.sortedBy { buildings[it].player }
        val small = HashSet<Int>()
        for (b in buildings.indices) {
            val l = rects[4 * b]; val t = rects[4 * b + 1]; val r = rects[4 * b + 2]; val bt = rects[4 * b + 3]
            if (l.isNaN() || r < -20 || l > w + 20 || bt < -20 || t > h + 20) continue
            if (maxOf(r - l, bt - t) >= BUILDING_MIN_PX) reserve(l - 1f, t - 1f, r + 1f, bt + 1f) else if (!buildings[b].scenery) small += b
        }
        for (b in order) {
            if (b !in small) continue
            val x = markers[2 * b]; val y = markers[2 * b + 1]
            if (x.isNaN() || x < -20 || y < -20 || x > w + 20 || y > h + 20) continue
            val half = BUILDING_BADGE_PX / 2f
            val top = Offset(x - half, y - half)
            val fill = if (buildings[b].player) Palette.gold else Color(buildings[b].colour)
            drawRect(Palette.ink, topLeft = top - Offset(1f, 1f), size = androidx.compose.ui.geometry.Size(BUILDING_BADGE_PX + 2f, BUILDING_BADGE_PX + 2f))
            drawRect(fill, topLeft = top, size = androidx.compose.ui.geometry.Size(BUILDING_BADGE_PX, BUILDING_BADGE_PX))
            if (buildings[b].player) drawRect(Palette.ink, topLeft = top + Offset(3f, 3f), size = androidx.compose.ui.geometry.Size(BUILDING_BADGE_PX - 6f, BUILDING_BADGE_PX - 6f))
            reserve(x - half - 1f, y - half - 1f, x + half + 1f, y + half + 1f)
        }
        // Badges first so a name that would cross one is the name that goes.
        badgeList.forEachIndexed { k, (_, items) ->
            val x = all[i + 2 * k]; val y = all[i + 2 * k + 1]
            if (x.isNaN()) return@forEachIndexed
            val count = items.sumOf { it.blocks.toInt() }
            val label = LabelText.measure(measurer, "$count", BadgeStyle, "badge")
            val at = Offset(x, y - 26f)
            val r = max(label.size.width, label.size.height) / 2f + 3f
            drawCircle(Palette.gold, radius = r, center = at)
            synchronized(LabelText.guard) { drawText(label, topLeft = at - Offset(label.size.width / 2f, label.size.height / 2f)) }
            reserve(at.x - r, at.y - r, at.x + r, at.y + r)
        }
        var n = 0
        cities.sortedByDescending { it.first.population }.forEach { (c, _) ->
            val idx = cities.indexOfFirst { it.first === c }
            val x = all[2 * idx]; val y = all[2 * idx + 1]
            n++
            if (x.isNaN() || x < -40 || y < -20 || x > w + 40 || y > h + 20) return@forEach
            drawCircle(Palette.primaryText.copy(alpha = 0.85f), radius = 2.5f, center = Offset(x, y))
            val label = LabelText.measure(measurer, c.name, CityStyle, "city")
            val lw = label.size.width.toFloat(); val lh = label.size.height.toFloat()
            val slots = floatArrayOf(x + 6f, y - lh / 2f, x - 6f - lw, y - lh / 2f, x - lw / 2f, y - lh - 5f, x - lw / 2f, y + 5f)
            val slot = place("c:${c.name}", lw, lh, slots)
            if (slot >= 0) synchronized(LabelText.guard) { drawText(label, topLeft = Offset(slots[2 * slot], slots[2 * slot + 1])) }
        }
        regions.forEachIndexed { k, (region, _) ->
            val idx = cities.size + k
            val x = all[2 * idx]; val y = all[2 * idx + 1]
            if (x.isNaN() || x < 0 || y < 0 || x > w || y > h) return@forEachIndexed
            val label = LabelText.measure(measurer, region.name.uppercase(), RegionStyle, "region")
            val lw = label.size.width.toFloat(); val lh = label.size.height.toFloat()
            val slots = floatArrayOf(x - lw / 2f, y + 22f, x - lw / 2f, y - 22f - lh)
            val slot = place("r:${region.code}", lw, lh, slots)
            if (slot >= 0) synchronized(LabelText.guard) { drawText(label, topLeft = Offset(slots[2 * slot], slots[2 * slot + 1])) }
        }
    }
}
