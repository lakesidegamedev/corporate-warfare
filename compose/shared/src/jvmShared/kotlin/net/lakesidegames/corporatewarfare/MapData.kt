package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.acos
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * The bundled world map, the same file as the iPhone app: public-domain Natural Earth countries
 * plus US Census detail for the scenario's states, in ten-thousandths of a degree. Rings and lines
 * are flat lon,lat lists. The map screen projects them at draw time.
 */
@Serializable
data class WorldMapFile(
    val countries: List<Country>,
    val provinces: List<List<Int>> = emptyList(),
    @kotlinx.serialization.SerialName("scenario_areas") val scenarioAreas: List<ScenarioArea> = emptyList(),
    val roads: List<List<Int>> = emptyList(),
    val states: List<Area>,
    val regions: List<RegionArea>,
    val counties: List<County> = emptyList(),
    val lakes: List<Rings>,
    val rivers: List<Lines> = emptyList(),
    val interstates: List<Lines>,
    val cities: List<City>,
    val hubs: Map<String, List<Double>>,
    /** The states each scenario plays in; the first is the default scenario. */
    val scenarios: List<ScenarioStates> = emptyList(),
) {
    @Serializable data class Country(val code: String, val name: String, val rings: List<List<Int>>)
    /** [scenario] tags a state added for one scenario, whose code may match another place (DE is also Delaware). */
    @Serializable data class Area(
        val code: String,
        val name: String,
        val play: Boolean,
        val rings: List<List<Int>>,
        val scenario: String? = null,
    )
    @Serializable data class RegionArea(val code: String, val state: String, val name: String, val rings: List<List<Int>>)
    @Serializable data class Rings(val rings: List<List<Int>>)
    @Serializable data class County(val state: String = "", val rings: List<List<Int>>)
    @Serializable data class ScenarioStates(val id: String, val states: List<String>)
    @Serializable data class ScenarioArea(val id: String, val rings: List<List<Int>>)
    /** [scenario] names the scenario a layer was added for; none means the default scenario's. */
    @Serializable data class Lines(val lines: List<List<Int>>, val scenario: String? = null)
    @Serializable data class City(
        val name: String,
        val region: String,
        val country: String? = null,
        val x: Int,
        val y: Int,
        val population: Int,
        val capital: Boolean,
        val scenario: String? = null,
    )
}

/** One ring or line on the sphere in radians, with a bounding cap for culling. */
class GeoRing(flat: List<Int>) {
    val size = flat.size / 2
    val lon = DoubleArray(size) { flat[2 * it] / 10_000.0 * PI / 180 }
    val lat = DoubleArray(size) { flat[2 * it + 1] / 10_000.0 * PI / 180 }
    val sinLat = DoubleArray(size) { sin(lat[it]) }
    val cosLat = DoubleArray(size) { cos(lat[it]) }
    val sinLon = DoubleArray(size) { sin(lon[it]) }
    val cosLon = DoubleArray(size) { cos(lon[it]) }
    val cx: Double
    val cy: Double
    val cz: Double
    val radius: Double

    init {
        var sx = 0.0; var sy = 0.0; var sz = 0.0
        for (i in 0 until size) {
            sx += cosLat[i] * cos(lon[i]); sy += cosLat[i] * sin(lon[i]); sz += sinLat[i]
        }
        val n = max(sqrt(sx * sx + sy * sy + sz * sz), 1e-9)
        cx = sx / n; cy = sy / n; cz = sz / n
        var r = 0.0
        for (i in 0 until size) {
            val dot = cosLat[i] * cos(lon[i]) * cx + cosLat[i] * sin(lon[i]) * cy + sinLat[i] * cz
            r = max(r, acos(dot.coerceIn(-1.0, 1.0)))
        }
        radius = r
    }

    companion object {
        fun point(lonDegrees: Double, latDegrees: Double): GeoRing =
            GeoRing(listOf((lonDegrees * 10_000).toInt(), (latDegrees * 10_000).toInt()))
    }
}

/**
 * Geometry turned into sphere points once. A map is for one scenario: its playable states,
 * regions, counties, roads, rivers and cities. A file with no scenario list shows everything.
 */
class WorldMap private constructor(val file: WorldMapFile, val scenarioId: String?) {
    class Feature(val code: String, val name: String, val rings: List<GeoRing>)

