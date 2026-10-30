package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.DrawScope
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
data class GpuCamera(val lon: Double, val lat: Double, val widthKm: Double, val tiltDeg: Double, val headingDeg: Double)

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

    /** Sends game state as JSON (plants, holdings, map mode, night, clouds). */
    fun update(json: String)

    /** Asks the render thread for a frame of the view; only the newest request is drawn. */
    fun request(camera: GpuCamera, width: Int, height: Int, moving: Boolean)

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
        "map: GPU ${session.adapter}$soft, ${"%.1f".format(session.renderMs)} ms render, ${"%.1f".format(session.copyMs)} ms copy"
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
    shaded: Map<String, Double>, night: Boolean, clouds: Int,
): String {
    fun q(s: String) = "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
    val plantJson = plants.joinToString(",", "[", "]") {
        """{"region":${q(it.region)},"industry":${q(it.industry)},"firm":${it.firm},"blocks":${it.blocks},"player":${it.player},"status":${q(it.status.lowercase())}}"""
    }
    val holdings = divisions.map { it.region }.distinct().joinToString(",", "[", "]") { q(it) }
    val heat = if (layer == MapLayer.Presence || layer == MapLayer.Freight) "{}"
    else shaded.entries.joinToString(",", "{", "}") { "${q(it.key)}:${(it.value * 3.999).toInt().coerceIn(0, 3)}" }
    return """{"plants":$plantJson,"holdings":$holdings,"heat":$heat,"heat_ramp":${q(layer.gpuRamp())},"night":$night,"clouds":$clouds}"""
}

/** Blits the newest frame over the whole view, scaled up with hard pixels when it was drawn smaller. */
internal fun DrawScope.drawGpuFrame(session: GpuMapSession) {
    session.version
    val image = session.frame ?: return
    drawImage(
        image, srcOffset = IntOffset.Zero, srcSize = IntSize(image.width, image.height),
        dstOffset = IntOffset.Zero, dstSize = IntSize(size.width.toInt(), size.height.toInt()), filterQuality = FilterQuality.None,
    )
}

/**
 * City names, region names and division badges over the GPU frame, placed by the same label
 * planner as the classic map. Positions come from the renderer's own camera, so a name sits on
 * the place the frame shows.
 */
internal fun DrawScope.drawGpuLabels(
    session: GpuMapSession, camera: GpuCamera, map: WorldMap, plants: List<uniffi.cw_ffi.MapPlantDto>,
    divisions: List<uniffi.cw_ffi.MapDivisionDto>, graphics: GraphicsConfig, widthKm: Double, measurer: TextMeasurer, planner: LabelPlanner,
) {
    if (graphics.labels == 0) return
    val w = size.width.toInt(); val h = size.height.toInt()
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
