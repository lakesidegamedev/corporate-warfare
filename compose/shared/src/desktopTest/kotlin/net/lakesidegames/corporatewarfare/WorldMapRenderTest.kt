package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** The world map renders as a globe, through the blend, and flat on the scenario. */
class WorldMapRenderTest {
    @Test
    fun theMapRendersAtEveryStageOfTheZoom() {
        val map = assertNotNull(WorldMap.shared, "the bundled world map loads")
        assertTrue(map.countries.size > 150, "every country is on the globe")
        assertTrue(map.regions.size == 31, "the scenario's regions load")
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)
        val scenario = MapCamera.scenario(androidx.compose.ui.geometry.Size(1200f, 800f), map)
        val stages = mapOf(
            "globe" to scenario.copy(zoom = 1.0),
            "unrolling" to scenario.copy(zoom = 3.5),
            "scenario" to scenario,
            "close" to scenario.zoomed(3.0),
            "pacific" to MapCamera(170.0, -5.0, 4.0),
        )
        for ((name, camera) in stages) {
            val scene = ImageComposeScene(1200, 800, Density(1f)) {
                val measurer = rememberTextMeasurer()
                Canvas(Modifier.fillMaxSize()) {
                    drawWorldMap(map, camera, Color(0xFFD4A84B), mapOf("OH-CEN" to 1.0), "MI-DET", emptyList(), measurer)
                }
            }
            val image = scene.render()
            scene.close()
            if (out != null) {
                out.mkdirs()
                File(out, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes)
            }
        }
        // A tap on a region's hub city selects that region, on the flat map and on the globe.
        for (camera in listOf(scenario, scenario.copy(zoom = 1.0))) {
            val projection = MapProjection(camera, androidx.compose.ui.geometry.Size(1200f, 800f))
            val tap = assertNotNull(projection.project(assertNotNull(map.anchor("OH-CEN"))))
            val hit = map.regions.firstOrNull { projection.contains(it.rings, tap) }?.code
            assertEquals("OH-CEN", hit, "tapping Columbus at zoom ${camera.zoom}")
        }
    }
}
