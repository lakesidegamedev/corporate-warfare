package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.unit.Density
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.Image
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/**
 * One map: the docked map and the Freight mode draw the 3D map (the software adapter where the machine has no card), the
 * map zoomed out and back in hands over to the globe and back at one zoom with a finished picture on each side, and while
 * it pans and zooms no frame of the 3D map shows a near-black patch. CW_MAP_RENDER_DIR receives the pictures.
 */
@OptIn(ExperimentalComposeUiApi::class)
class OneMapTest {
    init {
        Images.decode = { bytes -> Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }

    private fun store(): Pair<GameStore, AppSettings> {
        val dir = kotlin.io.path.createTempDirectory("cwonemap").toFile()
        val store = GameStore(dir, CoroutineScope(Dispatchers.Default))
        val settings = AppSettings(File(dir, "settings.properties"))
        settings.chooseGraphics(settings.graphics.copy(newMap = true, animations = false, directMap = false))
        store.startCareer(
            uniffi.cw_ffi.NewGameDto(
                seed = 7uL, scenarioId = "great-lakes-2000", guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
                startingCapitalCents = null, pricePath = "historical", timeline = "historical",
            ),
        )
        val deadline = System.currentTimeMillis() + 60_000
        while ((!store.ready || store.mapPlants.isEmpty()) && System.currentTimeMillis() < deadline) Thread.sleep(100)
        assertTrue(store.ready, "the career started")
        return store to settings
    }

    private fun adapter(): Boolean = try {
        openDesktopGpuMap("great-lakes-2000").close(); true
    } catch (e: Throwable) {
        println("no adapter, one map test skipped: ${e.message}"); false
    }

    private class Shot(val scene: ImageComposeScene) {
        var t = 0L
        fun frame(): Bitmap {
            t += 16_000_000L
            return Bitmap.makeFromImage(scene.render(t))
        }
    }

    private fun open(store: GameStore, settings: AppSettings, layer: MapLayer, w: Int, h: Int): Shot {
        val scene = ImageComposeScene(w, h, Density(1f)) {
            CorporateWarfareTheme {
                CompositionLocalProvider(LocalAppSettings provides settings) {
                    DockedMap(store, layer, {}, null, {}) {}
                }
            }
        }
        return Shot(scene)
    }

    /** Frames until the 3D map draws with a finished frame of its own (it opens and builds its scene first). */
    private fun untilGpu(shot: Shot, what: String): Bitmap {
        val deadline = System.currentTimeMillis() + 240_000
        var bmp = shot.frame()
        while (!(MapDrawProbe.lastGpu && MapDrawProbe.lastHadFrame) && System.currentTimeMillis() < deadline) {
            Thread.sleep(100)
            bmp = shot.frame()
        }
        assertTrue(MapDrawProbe.lastGpu, "$what: the 3D map never drew (classic frames ${MapDrawProbe.classicFrames})")
        assertTrue(MapDrawProbe.lastHadFrame, "$what: the 3D map drew without a frame")
        // Let a full-size frame land before the picture.
        repeat(20) { Thread.sleep(100); bmp = shot.frame() }
        return bmp
    }

    /** The darkest 12 by 12 block in the middle of the view (away from the chips, legend and buttons), as its brightest channel's mean. */
    private fun darkestMiddle(bmp: Bitmap): Float {
        var worst = Float.MAX_VALUE
        val x0 = bmp.width / 6; val x1 = bmp.width * 5 / 6; val y0 = bmp.height / 4; val y1 = bmp.height * 3 / 4
        for (by in y0 until y1 - 12 step 12) for (bx in x0 until x1 - 12 step 12) {
            var sum = 0
            for (y in 0 until 12) for (x in 0 until 12) {
                val c = bmp.getColor(bx + x, by + y)
                sum += maxOf((c shr 16) and 255, (c shr 8) and 255, c and 255)
            }
            worst = minOf(worst, sum / 144f)
        }
        return worst
    }

    private fun save(name: String, bmp: Bitmap) {
        val dir = out ?: return
        val png = Image.makeFromBitmap(bmp).encodeToData(EncodedImageFormat.PNG)!!.bytes
        File(dir, "$name.png").writeBytes(png)
    }

    @Test
    fun theDockedMapAndTheFreightModeDrawThe3DMap() {
        GpuMapHost.opener = ::openDesktopGpuMap
        GpuMapHost.surfaceHost = null
        if (!adapter()) return
        val (store, settings) = store()
        for ((name, layer, size) in listOf(
            Triple("docked", MapLayer.Presence, 560 to 760),
            Triple("docked-freight", MapLayer.Freight, 560 to 760),
            Triple("freight", MapLayer.Freight, 1200 to 750),
        )) {
            val shot = open(store, settings, layer, size.first, size.second)
            try {
                val bmp = untilGpu(shot, name)
                val classic = MapDrawProbe.classicFrames
                repeat(5) { shot.frame() }
                assertEquals(classic, MapDrawProbe.classicFrames, "$name: the classic map drew again after the 3D map took over")
                save("polish-after-$name", bmp)
                println("$name: 3D map after ${MapDrawProbe.gpuFrames} frames, zoom ${"%.2f".format(MapDrawProbe.lastZoom)}")
            } finally {
                shot.scene.close()
            }
        }
        // The same docked view on the classic map, as the dock showed it before whenever the 3D map was not used there.
        GpuMapHost.opener = null
        val shot = open(store, settings, MapLayer.Presence, 560, 760)
        try {
            var bmp = shot.frame()
            repeat(30) { Thread.sleep(100); bmp = shot.frame() }
            assertFalse(MapDrawProbe.lastGpu)
            save("polish-before-docked-classic", bmp)
        } finally {
            shot.scene.close()
            GpuMapHost.opener = ::openDesktopGpuMap
        }
    }

    @Test
    fun zoomingOutToTheGlobeAndBackHandsOverAtOneZoomWithAPictureOnEachSide() {
        GpuMapHost.opener = ::openDesktopGpuMap
        GpuMapHost.surfaceHost = null
        if (!adapter()) return
        val (store, settings) = store()
        val w = 560; val h = 760
        val shot = open(store, settings, MapLayer.Presence, w, h)
        try {
            untilGpu(shot, "docked")
            val centre = Offset(w / 2f, h / 2f)
            var darkest = Float.MAX_VALUE
            var crossings = 0
            var wasGpu = true
            val log = StringBuilder()
            fun step(what: String, input: () -> Unit) {
                input()
                val bmp = shot.frame()
                val gpu = MapDrawProbe.lastGpu
                val zoom = MapDrawProbe.lastZoom
                // One map a frame, chosen by the zoom alone: the 3D map from the hand-over in, the globe below it.
                assertEquals(zoom >= MapCamera.GPU_FROM, gpu, "$what: zoom ${"%.3f".format(zoom)} drew the ${if (gpu) "3D map" else "classic map"}")
                if (gpu) {
                    assertTrue(MapDrawProbe.lastHadFrame, "$what: the 3D map took over with no picture of its own")
                    val d = darkestMiddle(bmp)
                    darkest = minOf(darkest, d)
                    assertTrue(d >= 24f, "$what: a near-black patch in the middle of the 3D map (brightest channel mean $d)")
                } else if (wasGpu) {
                    assertFalse(MapDrawProbe.lastWarped, "$what: the globe took over with an old picture carried to the camera")
                }
                if (gpu != wasGpu) crossings++
                wasGpu = gpu
                log.append("$what zoom=${"%.2f".format(zoom)} ${if (gpu) "3D" else "globe"}\n")
            }
            // Out to the globe a notch at a time, then back in, with a drag at each end.
            repeat(18) { k -> step("out $k") { shot.scene.sendPointerEvent(PointerEventType.Scroll, centre, scrollDelta = Offset(0f, 1f)) } }
            repeat(18) { k -> step("in $k") { shot.scene.sendPointerEvent(PointerEventType.Scroll, centre, scrollDelta = Offset(0f, -1f)) } }
            shot.scene.sendPointerEvent(PointerEventType.Press, centre)
            repeat(12) { k ->
                step("drag $k") { shot.scene.sendPointerEvent(PointerEventType.Move, centre + Offset(-30f * (k + 1), 18f * (k + 1))) }
            }
            shot.scene.sendPointerEvent(PointerEventType.Release, centre + Offset(-360f, 216f))
            println(log)
            println("hand-overs: $crossings, darkest middle block of the 3D map: $darkest")
            assertTrue(crossings >= 2, "the zoom went out to the globe and back in ($crossings hand-overs)")
        } finally {
            shot.scene.close()
        }
    }
}