    private val scenarioStates: Set<String>? =
        scenarioId?.let { id -> file.scenarios.firstOrNull { it.id == id }?.states?.toSet() }
    private val defaultScenario: String? = file.scenarios.firstOrNull()?.id
    private fun isPlay(area: WorldMapFile.Area): Boolean {
        val states = scenarioStates ?: return area.play
        // A tagged state counts only in its own scenario; an untagged one is the default's.
        return states.contains(area.code) && if (area.scenario != null) area.scenario == scenarioId else area.play
    }
    private fun showsState(area: WorldMapFile.Area) = scenarioStates == null || area.scenario == null || area.scenario == scenarioId
    /** A layer tagged for another scenario is hidden; an untagged one belongs to the default. */
    private fun owns(tag: String?) = scenarioStates == null || (tag ?: defaultScenario) == scenarioId

    val countries = file.countries.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val playStates = file.states.filter { showsState(it) && isPlay(it) }.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val contextStates = file.states.filter { showsState(it) && !isPlay(it) }.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val regions = file.regions.filter { scenarioStates?.contains(it.state) ?: true }
        .map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val provinces = file.provinces.map(::GeoRing)
    /** Each scenario's area as outlines, for the menu globe. */
    val scenarioAreas: List<Pair<String, List<GeoRing>>> = file.scenarioAreas.map { it.id to it.rings.map(::GeoRing) }
    val roads = file.roads.map(::GeoRing)
    val counties = file.counties.filter { c -> scenarioStates?.contains(c.state) ?: true }
        .flatMap { c -> c.rings.map(::GeoRing) }
    val lakes = file.lakes.flatMap { l -> l.rings.map(::GeoRing) }
    val rivers = file.rivers.filter { owns(it.scenario) }.flatMap { r -> r.lines.map(::GeoRing) }
    val interstates = file.interstates.filter { owns(it.scenario) }.flatMap { r -> r.lines.map(::GeoRing) }
    val hubs: Map<String, GeoRing> = file.hubs.filterValues { it.size == 2 }
        .mapValues { GeoRing.point(it.value[0] / 10_000, it.value[1] / 10_000) }
    val cities: List<Pair<WorldMapFile.City, GeoRing>> = file.cities.filter { owns(it.scenario) }
        .map { it to GeoRing.point(it.x / 10_000.0, it.y / 10_000.0) }

    /** The scenario's playable states in degrees: west, south, east, north. */
    val playBounds: DoubleArray = run {
        var w = 180.0; var s = 90.0; var e = -180.0; var n = -90.0
        file.states.filter { isPlay(it) }.forEach { state ->
            state.rings.forEach { ring ->
                var i = 0
                while (i + 1 < ring.size) {
                    val lon = ring[i] / 10_000.0; val lat = ring[i + 1] / 10_000.0
                    w = min(w, lon); e = max(e, lon); s = min(s, lat); n = max(n, lat)
                    i += 2
                }
            }
        }
        doubleArrayOf(w, s, e, n)
    }

    fun regionName(code: String): String = file.regions.firstOrNull { it.code == code }?.name ?: code

    /** Where a region's markers sit: its hub city, else its first vertex. */
    fun anchor(code: String): GeoRing? = hubs[code] ?: regions.firstOrNull { it.code == code }?.rings?.firstOrNull()

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        private val loaded: WorldMapFile? by lazy {
            val stream = WorldMap::class.java.getResourceAsStream("/world-map.json") ?: return@lazy null
            json.decodeFromString(WorldMapFile.serializer(), stream.bufferedReader().readText())
        }
        private val byScenario = HashMap<String?, WorldMap>()

        /** The default scenario's map, loaded once from the app's resources; null if the file is missing. */
        val shared: WorldMap? by lazy { forScenario(loaded?.scenarios?.firstOrNull()?.id) }

        /** One scenario's map, built once. */
        fun forScenario(id: String?): WorldMap? {
            val file = loaded ?: return null
            return synchronized(byScenario) { byScenario.getOrPut(id) { WorldMap(file, id) } }
        }

