package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.toComposeImageBitmap

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
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
        MeshCanvas.cloudFlow = null
        val only = System.getenv("CW_PERF_SIZES")?.split(',')
        for ((w, h) in listOf(1200 to 800, 2560 to 1440)) if (only == null || "${w}x$h" in only) benchAt(map, w, h, report)
        println(report)
        java.io.File(System.getenv("CW_PERF")).takeIf { it.parentFile?.isDirectory == true }?.writeText(report.toString())
    }

    private fun benchAt(map: WorldMap, w: Int, h: Int, report: StringBuilder) {
        val tag = "${w}x$h"
        val bakeScope = CoroutineScope(Job())
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
        val presets = System.getenv("CW_PERF_PRESETS")?.split(',')
        for (preset in GraphicsPreset.entries) {
            if (presets != null && preset.name !in presets) continue
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
                // The same frame through the baked layers: the UI thread draws the moved bitmaps, then the structures and the overlay.
                val ground = AsyncLayer(bakeScope).apply { paused = true }
                val sky = AsyncLayer(bakeScope).apply { paused = true }
                val space = Color(0xFF05090E)
                fun DrawScope.paintGround(c: MapCamera, geo: MapGeometryCache) = drawWorldMap(
                    map, c, Palette.gold, shading, "MI-DET", divisions, measurer, geometry = geo, graphics = g, sun = sun, phase = MapPhase.Ground,
                )
                fun DrawScope.paintClouds(c: MapCamera, geo: MapGeometryCache) {
                    val gg = geo.of(map, c, size, g.detail, g.pitch)
                    with(Weather) { drawWeather(gg.p, sun, 0.0, g.clouds, map.playBounds, 46.0 * gg.scale * gg.p.t, c.zoom, g.lighting > 0, map.scenarioId) }
                }
                val bakeGround = time(runs(6)) { ground.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo -> paintGround(cam, geo) } }
                val bakeSky = if (g.clouds > 0) time(runs(6)) { sky.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo -> paintClouds(cam, geo) } } else 0.0
                // The same clouds through the runtime shader, in one pass.
                MeshCanvas.cloudFlow = desktopCloudFlow
                val bakeShader = if (g.clouds > 0) time(runs(6)) { sky.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo -> paintClouds(cam, geo) } } else 0.0
                val cloudsShader = time(runs(10)) { i -> frame { with(Weather) { drawWeather(geo.p, sun, i / 8.0, g.clouds, map.playBounds, 46.0 * geo.scale * geo.p.t, cam.zoom, g.lighting > 0, map.scenarioId) } } }
                MeshCanvas.cloudFlow = null
                val uiCache = MapGeometryCache()
                val ui = time(runs(10)) { i ->
                    val c = cam.copy(lon = cam.lon + i * 0.001)
                    frame {
                        with(ground) { drawMoved(ground.baked!!, c, g.pitch, space) }
                        if (g.clouds > 0) with(sky) { drawMoved(sky.baked!!, c, g.pitch, null) }
                        drawWorldMap(map, c, Palette.gold, shading, "MI-DET", divisions, measurer, geometry = uiCache, graphics = g, sun = sun, phase = MapPhase.Structures)
                        val gg = uiCache.of(map, c, size, g.detail, g.pitch)
                        val o = gg.overlay(emptyList(), divisions, "MI-DET", shading, g, false)
                        drawMapOverlay(o, g, i / 60.0, false, sun, map.playBounds, c.zoom, 0.3, 1f)
                    }
                }
                // Where the UI frame goes: each piece alone, same camera.
                val cm = cam.copy(lon = cam.lon + 0.002)
                val tGround = time(runs(10)) { frame { with(ground) { drawMoved(ground.baked!!, cm, g.pitch, space) } } }
                val tSky = if (g.clouds > 0) time(runs(10)) { frame { with(sky) { drawMoved(sky.baked!!, cm, g.pitch, null) } } } else 0.0
                val tStructures = time(runs(10)) { i -> frame { drawWorldMap(map, cam.copy(lon = cam.lon + i * 0.001), Palette.gold, shading, "MI-DET", divisions, measurer, geometry = uiCache, graphics = g, sun = sun, phase = MapPhase.Structures) } }
                val tOverlay = time(runs(10)) { i ->
                    frame {
                        val gg = uiCache.of(map, cam, size, g.detail, g.pitch)
                        val o = gg.overlay(emptyList(), divisions, "MI-DET", shading, g, false)
                        drawMapOverlay(o, g, i / 60.0, false, sun, map.playBounds, cam.zoom, 0.3, 1f)
                    }
                }
                if (name == "close") for ((label, gv) in linkedMapOf("structures" to g, "no countryside" to g.copy(countryside = false), "no buildings" to g.copy(buildings = 0), "no rivals" to g.copy(rivals = false), "no labels" to g.copy(labels = 0), "no citylights" to g.copy(cityLights = false), "no lighting" to g.copy(lighting = 0))) {
                    val sc = MapGeometryCache()
                    val t = time(runs(10)) { i -> frame { drawWorldMap(map, cam.copy(lon = cam.lon + i * 0.001), Palette.gold, shading, "MI-DET", divisions, measurer, geometry = sc, graphics = gv, sun = sun, phase = MapPhase.Structures) } }
                    report.append("STRUCT $tag ${preset.name.padEnd(6)} close ${label.padEnd(15)} %6.1f\n".format(t))
                }
                report.append("UIPART $tag ${preset.name.padEnd(6)} ${name.padEnd(8)} ground blit %6.1f  cloud blit %6.1f  structures %6.1f  overlay %6.1f\n".format(tGround, tSky, tStructures, tOverlay))
                report.append("BAKED $tag ${preset.name.padEnd(6)} ${name.padEnd(8)} ui-frame %6.1f (legacy %6.1f)  bake ground %6.1f  bake clouds %6.1f  bake clouds (shader) %6.1f  clouds frame mesh %6.1f shader %6.1f\n".format(ui, pan + overlay + clouds, bakeGround, bakeSky, bakeShader, clouds, cloudsShader))
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
        // The menu as it is drawn now (globe and clouds) against the baked layers.
        val menuGraphics = GraphicsConfig.Default.copy(detail = MapDetail.Low, extrude = false, buildings = 0, traffic = 0)
        val menuCam = MapCamera(-82.0, 24.0, 1.25)
        val menuSun = Sun.side(MapProjection(menuCam, size))
        val menuLegacy = time(runs(10)) { i ->
            frame {
                val c = MapCamera(-82.0 + i * 0.04, 24.0, 1.25)
                drawWorldMap(map, c, Palette.gold, emptyMap(), null, emptyList(), measurer, space = false, geometry = menuCache, graphics = menuGraphics, sun = menuSun)
                with(Weather) { drawWeather(MapProjection(c, size), menuSun, i / 3.0, menuGraphics.clouds, map.playBounds, 0.0, 1.0, false) }
            }
        }
        val globeLayer = AsyncLayer(bakeScope).apply { paused = true }
        val skyLayer = AsyncLayer(bakeScope).apply { paused = true }
        val bakeGlobe = time(runs(6)) { globeLayer.bakeNow(menuCam, 0.0, null, size, Density(1f)) { geo -> drawWorldMap(map, menuCam, Palette.gold, emptyMap(), null, emptyList(), measurer, space = false, geometry = geo, graphics = menuGraphics, sun = menuSun) } }
        val bakeSky = time(runs(6)) { skyLayer.bakeNow(menuCam, 0.0, null, size, Density(1f)) { with(Weather) { drawWeather(MapProjection(menuCam, size), menuSun, 0.0, menuGraphics.clouds, map.playBounds, 0.0, 1.0, false) } } }
        val menuBaked = time(runs(20)) { i ->
            frame {
                val c = MapCamera(-82.0 + i * 0.04, 24.0, 1.25)
                with(globeLayer) { drawMoved(globeLayer.baked!!, c, 0.0, null) }
                with(skyLayer) { drawMoved(skyLayer.baked!!, c, 0.0, null) }
            }
        }
        report.append("BAKED $tag menu globe+clouds ui-frame %6.1f (legacy %6.1f)  bake globe %6.1f  bake clouds %6.1f\n".format(menuBaked, menuLegacy, bakeGlobe, bakeSky))
        report.append("HOST load after $tag ${java.io.File("/proc/loadavg").takeIf { it.exists() }?.readText()?.trim() ?: "n/a"}\n")
    }
}
