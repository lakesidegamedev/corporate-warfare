package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.test.Test
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * Every layer of the menu globe is drawn for the same camera: the scenario outlines and their labels sit on the
 * land they belong to, on the globe's disc, at every window size and on a phone, with the baked layer on or off.
 */
class MenuGlobeTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val ink = Color(0xFF05090E)

    private fun shoot(w: Int, h: Int, density: Float, offsetX: Float, focus: String?): IntArray {
        val scene = ImageComposeScene((w * density).toInt(), (h * density).toInt(), Density(density)) {
            Box(Modifier.fillMaxSize().background(ink)) { GlobeBackdrop(Modifier.fillMaxSize(), offsetX = offsetX, calm = true, focus = focus) }
        }
        scene.render(0L)
        Thread.sleep(800)
        val image = scene.render(2_000_000_000L).toComposeImageBitmap()
        scene.close()
        return IntArray(image.width * image.height).also { image.readPixels(it) }
    }

    private fun check(w: Int, h: Int, density: Float, offsetX: Float) {
        val map = assertNotNull(WorldMap.shared)
        val pw = (w * density).toInt(); val ph = (h * density).toInt()
        val size = Size(pw.toFloat(), ph.toFloat())
        // Where the globe's disc is: the menu draws it offset by a fraction of the window.
        val ox = pw * offsetX.toDouble(); val oy = ph * 0.08
        for ((id, rings) in map.scenarioAreas) {
            val camera = globeCamera(map, 0.0, id)
            val p = MapProjection(camera, size)
            val dcx = p.cx + ox; val dcy = p.cy + oy; val r = p.r
            val pixels = shoot(w, h, density, offsetX, id)
            val bg = ink.value.shr(32).toInt() and 0xFFFFFF
            // The disc is full of globe, and almost nothing but a few stars lies outside it.
            var holes = 0; var inDisc = 0; var stray = 0
            for (y in 0 until ph) for (x in 0 until pw) {
                val d = hypot(x - dcx, y - dcy)
                val rgb = pixels[y * pw + x] and 0xFFFFFF
                if (d < r * 0.97) { inDisc++; if (rgb == bg) holes++ } else if (d > r * 1.05 && rgb != bg) stray++
            }
            assertTrue(inDisc > 0 && holes < inDisc * 0.02, "$id at ${w}x$h: the globe is not where the outlines expect it ($holes of $inDisc disc pixels empty)")
            assertTrue(stray < pw * ph * 0.03, "$id at ${w}x$h: globe drawn outside its disc ($stray stray pixels)")
            // The outline: its bright stroke around the scenario's own centre.
            val (lon, lat) = assertNotNull(scenarioCentre(rings))
            val at = assertNotNull(p.project(GeoRing.point(lon, lat)))
            val cx = at.x + ox; val cy = at.y + oy
            assertTrue(hypot(cx - dcx, cy - dcy) < r, "$id's centre is off the disc")
            var white = 0
            val reach = r * 0.3
            for (y in 0 until ph) for (x in 0 until pw) {
                if (abs(x - cx) > reach || abs(y - cy) > reach) continue
                val c = pixels[y * pw + x]
                if ((c shr 16 and 255) >= 235 && (c shr 8 and 255) >= 235 && (c and 255) >= 235) white++
            }
            assertTrue(white >= 20, "$id at ${w}x$h: no outline or label near its own land ($white bright pixels)")
        }
    }

    @Test fun desktopWide() = check(1920, 1080, 1f, 0.22f)
    @Test fun desktopLaptop() = check(1400, 860, 1f, 0.22f)
    @Test fun desktopNarrow() = check(1000, 700, 1f, 0.22f)
    @Test fun phone() = check(390, 844, 2f, 0.22f)

    @Test fun bakedLayerAgrees() {
        MapBake.enabled = true
        try { check(1400, 860, 1f, 0.22f) } finally { MapBake.enabled = false }
    }
}
