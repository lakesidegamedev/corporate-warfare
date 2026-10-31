package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.asSkiaBitmap
import kotlin.math.max
import kotlin.math.min
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The new map in motion, the GPU twin of MapCoherenceTest: while the camera is dragged every frame, the picture and
 * every layer placed over it (names, badges, the reserved building footprints) must be for the same camera, and a
 * building must stay a fixed object on the ground. The test drags across a player's building and, for every frame
 * it is shown, projects that building with the camera the frame was drawn for and looks for its gold roof in the
 * picture at exactly that place.
 */
class GpuMapDragTest {
    private fun career(scenario: String) = uniffi.cw_ffi.SessionHandle.start(
        uniffi.cw_ffi.NewGameDto(
            seed = 7uL, scenarioId = scenario, guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
            startingCapitalCents = null, pricePath = "historical", timeline = "historical",
        ),
    )

    /** Pixels in the player's roof gold (the holdings tint is off in this test, so nothing else is gold). */
    private fun roofPixels(frame: androidx.compose.ui.graphics.ImageBitmap, l: Float, t: Float, r: Float, b: Float, viewW: Int, viewH: Int): Int {
        val bmp = frame.asSkiaBitmap()
        val sx = frame.width.toFloat() / viewW; val sy = frame.height.toFloat() / viewH
        val x0 = max(0, ((l - 2f) * sx).toInt()); val x1 = min(frame.width - 1, ((r + 2f) * sx).toInt())
        val y0 = max(0, ((t - 2f) * sy).toInt()); val y1 = min(frame.height - 1, ((b + 2f) * sy).toInt())
        var n = 0
        for (y in y0..y1) for (x in x0..x1) {
            val c = bmp.getColor(x, y)
            val red = (c shr 16) and 0xFF; val green = (c shr 8) and 0xFF; val blue = c and 0xFF
            if (red > 180 && green > 130 && blue < 120 && red > blue + 90) n++
        }
        return n
    }

    @Test
    fun everyLayerIsDrawnForTheFrameCameraWhileDragging() {
        val id = "texas-1981"
        val session = try {
            openDesktopGpuMap(id)
        } catch (e: Throwable) {
            println("no adapter, skipping: ${e.message}")
            return
        }
        try {
            val game = career(id)
            val plants = game.mapPlants()
            // Plants and no holdings, so the only gold on the ground is a player's roof.
            session.update(gpuUpdateJson(plants, emptyList(), MapLayer.Presence, emptyMap(), night = false, clouds = 0))
            val mine = session.buildings().filter { it.player }
            assertTrue(mine.isNotEmpty(), "the career has a building of the player's")
            val target = mine.first()
            val (w, h) = 1280 to 800
            var good = 0
            var settledFrames = 0
            // Drag a few kilometres either way across the building, close enough for its roof to be many pixels across.
            val steps = 30
            for (i in 0..steps) {
                val cam = GpuCamera(target.lon + (i - steps / 2) * 0.001, target.lat + (i - steps / 2) * 0.0004, 30.0, 40.0, i * 0.7)
                session.request(cam, w, h, true)
                val before = session.version
                val deadline = System.currentTimeMillis() + 60_000
                while (session.version == before && System.currentTimeMillis() < deadline) Thread.sleep(5)
                // One read: the picture and the camera it was drawn for.
                val shown = session.shown ?: run { println("step $i: no frame yet"); null } ?: continue
                val shownCam = shown.camera
                // The frame is the view plus its margin: its boxes are in the frame's own pixels.
                val fw = shown.image.width; val fh = shown.image.height
                val rects = session.buildingRects(shownCam, fw, fh)
                val b = session.buildings().indexOf(target)
                val l = rects[4 * b]; val t = rects[4 * b + 1]; val r = rects[4 * b + 2]; val bt = rects[4 * b + 3]
                if (l.isNaN() || r < 0 || l > fw || bt < 0 || t > fh) { println("step $i: building off screen ($l,$t,$r,$bt) camera=$shownCam"); continue }
                val px = roofPixels(shown.image, l, t, r, bt, fw, fh)
                // The roof is where the shown camera says the building is: it has gold there.
                if (i % 10 == 0) println("step $i: $px gold pixels, box ($l,$t,$r,$bt), image ${shown.image.width}x${shown.image.height}")
                if (px > 0) good++ else println("step $i: no gold in the building's box ($l,$t,$r,$bt) camera=$shownCam")
                if (shown.factor == 1f) settledFrames++
            }
            assertTrue(good >= steps * 0.8, "the roof was at the projected place in only $good of ${steps + 1} frames")
            // Stopped: the full size frame for exactly the last camera arrives, and the roof is inside its box.
            val last = GpuCamera(target.lon + (steps - steps / 2) * 0.001, target.lat + (steps - steps / 2) * 0.0004, 30.0, 40.0, steps * 0.7)
            session.request(last, w, h, false)
            val deadline = System.currentTimeMillis() + 60_000
            while ((session.shown?.view != last || session.shown?.factor != 1f) && System.currentTimeMillis() < deadline) Thread.sleep(20)
            val s = session.shown!!
            assertEquals(last, s.view, "the settled frame is for the last camera")
            assertEquals(w + 2 * s.margin, s.image.width, "the settled frame is the view plus its margin at full size")
        } finally {
            session.close()
        }
    }

    @Test
    fun aBuildingKeepsItsSizeOnTheGroundAtEveryZoom() {
        val id = "great-lakes-2000"
        val session = try {
            openDesktopGpuMap(id)
        } catch (e: Throwable) {
            println("no adapter, skipping: ${e.message}")
            return
        }
        try {
            val plants = career(id).mapPlants()
            session.update(gpuUpdateJson(plants, emptyList(), MapLayer.Presence, emptyMap(), night = false, clouds = 0))
            val mine = session.buildings().indexOfFirst { it.player }
            assertTrue(mine >= 0)
            val b = session.buildings()[mine]
            // Width on screen over width of the view is the building's size on the ground: the same at 200 km and at 1200 km.
            val sizes = listOf(200.0, 400.0, 800.0, 1200.0).map { km ->
                val rects = session.buildingRects(GpuCamera(b.lon, b.lat, km, 0.0, 0.0), 1600, 1000)
                val across = max(rects[4 * mine + 2] - rects[4 * mine], 0f).toDouble()
                across / 1600.0 * km
            }
            println("building ground size by zoom (km): $sizes")
            // About one to three kilometres across and not changing with zoom.
            assertTrue(sizes.all { it in 0.6..4.5 }, "a plant is a few kilometres across: $sizes")
            assertTrue(sizes.max() - sizes.min() < 0.35 * sizes.max(), "the building's ground size stays the same at every zoom: $sizes")
        } finally {
            session.close()
        }
    }
}
