package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
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
    val counties: List<Rings> = emptyList(),
    val lakes: List<Rings>,
    val rivers: List<Lines> = emptyList(),
    val interstates: List<Lines>,
    val cities: List<City>,
    val hubs: Map<String, List<Double>>,
) {
    @Serializable data class Country(val code: String, val name: String, val rings: List<List<Int>>)
    @Serializable data class Area(val code: String, val name: String, val play: Boolean, val rings: List<List<Int>>)
    @Serializable data class RegionArea(val code: String, val state: String, val name: String, val rings: List<List<Int>>)
    @Serializable data class Rings(val rings: List<List<Int>>)
    @Serializable data class ScenarioArea(val id: String, val rings: List<List<Int>>)
    @Serializable data class Lines(val lines: List<List<Int>>)
    @Serializable data class City(
        val name: String,
        val region: String,
        val country: String? = null,
        val x: Int,
        val y: Int,
        val population: Int,
        val capital: Boolean,
    )
}

/** One ring or line on the sphere in radians, with a bounding cap for culling. */
class GeoRing(flat: List<Int>) {
    val size = flat.size / 2
    val lon = DoubleArray(size) { flat[2 * it] / 10_000.0 * PI / 180 }
    val lat = DoubleArray(size) { flat[2 * it + 1] / 10_000.0 * PI / 180 }
    val sinLat = DoubleArray(size) { sin(lat[it]) }
    val cosLat = DoubleArray(size) { cos(lat[it]) }
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

/** Geometry turned into sphere points once. */
class WorldMap private constructor(val file: WorldMapFile) {
    class Feature(val code: String, val name: String, val rings: List<GeoRing>)

    val countries = file.countries.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val playStates = file.states.filter { it.play }.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val contextStates = file.states.filter { !it.play }.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val regions = file.regions.map { Feature(it.code, it.name, it.rings.map(::GeoRing)) }
    val provinces = file.provinces.map(::GeoRing)
    /** Each scenario's area as outlines, for the menu globe. */
    val scenarioAreas: List<Pair<String, List<GeoRing>>> = file.scenarioAreas.map { it.id to it.rings.map(::GeoRing) }
    val roads = file.roads.map(::GeoRing)
    val counties = file.counties.flatMap { c -> c.rings.map(::GeoRing) }
    val lakes = file.lakes.flatMap { l -> l.rings.map(::GeoRing) }
    val rivers = file.rivers.flatMap { r -> r.lines.map(::GeoRing) }
    val interstates = file.interstates.flatMap { r -> r.lines.map(::GeoRing) }
    val hubs: Map<String, GeoRing> = file.hubs.filterValues { it.size == 2 }
        .mapValues { GeoRing.point(it.value[0] / 10_000, it.value[1] / 10_000) }
    val cities: List<Pair<WorldMapFile.City, GeoRing>> = file.cities.map { it to GeoRing.point(it.x / 10_000.0, it.y / 10_000.0) }

    /** The scenario's playable states in degrees: west, south, east, north. */
    val playBounds: DoubleArray = run {
        var w = 180.0; var s = 90.0; var e = -180.0; var n = -90.0
        file.states.filter { it.play }.forEach { state ->
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

        /** Loaded once from the app's resources; null if the file is missing. */
        val shared: WorldMap? by lazy {
            val stream = WorldMap::class.java.getResourceAsStream("/world-map.json") ?: return@lazy null
            WorldMap(json.decodeFromString(WorldMapFile.serializer(), stream.bufferedReader().readText()))
        }
    }
}

/** Where the map looks: a centre in degrees and a zoom where 1 fits the whole globe. */
data class MapCamera(val lon: Double, val lat: Double, val zoom: Double) {
    /** 0 shows the globe, 1 the flat map, smoothly between. */
    val flatness: Double
        get() {
            val x = ((zoom - FLAT_START) / (FLAT_END - FLAT_START)).coerceIn(0.0, 1.0)
            return x * x * (3 - 2 * x)
        }

    fun radius(size: Size): Double = zoom * 0.45 * min(size.width, size.height)

    /** Moves the centre by a screen drag, so the ground follows the finger. */
    fun panned(delta: Offset, size: Size): MapCamera {
        val r = radius(size)
        val east = 1 + flatness * (max(cos(lat * PI / 180), 0.2) - 1)
        val nextLon = lon - delta.x / (r * east) * 180 / PI
        val nextLat = (lat + delta.y / r * 180 / PI).coerceIn(-80.0, 80.0)
        return copy(lon = ((nextLon + 540) % 360) - 180, lat = nextLat)
    }

    fun zoomed(factor: Double): MapCamera = copy(zoom = (zoom * factor).coerceIn(MIN_ZOOM, MAX_ZOOM))

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
            return MapCamera(a.lon + dLon * f, a.lat + (b.lat - a.lat) * f, exp(ln(a.zoom) + (ln(b.zoom) - ln(a.zoom)) * f))
        }
    }
}

/**
 * Orthographic globe blended into a flat equirectangular map around the camera centre. The two
 * agree at the centre, so the blend reads as the globe unrolling rather than a cut.
 */
class MapProjection(camera: MapCamera, size: Size) {
    val r = camera.radius(size)
    val t = camera.flatness
    val cx = size.width / 2.0
    val cy = size.height / 2.0
    private val lon0 = camera.lon * PI / 180
    private val lat0 = camera.lat * PI / 180
    private val sin0 = sin(lat0)
    private val cos0 = cos(lat0)
    private val vx = cos0 * cos(lon0)
    private val vy = cos0 * sin(lon0)
    private val vz = sin0
    private val reach = min(PI / 2 + 0.05 + t, sqrt((size.width * size.width + size.height * size.height).toDouble()) / 2 / r * 1.4 + 0.05)

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
    private fun at(ring: GeoRing, i: Int, dl: Double): Offset {
        val cosDl = cos(dl)
        val cosc = sin0 * ring.sinLat[i] + cos0 * ring.cosLat[i] * cosDl
        var ox = ring.cosLat[i] * sin(dl)
        var oy = cos0 * ring.sinLat[i] - sin0 * ring.cosLat[i] * cosDl
        front = cosc >= 0 || t > 0.5
        if (cosc < 0) {
            val n = sqrt(ox * ox + oy * oy)
            if (n > 1e-9) { ox /= n; oy /= n }
        }
        val fx = dl * cos0
        val fy = ring.lat[i] - lat0
        return Offset((cx + r * (ox + t * (fx - ox))).toFloat(), (cy - r * (oy + t * (fy - oy))).toFloat())
    }

    /** A single point, or null when it is behind the globe. */
    fun project(point: GeoRing): Offset? {
        val p = at(point, 0, wrap(point.lon[0] - lon0))
        return if (front) p else null
    }

    /** Filled rings, longitude unwrapped along each ring so the flat map never tears it. */
    fun addPolygons(rings: List<GeoRing>, path: Path) {
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
                val p = at(ring, i, dl)
                if (i == 0) path.moveTo(p.x, p.y) else path.lineTo(p.x, p.y)
            }
            path.close()
        }
    }

    fun polygons(rings: List<GeoRing>): Path = Path().apply {
        fillType = PathFillType.EvenOdd
        addPolygons(rings, this)
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
                if (drawing && abs(p.x - last.x) + abs(p.y - last.y) < jump) lineTo(p.x, p.y) else { moveTo(p.x, p.y); drawing = true }
                last = p
            }
        }
    }

    /** Even-odd test of a screen point against a feature's projected rings. */
    fun contains(rings: List<GeoRing>, point: Offset): Boolean {
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
                val p = at(ring, i, dl)
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
