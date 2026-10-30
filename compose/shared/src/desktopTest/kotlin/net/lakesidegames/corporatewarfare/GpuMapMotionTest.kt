package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.remember
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import org.jetbrains.skia.Paint
import org.jetbrains.skia.Surface
import java.io.File
import kotlin.math.atan
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin
import kotlin.math.tan
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The new map in motion. A fake renderer with an exact pinhole projection proves the warp of a finished frame to the live camera
 * lands every ground point where the live camera puts it (pan, zoom, turn and tilt), and the real renderer (the software adapter
 * where the machine has no card) proves the same on real frames, that no pixel of the view is ever the dark clear colour, that the
 * names use the camera of the picture drawn, and that a click on a plant's box finds its division.
 */
class GpuMapMotionTest {
    private val ink = 0xFF0B1622.toInt()

    /** A flat ground seen by the same camera model the renderer uses; frames are dots at known ground points. */
    private class FakeSession : GpuMapSession {
        override val adapter = "fake"
        override val software = true
        override val renderMs = 0.0
        override val copyMs = 0.0
        override val version = 0L
        override var shown: GpuShown? = null
        override val frame: ImageBitmap? get() = shown?.image
        override val frameCamera: GpuCamera? get() = shown?.camera
        override fun update(json: String) {}
        override fun request(camera: GpuCamera, width: Int, height: Int, moving: Boolean) {}
        override fun buildings(): List<GpuBuilding> = emptyList()
        override fun buildingRects(camera: GpuCamera, width: Int, height: Int) = FloatArray(0)
        override fun pick(camera: GpuCamera, width: Int, height: Int, x: Float, y: Float): String? = null
        override fun close() {}
        override fun project(camera: GpuCamera, width: Int, height: Int, lonLat: DoubleArray): FloatArray {
            val out = FloatArray(lonLat.size)
            for (k in 0 until lonLat.size / 2) {
                val p = projectOne(camera, width, height, lonLat[2 * k], lonLat[2 * k + 1])
                out[2 * k] = p?.first ?: Float.NaN; out[2 * k + 1] = p?.second ?: Float.NaN
            }
            return out
        }
    }

    private companion object {
        fun projectOne(c: GpuCamera, w: Int, h: Int, lon: Double, lat: Double): Pair<Float, Float>? {
            val east = (lon - c.lon) * 111.32 * cos(Math.toRadians(c.lat))
            val north = (lat - c.lat) * 111.0
            val hd = Math.toRadians(c.headingDeg); val t = Math.toRadians(c.tiltDeg.coerceAtLeast(1.0))
            val tanHalf = tan(Math.toRadians(c.fovDeg / 2)); val aspect = w.toDouble() / h
            val dist = (c.widthKm / 2) / (tanHalf * aspect)
            val fx = sin(hd); val fy = cos(hd)
            val along = east * fx + north * fy + dist * sin(t)
            val across = east * cos(hd) - north * sin(hd)
            val rz = -dist * cos(t)
            val depth = along * sin(t) - rz * cos(t)
            if (depth <= 0.01 * dist) return null
            val up = along * cos(t) + rz * sin(t)
            val nx = across / (depth * tanHalf * aspect); val ny = up / (depth * tanHalf)
            return Pair(((nx * 0.5 + 0.5) * w).toFloat(), ((0.5 - ny * 0.5) * h).toFloat())
        }
    }

    private fun scene(w: Int, h: Int, draw: androidx.compose.ui.graphics.drawscope.DrawScope.() -> Unit): Bitmap {
        val s = ImageComposeScene(w, h, Density(1f)) { Canvas(Modifier.fillMaxSize()) { draw() } }
        val image = s.render()
        s.close()
        return Bitmap.makeFromImage(image)
    }

    /** A frame for [view] with the overscan margin, red 5 pixel dots on a green field at a grid of ground points. */
    private fun dotFrame(s: FakeSession, view: GpuCamera, w: Int, h: Int, factor: Float, grid: List<Pair<Double, Double>>) {
        val mx = Math.round(w * GPU_OVERSCAN).toInt(); val my = Math.round(h * GPU_OVERSCAN).toInt()
        val cam = overscanCamera(view, w, h, mx, my)
        val rw = Math.round((w + 2 * mx) * factor); val rh = Math.round((h + 2 * my) * factor)
        val surface = Surface.makeRasterN32Premul(rw, rh)
        surface.canvas.clear(0xFF2E8B57.toInt())
        val paint = Paint().apply { color = 0xFFFF0000.toInt() }
        for ((lon, lat) in grid) {
            val p = projectOne(cam, rw, rh, lon, lat) ?: continue
            surface.canvas.drawCircle(p.first, p.second, 2.5f * factor.coerceAtLeast(1f), paint)
        }
        s.shown = GpuShown(surface.makeImageSnapshot().toComposeImageBitmap(), cam, view, w, h, mx, factor)
    }