        /** The map of the scenario whose regions these codes belong to (their state prefix). */
        fun forRegions(codes: Collection<String>): WorldMap? {
            val state = codes.firstOrNull()?.substringBefore('-') ?: return shared
            val id = loaded?.scenarios?.firstOrNull { state in it.states }?.id ?: return shared
            return forScenario(id)
        }
    }
}

/** Where the map looks: a centre in degrees and a zoom where 1 fits the whole globe. */
/**
 * Where the map looks: a centre in degrees, a zoom where 1 fits the whole globe, and a [bearing]
 * in degrees, the compass direction at the top of the screen (0 is north up).
 */
data class MapCamera(val lon: Double, val lat: Double, val zoom: Double, val bearing: Double = 0.0) {
    /** 0 shows the globe, 1 the flat map, smoothly between. */
    val flatness: Double
        get() {
            val x = ((zoom - FLAT_START) / (FLAT_END - FLAT_START)).coerceIn(0.0, 1.0)
            return x * x * (3 - 2 * x)
        }

    fun radius(size: Size): Double = zoom * 0.45 * min(size.width, size.height)

    /** Moves the centre by a screen drag, so the ground follows the finger, tilted or turned. */
    fun panned(delta: Offset, size: Size, pitch: Double = 0.0): MapCamera {
        val r = radius(size)
        val east = 1 + flatness * (max(cos(lat * PI / 180), 0.2) - 1)
        // Turn the drag back into the map's own east and south before moving the centre.
        val b = bearing * PI / 180
        val dx = delta.x * cos(b) + delta.y * sin(b)
        val dy = -delta.x * sin(b) + delta.y * cos(b)
        val nextLon = lon - dx / (r * east) * 180 / PI
        val nextLat = (lat + dy / cos(pitch * flatness) / r * 180 / PI).coerceIn(-89.0, 89.0)
        return copy(lon = ((nextLon + 540) % 360) - 180, lat = nextLat)
    }

    fun zoomed(factor: Double): MapCamera = copy(zoom = (zoom * factor).coerceIn(MIN_ZOOM, MAX_ZOOM))

    /** Turned by [degrees], clockwise on screen. */
    fun rotated(degrees: Double): MapCamera = copy(bearing = (((bearing + degrees) % 360) + 360) % 360)

    companion object {
        const val MIN_ZOOM = 0.8
        const val MAX_ZOOM = 90.0
        const val FLAT_START = 2.4
        const val FLAT_END = 6.0

        /** The camera that frames the scenario's playable states. */
        fun scenario(size: Size, map: WorldMap): MapCamera {
            val b = map.playBounds
            val lat = (b[1] + b[3]) / 2
            val lon = (b[0] + b[2]) / 2
            val dLon = (b[2] - b[0]) * PI / 180 * cos(lat * PI / 180) * 1.12
            val dLat = (b[3] - b[1]) * PI / 180 * 1.12
            // A tall phone frames the middle of the scenario and the player pans east or west.
            val portrait = size.height > size.width * 1.2f
            val shown = if (portrait) dLon * 0.58 else dLon
            val r = min(size.width / max(shown, 1e-6), size.height / max(dLat, 1e-6))
            val zoom = r / (0.45 * max(min(size.width, size.height), 1f))
            return MapCamera(lon, lat, zoom.coerceIn(FLAT_END, MAX_ZOOM))
        }

        /** Steps between two cameras: shortest way round in longitude, even in zoom. */
        fun interpolate(a: MapCamera, b: MapCamera, f: Double): MapCamera {
            var dLon = b.lon - a.lon
            if (dLon > 180) dLon -= 360
            if (dLon < -180) dLon += 360
            var dBearing = b.bearing - a.bearing
            if (dBearing > 180) dBearing -= 360
            if (dBearing < -180) dBearing += 360
            return MapCamera(a.lon + dLon * f, a.lat + (b.lat - a.lat) * f, exp(ln(a.zoom) + (ln(b.zoom) - ln(a.zoom)) * f), a.bearing + dBearing * f)
        }
    }
}

/**
 * Orthographic globe blended into a flat equirectangular map around the camera centre. The two
 * agree at the centre, so the blend reads as the globe unrolling rather than a cut. Once flat the
 * plane can tilt away from the viewer by [pitchSetting] radians, in perspective, and points can
 * stand [lift]ed above the ground.
 */
