package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** The baked map layer: a bitmap carried to a nearby camera must land closer to a fresh render than the bitmap left where it was. */
class AsyncLayerTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val size = Size(640f, 400f)
    private val measurer = TextMeasurer(createFontFamilyResolver(), Density(1f), LayoutDirection.Ltr, cacheSize = 16)
    private val shading = mapOf("OH-CEN" to 1.0, "MI-DET" to 0.6)

    private fun render(block: DrawScope.() -> Unit): IntArray {
        val image = ImageBitmap(size.width.toInt(), size.height.toInt())
        CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, Canvas(image), size, block)
        return IntArray(image.width * image.height).also { image.readPixels(it) }
    }

    private fun error(a: IntArray, b: IntArray): Double {
        var total = 0L
        for (i in a.indices) for (shift in intArrayOf(0, 8, 16)) total += abs(((a[i] shr shift) and 255) - ((b[i] shr shift) and 255))
        return total / (a.size * 3.0)
    }

    @Test
    fun movedBitmapBeatsUnmovedForPanZoomAndTurn() {
        val map = WorldMap.shared!!
        val g = GraphicsPreset.High.config(true, false)
        val sun = Sun.moving(40.0)
        val base = MapCamera.scenario(size, map).zoomed(2.0)
        fun DrawScope.paint(cam: MapCamera, geo: MapGeometryCache) = drawWorldMap(
            map, cam, Palette.gold, shading, null, emptyList(), measurer, geometry = geo, graphics = g, pitch = g.pitch, sun = sun, phase = MapPhase.Ground,
        )
        val layer = AsyncLayer(CoroutineScope(Job()))
        layer.bakeNow(base, g.pitch, null, size, Density(1f)) { geo -> paint(base, geo) }
        val baked = layer.baked!!
        val cases = listOf("pan" to base.copy(lon = base.lon + 0.4, lat = base.lat + 0.2), "zoom" to base.zoomed(1.08), "turn" to base.rotated(3.0))
        val lines = StringBuilder()
        for ((name, cam) in cases) {
            val fresh = render { paint(cam, MapGeometryCache()) }
            val unmoved = render { with(layer) { drawBitmap(baked.image) } }
            val moved = render { with(layer) { drawMoved(baked, cam, g.pitch, null) } }
            lines.append("$name unmoved %.2f moved %.2f\n".format(error(fresh, unmoved), error(fresh, moved)))
            assertTrue(error(fresh, moved) < error(fresh, unmoved) * 0.8, "$name: the moved bitmap should be nearer the fresh render\n$lines")
        }
        println(lines)
        // The exact camera draws the bitmap as it is.
        assertEquals(0.0, error(render { with(layer) { drawBitmap(baked.image) } }, render { with(layer) { drawBitmap(baked.image) } }))
    }
}