    private fun reddish(bmp: Bitmap, x: Int, y: Int): Boolean {
        if (x < 0 || y < 0 || x >= bmp.width || y >= bmp.height) return false
        val c = bmp.getColor(x, y)
        return ((c shr 16) and 255) > ((c shr 8) and 255) + 70
    }

    /** Where the red dots drawn near (px, py) centre: the mean of the strongly red pixels within [r] pixels. */
    private fun dotCentre(bmp: Bitmap, px: Float, py: Float, r: Int): Pair<Float, Float>? {
        var n = 0; var sx = 0.0; var sy = 0.0
        for (y in (py.toInt() - r)..(py.toInt() + r)) for (x in (px.toInt() - r)..(px.toInt() + r)) {
            if (reddish(bmp, x, y)) { n++; sx += x + 0.5; sy += y + 0.5 }
        }
        return if (n < 3) null else Pair((sx / n).toFloat(), (sy / n).toFloat())
    }

    @Test
    fun aFinishedFrameCarriedToTheLiveCameraLandsEveryGroundPointWhereTheLiveCameraPutsIt() {
        val w = 800; val h = 500
        val base = GpuCamera(-82.0, 41.9, 1200.0, 28.0, 10.0)
        val grid = (-6..6).flatMap { i -> (-6..6).map { j -> Pair(base.lon + i * 0.55, base.lat + j * 0.4) } }
        val lives = listOf(
            "same view" to base,
            "dragged east" to base.copy(lon = base.lon + 1.4),
            "dragged north" to base.copy(lat = base.lat + 0.9),
            "zoomed out" to base.copy(widthKm = base.widthKm * 1.4),
            "zoomed in" to base.copy(widthKm = base.widthKm * 0.8),
            "turned" to base.copy(headingDeg = 22.0),
            "tilted" to base.copy(tiltDeg = 40.0),
            "all at once" to base.copy(lon = base.lon - 0.8, lat = base.lat + 0.5, widthKm = base.widthKm * 1.25, headingDeg = -15.0, tiltDeg = 35.0),
        )
        for (factor in listOf(1f, 0.75f)) {
            val fake = FakeSession()
            dotFrame(fake, base, w, h, factor, grid)
            for ((name, live) in lives) {
                var camera: GpuCamera? = null
                val bmp = scene(w, h) { camera = drawGpuFrame(fake, live) }
                assertEquals(live, camera, "$name: names and badges must use the live camera")
                var checked = 0
                for ((lon, lat) in grid) {
                    val want = projectOne(live, w, h, lon, lat) ?: continue
                    if (want.first < 12 || want.second < 12 || want.first > w - 12 || want.second > h - 12) continue
                    // Skip points whose dot was cut off or merged: only dots that were in the frame.
                    val inFrame = projectOne(fake.shown!!.camera, fake.shown!!.image.width, fake.shown!!.image.height, lon, lat)
                        ?.let { it.first in 8f..(fake.shown!!.image.width - 8f) && it.second in 8f..(fake.shown!!.image.height - 8f) } ?: false
                    if (!inFrame) continue
                    val got = assertNotNull(dotCentre(bmp, want.first, want.second, 9), "$name factor $factor: no dot near $want")
                    val off = hypot((got.first - want.first).toDouble(), (got.second - want.second).toDouble())
                    assertTrue(off <= 2.0, "$name factor $factor: ground point ($lon, $lat) is ${"%.2f".format(off)} px from where the live camera puts it")
                    checked++
                }
                assertTrue(checked > 20, "$name: only $checked dots could be checked")
            }
        }
    }

    @Test
    fun anOverscannedFrameShowsTheSameGroundAsAPlainOne() {
        val c = GpuCamera(-82.0, 41.9, 1500.0, 30.0, 15.0)
        val w = 800; val h = 500
        val mx = 160; val my = 100
        val big = overscanCamera(c, w, h, mx, my)
        val bigW = w + 2 * mx; val bigH = h + 2 * my
        for ((lon, lat) in listOf(Pair(-82.0, 41.9), Pair(-80.5, 42.6), Pair(-84.0, 40.7), Pair(-81.0, 41.0))) {
            val a = projectOne(c, w, h, lon, lat)!!
            val b = projectOne(big, bigW, bigH, lon, lat)!!
            assertTrue(hypot((a.first + mx - b.first).toDouble(), (a.second + my - b.second).toDouble()) < 0.05, "the margin must only extend the view: $a vs $b")
        }
    }

