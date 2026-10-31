package net.lakesidegames.corporatewarfare

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
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Font
import org.jetbrains.skia.Image
import org.jetbrains.skia.Paint
import org.jetbrains.skia.Rect
import org.jetbrains.skia.Surface
import java.io.File
import kotlin.math.cos
import kotlin.test.Test

/**
 * Evidence for the drag fix. Run with CW_COH_RENDER=<directory>: an 8 frame strip of a real drag drawn the old way (three
 * independent baked layers, traffic and effects live) and the new way (one set), and one building at four zooms.
 */
class CoherenceRender {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val scene = HierarchyScene
    private val size = Size(1600f, 900f)
    private val g = GraphicsPreset.High.config(true, false)
    private val sun = Sun.moving(40.0)
    private val map = WorldMap.shared!!
    private val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 256)
    private val sectors = firmSectors(scene.plants)

    private fun DrawScope.ground(cam: MapCamera, geo: MapGeometryCache) = drawWorldMap(
        map, cam, Palette.gold, scene.shading, "MI-DET", scene.divisions, measurer, geometry = geo, graphics = g, pitch = g.pitch, sun = sun,
        phase = MapPhase.Ground, lanes = scene.lanes, freightLayer = false,
    )

    private fun DrawScope.clouds(cam: MapCamera, geo: MapGeometryCache) {
        val gg = geo.of(map, cam, size, g.detail, g.pitch)
        with(Weather) { drawWeather(gg.p, sun, 3.0, g.clouds, map.playBounds, 46.0 * gg.scale * gg.p.t, cam.zoom, g.lighting > 0, map.scenarioId) }
    }

    private fun DrawScope.structures(cam: MapCamera, geo: MapGeometryCache, labels: LabelPlanner) = drawWorldMap(
        map, cam, Palette.gold, scene.shading, "MI-DET", scene.divisions, measurer, geometry = geo, graphics = g, pitch = g.pitch, sun = sun,
        phase = MapPhase.Structures, plants = scene.plants, labels = labels, lanes = scene.lanes, freightLayer = false,
    )

    private fun DrawScope.overlay(cam: MapCamera, pitch: Double, ui: MapGeometryCache, part: OverlayPart) {
        val gg = ui.of(map, cam, size, g.detail, pitch)
        val o = gg.overlay(scene.lanes, scene.divisions, "MI-DET", scene.shading, g, false, 1f, sectors)
        drawMapOverlay(o, g, 12.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f, map.scenarioId, part = part, wallSeconds = 12.0)
    }

    private fun png(surface: Surface, file: File) = file.writeBytes(surface.makeImageSnapshot().encodeToData(EncodedImageFormat.PNG)!!.bytes)

    private fun jpeg(surface: Surface, file: File, width: Int) {
        val image = surface.makeImageSnapshot()
        val h = image.height * width / image.width
        val small = Surface.makeRasterN32Premul(width, h)
        small.canvas.drawImageRect(image, Rect.makeWH(width.toFloat(), h.toFloat()))
        file.writeBytes(small.makeImageSnapshot().encodeToData(EncodedImageFormat.JPEG, 82)!!.bytes)
    }

    private fun cameraAt(i: Int): MapCamera {
        val start = MapCamera.scenario(size, map).zoomed(3.0).copy(lon = -83.4, lat = 42.2)
        return start.copy(lon = start.lon + i * 0.16, lat = start.lat + i * 0.03).rotated(i * 0.9)
    }

    private fun contact(frames: List<Surface>, caption: String, file: File) {
        val w = 800; val h = 450
        val sheet = Surface.makeRasterN32Premul(w * 4, h * 2 + 40)
        val c = sheet.canvas
        c.clear(0xFF05090E.toInt())
        frames.forEachIndexed { i, s ->
            c.drawImageRect(s.makeImageSnapshot(), Rect.makeXYWH((i % 4) * w.toFloat(), 40f + (i / 4) * h, w.toFloat(), h.toFloat()))
        }
        val font = Font(null, 22f)
        c.drawString(caption, 12f, 28f, font, Paint().apply { color = 0xFFFFFFFF.toInt() })
        png(sheet, file)
    }

    /** A real drag of 56 frames at 60 per second with the bake threads running; eight frames from the end of it are kept. */
    private fun strip(name: String, dir: File, newWay: Boolean) {
        val before = MapBake.enabled
        MapBake.enabled = true
        val scope = CoroutineScope(Job())
        val ui = MapGeometryCache()
        val labels = LabelPlanner()
        val set = MapLayerSet(scope)
        val groundLayer = AsyncLayer(scope); val cloudLayer = AsyncLayer(scope); val structureLayer = AsyncLayer(scope)
        val kept = ArrayList<Surface>()
        try {
            for (i in 0 until 56) {
                val cam = cameraAt(i)
                val surface = Surface.makeRasterN32Premul(size.width.toInt(), size.height.toInt())
                CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, surface.canvas.asComposeCanvas(), size) {
                    if (newWay) {
                        val frame = MapLayerSet.Frame(
                            ground = MapLayerSet.PartSpec("g", 400) { geo -> ground(cam, geo) },
                            clouds = MapLayerSet.PartSpec("c", 500) { geo -> clouds(cam, geo) },
                            structures = MapLayerSet.PartSpec("s", 400) { geo -> structures(cam, geo, labels) },
                            traffic = { c, p -> overlay(c, p, ui, OverlayPart.Traffic) },
                            effects = { c, p -> overlay(c, p, ui, OverlayPart.Effects) },
                        )
                        set.draw(this, cam, g.pitch, frame, ui, Color(0xFF05090E))
                    } else {
                        // The old pipeline: three layers, each drawn moved from its own baked camera, traffic and effects live.
                        groundLayer.draw(this, cam, g.pitch, "g", 400, Color(0xFF05090E), { geo -> ground(cam, geo) }, { ground(cam, ui) })
                        overlay(cam, g.pitch, ui, OverlayPart.Traffic)
                        cloudLayer.draw(this, cam, g.pitch, "c", 500, null, { geo -> clouds(cam, geo) }, { clouds(cam, ui) })
                        structureLayer.draw(this, cam, g.pitch, "s", 400, null, { geo -> structures(cam, geo, labels) }, { structures(cam, ui, labels) })
                        overlay(cam, g.pitch, ui, OverlayPart.Effects)
                    }
                }
                if (i >= 24 && (i - 24) % 4 == 0 && kept.size < 8) kept += surface
                Thread.sleep(16)
            }
        } finally {
            MapBake.enabled = before
            set.close(); groundLayer.close(); cloudLayer.close(); structureLayer.close()
        }
        kept.forEachIndexed { i, s ->
            png(s, File(dir, "$name-frame${i + 1}.png"))
            jpeg(s, File(dir, "$name-frame${i + 1}-phone.jpg"), 1080)
        }
        contact(kept, if (newWay) "AFTER: one set, every layer carried by the same warp (drag frames 25 to 53, every 4th)" else "BEFORE: three layers each baked for its own camera, traffic live (same drag)", File(dir, "$name-strip.png"))
    }

    private fun zoomStrip(dir: File) {
        // MI-DET energy: its lot, as worked out from the hub in metres.
        val hub = map.anchor("MI-DET")!!
        val (east, north) = LotLand.dry(map, "MI-DET", WorldScale.playerLot("energy"), LotLand.PLAYER_RADIUS_M)
        val lon = hub.lon[0] + east / (WorldScale.EARTH_RADIUS_M * cos(hub.lat[0]))
        val lat = hub.lat[0] + north / WorldScale.EARTH_RADIUS_M
        val scenario = MapCamera.scenario(size, map)
        val panels = ArrayList<Surface>()
        val captions = ArrayList<String>()
        for (factor in listOf(2.0, 3.5, 6.0, 10.0)) {
            val cam = scenario.copy(lon = lon * 180 / Math.PI, lat = lat * 180 / Math.PI).zoomed(factor).rotated(18.0)
            val surface = Surface.makeRasterN32Premul(size.width.toInt(), size.height.toInt())
            val geo = MapGeometryCache()
            val labels = LabelPlanner()
            CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, surface.canvas.asComposeCanvas(), size) {
                ground(cam, geo)
                structures(cam, geo, labels)
            }
            val gg = geo.of(map, cam, size, g.detail, g.pitch)
            captions += "zoom x%.0f  |  1 km = %.2f px  |  voxel %.1f px".format(factor, gg.pxPerMetre * 1000, gg.unit(WorldScale.PLAYER_VOXEL_M))
            panels += surface
        }
        val w = 800; val h = 450
        val sheet = Surface.makeRasterN32Premul(w * 2, h * 2)
        panels.forEachIndexed { i, s ->
            val x = (i % 2) * w.toFloat(); val y = (i / 2) * h.toFloat()
            // Crop the middle half of each view, where the building stands, so the size change is the world's own.
            sheet.canvas.drawImageRect(s.makeImageSnapshot(), Rect.makeXYWH(400f, 225f, 800f, 450f), Rect.makeXYWH(x, y, w.toFloat(), h.toFloat()))
            sheet.canvas.drawString(captions[i], x + 12, y + 28, Font(null, 22f), Paint().apply { color = 0xFFFFFFFF.toInt() })
        }
        png(sheet, File(dir, "zoom-strip.png"))
        jpeg(sheet, File(dir, "zoom-strip-phone.jpg"), 1080)
    }

    @Test
    fun renders() {
        val dir = System.getenv("CW_COH_RENDER")?.let(::File) ?: return
        dir.mkdirs()
        strip("before", dir, newWay = false)
        strip("after", dir, newWay = true)
        zoomStrip(dir)
    }
}
