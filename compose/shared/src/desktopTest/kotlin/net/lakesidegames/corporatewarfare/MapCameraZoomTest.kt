package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** A double-tap zooms in on the tapped ground: it stays under the finger, and the zoom is applied once. */
class MapCameraZoomTest {
    @Test
    fun groundUnderTheFingerStaysUnderIt() {
        val size = Size(1200f, 800f)
        val camera = MapCamera(-85.0, 40.0, 100.0)
        val ground = GeoRing.point(-84.9, 40.08)
        val tap = MapProjection(camera, size).project(ground)!!
        val zoomed = camera.zoomedAround(3.0, tap, size)
        assertEquals(300.0, zoomed.zoom, 1e-9)
        val after = MapProjection(zoomed, size).project(ground)!!
        assertTrue(abs(after.x - tap.x) < 4f && abs(after.y - tap.y) < 4f, "moved from $tap to $after")
    }

    @Test
    fun reachesTheMaximumAndStaysThere() {
        val size = Size(400f, 800f)
        val near = MapCamera(-85.0, 40.0, 1000.0)
        assertEquals(MapCamera.MAX_ZOOM, near.zoomedAround(3.0, Offset(300f, 200f), size).zoom, 1e-9)
        assertTrue(MapCamera.MAX_ZOOM >= MapCamera.MODEL_ZOOM)
        val atMax = MapCamera(-85.0, 40.0, MapCamera.MAX_ZOOM)
        val same = atMax.zoomedAround(3.0, Offset(300f, 200f), size)
        assertEquals(atMax.lon, same.lon, 1e-12)
        assertEquals(atMax.lat, same.lat, 1e-12)
    }
}
