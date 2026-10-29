package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.asSkiaPath
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.min
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The interstates' centre line is cut on the ground: each drawn dash starts and ends at its own
 * ground points projected for the camera, at two camera centres and both tilts. The old screen dash
 * pattern (4 on, 4 off from each line's projected start) sat at a different place on the road in
 * each view, which is the crawl the player saw.
 */
class GroundDashTest {
    @Test
    fun centreLineDashesStayOnTheGround() {
        val map = WorldMap.shared!!
        val size = Size(1600f, 900f)
        val road = map.interstates.maxBy { it.size }
        val dashes = GroundDashes.cut(road, GroundDashes.CENTRE_DASH, GroundDashes.CENTRE_GAP)
        assertTrue(dashes.size > 10, "a long interstate is cut into many dashes")
        val middle = dashes.size / 2
        val dash = dashes[middle]
        val length = (1 until dash.size).sumOf { GroundDashes.km(dash, it - 1, it) }
        assertEquals(GroundDashes.CENTRE_DASH, length, 0.05, "a dash is its length on the ground")
        // Where the dash starts along the road, on the ground.
        val along = middle * (GroundDashes.CENTRE_DASH + GroundDashes.CENTRE_GAP)
        val start = GeoRing.point(Math.toDegrees(dash.lon[0]), Math.toDegrees(dash.lat[0]))
        val end = GeoRing.point(Math.toDegrees(dash.lon[dash.size - 1]), Math.toDegrees(dash.lat[dash.size - 1]))

        val first = MapCamera.scenario(size, map).zoomed(3.0).rotated(30.0)
            .copy(lon = Math.toDegrees(dash.lon[0]), lat = Math.toDegrees(dash.lat[0]))
        val phases = ArrayList<Double>()
        for (pitch in listOf(28 * PI / 180, 48 * PI / 180)) {
            val second = first.panned(Offset(180f, 260f), size, pitch)
            for (camera in listOf(first, second)) {
                val p = MapProjection(camera, size, pitch)
                val drawn = p.lines(listOf(dash)).asSkiaPath().points.filterNotNull()
                val a = p.project(start)!!
                val b = p.project(end)!!
                assertTrue(drawn.size >= 2, "the dash is drawn")
                assertEquals(0f, (Offset(drawn.first().x, drawn.first().y) - a).getDistance(), 0.5f, "the dash starts on its ground point")
                assertEquals(0f, (Offset(drawn.last().x, drawn.last().y) - b).getDistance(), 0.5f, "the dash ends on its ground point")
                // The old way: how far along the drawn road the dash's ground point is, in pixels,
                // against the 8 pixel screen pattern.
                var s = 0.0
                var km = 0.0
                for (i in 1 until road.size) {
                    val p0 = p.project(GeoRing.point(Math.toDegrees(road.lon[i - 1]), Math.toDegrees(road.lat[i - 1])))!!
                    val p1 = p.project(GeoRing.point(Math.toDegrees(road.lon[i]), Math.toDegrees(road.lat[i])))!!
                    val step = GroundDashes.km(road, i - 1, i)
                    if (km + step >= along) {
                        s += (p1 - p0).getDistance() * (along - km) / step.coerceAtLeast(1e-9)
                        break
                    }
                    s += (p1 - p0).getDistance()
                    km += step
                }
                phases += s % 8
            }
        }
        var worstOld = 0.0
        for (i in phases.indices) for (j in i + 1 until phases.size) {
            val d = abs(phases[i] - phases[j])
            worstOld = maxOf(worstOld, min(d, 8 - d))
        }
        assertTrue(worstOld > 1, "the old screen dashes moved along the road between views ($worstOld px)")
    }
}
