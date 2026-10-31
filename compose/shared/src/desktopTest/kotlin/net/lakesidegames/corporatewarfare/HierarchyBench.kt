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
import kotlin.test.Test

/**
 * Close-zoom frame costs at High with rivals, cities, labels and lanes, in milliseconds. Run with CW_HIER_BENCH=<file>.
 * Written to compile against the code before and after the readable-close-zoom change: "direct" is the structures
 * painted on the UI thread (what the UI paid before baking), "bake" is the same painted off the UI thread, "blit" is
 * what the UI thread pays once it is baked.
 */
class HierarchyBench {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    @Test
    fun bench() {
        val out = System.getenv("CW_HIER_BENCH") ?: return
        val map = WorldMap.shared!!
        val report = StringBuilder("HOST load ${java.io.File("/proc/loadavg").takeIf { it.exists() }?.readText()?.trim() ?: "n/a"}\n")
        for ((w, h) in listOf(1200 to 800, 2560 to 1440)) {
            val size = Size(w.toFloat(), h.toFloat())
            val surface = org.jetbrains.skia.Surface.makeRasterN32Premul(w, h)
            val canvas = surface.canvas.asComposeCanvas()
            val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 256)
            val scope = CanvasDrawScope()
            fun frame(block: DrawScope.() -> Unit) = scope.draw(Density(1f), LayoutDirection.Ltr, canvas, size, block)
            fun time(runs: Int, block: (Int) -> Unit): Double {
                repeat(3) { block(it) }
                val start = System.nanoTime()
                repeat(runs) { block(it) }
                return (System.nanoTime() - start) / 1e6 / runs
            }
            val g = GraphicsPreset.High.config(true, false)
            val sun = Sun.moving(40.0)
            val scenario = MapCamera.scenario(size, map)
            val bakeScope = CoroutineScope(Job())
            val big = w > 2000
            val runs = if (big) 6 else 14
            for ((name, cam) in linkedMapOf("close" to scenario.zoomed(3.0), "ultra" to scenario.copy(lon = -83.05, lat = 42.33).zoomed(14.0))) {
                fun DrawScope.paint(c: MapCamera, geo: MapGeometryCache, phase: MapPhase) = drawWorldMap(
                    map, c, Palette.gold, HierarchyScene.shading, "MI-DET", HierarchyScene.divisions, measurer, geometry = geo, graphics = g, sun = sun,
                    phase = phase, plants = HierarchyScene.plants, lanes = HierarchyScene.lanes, freightLayer = false,
                )
                val cache = MapGeometryCache()
                val direct = time(runs) { i -> frame { paint(cam.copy(lon = cam.lon + i * 0.001), cache, MapPhase.Structures) } }
                val overlayCache = MapGeometryCache()
                // Vehicle pictures are painted in the background after first sight: let a few frames ask, then wait for them.
                repeat(40) { i ->
                    frame {
                        val geo = overlayCache.of(map, cam, size, g.detail, g.pitch)
                        val o = geo.overlay(HierarchyScene.lanes, HierarchyScene.divisions, "MI-DET", HierarchyScene.shading, g, false)
                        drawMapOverlay(o, g, 5.0 + i / 30.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f, map.scenarioId, part = OverlayPart.Traffic)
                    }
                }
                VehicleSprites.settle()
                val traffic = time(runs * 2) { i ->
                    frame {
                        val geo = overlayCache.of(map, cam, size, g.detail, g.pitch)
                        val o = geo.overlay(HierarchyScene.lanes, HierarchyScene.divisions, "MI-DET", HierarchyScene.shading, g, false)
                        drawMapOverlay(o, g, 5.0 + i / 30.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f, map.scenarioId, part = OverlayPart.Traffic)
                    }
                }
                val effects = time(runs * 2) { i ->
                    frame {
                        val geo = overlayCache.of(map, cam, size, g.detail, g.pitch)
                        val o = geo.overlay(HierarchyScene.lanes, HierarchyScene.divisions, "MI-DET", HierarchyScene.shading, g, false)
                        drawMapOverlay(o, g, 5.0 + i / 30.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f, map.scenarioId, part = OverlayPart.Effects)
                    }
                }
                val layer = AsyncLayer(bakeScope).apply { paused = true }
                val bake = time(runs) { layer.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo -> paint(cam, geo, MapPhase.Structures) } }
                val moved = cam.copy(lon = cam.lon + 0.002)
                val blit = time(runs * 2) { frame { with(layer) { drawMoved(layer.baked!!, moved, g.pitch, null) } } }
                report.append(
                    "HIER ${w}x$h $name  structures direct %6.1f  traffic %6.1f  effects %6.1f  UI before (direct+traffic+effects) %6.1f  |  bake (background) %6.1f  blit %5.1f  UI after (blit+traffic+effects) %6.1f\n"
                        .format(direct, traffic, effects, direct + traffic + effects, bake, blit, blit + traffic + effects),
                )
            }
        }
        report.append("HOST load after ${java.io.File("/proc/loadavg").takeIf { it.exists() }?.readText()?.trim() ?: "n/a"}\n")
        println(report)
        java.io.File(out).writeText(report.toString())
    }
}
