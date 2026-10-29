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
import org.jetbrains.skia.Surface
import java.io.File
import kotlin.test.Test

/** Writes close, turned and ultra close views at High with rivals, cities, labels and traffic. Run with CW_HIER_RENDER=<directory>. */
class HierarchyRender {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    @Test
    fun renders() {
        val dir = System.getenv("CW_HIER_RENDER")?.let(::File) ?: return
        dir.mkdirs()
        val map = WorldMap.shared!!
        val size = Size(1600f, 900f)
        val g = GraphicsPreset.High.config(true, false)
        val sun = Sun.moving(40.0)
        val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 256)
        val scenario = MapCamera.scenario(size, map)
        val detroit = scenario.copy(lon = -83.05, lat = 42.33)
        val views = linkedMapOf(
            "close" to scenario.zoomed(3.0),
            "turned" to scenario.zoomed(3.0).rotated(38.0),
            "ultra-close" to detroit.zoomed(14.0),
            "ultra-turned" to detroit.zoomed(14.0).rotated(-52.0),
        )
        val sectors = firmSectors(HierarchyScene.plants)
        for ((name, cam) in views) {
            val planner = LabelPlanner()
            fun DrawScope.paint(geo: MapGeometryCache, phase: MapPhase) = drawWorldMap(
                map, cam, Palette.gold, HierarchyScene.shading, "MI-DET", HierarchyScene.divisions, measurer, geometry = geo, graphics = g, sun = sun,
                phase = phase, plants = HierarchyScene.plants, labels = planner, lanes = HierarchyScene.lanes, freightLayer = false,
            )
            val ground = AsyncLayer(CoroutineScope(Job()))
            val structures = AsyncLayer(CoroutineScope(Job()))
            ground.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo -> paint(geo, MapPhase.Ground) }
            structures.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo -> paint(geo, MapPhase.Structures) }
            val surface = Surface.makeRasterN32Premul(size.width.toInt(), size.height.toInt())
            val ui = MapGeometryCache()
            CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, surface.canvas.asComposeCanvas(), size) {
                drawRect(Color(0xFF05090E))
                with(ground) { drawBitmap(ground.baked!!.image) }
                val geo = ui.of(map, cam, size, g.detail, g.pitch)
                val o = geo.overlay(HierarchyScene.lanes, HierarchyScene.divisions, "MI-DET", HierarchyScene.shading, g, false, 1f, sectors)
                drawMapOverlay(o, g, 12.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f, map.scenarioId, part = OverlayPart.Traffic)
                with(structures) { drawBitmap(structures.baked!!.image) }
            }
            File(dir, "after-$name.png").writeBytes(surface.makeImageSnapshot().encodeToData(EncodedImageFormat.PNG)!!.bytes)
        }
    }
}
