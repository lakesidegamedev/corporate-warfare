package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.toComposeImageBitmap

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.asComposeCanvas
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import uniffi.cw_ffi.MapDivisionDto
import kotlin.test.Test

/**
 * Frame costs of the map and the menu, in milliseconds, for each preset. Run with CW_PERF=1. A
 * "pan" frame moves the camera slightly so everything is projected again; a "still" frame redraws
 * an unchanged view; "overlay" is one frame of moving effects; "menu" is one frame of the menu
 * globe. Software raster, so absolute numbers are higher than on a GPU; compare runs.
 */
class MapPerfBench {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val divisions = listOf(
        MapDivisionDto(1uL, "OH-CEN", "manufacturing", 6u, 6u), MapDivisionDto(1uL, "MI-DET", "energy", 9u, 9u),
        MapDivisionDto(1uL, "PA-WES", "technology", 2u, 2u), MapDivisionDto(1uL, "IL-CHI", "logistics", 4u, 4u),
    )
    private val shading = mapOf("OH-CEN" to 1.0, "MI-DET" to 0.8, "PA-WES" to 0.3, "IL-CHI" to 0.5)

    @Test
    fun bench() {
        if (System.getenv("CW_PERF") == null) return
        val map = WorldMap.shared!!
        val report = StringBuilder()
        report.append("HOST load ${java.io.File("/proc/loadavg").takeIf { it.exists() }?.readText()?.trim() ?: "n/a"}\n")
        for ((w, h) in listOf(1200 to 800, 2560 to 1440)) benchAt(map, w, h, report)
        println(report)
        java.io.File(System.getenv("CW_PERF")).takeIf { it.parentFile?.isDirectory == true }?.writeText(report.toString())
    }

    private fun benchAt(map: WorldMap, w: Int, h: Int, report: StringBuilder) {
        val tag = "${w}x$h"
        val big = w > 2000
        fun runs(n: Int) = if (big) maxOf(3, n / 3) else n
        val size = Size(w.toFloat(), h.toFloat())
        val surface = org.jetbrains.skia.Surface.makeRasterN32Premul(w, h)
        val canvas = surface.canvas.asComposeCanvas()
        val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 256)
        val scope = CanvasDrawScope()
        fun frame(block: androidx.compose.ui.graphics.drawscope.DrawScope.() -> Unit) = scope.draw(Density(1f), LayoutDirection.Ltr, canvas, size, block)
        fun time(runs: Int, block: (Int) -> Unit): Double {
            repeat(3) { block(it) }
            val start = System.nanoTime()
            repeat(runs) { block(it) }
            return (System.nanoTime() - start) / 1e6 / runs
        }
        val scenario = MapCamera.scenario(size, map)
        val stages = mapOf("globe" to scenario.copy(zoom = 1.0), "scenario" to scenario, "close" to scenario.zoomed(3.0))
        val sun = Sun.moving(40.0)
        for (preset in GraphicsPreset.entries) {
            val g = preset.config(true, false)
            for ((name, cam) in stages) {
                val cache = MapGeometryCache()
                val project = time(runs(10)) { i ->
                    val c = cam.copy(lon = cam.lon + i * 0.001)
                    val geo = cache.of(map, c, size, g.detail, g.pitch)
                    geo.land; geo.biomes; geo.provinces; geo.roads; geo.counties; geo.lakes; geo.rivers; geo.interstates; geo.regionPaths
                    geo.raised(shading); geo.night(sun)
                }
                val pan = time(runs(10)) { i ->
                    frame { drawWorldMap(map, cam.copy(lon = cam.lon + i * 0.001), Palette.gold, shading, "MI-DET", divisions, measurer, geometry = cache, graphics = g, sun = sun) }
                }
                val still = time(runs(10)) { frame { drawWorldMap(map, cam, Palette.gold, shading, "MI-DET", divisions, measurer, geometry = cache, graphics = g, sun = sun) } }
                val geo = cache.of(map, cam, size, g.detail, g.pitch)
                val overlay = time(runs(30)) { i ->
                    val o = geo.overlay(emptyList(), divisions, "MI-DET", shading, g, false)
                    frame { drawMapOverlay(o, g, i / 60.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f) }
                }
                val clouds = time(runs(10)) { i -> frame { with(Weather) { drawWeather(geo.p, sun, i / 8.0, g.clouds, map.playBounds, 46.0 * geo.scale * geo.p.t, cam.zoom, g.lighting > 0, map.scenarioId) } } }
                report.append("PERF $tag ${preset.name.padEnd(6)} ${name.padEnd(8)} project %6.1f  pan %6.1f  still %6.1f  overlay %5.1f  clouds %5.1f\n".format(project, pan, still, overlay, clouds))
            }
        }
        // What each effect costs: High with one thing switched off, on a still scenario and globe frame.
        val high = GraphicsPreset.High.config(true, false)
        val variants = linkedMapOf(
            "high" to high, "no lighting" to high.copy(lighting = 0), "lighting 1" to high.copy(lighting = 1),
            "no daynight" to high.copy(dayNight = false), "no extrude" to high.copy(extrude = false), "detail low" to high.copy(detail = MapDetail.Low),
            "no buildings" to high.copy(buildings = 0), "no citylights" to high.copy(cityLights = false), "no traffic" to high.copy(traffic = 0),
        )
        if (!big) for ((label, g) in variants) for ((name, cam) in listOf("scenario" to scenario, "globe" to scenario.copy(zoom = 1.0))) {
            val cache = MapGeometryCache()
            val s = if (g.dayNight) sun else null
            val still = time(runs(8)) { frame { drawWorldMap(map, cam, Palette.gold, shading, "MI-DET", divisions, measurer, geometry = cache, graphics = g, sun = s) } }
            report.append("COST $tag ${label.padEnd(14)} ${name.padEnd(8)} still %6.1f\n".format(still))
        }
        if (!big) for ((name, cam) in listOf("scenario" to scenario, "globe" to scenario.copy(zoom = 1.0))) {
            val cache = MapGeometryCache()
            repeat(3) { frame { drawWorldMap(map, cam, Palette.gold, shading, "MI-DET", divisions, measurer, geometry = cache, graphics = high, sun = sun) } }
            MapProfiler.totals.clear(); MapProfiler.on = true
            repeat(8) { frame { drawWorldMap(map, cam, Palette.gold, shading, "MI-DET", divisions, measurer, geometry = cache, graphics = high, sun = sun) } }
            MapProfiler.on = false
            MapProfiler.totals.forEach { (k, v) -> report.append("SECTION $tag $name ${k.padEnd(18)} %6.1f\n".format(v / 8)) }
        }
        // The menu globe: the camera turns every frame.
        val menuCache = MapGeometryCache()
        val menu = time(runs(20)) { i ->
            frame {
                drawWorldMap(map, MapCamera(-82.0 + i * 0.04, 24.0, 1.25), Palette.gold, emptyMap(), null, emptyList(), measurer, space = false,
                    geometry = menuCache, graphics = GraphicsConfig.Default.copy(detail = MapDetail.Low, extrude = false, buildings = 0, traffic = 0))
            }
        }
        report.append("PERF $tag menu globe frame %6.1f\n".format(menu))
        report.append("HOST load after $tag ${java.io.File("/proc/loadavg").takeIf { it.exists() }?.readText()?.trim() ?: "n/a"}\n")
    }
}
