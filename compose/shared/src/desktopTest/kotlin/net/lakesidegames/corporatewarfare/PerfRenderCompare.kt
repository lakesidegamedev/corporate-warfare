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
import kotlin.math.abs
import kotlin.test.Test

/**
 * Writes the map at High as it was drawn before (everything on the UI thread) and as it is now (baked ground and clouds,
 * then structures), on an opaque background, and the difference between them. Run with CW_PERF_RENDER=<directory>.
 */
class PerfRenderCompare {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    @Test
    fun renders() {
        val dir = System.getenv("CW_PERF_RENDER")?.let(::File) ?: return
        dir.mkdirs()
        val map = WorldMap.shared!!
        val size = Size(1600f, 900f)
        val g = GraphicsPreset.High.config(true, false)
        val sun = Sun.moving(40.0)
        val shading = mapOf("OH-CEN" to 1.0, "MI-DET" to 0.8, "PA-WES" to 0.3, "IL-CHI" to 0.5)
        val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 64)
        val scenario = MapCamera.scenario(size, map)
        val report = StringBuilder()
        for ((name, cam) in mapOf("globe" to scenario.copy(zoom = 1.0), "scenario" to scenario, "close" to scenario.zoomed(3.0))) {
            fun DrawScope.ground(geo: MapGeometryCache, phase: MapPhase) = drawWorldMap(
                map, cam, Palette.gold, shading, "MI-DET", emptyList(), measurer, geometry = geo, graphics = g, sun = sun, phase = phase,
            )
            fun DrawScope.clouds(geo: MapGeometryCache) {
                val gg = geo.of(map, cam, size, g.detail, g.pitch)
                with(Weather) { drawWeather(gg.p, sun, 3.0, g.clouds, map.playBounds, 46.0 * gg.scale * gg.p.t, cam.zoom, g.lighting > 0, map.scenarioId) }
            }
            fun frame(block: DrawScope.() -> Unit): Pair<ByteArray, IntArray> {
                val surface = Surface.makeRasterN32Premul(size.width.toInt(), size.height.toInt())
                CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, surface.canvas.asComposeCanvas(), size) {
                    drawRect(Color(0xFF05090E))
                    block()
                }
                val image = surface.makeImageSnapshot()
                val bitmap = org.jetbrains.skia.Bitmap().apply { allocPixels(image.imageInfo) }
                image.readPixels(bitmap)
                val pixels = bitmap.readPixels()!!
                val ints = IntArray(pixels.size / 4) { i -> (pixels[4 * i + 2].toInt() and 255) or ((pixels[4 * i + 1].toInt() and 255) shl 8) or ((pixels[4 * i].toInt() and 255) shl 16) }
                return image.encodeToData(EncodedImageFormat.PNG)!!.bytes to ints
            }
            // Before: everything on the UI thread, the clouds as meshes.
            MeshCanvas.cloudFlow = null
            val geo = MapGeometryCache()
            val (before, a) = frame { ground(geo, MapPhase.Ground); clouds(geo); ground(geo, MapPhase.Structures) }
            // After: baked layers, the clouds as in the game (meshes).
            val layer = AsyncLayer(CoroutineScope(Job()))
            val sky = AsyncLayer(CoroutineScope(Job()))
            layer.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo2 -> ground(geo2, MapPhase.Ground) }
            sky.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo2 -> clouds(geo2) }
            val ui = MapGeometryCache()
            val (after, b) = frame {
                with(layer) { drawBitmap(layer.baked!!.image) }
                with(sky) { drawBitmap(sky.baked!!.image) }
                ground(ui, MapPhase.Structures)
            }
            // The runtime shader's clouds, for comparison with the meshes.
            MeshCanvas.cloudFlow = desktopCloudFlow
            val shaderSky = AsyncLayer(CoroutineScope(Job()))
            shaderSky.bakeNow(cam, g.pitch, null, size, Density(1f)) { geo2 -> clouds(geo2) }
            val (shaded, c) = frame {
                with(layer) { drawBitmap(layer.baked!!.image) }
                with(shaderSky) { drawBitmap(shaderSky.baked!!.image) }
                ground(ui, MapPhase.Structures)
            }
            MeshCanvas.cloudFlow = null
            File(dir, "shader-$name.png").writeBytes(shaded)
            var shaderTotal = 0L; var shaderWorst = 0
            for (i in a.indices) for (shift in intArrayOf(0, 8, 16)) {
                val d = abs(((a[i] shr shift) and 255) - ((c[i] shr shift) and 255)); shaderTotal += d; shaderWorst = maxOf(shaderWorst, d)
            }
            report.append("$name shader clouds against meshes: mean channel difference %.3f, worst %d of 255\n".format(shaderTotal / (a.size * 3.0), shaderWorst))
            File(dir, "before-$name.png").writeBytes(before)
            File(dir, "after-$name.png").writeBytes(after)
            var total = 0L; var worst = 0
            for (i in a.indices) for (shift in intArrayOf(0, 8, 16)) {
                val d = abs(((a[i] shr shift) and 255) - ((b[i] shr shift) and 255)); total += d; worst = maxOf(worst, d)
            }
            report.append("$name mean channel difference %.3f, worst %d of 255\n".format(total / (a.size * 3.0), worst))
        }
        println(report)
        File(dir, "parity.txt").writeText(report.toString())
    }
}
