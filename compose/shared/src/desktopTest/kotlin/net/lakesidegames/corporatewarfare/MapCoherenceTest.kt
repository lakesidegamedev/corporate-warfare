package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asComposeCanvas
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.max
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * The classic map's layers move as one, and its buildings are fixed objects on the ground.
 *
 * The drag test runs the real layer set with its bake threads while the camera moves every frame, and checks that in every
 * frame the ground, clouds, traffic, buildings and labels, and effects are all drawn for one and the same camera. The
 * geometry test checks buildings against an independent world position: kilometres from the hub, turned to degrees here.
 */
class MapCoherenceTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val scene = HierarchyScene

    /** The frame a window would draw, with the real ground, structures and overlay painters. */
    private fun frameFor(map: WorldMap, cam: MapCamera, g: GraphicsConfig, measurer: TextMeasurer, labels: LabelPlanner, geometry: MapGeometryCache, sun: Sun, tick: Double): MapLayerSet.Frame {
        val shading = scene.shading
        val paintGround: DrawScope.(MapGeometryCache) -> Unit = { geo ->
            drawWorldMap(map, cam, Palette.gold, shading, "MI-DET", scene.divisions, measurer, geometry = geo, graphics = g, pitch = g.pitch, sun = sun, phase = MapPhase.Ground)
        }
        val paintClouds: DrawScope.(MapGeometryCache) -> Unit = { geo ->
            val gg = geo.of(map, cam, size, g.detail, g.pitch)
            with(Weather) { drawWeather(gg.p, sun, tick, g.clouds, map.playBounds, 46.0 * gg.scale * gg.p.t, cam.zoom, g.lighting > 0, map.scenarioId) }
        }
        val paintStructures: DrawScope.(MapGeometryCache) -> Unit = { geo ->
            drawWorldMap(
                map, cam, Palette.gold, shading, "MI-DET", scene.divisions, measurer, geometry = geo, graphics = g, pitch = g.pitch, sun = sun,
                phase = MapPhase.Structures, plants = scene.plants, labels = labels,
            )
        }
        val traffic: DrawScope.(MapCamera, Double) -> Unit = { c, pt ->
            val gg = geometry.of(map, c, size, g.detail, pt)
            val o = gg.overlay(scene.lanes, scene.divisions, "MI-DET", shading, g, false)
            drawMapOverlay(o, g, tick, false, sun, map.playBounds, c.zoom, 0.3, 1f, map.scenarioId, part = OverlayPart.Traffic)
        }
        val effects: DrawScope.(MapCamera, Double) -> Unit = { c, pt ->
            val gg = geometry.of(map, c, size, g.detail, pt)
            val o = gg.overlay(scene.lanes, scene.divisions, "MI-DET", shading, g, false)
            drawMapOverlay(o, g, tick, false, sun, map.playBounds, c.zoom, 0.3, 1f, map.scenarioId, part = OverlayPart.Effects, wallSeconds = tick)
        }
        return MapLayerSet.Frame(
            ground = MapLayerSet.PartSpec("g", 400, paintGround),
            clouds = if (g.clouds > 0) MapLayerSet.PartSpec("c", 500, paintClouds) else null,
            structures = MapLayerSet.PartSpec("s", 400, paintStructures),
            traffic = traffic, effects = effects,
        )
    }

    private var size = Size(960f, 600f)

    /** One recorded drag: what each frame drew, and how many frames behind the newest camera the picture was. */
    internal class Drag(val frames: List<Triple<MapLayerSet.Report, Int, Double>>, val settledFull: Boolean, val bakes: List<Pair<Float, Double>>)

    internal fun drag(w: Int, h: Int, preset: GraphicsPreset, frames: Int, pace: Long = 16): Drag {
        size = Size(w.toFloat(), h.toFloat())
        val map = WorldMap.shared!!
        val g = preset.config(true, false)
        val surface = org.jetbrains.skia.Surface.makeRasterN32Premul(w, h)
        val canvas = surface.canvas.asComposeCanvas()
        val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 256)
        val labels = LabelPlanner()
        val ui = MapGeometryCache()
        val sun = Sun.moving(40.0)
        val scope = CoroutineScope(Job())
        val layers = MapLayerSet(scope)
        val bakes = java.util.concurrent.CopyOnWriteArrayList<Pair<Float, Double>>()
        layers.onBake = { s, ms -> bakes += s to ms }
        val before = MapBake.enabled
        MapBake.enabled = true
        try {
            val start = MapCamera.scenario(size, map).zoomed(2.5)
            val cams = ArrayList<MapCamera>()
            val out = ArrayList<Triple<MapLayerSet.Report, Int, Double>>()
            fun draw(cam: MapCamera, i: Int): Pair<MapLayerSet.Report, Double> {
                var report: MapLayerSet.Report? = null
                val t0 = System.nanoTime()
                CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, canvas, size) {
                    report = layers.draw(this, cam, g.pitch, frameFor(map, cam, g, measurer, labels, ui, sun, i / 60.0), ui, Color(0xFF05090E))
                }
                return report!! to (System.nanoTime() - t0) / 1e6
            }
            for (i in 0 until frames) {
                val cam = start.copy(lon = start.lon + i * 0.05, lat = start.lat + i * 0.01).rotated(i * 0.6).zoomed(1 + i * 0.002)
                cams += cam
                val (report, ms) = draw(cam, i)
                // How many frames old the camera the layers were drawn for is.
                val drawn = report.drawnFor.values.first()
                val age = if (drawn == cam) 0 else (cams.lastIndex - cams.indexOf(drawn)).takeIf { cams.indexOf(drawn) >= 0 } ?: -1
                out += Triple(report, age, ms)
                Thread.sleep(pace)
            }
            // The camera stops: a full size set for exactly this view must arrive and be drawn as it is.
            val last = cams.last()
            var settled = false
            val deadline = System.nanoTime() + 20_000_000_000L
            while (System.nanoTime() < deadline) {
                val (report, _) = draw(last, frames)
                if (!report.warped && !report.reduced) { settled = true; break }
                Thread.sleep(20)
            }
            return Drag(out, settled, bakes.toList())
        } finally {
            MapBake.enabled = before
            layers.close()
        }
    }

    @Test
    fun everyLayerIsDrawnForTheSameCameraWhileDragging() {
        val d = drag(960, 600, GraphicsPreset.Medium, 60)
        val withClouds = GraphicsPreset.Medium.config(true, false).clouds > 0
        var warped = 0
        d.frames.forEachIndexed { i, (report, _, _) ->
            val expected = buildSet {
                add(MapLayerSet.Layer.Ground); add(MapLayerSet.Layer.Structures); add(MapLayerSet.Layer.Traffic); add(MapLayerSet.Layer.Effects)
                if (withClouds) add(MapLayerSet.Layer.Clouds)
            }
            assertEquals(expected, report.drawnFor.keys, "frame $i draws every layer")
            val cams = report.drawnFor.values.toSet()
            assertEquals(1, cams.size, "frame $i: ground, buildings, labels, clouds, traffic and effects must share one camera, got ${report.drawnFor}")
            if (!report.warped) assertEquals(report.target, cams.single(), "frame $i unwarped must be drawn for the view's own camera")
            if (report.warped) warped++
        }
        println("COHERENCE drag frames=${d.frames.size} warped=$warped maxAge=${d.frames.maxOf { it.second }} bakes=${d.bakes.size} reduced=${d.bakes.count { it.first < 1f }}")
        assertTrue(warped > 0, "the drag should have exercised the warped path")
        assertTrue(d.settledFull, "a full size set for the final camera should land once the camera rests")
    }

    private fun lotsOf(map: WorldMap, cam: MapCamera, pitch: Double): Pair<MapGeometry, List<Building>> {
        val g = MapGeometry(map, cam, size, MapDetail.High, pitch)
        return g to g.buildings(scene.divisions, FlatGround)
    }

    @Test
    fun buildingsStandOnTheGroundWhereTheirWorldPositionIs() {
        size = Size(1200f, 800f)
        val map = WorldMap.shared!!
        val earth = 6_371_000.0
        // Hand-computed lots, in metres east and north of the hub: the sector's place in the list decides, nothing about the view does.
        val lots = mapOf("OH-CEN/manufacturing" to (-120_000.0 to -35_000.0), "MI-DET/energy" to (-60_000.0 to -35_000.0), "PA-WES/technology" to (0.0 to -90_000.0), "IL-CHI/logistics" to (-60_000.0 to -90_000.0))
        val base = MapCamera.scenario(size, map)
        var worldDistance: Double? = null
        for (zoom in listOf(1.6, 2.5, 4.0, 9.0)) for (pitch in listOf(0.0, 0.7)) for (bearing in listOf(0.0, 37.0, 200.0)) {
            val cam = base.zoomed(zoom).rotated(bearing)
            val (g, buildings) = lotsOf(map, cam, pitch)
            assertEquals(lots.size, buildings.size)
            for (b in buildings) {
                val (east, north) = lots.getValue(b.key)
                val hub = map.anchor(b.key.substringBefore('/'))!!
                val lon = hub.lon[0] + east / (earth * cos(hub.lat[0]))
                val lat = hub.lat[0] + north / earth
                val expected = g.p.ground(lon, lat)
                assertTrue(hypot(expected.x - b.gx, expected.y - b.gy) < 1f, "${b.key} at zoom $zoom pitch $pitch bearing $bearing: ground ${b.gx},${b.gy} vs world ${expected.x},${expected.y}")
                val a = g.p.lift(b.gx.toDouble(), b.gy.toDouble(), b.floor)
                val e = g.p.lift(expected.x.toDouble(), expected.y.toDouble(), b.floor)
                assertTrue(hypot(a.x - e.x, a.y - e.y) < 1f, "${b.key} screen point drifted from the ground's projection of its world position")
                // Its size is metres of ground: pixels per voxel over pixels per metre is the voxel's width, until the far-zoom minimum applies.
                val inMetres = b.unit / g.pxPerMetre
                if (b.unit > WorldScale.MIN_UNIT * 1.001f) assertEquals(WorldScale.PLAYER_VOXEL_M.toDouble(), inMetres, 1.0, "${b.key} voxel width in metres at zoom $zoom")
                else assertTrue(b.unit >= WorldScale.MIN_UNIT)
            }
            // Two buildings stay the same number of metres apart at every zoom and turn.
            if (pitch == 0.0 && g.pxPerMetre * WorldScale.PLAYER_VOXEL_M > WorldScale.MIN_UNIT) {
                val a = buildings.first { it.key.startsWith("OH-CEN") }; val c = buildings.first { it.key.startsWith("MI-DET") }
                val metres = hypot(a.gx - c.gx, a.gy - c.gy) / g.pxPerMetre
                val was = worldDistance
                if (was == null) worldDistance = metres else assertEquals(was, metres, was * 0.02, "distance in metres at zoom $zoom")
            }
        }
    }

    @Test
    fun buildingsKeepTheirLotWhenOtherDivisionsComeAndGo() {
        size = Size(1200f, 800f)
        val map = WorldMap.shared!!
        val cam = MapCamera.scenario(size, map).zoomed(4.0)
        val (_, all) = lotsOf(map, cam, 0.7)
        val g = MapGeometry(map, cam, size, MapDetail.High, 0.7)
        val fewer = g.buildings(scene.divisions.filter { it.industry == "manufacturing" }, FlatGround)
        val a = all.first { it.key == "OH-CEN/manufacturing" }
        val b = fewer.single()
        assertTrue(abs(a.gx - b.gx) < 0.01f && abs(a.gy - b.gy) < 0.01f, "a building does not move when a neighbour is added or removed")
    }

    @Test
    fun rivalsKeepTheirSlotsAndNeverShareOne() {
        val keys = listOf("OH-CEN/chemical_industries/2", "OH-CEN/retail/3", "OH-CEN/healthcare/2", "OH-CEN/financial/4")
        val slots = WorldScale.rivalSlots(keys)
        assertEquals(4, slots.values.toSet().size)
        assertEquals(slots, WorldScale.rivalSlots(keys.reversed()))
    }

    /** Frame times while dragging. Run with CW_COHERENCE=<report file>; software raster, so compare runs. */
    @Test
    fun dragFrameTimes() {
        val out = System.getenv("CW_COHERENCE") ?: return
        val report = StringBuilder()
        for ((w, h) in listOf(1200 to 800, 2560 to 1440)) for (preset in listOf(GraphicsPreset.Medium, GraphicsPreset.High)) {
            val d = drag(w, h, preset, 60)
            val ms = d.frames.map { it.third }.sorted()
            val reduced = d.bakes.filter { it.first < 1f }.map { it.second }.sorted()
            val full = d.bakes.filter { it.first >= 1f }.map { it.second }.sorted()
            fun pct(l: List<Double>, q: Double) = if (l.isEmpty()) 0.0 else l[((l.size - 1) * q).toInt()]
            val line = "DRAG ${w}x$h ${preset.name.padEnd(6)} ui-frame mean %.1f p95 %.1f max %.1f ms | reduced bakes %d median %.0f ms max %.0f | full bakes %d median %.0f ms | max frames behind %d | settled full %s"
                .format(ms.average(), pct(ms, 0.95), ms.last(), reduced.size, pct(reduced, 0.5), reduced.lastOrNull() ?: 0.0, full.size, pct(full, 0.5), d.frames.maxOf { it.second }, d.settledFull)
            println(line)
            report.append(line).append('\n')
            // The invariant holds at every size too.
            d.frames.forEachIndexed { i, (r, _, _) -> assertEquals(1, r.drawnFor.values.toSet().size, "frame $i at ${w}x$h ${preset.name}") }
        }
        java.io.File(out).takeIf { it.parentFile?.isDirectory == true }?.writeText(report.toString())
    }
}