    private fun career(scenario: String) = uniffi.cw_ffi.SessionHandle.start(
        uniffi.cw_ffi.NewGameDto(
            seed = 7uL, scenarioId = scenario, guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
            startingCapitalCents = null, pricePath = "historical", timeline = "historical",
        ),
    )

    private fun waitFor(session: GpuMapSession, ok: (GpuShown?) -> Boolean): GpuShown {
        val deadline = System.currentTimeMillis() + 180_000
        while (!ok(session.shown) && System.currentTimeMillis() < deadline) Thread.sleep(20)
        return assertNotNull(session.shown?.takeIf(ok), "the frame did not arrive")
    }

    @Test
    fun aDragAndAZoomOutOnTheRealRendererNeverJumpOrGoBlack() {
        GpuMapHost.opener = ::openDesktopGpuMap
        val id = "great-lakes-2000"
        val session = try { openDesktopGpuMap(id) } catch (e: Throwable) { println("no adapter, motion test skipped: ${e.message}"); return }
        val out = System.getenv("CW_GPU_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val timings = StringBuilder("| tick | phase | frame | image | render ms | copy ms | stale ticks |\n|---|---|---|---|---|---|---|\n")
        try {
            val game = career(id)
            val plants = game.mapPlants(); val divisions = game.mapDivisions()
            session.update(gpuUpdateJson(plants, divisions, MapLayer.Presence, emptyMap(), night = false, clouds = 2))
            val map = assertNotNull(WorldMap.forScenario(id))
            val w = 960; val h = 600
            val start = GpuCamera(-82.0, 41.9, 2250.0, 24.0, 0.0)
            val kmLon = 111.32 * cos(Math.toRadians(start.lat))
            // 15 ticks of dragging east, then 15 of pulling back while turning.
            val path = (0 until 30).map { i ->
                if (i < 15) start.copy(lon = start.lon + i * 0.03 * start.widthKm / kmLon)
                else {
                    val k = i - 14
                    start.copy(lon = start.lon + 14 * 0.03 * start.widthKm / kmLon, widthKm = start.widthKm * Math.pow(1.06, k.toDouble()), headingDeg = 2.0 * k)
                }
            }
            val saved = setOf(2, 5, 8, 11, 13, 14, 17, 20, 23, 26, 28, 29)
            var shownFor = path[0]
            for (i in path.indices) {
                val live = path[i]
                // A new frame is asked for every third tick (a software card is slow), so the ticks between draw a stale one.
                if (i % 3 == 0) {
                    session.request(live, w, h, true)
                    waitFor(session) { it?.view == live }
                    shownFor = live
                }
                val shown = assertNotNull(session.shown)
                val stale = i - path.indexOf(shown.view)
                var drawn: GpuCamera? = null
                val bmp = scene(w, h) { drawn = drawGpuFrame(session, live) }
                assertEquals(live, drawn, "tick $i: the picture was carried to the live camera, so names must use it")
                // The old clear colour filled the empty edges of the view; a few pixels of it may still be ground (outlines), a void is not.
                var dark = 0; var total = 0
                for (y in 0 until h step 3) for (x in 0 until w step 3) { total++; if (bmp.getColor(x, y) == ink) dark++ }
                assertTrue(dark * 200 < total, "tick $i: $dark of $total sampled pixels of the view are the dark clear colour")
                // A known ground point: the live camera's own centre, and one east of it, land within 2 px where the warp puts them.
                val kmPoint = doubleArrayOf(live.lon, live.lat, live.lon + 0.1 * live.widthKm / kmLon, live.lat + 0.05 * live.widthKm / 111.0)
                val want = session.project(live, w, h, kmPoint)
                val hom = assertNotNull(groundHomography(session::project, shown.camera, shown.image.width, shown.image.height, live, w, h), "tick $i: no warp")
                val src = session.project(shown.camera, shown.image.width, shown.image.height, kmPoint)
                for (k in 0 until 2) {
                    val (gx, gy) = applyHomography(hom, src[2 * k].toDouble(), src[2 * k + 1].toDouble())
                    val off = hypot(gx - want[2 * k], gy - want[2 * k + 1])
                    assertTrue(off <= 2.0, "tick $i point $k: ${"%.2f".format(off)} px from where the live camera puts it")
                }
                timings.append("| $i | ${if (i < 15) "drag" else "zoom out"} | ${live.widthKm.toInt()} km | ${shown.image.width}x${shown.image.height} | ${"%.1f".format(session.renderMs)} | ${"%.1f".format(session.copyMs)} | ${if (stale < 0) 0 else stale} |\n")
                if (out != null && i in saved) {
                    val labelled = ImageComposeScene(w, h, Density(1f)) {
                        val measurer = rememberTextMeasurer()
                        val planner = remember { LabelPlanner() }
                        Canvas(Modifier.fillMaxSize()) {
                            val cam = drawGpuFrame(session, live) ?: live
                            drawGpuLabels(session, cam, map, plants, divisions, GraphicsConfig.Default, cam.widthKm, measurer, planner)
                        }
                    }
                    val png = labelled.render().encodeToData(EncodedImageFormat.PNG)!!.bytes
                    labelled.close()
                    val name = "${if (i < 15) "drag" else "zoomout"}-%02d".format(i)
                    File(out, "$name.png").writeBytes(png)
                    File(out, "$name-phone.jpg").writeBytes(
                        Image.makeFromEncoded(png).let { img ->
                            val sf = Surface.makeRasterN32Premul(600, 375)
                            sf.canvas.drawImageRect(img, org.jetbrains.skia.Rect.makeWH(600f, 375f))
                            sf.makeImageSnapshot().encodeToData(EncodedImageFormat.JPEG, 82)!!.bytes
                        },
                    )
                }
            }
            // Once the view is still, the frame settles at the real pixel size of the view.
            val last = path.last()
            session.request(last, w, h, false)
            val settled = waitFor(session) { it?.view == last && it.factor == 1f }
            assertEquals(w + 2 * settled.margin, settled.image.width, "the settled frame is the view plus its margin at real pixel size")
            println(timings)
            println("map line: ${gpuMapLine(true, session)}")
            if (out != null) File(out, "motion-timings.md").writeText(timings.toString())
        } finally {
            session.close()
        }
    }

    @Test
    fun aClickOnAPlantsBoxFindsThatPlantsDivision() {
        GpuMapHost.opener = ::openDesktopGpuMap
        val id = "great-lakes-2000"
        val session = try { openDesktopGpuMap(id) } catch (e: Throwable) { println("no adapter, click test skipped: ${e.message}"); return }
        try {
            val game = career(id)
            val plants = game.mapPlants(); val divisions = game.mapDivisions()
            session.update(gpuUpdateJson(plants, divisions, MapLayer.Presence, emptyMap(), night = false, clouds = 2))
            val buildings = session.buildings()
            val ours = buildings.filter { !it.scenery }
            assertTrue(ours.isNotEmpty(), "the career has plant buildings")
            var identified = 0
            for (b in ours) {
                if (b.region == null) continue
                assertTrue(
                    plants.any { it.firm.toLong() == b.firm && it.region == b.region && it.industry == b.industry && it.player == b.player },
                    "a building says it is ${b.industry} of firm ${b.firm} in ${b.region}, which is no plant of the game",
                )
                identified++
                val cam = GpuCamera(b.lon, b.lat, 60.0, 45.0, 20.0)
                val rects = session.buildingRects(cam, 1200, 800)
                val i = buildings.indexOf(b)
                val cx = (rects[4 * i] + rects[4 * i + 2]) / 2; val cy = (rects[4 * i + 1] + rects[4 * i + 3]) / 2
                val hit = assertNotNull(gpuBuildingAt(session, cam, 1200, 800, cx, cy), "a click in the middle of a plant's box finds a plant")
                assertTrue(hit.region == b.region && hit.industry == b.industry && hit.firm == b.firm, "the click finds the plant it is on")
                // Far from every box nothing is found.
                assertEquals(null, gpuBuildingAt(session, cam, 1200, 800, 3f, 3f))
            }
            assertTrue(identified >= ours.size * 3 / 4, "most plant buildings know their division ($identified of ${ours.size})")
            val me = game.overview().primaryFirm!!
            val mine = game.divisions(me)
            val owned = ours.filter { it.player && it.region != null }
            for (b in owned) {
                val division = gpuPlantDivision(b, mine)
                if (mine.any { it.region == b.region && it.industry == b.industry }) assertNotNull(division, "a click on our ${b.industry} plant opens that division")
            }
            println("click test: ${ours.size} plant buildings, $identified identified, ${owned.size} ours, ${mine.size} divisions")
        } finally {
            session.close()
        }
    }
}
