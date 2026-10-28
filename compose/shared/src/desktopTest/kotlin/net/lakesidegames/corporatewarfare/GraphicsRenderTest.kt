package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.toComposeImageBitmap

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import uniffi.cw_ffi.LaneDto
import uniffi.cw_ffi.MapDivisionDto
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * The 3D map and the menu render at every preset and stage, the still layer plus one frame of the
 * moving one. Set CW_MAP_RENDER_DIR to keep the pictures.
 */
class GraphicsRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val divisions = listOf(
        MapDivisionDto(1uL, "OH-CEN", "manufacturing", 6u, 6u),
        MapDivisionDto(1uL, "OH-CEN", "retail", 3u, 3u),
        MapDivisionDto(1uL, "MI-DET", "energy", 9u, 9u),
        MapDivisionDto(1uL, "PA-WES", "technology", 2u, 2u),
        MapDivisionDto(1uL, "IL-CHI", "logistics", 4u, 4u),
    )

    private fun lane(a: String, b: String, mode: String, lots: Int) = LaneDto(
        a = a, b = b, aName = a, bName = b, mode = 0u, modeId = mode, modeName = mode, km = 300u,
        capacityLots = lots, usedLots = lots / 2, heldLots = 0, outsideLots = 0, fillBps = 5000,
        rateCentsPerLot = 100, outsideRateCentsPerLot = 120, hotWeeks = 0u, owners = emptyList(), contracts = 0u,
        myCapacityLots = 0, myShippedLots = 0, myContractLots = 0, reservableLots = 0,
    )

    private val lanes = listOf(
        lane("OH-CEN", "MI-DET", "road", 40), lane("OH-CEN", "PA-WES", "rail", 30), lane("IL-CHI", "MI-DET", "sea", 20),
        lane("OH-NE", "NY-WES", "road", 25), lane("IN-CEN", "IL-CHI", "rail", 35), lane("PA-PHL", "NY-NYC", "road", 50),
    )

    private val shading = mapOf("OH-CEN" to 1.0, "MI-DET" to 0.8, "PA-WES" to 0.3, "IL-CHI" to 0.5)

    @Test
    fun everyPresetRendersAtEveryStage() {
        val map = assertNotNull(WorldMap.shared)
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val size = Size(1200f, 800f)
        val scenario = MapCamera.scenario(size, map)
        val stages = mapOf(
            "globe" to scenario.copy(zoom = 1.0),
            "unrolling" to scenario.copy(zoom = 3.5),
            "scenario" to scenario,
            "close" to scenario.zoomed(3.0),
            "turned" to scenario.zoomed(2.0).rotated(35.0),
        )
        val sun = Sun.moving(40.0)
        for (preset in GraphicsPreset.entries) {
            val graphics = preset.config(animations = true, showFps = false)
            for ((name, camera) in stages) {
                val scene = ImageComposeScene(1200, 800, Density(1f)) {
                    val measurer = rememberTextMeasurer()
                    val geometry = androidx.compose.runtime.remember { MapGeometryCache() }
                    Canvas(Modifier.fillMaxSize()) {
                        // The layers in the game's order: ground, traffic, clouds, structures, effects.
                        fun still(phase: MapPhase) = drawWorldMap(
                            map, camera, Palette.gold, shading, "MI-DET", divisions, measurer, lanes = lanes, freightLayer = false,
                            geometry = geometry, graphics = graphics, sun = if (graphics.dayNight) sun else null, phase = phase,
                        )
                        still(MapPhase.Ground)
                        val g = geometry.of(map, camera, size, graphics.detail, graphics.pitch)
                        val o = g.overlay(lanes, divisions, "MI-DET", shading, graphics, false)
                        drawMapOverlay(o, graphics, 12.5, false, sun, map.playBounds, camera.zoom, 0.5, 1f, map.scenarioId, part = OverlayPart.Traffic)
                        with(Weather) { drawWeather(g.p, sun, 12.5, graphics.clouds, map.playBounds, 46.0 * g.scale * g.p.t, camera.zoom, graphics.lighting > 0, map.scenarioId) }
                        still(MapPhase.Structures)
                        drawMapOverlay(o, graphics, 12.5, false, sun, map.playBounds, camera.zoom, 0.5, 1f, map.scenarioId, part = OverlayPart.Effects)
                    }
                }
                val image = scene.render()
                scene.close()
                out?.let { File(it, "${preset.name.lowercase()}-$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
            }
        }
    }

    @Test
    fun theMenuRenders() {
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val scene = ImageComposeScene(1400, 860, Density(1f)) {
            androidx.compose.foundation.layout.Box(Modifier.fillMaxSize().then(Modifier.background(androidx.compose.ui.graphics.Color(0xFF05090E)))) {
                GlobeBackdrop(Modifier.fillMaxSize())
            }
        }
        scene.render(0L)
        val image = scene.render(9_000_000_000L)
        scene.close()
        out?.let { File(it, "lander.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
    }

    @Test
    fun aTapOnARaisedTileSelectsIt() {
        val map = assertNotNull(WorldMap.shared)
        val size = Size(1200f, 800f)
        val camera = MapCamera.scenario(size, map)
        val graphics = GraphicsPreset.Ultra.config(animations = true, showFps = false)
        val g = MapGeometryCache().of(map, camera, size, graphics.detail, graphics.pitch)
        val solid = g.raised(shading).first { it.code == "OH-CEN" }
        assertTrue(solid.h > 10, "Ohio Central stands tall on the presence layer")
        val region = map.regions.first { it.code == "OH-CEN" }
        val hub = assertNotNull(map.anchor("OH-CEN"))
        val top = assertNotNull(g.p.project(hub, solid.h))
        assertTrue(g.p.contains(region.rings, top, solid.h), "the hub on the tile's top is inside the raised region")
        assertEquals(31, g.raised(shading).size)
    }

    @Test
    fun aTapOnATurnedMapSelectsTheRegionUnderIt() {
        val map = assertNotNull(WorldMap.shared)
        val size = Size(1200f, 800f)
        for (bearing in listOf(0.0, 35.0, 120.0, 270.0)) {
            val camera = MapCamera.scenario(size, map).zoomed(1.5).rotated(bearing)
            val p = MapProjection(camera, size, 28 * kotlin.math.PI / 180)
            val tap = assertNotNull(p.project(assertNotNull(map.anchor("OH-CEN"))))
            assertEquals("OH-CEN", map.regions.firstOrNull { p.contains(it.rings, tap) }?.code, "a tap on Columbus at bearing $bearing")
            // Turning and dragging: a drag to the right moves the centre west on screen whatever the bearing.
            val moved = camera.panned(androidx.compose.ui.geometry.Offset(100f, 0f), size)
            val before = p.project(GeoRing.point(camera.lon, camera.lat))!!
            val after = MapProjection(moved, size, 28 * kotlin.math.PI / 180).project(GeoRing.point(camera.lon, camera.lat))!!
            assertTrue(after.x - before.x > 50f, "the ground follows the finger at bearing $bearing")
        }
    }

    @Test
    fun presetsRoundTripAndCustomIsDetected() {
        GraphicsPreset.entries.forEach { assertEquals(it, it.config(true, false).preset) }
        assertEquals(null, GraphicsPreset.High.config(true, false).copy(traffic = 2).preset)
    }
}
