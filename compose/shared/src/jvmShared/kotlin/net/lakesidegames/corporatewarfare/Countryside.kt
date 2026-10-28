package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sqrt

/**
 * The diorama up close: small voxel props scattered over the satellite ground by what the ground
 * is. Forest gets conifers in the north and leafy trees further south, farmland gets crop rows and
 * the odd barn, dry land a few rocks, and houses gather round each city, spreading further in a
 * boom and pulling in during a recession. Each prop has a summer, autumn and winter look, so the
 * countryside follows the seasons with the Earth beneath it.
 */
internal object Countryside {
    /** A prop on the ground: where (degrees), what, and a stable seed for small variations. */
    class Prop(val lon: Double, val lat: Double, val kind: String, val seed: Int, val city: Double = 0.0)

    private val cache = HashMap<String, List<Prop>>()

    /** Every prop for a scenario, worked out once from its satellite crop and its cities. */
    fun props(map: WorldMap): List<Prop> {
        val id = map.scenarioId ?: return emptyList()
        cache[id]?.let { return it }
        val region = CloudRegions[id] ?: return emptyList()
        val image = Images.load("earth-$id.jpg") ?: return emptyList()
        val pixels = IntArray(image.width * image.height)
        image.readPixels(pixels)
        // The land mask from the game's own outlines, so nothing stands in a lake.
        val land = Images.load("earth-land-$id.png")
        val landPixels = land?.let { IntArray(it.width * it.height).also { px -> it.readPixels(px) } }
        fun onLand(lon: Double, lat: Double): Boolean {
            val m = land ?: return true
            val u = ((lon - region.west) / (region.east - region.west) * m.width).toInt()
            val v = ((region.north - lat) / (region.north - region.south) * m.height).toInt()
            return u in 0 until m.width && v in 0 until m.height && (landPixels!![v * m.width + u] ushr 24) > 128
        }
        val b = map.playBounds
        val west = b[0] - 3; val east = b[2] + 3; val south = b[1] - 3; val north = b[3] + 3
        val step = 0.14
        val out = ArrayList<Prop>()
        val cities = map.cities.map { (c, p) -> Triple(p.lon[0] * 180 / PI, p.lat[0] * 180 / PI, c.population.toDouble()) }
        var lat = south
        var row = 0
        while (lat < north) {
            var lon = west
            var col = 0
            while (lon < east) {
                val seed = stableHash("$id/$row/$col")
                val jx = ((seed % 1000) / 1000.0 - 0.5) * step * 0.9
                val jy = (((seed / 1000) % 1000) / 1000.0 - 0.5) * step * 0.9
                val x = lon + jx
                val y = lat + jy
                val roll = (seed / 7) % 100
                if (!onLand(x, y)) { lon += step; col++; continue }
                // Near a city: houses, thicker towards the middle.
                val near = cities.minOfOrNull { (cx, cy, pop) ->
                    val d = sqrt(((x - cx) * cos(y * PI / 180)).let { it * it } + (y - cy) * (y - cy))
                    d / (0.06 * sqrt(pop / 1e5).coerceAtLeast(1.0))
                } ?: 9.0
                if (near < 1.0) {
                    if (roll < 45) out += Prop(x, y, "house", seed, near)
                } else {
                    val u = ((x - region.west) / (region.east - region.west) * image.width).toInt()
                    val v = ((region.north - y) / (region.north - region.south) * image.height).toInt()
                    if (u in 0 until image.width && v in 0 until image.height) {
                        val c = pixels[v * image.width + u]
                        val r = (c shr 16) and 255; val g = (c shr 8) and 255; val bl = c and 255
                        val lum = (r + g + bl) / 3
                        val kind = when {
                            bl > r + 8 && bl >= g -> null // water
                            lum > 200 -> null // snow, ice or cloud in the source
                            // Forest is the darkest, greenest ground; farmland is a little lighter.
                            g > r + 6 && lum < 34 -> if (roll < 18) (if (y > 44 || roll < 6) "conifer" else "broadleaf") else null
                            g >= bl && lum in 34..150 -> when { roll < 11 -> "field"; roll < 13 -> "barn"; roll < 15 -> "broadleaf"; else -> null }
                            r > g && lum > 100 -> if (roll < 8) "rock" else null
                            else -> if (roll < 4) "broadleaf" else null
                        }
                        if (kind != null) out += Prop(x, y, kind, seed)
                    }
                }
                lon += step
                col++
            }
            lat += step
            row++
        }
        cache[id] = out
        return out
    }

    /**
     * Draws the props on screen for the current camera, far ones first, at most [limit]. Houses
     * further from a city than the economy supports are left out: towns grow in a boom.
     */
    fun DrawScope.drawCountryside(
        p: MapProjection, map: WorldMap, week: Int, scale: Int, lift: (String?) -> Double, economyPhase: String?, limit: Int = 900,
    ) {
        val reach = when (economyPhase) { "boom" -> 1.0; "expansion" -> 0.9; "recovery" -> 0.8; "slowdown" -> 0.7; "recession" -> 0.6; else -> 0.85 }
        val shown = ArrayList<Pair<Offset, Prop>>()
        for (prop in props(map)) {
            if (prop.kind == "house" && prop.city > reach) continue
            val g = p.ground(prop.lon * PI / 180, prop.lat * PI / 180)
            if (!g.isSpecified) continue
            val at = p.lift(g.x.toDouble(), g.y.toDouble(), lift(null))
            if (at.x < -20 || at.y < -20 || at.x > size.width + 20 || at.y > size.height + 20) continue
            shown += at to prop
            if (shown.size >= limit) break
        }
        shown.sortBy { it.first.y }
        for ((at, prop) in shown) {
            with(Voxels) { drawVariant(prop.kind, season(week, prop.lat), at, scale) }
        }
    }

    /** The prop variant for the season: 0 summer, 1 autumn, 2 winter; size slots carry it. */
    private fun season(week: Int, lat: Double): Int {
        val shifted = if (lat >= 0) week else (week + 26) % 52
        val winterNow = shifted <= 10 || shifted >= 49 || (shifted >= 46 && abs(lat) > 45)
        return when {
            winterNow && abs(lat) > 38 -> 2
            shifted in 38..47 && abs(lat) > 30 -> 1
            else -> 0
        }
    }
}
