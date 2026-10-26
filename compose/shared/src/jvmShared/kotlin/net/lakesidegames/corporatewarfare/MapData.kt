package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * The bundled corridor map, the same file as the iPhone app: public-domain Census and Natural
 * Earth geometry, pre-projected into a 10000-unit-wide view box with integer coordinates. Rings
 * and lines are flat x,y lists.
 */
@Serializable
data class CorridorMapFile(
    val viewbox: List<Double>,
    val states: List<Area>,
    val regions: List<RegionArea>,
    val lakes: List<Water>,
    val interstates: List<Line>,
    val cities: List<City>,
    val hubs: Map<String, List<Double>>,
) {
    @Serializable data class Area(val code: String, val name: String, val play: Boolean, val rings: List<List<Int>>)
    @Serializable data class RegionArea(val code: String, val state: String, val name: String, val rings: List<List<Int>>)
    @Serializable data class Water(val name: String, val rings: List<List<Int>>)
    @Serializable data class Line(val name: String, val lines: List<List<Int>>)
    @Serializable data class City(val name: String, val region: String, val x: Int, val y: Int, val population: Int, val capital: Boolean)
}

/** Drawable paths in view-box units, built once. */
class CorridorMap private constructor(val file: CorridorMapFile) {
    val width = file.viewbox[0].toFloat()
    val height = file.viewbox[1].toFloat()
    val playStates: List<Path> = file.states.filter { it.play }.map { polygons(it.rings) }
    val otherStates: List<Path> = file.states.filter { !it.play }.map { polygons(it.rings) }
    val regions: Map<String, Path> = file.regions.associate { it.code to polygons(it.rings) }
    val lakes: List<Path> = file.lakes.map { polygons(it.rings) }
    val interstates: List<Path> = file.interstates.map { polylines(it.lines) }

    /** The region whose outline contains a view-box point. */
    fun regionAt(point: Offset): String? = file.regions.firstOrNull { area ->
        area.rings.count { ring -> contains(ring, point.x, point.y) } % 2 == 1
    }?.code

    fun regionName(code: String): String = file.regions.firstOrNull { it.code == code }?.name ?: code

    /** Where a region's markers sit: its hub city, else its first vertex. */
    fun anchor(code: String): Offset? {
        file.hubs[code]?.let { return Offset(it[0].toFloat(), it[1].toFloat()) }
        val ring = file.regions.firstOrNull { it.code == code }?.rings?.firstOrNull() ?: return null
        return Offset(ring[0].toFloat(), ring[1].toFloat())
    }

    companion object {
        private val json = Json { ignoreUnknownKeys = true }

        /** Loaded once from the app's resources; null if the file is missing. */
        val shared: CorridorMap? by lazy {
            val stream = CorridorMap::class.java.getResourceAsStream("/corridor-map.json") ?: return@lazy null
            CorridorMap(json.decodeFromString(CorridorMapFile.serializer(), stream.bufferedReader().readText()))
        }

        private fun polygons(rings: List<List<Int>>): Path = Path().apply {
            for (ring in rings) {
                if (ring.size < 6) continue
                moveTo(ring[0].toFloat(), ring[1].toFloat())
                var i = 2
                while (i + 1 < ring.size) {
                    lineTo(ring[i].toFloat(), ring[i + 1].toFloat())
                    i += 2
                }
                close()
            }
        }

        private fun polylines(lines: List<List<Int>>): Path = Path().apply {
            for (line in lines) {
                if (line.size < 4) continue
                moveTo(line[0].toFloat(), line[1].toFloat())
                var i = 2
                while (i + 1 < line.size) {
                    lineTo(line[i].toFloat(), line[i + 1].toFloat())
                    i += 2
                }
            }
        }

        /** Even-odd ray cast against one flat ring. */
        private fun contains(ring: List<Int>, x: Float, y: Float): Boolean {
            var inside = false
            val n = ring.size / 2
            var j = n - 1
            for (i in 0 until n) {
                val xi = ring[2 * i].toFloat(); val yi = ring[2 * i + 1].toFloat()
                val xj = ring[2 * j].toFloat(); val yj = ring[2 * j + 1].toFloat()
                if ((yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) inside = !inside
                j = i
            }
            return inside
        }
    }
}