class MapProjection(camera: MapCamera, size: Size, pitchSetting: Double = 0.0) {
    private companion object {
        /** The shortest step, in pixels across plus down, worth adding to a path. */
        const val MIN_STEP = 0.8f
    }

    val r = camera.radius(size)
    val t = camera.flatness
    val cx = size.width / 2.0
    val cy = size.height / 2.0
    /** The tilt actually applied: none on the globe, the full setting once flat. */
    val pitch = pitchSetting * t
    private val sinP = sin(pitch)
    private val cosP = cos(pitch)
    private val focal = 1.6 * max(size.width, size.height)
    /** How far a height shows on screen; an untilted map keeps a slight oblique view of it. */
    private val rise = max(sinP, 0.40 * t)
    val lon0 = camera.lon * PI / 180
    val lat0 = camera.lat * PI / 180
    private val sinLon0 = sin(lon0)
    private val cosLon0 = cos(lon0)
    private val sinB = sin(camera.bearing * PI / 180)
    private val cosB = cos(camera.bearing * PI / 180)
    private val sin0 = sin(lat0)
    private val cos0 = cos(lat0)
    private val vx = cos0 * cos(lon0)
    private val vy = cos0 * sin(lon0)
    private val vz = sin0
    private val reach = min(PI / 2 + 0.05 + t, sqrt((size.width * size.width + size.height * size.height).toDouble()) / 2 / r * 1.4 * (1 + 1.5 * sinP) + 0.05)

    /** How squarely a ring's centre faces the viewer: 1 dead centre, 0 at the horizon, negative behind. */
    fun facing(ring: GeoRing): Double = ring.cx * vx + ring.cy * vy + ring.cz * vz

    /** Whether any of a ring could be on screen. */
    fun mayShow(ring: GeoRing): Boolean =
        acos((ring.cx * vx + ring.cy * vy + ring.cz * vz).coerceIn(-1.0, 1.0)) - ring.radius < reach

    private fun wrap(dl: Double): Double = when {
        dl > PI -> dl - 2 * PI
        dl < -PI -> dl + 2 * PI
        else -> dl
    }

    private var front = true

    /** Screen point for vertex [i]; hidden points sit on the globe's edge. Sets [front]. */
    private fun at(ring: GeoRing, i: Int, dl: Double, h: Double = 0.0): Offset {
        // The angle-difference identities give cos and sin of dl from values kept on the ring,
        // whichever way round the world dl was unwrapped: no trigonometry per point.
        val cosDl = ring.cosLon[i] * cosLon0 + ring.sinLon[i] * sinLon0
        val sinDl = ring.sinLon[i] * cosLon0 - ring.cosLon[i] * sinLon0
        val cosc = sin0 * ring.sinLat[i] + cos0 * ring.cosLat[i] * cosDl
        var ox = ring.cosLat[i] * sinDl
        var oy = cos0 * ring.sinLat[i] - sin0 * ring.cosLat[i] * cosDl
        front = cosc >= 0 || t > 0.5
        if (cosc < 0) {
            val n = sqrt(ox * ox + oy * oy)
            if (n > 1e-9) { ox /= n; oy /= n }
        }
        val fx = dl * cos0
        val fy = ring.lat[i] - lat0
        val gx = r * (ox + t * (fx - ox))
        val gy = -r * (oy + t * (fy - oy))
        return lift(turnX(gx, gy), turnY(gx, gy), h)
    }

    /**
     * Screen point for a ground offset (dx, dy) from the view centre, [h] pixels above the ground,
     * through the tilt and its perspective.
     */
    fun lift(dx: Double, dy: Double, h: Double): Offset {
        if (pitch == 0.0 && h == 0.0) return Offset((cx + dx).toFloat(), (cy + dy).toFloat())
        val s = focal / max(focal - dy * sinP, 0.25 * focal)
        return Offset((cx + dx * s).toFloat(), (cy + (dy * cosP - h * rise) * s).toFloat())
    }

