package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import androidx.compose.runtime.remember
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The new GPU map inside the game's own map code: a Great Lakes and a Texas career drawn by the
 * renderer (the software adapter where the machine has no card), the labels over it, a tap
 * picking a region, and the frame counter line. Where no adapter exists the test checks the
 * fallback and says so. CW_GPU_RENDER_DIR receives the pictures and the timing table.
 */
class GpuMapRenderTest {
    private fun career(scenario: String) = uniffi.cw_ffi.SessionHandle.start(
        uniffi.cw_ffi.NewGameDto(
            seed = 7uL, scenarioId = scenario, guided = true, fiveYear = false, difficulty = "standard", economy = "historic",
            startingCapitalCents = null, pricePath = "historical", timeline = "historical",
        ),
    )

    @Test
    fun theGameMapDrawsOnTheNewRenderer() {
        GpuMapHost.opener = ::openDesktopGpuMap
        val out = System.getenv("CW_GPU_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val table = StringBuilder("| scenario | view | internal size | render ms | copy ms |\n|---|---|---|---|---|\n")
        var drew = false
        for (id in listOf("great-lakes-2000", "texas-1981")) {
            val session = try {
                GpuMapHost.failure = null
                openDesktopGpuMap(id)
            } catch (e: Throwable) {
                // No adapter: the game keeps the classic map and the counter says so.
                GpuMapHost.failure = e.message ?: "none"
                assertEquals("map: classic, no GPU", gpuMapLine(true, null))
                continue
            }
            try {
                drew = true
                val game = career(id)
                val plants = game.mapPlants()
                val divisions = game.mapDivisions()
                assertTrue(plants.isNotEmpty(), "$id: the career has plants")
                session.update(gpuUpdateJson(plants, divisions, MapLayer.Presence, emptyMap(), night = false, clouds = 2))
                val map = assertNotNull(WorldMap.forScenario(id))
                val size = Size(1280f, 800f)
                val cam = MapCamera.scenario(size, map)
                for ((view, pitch) in listOf("flat" to 0.0, "tilted" to Math.toRadians(48.0))) {
                    val gc = gpuCamera(cam, size.width, size.height, pitch)
                    val before = session.version
                    session.request(gc, 1280, 800, true)
                    val deadline = System.currentTimeMillis() + 120_000
                    // The full-size frame follows the quick one once the view has settled.
                    while ((session.version < before + 2 || session.frame?.width != 1280) && System.currentTimeMillis() < deadline) Thread.sleep(50)
                    val frame = assertNotNull(session.frame, "$id $view: a frame was drawn")
                    table.append("| $id | $view | ${frame.width}x${frame.height} | ${"%.1f".format(session.renderMs)} | ${"%.1f".format(session.copyMs)} |\n")
                    val scene = ImageComposeScene(1280, 800, Density(1f)) {
                        val measurer = rememberTextMeasurer()
                        val planner = remember { LabelPlanner() }
                        Canvas(Modifier.fillMaxSize()) {
                            drawGpuFrame(session)
                            drawGpuLabels(session, gc, map, plants, divisions, GraphicsConfig.Default, gc.widthKm, measurer, planner)
                        }
                    }
                    val image = scene.render()
                    scene.close()
                    if (out != null) {
                        val png = image.encodeToData(EncodedImageFormat.PNG)!!.bytes
                        File(out, "$id-$view.png").writeBytes(png)
                        File(out, "$id-$view-phone.jpg").writeBytes(
                            org.jetbrains.skia.Image.makeFromEncoded(png).let { img ->
                                val s = org.jetbrains.skia.Surface.makeRasterN32Premul(960, 600)
                                s.canvas.drawImageRect(img, org.jetbrains.skia.Rect.makeWH(960f, 600f))
                                s.makeImageSnapshot().encodeToData(EncodedImageFormat.JPEG, 82)!!.bytes
                            },
                        )
                    }
                    // A tap on a region's own anchor (projected by the renderer) picks a region of this scenario.
                    val anchors = map.regions.mapNotNull { r -> map.anchor(r.code)?.let { r.code to it } }
                    val pts = DoubleArray(anchors.size * 2)
                    anchors.forEachIndexed { k, (_, a) -> pts[2 * k] = Math.toDegrees(a.lon[0]); pts[2 * k + 1] = Math.toDegrees(a.lat[0]) }
                    val screen = session.project(gc, 1280, 800, pts)
                    val hits = anchors.indices.mapNotNull { k ->
                        val x = screen[2 * k]; val y = screen[2 * k + 1]
                        if (x.isNaN() || x < 0 || y < 0 || x > 1280 || y > 800) null else session.pick(gc, 1280, 800, x, y)
                    }
                    assertTrue(hits.all { code -> map.regions.any { it.code == code } }, "$id $view: picked $hits")
                    if (view == "flat") assertTrue(hits.isNotEmpty(), "$id: a region anchor on screen picks a region")
                }
                val line = gpuMapLine(true, session)
                assertTrue(line!!.startsWith("map: GPU "), line)
                println("frame counter: $line")
            } finally {
                session.close()
            }
        }
        println(table)
        if (out != null) File(out, "timings-${if (drew) "gpu" else "none"}.md").writeText(table.toString())
    }

    @Test
    fun withoutAnAdapterTheClassicMapStaysAndTheCounterSaysSo() {
        GpuMapHost.opener = { error("no graphics adapter") }
        GpuMapHost.failure = "no graphics adapter"
        assertEquals("map: classic, no GPU", gpuMapLine(true, null))
        GpuMapHost.opener = ::openDesktopGpuMap
        assertEquals(null, gpuMapLine(false, null), "nothing is shown while the option is off")
    }
}
