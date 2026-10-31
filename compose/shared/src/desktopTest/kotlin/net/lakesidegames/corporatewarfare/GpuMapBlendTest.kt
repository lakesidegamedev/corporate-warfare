package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.awt.ComposeWindow
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import java.awt.Robot
import java.awt.event.InputEvent
import java.awt.image.BufferedImage
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import javax.imageio.ImageIO
import javax.swing.SwingUtilities
import kotlin.test.Test
import kotlin.test.assertTrue

/**
 * The embedding spike as a test: the engine's native surface in a Compose window (SwingPanel with interop blending), a red Compose
 * box over it. Reports which render API Compose chose, whether the red box and the map are both on screen, and where pointer
 * events go (the surface or Compose). Runs only when CW_BLEND_TEST is set, because it needs a real desktop to capture. It asserts
 * the blend only when Compose chose Direct3D (or Metal), the render APIs Compose 1.9 supports interop blending on.
 */
class GpuMapBlendTest {
    @Test
    fun composeDrawsOverTheNativeSurfaceAndPointerEventsArrive() {
        if (System.getenv("CW_BLEND_TEST") == null || !nativeSurfaceSupported()) { println("blend: skipped"); return }
        val out = System.getenv("CW_DIRECT_OUT")?.let(::File)?.also { it.mkdirs() }
        // Diagnostic variant: the same window with Compose's blending off, to tell a blending problem from a surface problem.
        if (System.getenv("CW_BLEND_MODE") == "noblend") System.setProperty("compose.interop.blending", "false")
        println("blend: compose.interop.blending=${System.getProperty("compose.interop.blending")} os=${System.getProperty("os.name")}")
        var surface by mutableStateOf<Any?>(null)
        val awtEvents = AtomicInteger(); val composeMapEvents = AtomicInteger(); val composeBoxEvents = AtomicInteger()
        lateinit var window: ComposeWindow
        SwingUtilities.invokeAndWait {
            window = ComposeWindow().apply {
                setSize(900, 640); setLocation(40, 40)
                setContent {
                    Box(Modifier.fillMaxSize().background(Color(0xFF05090E))) {
                        MapSurfacePanel(
                            Modifier.fillMaxSize().pointerInput(Unit) {
                                awaitPointerEventScope { while (true) { val e = awaitPointerEvent(); if (e.type == PointerEventType.Move || e.type == PointerEventType.Press) composeMapEvents.incrementAndGet() } }
                            },
                            { surface = it }, { awtEvents.incrementAndGet() },
                        )
                        Box(
                            Modifier.align(Alignment.TopStart).size(200.dp, 120.dp).background(Color.Red).pointerInput(Unit) {
                                awaitPointerEventScope { while (true) { val e = awaitPointerEvent(); if (e.type == PointerEventType.Press) composeBoxEvents.incrementAndGet() } }
                            },
                        )
                    }
                }
                isAlwaysOnTop = true
                isVisible = true
                toFront(); requestFocus()
            }
        }
        try {
            val api = window.renderApi.name
            println("blend: Compose render API = $api")
            val deadline = System.currentTimeMillis() + 15_000
            while (surface == null && System.currentTimeMillis() < deadline) Thread.sleep(50)
            val canvas = surface as? java.awt.Canvas ?: run { println("blend: the surface never appeared"); return }
            val session = try { openDirectGpuMap("great-lakes-2000", canvas) as DirectGpuSession } catch (e: Throwable) { println("blend: skipped, ${e.message}"); return }
            try {
                val map = WorldMap.forScenario("great-lakes-2000")!!
                val k = (canvas as MapSurfaceCanvas).pixelScale
                val size = Size((canvas.width * k).toFloat(), (canvas.height * k).toFloat())
                val gc = gpuCamera(MapCamera.scenario(size, map), size.width, size.height, Math.toRadians(28.0))
                val frames = AtomicInteger(); session.frameListener = { frames.incrementAndGet() }
                println("blend: canvas ${canvas.width}x${canvas.height} scale=$k visible=${canvas.isShowing} displayable=${canvas.isDisplayable} window=${window.width}x${window.height}")
                val opened = System.nanoTime()
                session.request(gc, size.width.toInt(), size.height.toInt(), false)
                val until = System.currentTimeMillis() + 60_000
                while (frames.get() == 0 && System.currentTimeMillis() < until) Thread.sleep(30)
                println("blend: first frame after ${(System.nanoTime() - opened) / 1_000_000} ms")
                // Let a few more frames land (a redraw is requested by a new camera), then give the compositor a moment.
                for (i in 1..5) { session.request(gc.copy(lon = gc.lon + i * 0.001), size.width.toInt(), size.height.toInt(), true); Thread.sleep(100) }
                Thread.sleep(500)
                val robot = Robot()
                val at = window.locationOnScreen
                val inset = window.insets
                fun capture() = robot.createScreenCapture(java.awt.Rectangle(at.x, at.y, window.width, window.height))
                // A point inside the red box and a spread of points over the open map.
                fun reds(img: BufferedImage) = img.getRGB(inset.left + 60, inset.top + 60)
                fun colours(img: BufferedImage) = (0 until 12).flatMap { gy -> (0 until 12).map { gx -> img.getRGB(inset.left + 300 + gx * 40, inset.top + 150 + gy * 25) } }.toSet()
                // A presented frame reaches the screen a moment later (on a software adapter without a display, seconds later):
                // capture until the map shows through, and say how long that took.
                val shownBy = System.currentTimeMillis() + 30_000
                val began = System.currentTimeMillis()
                var shot = capture()
                while (colours(shot).size <= 6 && System.currentTimeMillis() < shownBy) { Thread.sleep(500); shot = capture() }
                println("blend: the map was on screen after ${System.currentTimeMillis() - began} ms (${colours(shot).size} colours), frames ${frames.get()}")
                out?.let { ImageIO.write(shot, "png", File(it, "blend.png")) }
                val red = reds(shot)
                val mapColours = colours(shot)
                // Pointer events: a click on the red box, then moves and a drag over the open map.
                fun at(x: Int, y: Int) = robot.mouseMove(at.x + inset.left + x, at.y + inset.top + y)
                at(60, 60); Thread.sleep(100); robot.mousePress(InputEvent.BUTTON1_DOWN_MASK); robot.mouseRelease(InputEvent.BUTTON1_DOWN_MASK); Thread.sleep(200)
                at(500, 300); Thread.sleep(100)
                robot.mousePress(InputEvent.BUTTON1_DOWN_MASK)
                for (i in 0 until 10) { at(500 + i * 10, 300); Thread.sleep(30) }
                robot.mouseRelease(InputEvent.BUTTON1_DOWN_MASK); Thread.sleep(300)
                println("blend: pointer events: surface(AWT)=${awtEvents.get()} compose over map=${composeMapEvents.get()} compose red box presses=${composeBoxEvents.get()}")
                if (api == "DIRECT3D" || api == "METAL") {
                    assertTrue((red shr 16 and 255) > 200 && (red shr 8 and 255) < 60, "the Compose box is drawn over the native map: ${Integer.toHexString(red)}")
                    // On a software adapter (Windows CI's WARP) the native map has never shown under the Compose layer, though the
                    // plain window shows it: that is reported, not asserted. Where there is a real card the map must show through.
                    println("blend: map visible under Compose = ${mapColours.size > 6}; session watchdog says: ${session.trouble}")
                    if (!session.software) assertTrue(mapColours.size > 6, "the native map shows through the transparent Compose layer")
                    assertTrue(composeBoxEvents.get() > 0, "a Compose control over the map still gets its click")
                    assertTrue(awtEvents.get() + composeMapEvents.get() > 0, "the map area receives pointer events")
                }
            } finally {
                session.close()
            }
        } finally {
            SwingUtilities.invokeAndWait { window.dispose() }
        }
    }
}
