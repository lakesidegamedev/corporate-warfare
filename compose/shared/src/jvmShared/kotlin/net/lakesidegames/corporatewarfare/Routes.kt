package net.lakesidegames.corporatewarfare

import java.util.PriorityQueue
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.roundToLong
import kotlin.math.sqrt

/**
 * Freight follows the real networks: trucks the interstates, trains the railways (Natural Earth,
 * in the bundled map). Each network is a graph whose nodes are the points of its lines snapped
 * to a hundredth of a degree, so lines that cross or meet join; a lane's route is the shortest
 * path between the network points nearest its two hubs. Routes are worked out once per map and
 * kept. A lane with no sensible route on the network keeps the straight line.
 */
internal class RoadNetwork(rings: List<GeoRing>) {
    private val lon = ArrayList<Double>()
    private val lat = ArrayList<Double>()
    private val edges = ArrayList<MutableList<Pair<Int, Double>>>()
    private val ids = HashMap<Long, Int>()

    init {
        for (ring in rings) {
            var previous = -1
            for (i in 0 until ring.size) {
                val node = node(ring.lon[i], ring.lat[i])
                if (previous >= 0 && previous != node) {
                    val d = distance(previous, node)
                    edges[previous] += node to d
                    edges[node] += previous to d
                }
                previous = node
            }
        }
    }

    private fun node(lonRad: Double, latRad: Double): Int {
        val key = (lonRad * 180 / PI * 100).roundToLong() * 100_000 + (latRad * 180 / PI * 100).roundToLong()
        return ids.getOrPut(key) {
            lon.add(lonRad); lat.add(latRad); edges.add(ArrayList())
            lon.size - 1
        }
    }

    private fun distance(a: Int, b: Int): Double {
        val dx = (lon[b] - lon[a]) * cos((lat[a] + lat[b]) / 2)
        val dy = lat[b] - lat[a]
        return sqrt(dx * dx + dy * dy)
    }

    private fun nearest(lonRad: Double, latRad: Double): Int {
        var best = -1
        var bestD = Double.MAX_VALUE
        val c = cos(latRad)
        for (i in lon.indices) {
            if (edges[i].isEmpty()) continue
            val dx = (lon[i] - lonRad) * c
            val dy = lat[i] - latRad
            val d = dx * dx + dy * dy
            if (d < bestD) { bestD = d; best = i }
        }
        // Further than about two degrees from the network, there is no road to take.
        return if (bestD < (2 * PI / 180) * (2 * PI / 180)) best else -1
    }

    /** The route from one place to another as longitude and latitude in radians, or null. */
    fun route(fromLon: Double, fromLat: Double, toLon: Double, toLat: Double): Pair<DoubleArray, DoubleArray>? {
        val start = nearest(fromLon, fromLat)
        val goal = nearest(toLon, toLat)
        if (start < 0 || goal < 0 || start == goal) return null
        val dist = DoubleArray(lon.size) { Double.MAX_VALUE }
        val back = IntArray(lon.size) { -1 }
        val queue = PriorityQueue<Pair<Double, Int>>(compareBy { it.first })
        dist[start] = 0.0
        queue += 0.0 to start
        while (queue.isNotEmpty()) {
            val (d, at) = queue.poll()
            if (at == goal) break
            if (d > dist[at]) continue
            for ((next, w) in edges[at]) {
                val nd = d + w
                if (nd < dist[next]) {
                    dist[next] = nd
                    back[next] = at
                    queue += nd to next
                }
            }
        }
        if (back[goal] < 0) return null
        // A route far longer than the straight line means the network is broken between them.
        val dx = (toLon - fromLon) * cos((fromLat + toLat) / 2)
        val dy = toLat - fromLat
        if (dist[goal] > 3 * sqrt(dx * dx + dy * dy) + 0.02) return null
        val path = ArrayList<Int>()
        var at = goal
        while (at >= 0) { path += at; at = back[at] }
        path.reverse()
        val lons = DoubleArray(path.size + 2)
        val lats = DoubleArray(path.size + 2)
        lons[0] = fromLon; lats[0] = fromLat
        path.forEachIndexed { i, n -> lons[i + 1] = lon[n]; lats[i + 1] = lat[n] }
        lons[path.size + 1] = toLon; lats[path.size + 1] = toLat
        return lons to lats
    }
}

internal object Routes {
    private val networks = HashMap<Pair<WorldMap, String>, RoadNetwork>()
    private val routes = HashMap<String, Pair<DoubleArray, DoubleArray>?>()

    private fun network(map: WorldMap, mode: String): RoadNetwork? = when (mode) {
        "road" -> networks.getOrPut(map to mode) { RoadNetwork(map.interstates) }
        "rail" -> if (map.railroads.isEmpty()) null else networks.getOrPut(map to mode) { RoadNetwork(map.railroads) }
        else -> null
    }

    /** The route for a lane between two regions by its mode, or null to keep the straight line. */
    fun of(map: WorldMap, mode: String, a: String, b: String): Pair<DoubleArray, DoubleArray>? = synchronized(this) {
        val key = "${map.scenarioId}/$mode/$a>$b"
        if (key in routes) return routes[key]
        val from = map.anchor(a)
        val to = map.anchor(b)
        val found = if (from == null || to == null) null else network(map, mode)?.route(from.lon[0], from.lat[0], to.lon[0], to.lat[0])
        routes[key] = found
        found
    }
}