    /** The ground offset from the view centre of a point in radians, before the tilt; NaN when behind the globe. */
    fun ground(lon: Double, lat: Double): Offset {
        val dl = wrap(lon - lon0)
        val sinLat = sin(lat)
        val cosLat = cos(lat)
        val cosDl = cos(dl)
        val cosc = sin0 * sinLat + cos0 * cosLat * cosDl
        if (cosc < 0 && t <= 0.5) return Offset.Unspecified
        var ox = cosLat * sin(dl)
        var oy = cos0 * sinLat - sin0 * cosLat * cosDl
        if (cosc < 0) {
            val n = sqrt(ox * ox + oy * oy)
            if (n > 1e-9) { ox /= n; oy /= n }
        }
        val gx = r * (ox + t * (dl * cos0 - ox))
        val gy = -r * (oy + t * (lat - lat0 - oy))
        return Offset(turnX(gx, gy).toFloat(), turnY(gx, gy).toFloat())
    }

    /** A ground offset turned by the camera's bearing (screen y points down). */
    private fun turnX(x: Double, y: Double) = x * cosB - y * sinB
    private fun turnY(x: Double, y: Double) = x * sinB + y * cosB

    /** Screen point of a place in radians [h] pixels up, or [Offset.Unspecified] when behind the globe. */
    fun at(lon: Double, lat: Double, h: Double = 0.0): Offset {
        val g = ground(lon, lat)
        return if (g.isSpecified) lift(g.x.toDouble(), g.y.toDouble(), h) else g
    }

    /** A direction on the sphere in view space: x right, y up, z towards the viewer. */
    fun toView(lon: Double, lat: Double): DoubleArray {
        val dl = lon - lon0
        val cosLat = cos(lat)
        val x = cosLat * sin(dl)
        val y = cos0 * sin(lat) - sin0 * cosLat * cos(dl)
        // Turned with the bearing, as the screen shows it (view y points up).
        return doubleArrayOf(x * cosB + y * sinB, -x * sinB + y * cosB, sin0 * sin(lat) + cos0 * cosLat * cos(dl))
    }

    /** Longitude and latitude in radians of a view-space direction. */
    fun fromView(xTurned: Double, yTurned: Double, z: Double): DoubleArray {
        val x = xTurned * cosB - yTurned * sinB
        val y = xTurned * sinB + yTurned * cosB
        // World = x * east + y * north + z * centre.
        val wx = -sin(lon0) * x - sin0 * cos(lon0) * y + vx * z
        val wy = cos(lon0) * x - sin0 * sin(lon0) * y + vy * z
        val wz = cos0 * y + vz * z
        return doubleArrayOf(kotlin.math.atan2(wy, wx), kotlin.math.asin((wz / max(sqrt(wx * wx + wy * wy + wz * wz), 1e-9)).coerceIn(-1.0, 1.0)))
    }

    /** A single point, or null when it is behind the globe. */
    fun project(point: GeoRing): Offset? {
        val p = at(point, 0, wrap(point.lon[0] - lon0))
        return if (front) p else null
    }

    /** A single point [h] pixels above the ground, or null when it is behind the globe. */
    fun project(point: GeoRing, h: Double): Offset? {
        val g = ground(point.lon[0], point.lat[0])
        return if (g.isSpecified) lift(g.x.toDouble(), g.y.toDouble(), h) else null
    }

    /** Filled rings, longitude unwrapped along each ring so the flat map never tears it; [h] pixels up. */
    fun addPolygons(rings: List<GeoRing>, path: Path, h: Double = 0.0) {
        var lastX = 0f
        var lastY = 0f
        for (ring in rings) {
            if (ring.size < 3 || !mayShow(ring)) continue
            var previous = 0.0
            for (i in 0 until ring.size) {
                var dl = ring.lon[i] - lon0
                if (i == 0) dl = wrap(dl) else {
                    while (dl - previous > PI) dl -= 2 * PI
                    while (dl - previous < -PI) dl += 2 * PI
                }
                previous = dl
                val p = at(ring, i, dl, h)
                if (i == 0) {
                    path.moveTo(p.x, p.y)
                    lastX = p.x; lastY = p.y
                } else if (i == ring.size - 1 || abs(p.x - lastX) + abs(p.y - lastY) >= MIN_STEP) {
                    // Points closer than a pixel to the last one add nothing but work.
                    path.lineTo(p.x, p.y)
                    lastX = p.x; lastY = p.y
                }
            }
            path.close()
        }
    }

