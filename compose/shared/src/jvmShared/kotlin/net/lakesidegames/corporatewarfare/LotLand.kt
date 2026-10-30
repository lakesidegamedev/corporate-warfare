package net.lakesidegames.corporatewarfare

import java.util.concurrent.ConcurrentHashMap
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin

/**
 * Keeps a lot on dry land. A lot is a fixed offset in metres from its region's hub, and for a hub on
 * a coast or beside a lake that offset can land in the water. The lot is then walked toward the hub
 * and round it, the smallest move first, until its whole footprint is on land (inside a country,
 * outside every lake) and still inside its region. The same lot always gets the same answer, so a
 * building never hops between frames or between players.
 */
internal object LotLand {
    /** How far from a lot's centre its footprint reaches, in metres. */
    const val PLAYER_RADIUS_M = 6_000.0
    const val RIVAL_RADIUS_M = 3_000.0

    /** The rings that matter round one hub: land, lakes and the region itself. */
    private class Around(val land: List<GeoRing>, val lakes: List<GeoRing>, val region: List<GeoRing>)

    private val arounds = ConcurrentHashMap<String, Around>()
    private val answers = ConcurrentHashMap<String, Pair<Double, Double>>()

    private fun around(map: WorldMap, region: String): Around? = arounds.getOrPut("${System.identityHashCode(map)}/$region") {
        val hub = map.anchor(region) ?: return null
        // Rings whose box comes within about three degrees of the hub: far more than any lot reaches.
        val m = 0.06
        val near = { r: GeoRing -> r.size >= 3 && r.lon.min() - m <= hub.lon[0] && r.lon.max() + m >= hub.lon[0] && r.lat.min() - m <= hub.lat[0] && r.lat.max() + m >= hub.lat[0] }
        Around(
            map.countries.flatMap { it.rings }.filter(near),
            map.lakes.filter(near),
            map.regions.firstOrNull { it.code == region }?.rings.orEmpty(),
        )
    }

    private fun onLand(a: Around, lon: Double, lat: Double): Boolean =
        a.land.any { geoInside(it, lon, lat) } && a.lakes.none { geoInside(it, lon, lat) }

    /** The point [east] and [north] metres from a hub, in radians. */
    private fun at(hub: GeoRing, east: Double, north: Double): Pair<Double, Double> =
        (hub.lon[0] + east / (WorldScale.EARTH_RADIUS_M * cos(hub.lat[0]))) to (hub.lat[0] + north / WorldScale.EARTH_RADIUS_M)

    /** Is a footprint of [radius] metres round the lot dry, and (if [inRegion]) is its centre inside the region. */
    private fun fits(hub: GeoRing, a: Around, east: Double, north: Double, radius: Double, inRegion: Boolean): Boolean {
        for (k in 0..8) {
            val (e, n) = if (k == 0) east to north else {
                val t = (k - 1) * PI / 4
                (east + radius * cos(t)) to (north + radius * sin(t))
            }
            val (lon, lat) = at(hub, e, n)
            if (!onLand(a, lon, lat)) return false
        }
        if (!inRegion) return true
        val (lon, lat) = at(hub, east, north)
        return a.region.any { geoInside(it, lon, lat) }
    }

    /** Is the lot at its offset from [region]'s hub wholly dry (a footprint of [radius] metres)? For tests. */
    fun isDry(map: WorldMap, region: String, east: Double, north: Double, radius: Double): Boolean {
        val hub = map.anchor(region) ?: return true
        val a = around(map, region) ?: return true
        return fits(hub, a, east, north, radius, false)
    }

    /** Is the lot's centre inside [region]? For tests. */
    fun isInRegion(map: WorldMap, region: String, east: Double, north: Double): Boolean {
        val hub = map.anchor(region) ?: return true
        val a = around(map, region) ?: return true
        val (lon, lat) = at(hub, east, north)
        return a.region.any { geoInside(it, lon, lat) }
    }

    /**
     * The lot at [offset] from [region]'s hub, moved onto dry land inside the region if it is not
     * already: the nearest spot to the offset among those on the way to the hub and round it.
     * A lot nothing fits (a hub with no dry land round it) stays where it is.
     */
    fun dry(map: WorldMap, region: String, offset: Pair<Double, Double>, radius: Double): Pair<Double, Double> =
        answers.getOrPut("${System.identityHashCode(map)}/$region/${offset.first}/${offset.second}/$radius") {
            val hub = map.anchor(region) ?: return offset
            val a = around(map, region) ?: return offset
            val (east, north) = offset
            if (fits(hub, a, east, north, radius, true)) return@getOrPut offset
            val tries = ArrayList<Triple<Double, Double, Double>>()
            for (scale in 10 downTo 0) {
                val s = scale / 10.0
                for (step in 0..24) {
                    // 0, +15, -15, +30, -30 ... degrees round the hub.
                    val turn = ((step + 1) / 2) * (if (step % 2 == 1) 1 else -1) * 15.0 * PI / 180
                    val e = (east * cos(turn) - north * sin(turn)) * s
                    val n = (east * sin(turn) + north * cos(turn)) * s
                    tries.add(Triple(e, n, hypot(e - east, n - north)))
                }
            }
            tries.sortBy { it.third }
            val best = tries.firstOrNull { fits(hub, a, it.first, it.second, radius, true) }
                ?: tries.firstOrNull { fits(hub, a, it.first, it.second, radius, false) }
            if (best == null) offset else best.first to best.second
        }
}
