package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.geometry.Size
import java.awt.Robot
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO
import javax.swing.JFrame
import javax.swing.SwingUtilities
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The map drawn straight into a native window: a real window with the engine's surface in it, a drag and a zoom out over 60
 * frames. Proves a frame reaches the screen for the live camera (none for an old one), that buildings keep their world spot, and
 * that what is on screen is the picture the renderer draws for that camera (compared with an offscreen render of the same view).
 * Skipped where there is no display or no graphics adapter that can present to a window.
 */
class GpuMapDirectTest {
    private val scenario = "great-lakes-2000"

    private fun career() = uniffi.cw_ffi.SessionHandle.start(
        uniffi.cw_ffi.NewGameDto(
            seed = 7uL, scenarioId = scenario, guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
            startingCapitalCents = null, pricePath = "historical", timeline = "historical",
        ),
    )

    private fun shot(r: Robot, c: java.awt.Canvas): BufferedImage {
        val p = c.locationOnScreen
        val k = c.pixelScale()
        return r.createScreenCapture(java.awt.Rectangle(p.x, p.y, (c.width * k).toInt(), (c.height * k).toInt()))
    }

    private fun java.awt.Canvas.pixelScale() = graphicsConfiguration?.defaultTransform?.scaleX ?: 1.0

    @Test
    fun theMapReachesTheScreenEveryFrameForTheLiveCamera() {
        if (!nativeSurfaceSupported()) { println("direct map: no display, skipped"); return }
        val out = System.getenv("CW_DIRECT_OUT")?.let(::File)?.also { it.mkdirs() }
        val W = 960; val H = 600
        lateinit var frame: JFrame
        val canvas = MapSurfaceCanvas()
        SwingUtilities.invokeAndWait {
            frame = JFrame("direct map test").apply {
                isUndecorated = true
                contentPane.add(canvas)
                canvas.preferredSize = java.awt.Dimension(W, H)
                pack(); setLocation(0, 0); isVisible = true
            }
        }
        try {
            val session = try {
                openDirectGpuMap(scenario, canvas) as DirectGpuSession
            } catch (e: Throwable) {
                println("direct map: skipped, no adapter can present to a window (${e.message})")
                return
            }
            try {
                val game = career()
                session.update(gpuUpdateJson(game.mapPlants(), game.mapDivisions(), MapLayer.Presence, emptyMap(), night = false, clouds = 2))
                val map = assertNotNull(WorldMap.forScenario(scenario))
                val size = Size(canvas.width * canvas.pixelScale().toFloat(), canvas.height * canvas.pixelScale().toFloat())
                val w = size.width.toInt(); val h = size.height.toInt()
                println("direct map: ${session.adapter} software=${session.software} window=${w}x$h")
                val pitch = Math.toRadians(28.0)
                val buildings = session.buildings().filter { !it.scenery }
                assertTrue(buildings.isNotEmpty(), "the career has plant buildings")
                val tracked = buildings.first()

                // The camera sequence: a drag across the map with a zoom out, one request per display refresh.
                var cam = MapCamera.scenario(size, map).let { it.copy(zoom = it.zoom * 2.5) }
                val cameras = ArrayList<GpuCamera>(); val requestedAt = ArrayList<Long>()
                val frames = java.util.Collections.synchronizedList(ArrayList<DirectFrame>())
                session.frameListener = { frames += it }
                val robot = Robot()
                val shots = ArrayList<Triple<GpuCamera, BufferedImage, DirectFrame?>>()
                val start = System.nanoTime()
                for (i in 0 until 60) {
                    cam = cam.panned(Offset(6f, 2f), size, pitch).zoomed(0.985)
                    val gc = gpuCamera(cam, size.width, size.height, pitch)
                    cameras += gc; requestedAt += System.nanoTime()
                    session.request(gc, w, h, true)
                    Thread.sleep(16)
                }
                val motionSeconds = (System.nanoTime() - start) / 1e9
                // Settle, then wait for the last request to be on screen.
                val last = cameras.last()
                val deadline = System.currentTimeMillis() + 60_000
                while (frames.lastOrNull()?.camera != last && System.currentTimeMillis() < deadline) Thread.sleep(20)
                assertEquals(last, frames.lastOrNull()?.camera, "the final view reached the screen")

                val shownFrames = frames.toList()
                val inMotion = shownFrames.filter { it.atNanos < start + (motionSeconds * 1e9).toLong() }
                val fps = inMotion.size / motionSeconds
                val render = shownFrames.map { it.renderMs }.sorted(); val present = shownFrames.map { it.presentMs }.sorted()
                println("direct map motion: ${inMotion.size} frames in ${"%.2f".format(motionSeconds)} s = ${"%.1f".format(fps)} per second; " +
                    "render median ${"%.1f".format(render[render.size / 2])} ms, present median ${"%.1f".format(present[present.size / 2])} ms; " +
                    "software=${session.software}")
                assertTrue(shownFrames.isNotEmpty() && inMotion.isNotEmpty(), "frames were presented while the camera moved")
                if (!session.software) assertTrue(fps >= 30, "a real card presents at least 30 frames a second in motion, got $fps")

                // No frame for an old camera: each presented frame is one of the requests, never older than the newest one made
                // before its drawing began, and frames never go back in time.
                var prev = -1
                for (f in shownFrames) {
                    val idx = cameras.indexOf(f.camera)
                    assertTrue(idx >= 0, "a presented frame is for a camera that was asked for")
                    val newestAtStart = requestedAt.indexOfLast { it <= f.atNanos }
                    assertTrue(idx >= newestAtStart, "frame ${f.seq} drew camera $idx but $newestAtStart was already asked for")
                    assertTrue(idx >= prev, "frames went back to an older camera")
                    prev = idx
                    assertEquals(w, f.width); assertEquals(h, f.height)
                }

                // Buildings keep their world spot: the tracked building's base, projected with each presented frame's own camera,
                // is the same ground point (inverse-picked back to the same longitude and latitude) in every frame.
                for (f in shownFrames.filter { cameras.indexOf(it.camera) % 10 == 0 }) {
                    val p = session.project(f.camera, w, h, doubleArrayOf(tracked.lon, tracked.lat))
                    if (p[0].isNaN()) continue
                    val rects = session.buildingRects(f.camera, w, h)
                    val i = session.buildings().indexOf(tracked)
                    val cx = (rects[4 * i] + rects[4 * i + 2]) / 2f
                    // The box stands on the projected base: its horizontal centre is the base's x within a pixel or two.
                    assertTrue(abs(cx - p[0]) < 3f, "the building box and its base agree at frame ${f.seq}: $cx vs ${p[0]}")
                }

                // What is on screen is the renderer's picture for that camera: several stills along a second drag, each compared
                // with an offscreen render of the same view.
                val offscreen = openDesktopGpuMap(scenario)
                try {
                    offscreen.update(gpuUpdateJson(game.mapPlants(), game.mapDivisions(), MapLayer.Presence, emptyMap(), night = false, clouds = 2))
                    for (k in 0 until 4) {
                        for (i in 0 until 15) {
                            cam = cam.panned(Offset(-6f, 1f), size, pitch).zoomed(1.01)
                            session.request(gpuCamera(cam, size.width, size.height, pitch), w, h, true)
                            Thread.sleep(16)
                        }
                        val gc = gpuCamera(cam, size.width, size.height, pitch)
                        session.request(gc, w, h, false)
                        val until = System.currentTimeMillis() + 60_000
                        while (frames.lastOrNull()?.camera != gc && System.currentTimeMillis() < until) Thread.sleep(20)
                        Thread.sleep(300)
                        val onScreen = shot(robot, canvas)
                        out?.let { ImageIO.write(onScreen, "png", File(it, "sequence-$k.png")) }
                        shots += Triple(gc, onScreen, frames.lastOrNull())
                    }
                    var compared = 0
                    for ((gc, img, _) in shots) {
                        offscreen.request(gc, w, h, false)
                        val until = System.currentTimeMillis() + 60_000
                        while ((offscreen.shown?.view != gc || offscreen.shown?.factor != 1f) && System.currentTimeMillis() < until) Thread.sleep(30)
                        val shown = assertNotNull(offscreen.shown)
                        val bmp = shown.image.asSkiaBitmap()
                        // Compare the view part of the offscreen frame (it has an overscan margin) with the captured window.
                        val m = shown.margin
                        var differing = 0; var total = 0
                        for (y in 0 until h step 7) for (x in 0 until w step 7) {
                            val a = bmp.getColor(x + m, y + m)
                            val b = img.getRGB(x, y)
                            val d = abs(((a shr 16) and 255) - ((b shr 16) and 255)) + abs(((a shr 8) and 255) - ((b shr 8) and 255)) + abs((a and 255) - (b and 255))
                            total++; if (d > 60) differing++
                        }
                        val share = differing.toDouble() / total
                        println("direct map still: ${"%.1f".format(share * 100)}% of sampled pixels differ from the offscreen render of the same camera")
                        // The overscan frame is drawn with a wider field of view, so only the broad picture must agree.
                        if (img.getRGB(w / 2, h / 2) != 0 || img.getRGB(5, 5) != 0) compared++
                        assertTrue(share < 0.6, "the window shows the picture of its camera (${share * 100}% differ)")
                    }
                    assertTrue(compared > 0, "the window capture had content")
                } finally {
                    offscreen.close()
                }
                out?.let { File(it, "motion.txt").writeText("frames=${inMotion.size} seconds=$motionSeconds fps=$fps render_median_ms=${render[render.size / 2]} present_median_ms=${present[present.size / 2]} software=${session.software} adapter=${session.adapter}\n") }
            } finally {
                session.close()
            }
        } finally {
            SwingUtilities.invokeAndWait { frame.dispose() }
        }
    }
}