    fun polygons(rings: List<GeoRing>, h: Double = 0.0): Path = Path().apply {
        fillType = PathFillType.EvenOdd
        addPolygons(rings, this, h)
    }

    /**
     * The visible side walls of rings raised [h] pixels, as one path per light level (dark, mid,
     * lit). A wall shows when it faces the viewer; its light depends on how squarely it faces the
     * sun in the upper left.
     */
    fun walls(rings: List<GeoRing>, h: Double, out: Array<Path>) {
        for (ring in rings) {
            if (ring.size < 3 || !mayShow(ring)) continue
            val xs = FloatArray(ring.size * 2)
            val ys = FloatArray(ring.size * 2)
            var previous = 0.0
            var area = 0.0
            for (i in 0 until ring.size) {
                var dl = ring.lon[i] - lon0
                if (i == 0) dl = wrap(dl) else {
                    while (dl - previous > PI) dl -= 2 * PI
                    while (dl - previous < -PI) dl += 2 * PI
                }
                previous = dl
                val b = at(ring, i, dl, 0.0)
                val top = at(ring, i, dl, h)
                xs[2 * i] = b.x; ys[2 * i] = b.y; xs[2 * i + 1] = top.x; ys[2 * i + 1] = top.y
            }
            for (i in 0 until ring.size) {
                val j = (i + 1) % ring.size
                area += xs[2 * i].toDouble() * ys[2 * j] - xs[2 * j].toDouble() * ys[2 * i]
            }
            val sign = if (area >= 0) 1f else -1f
            for (i in 0 until ring.size) {
                val j = (i + 1) % ring.size
                val ex = xs[2 * j] - xs[2 * i]
                val ey = ys[2 * j] - ys[2 * i]
                val len = sqrt(ex * ex + ey * ey)
                if (len < 0.3f) continue
                val nx = sign * ey / len
                val ny = -sign * ex / len
                if (ny < -0.05f) continue
                val light = nx * -0.55f + ny * -0.83f
                val path = out[if (light > 0.35f) 2 else if (light > -0.2f) 1 else 0]
                path.moveTo(xs[2 * i], ys[2 * i])
                path.lineTo(xs[2 * j], ys[2 * j])
                path.lineTo(xs[2 * j + 1], ys[2 * j + 1])
                path.lineTo(xs[2 * i + 1], ys[2 * i + 1])
                path.close()
            }
        }
    }

    /** Stroked lines, broken where they pass behind the globe or across the date line. */
    fun lines(rings: List<GeoRing>): Path = Path().apply {
        val jump = (r * 0.5).toFloat()
        for (ring in rings) {
            if (ring.size < 2 || !mayShow(ring)) continue
            var drawing = false
            var last = Offset.Zero
            for (i in 0 until ring.size) {
                val p = at(ring, i, wrap(ring.lon[i] - lon0))
                if (!front) { drawing = false; continue }
                val step = abs(p.x - last.x) + abs(p.y - last.y)
                if (drawing && step < jump) {
                    if (step < MIN_STEP && i != ring.size - 1) continue
                    lineTo(p.x, p.y)
                } else { moveTo(p.x, p.y); drawing = true }
                last = p
            }
        }
    }

    /** Even-odd test of a screen point against a feature's projected rings, raised [h] pixels. */
    fun contains(rings: List<GeoRing>, point: Offset, h: Double = 0.0): Boolean {
        var inside = false
        for (ring in rings) {
            if (ring.size < 3 || !mayShow(ring)) continue
            val xs = FloatArray(ring.size)
            val ys = FloatArray(ring.size)
            var previous = 0.0
            for (i in 0 until ring.size) {
                var dl = ring.lon[i] - lon0
                if (i == 0) dl = wrap(dl) else {
                    while (dl - previous > PI) dl -= 2 * PI
                    while (dl - previous < -PI) dl += 2 * PI
                }
                previous = dl
                val p = at(ring, i, dl, h)
                xs[i] = p.x; ys[i] = p.y
            }
            var j = ring.size - 1
            for (i in 0 until ring.size) {
                if ((ys[i] > point.y) != (ys[j] > point.y) &&
                    point.x < (xs[j] - xs[i]) * (point.y - ys[i]) / (ys[j] - ys[i]) + xs[i]
                ) inside = !inside
                j = i
            }
        }
        return inside
    }
}
