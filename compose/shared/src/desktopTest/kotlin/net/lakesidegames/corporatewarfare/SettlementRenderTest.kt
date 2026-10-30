package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.math.PI
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/**
 * Houses follow where people move. Every house stands at a fixed place; a region people are moving into
 * shows more of the houses that could stand there, and one they leave shows fewer. Set CW_MAP_RENDER_DIR
 * to keep the pictures: a gaining and a losing region before and after, desktop PNG and phone JPEG.
 */
class SettlementRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
    }

    private val map get() = assertNotNull(WorldMap.shared)

    // Net migration over a year in basis points of a region's people, as a 156 week guided career reports it
    // for one region that gains hugely, one that loses a little, and one that loses a lot (a stress case).
    private val gain = "KY-EAS"
    private val loss = "KY-BLU"
    private val settled = mapOf(gain to 2975, loss to -106, "OH-CEN" to -61)

    private fun houses(settlement: Map<String, Int>): List<Countryside.Prop> =
        Countryside.props(map).filter { it.kind == "house" && it.rank < Countryside.houseLimit(settlement[it.region]) }

    @Test
    fun housesStayWhereTheyAreAndOnlyTheirNumberFollowsMigration() {
        val all = Countryside.props(map)
        val steady = houses(emptyMap())
        val gaining = houses(mapOf(gain to 400))
        val losing = houses(mapOf(gain to -400))
        // The steady set is exactly what the economy phase always showed: rank under 45.
        assertTrue(steady.all { it.rank < 45 })
        // More people, more houses among the same fixed set; fewer people, fewer. Nothing else moves.
        assertTrue(gaining.size > steady.size, "${gaining.size} against ${steady.size}")
        assertTrue(losing.size < steady.size, "${losing.size} against ${steady.size}")
        assertTrue(steady.all { it in gaining } && losing.all { it in steady })
        assertTrue(houses(mapOf(loss to -106)).size < steady.size)
        // Other regions are untouched by one region's movement.
        assertEquals(steady.filter { it.region != gain }.toSet(), gaining.filter { it.region != gain }.toSet())
        // The props are worked out once: the same objects at the same places however the people move.
        assertTrue(Countryside.props(map) === all)
        // The rank limit is monotone and bounded.
        var last = 0.0
        for (bps in -500..500 step 25) {
            val limit = Countryside.houseLimit(bps)
            assertTrue(limit >= last && limit in 20.0..70.0)
            last = limit
        }
        // Houses that could ever stand are generated up front, in every region with a city.
        assertTrue(all.count { it.kind == "house" && it.region == gain } > 0, "houses in $gain")
    }

    private fun centre(code: String): Pair<Double, Double> {
        val f = map.regions.first { it.code == code }
        val points = f.rings.flatMap { r -> (0 until r.size).map { r.lon[it] to r.lat[it] } }
        return points.map { it.first }.average() * 180 / PI to points.map { it.second }.average() * 180 / PI
    }

    private fun picture(name: String, code: String, settlement: Map<String, Int>, width: Int, height: Int, jpeg: Boolean) {
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val size = Size(width.toFloat(), height.toFloat())
        val (lon, lat) = centre(code)
        val camera = MapCamera.scenario(size, map).zoomed(7.0).copy(lon = lon, lat = lat)
        val graphics = GraphicsPreset.High.config(animations = false, showFps = false).copy(countryside = true)
        val sun = Sun.moving(40.0)
        val scene = ImageComposeScene(width, height, Density(1f)) {
            val measurer = rememberTextMeasurer()
            val geometry = androidx.compose.runtime.remember { MapGeometryCache() }
            Canvas(Modifier.fillMaxSize()) {
                fun still(phase: MapPhase) = drawWorldMap(
                    map, camera, Palette.gold, emptyMap(), code, emptyList(), measurer, geometry = geometry, graphics = graphics,
                    sun = sun, phase = phase, economyPhase = "boom", settlement = settlement, week = 28,
                )
                still(MapPhase.Ground)
                still(MapPhase.Structures)
            }
        }
        val image = scene.render()
        scene.close()
        out?.let {
            File(it, "$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes)
            if (jpeg) File(it, "$name.jpg").writeBytes(image.encodeToData(EncodedImageFormat.JPEG, 90)!!.bytes)
        }
        lastBytes[name] = image.encodeToData(EncodedImageFormat.PNG)!!.bytes
    }

    private val lastBytes = HashMap<String, ByteArray>()

    @Test
    fun aGainingAndALosingRegionBeforeAndAfter() {
        for ((label, code) in listOf("gaining" to gain, "losing" to loss)) {
            picture("desktop-$label-before", code, emptyMap(), 1200, 800, false)
            picture("desktop-$label-after", code, settled, 1200, 800, false)
            picture("phone-$label-before", code, emptyMap(), 390, 844, true)
            picture("phone-$label-after", code, settled, 390, 844, true)
            assertTrue(!lastBytes["desktop-$label-before"]!!.contentEquals(lastBytes["desktop-$label-after"]!!), "$label region changes the picture")
        }
    }

    @Test
    fun theHouseLayerIsDrawnForTheSameCameraWhileMoving() {
        // The same region at a walk of cameras: a house keeps its place in the world; only the camera decides where it lands.
        val size = Size(1200f, 800f)
        val (lon, lat) = centre(gain)
        val base = MapCamera.scenario(size, map).zoomed(7.0).copy(lon = lon, lat = lat)
        val geometry = MapGeometryCache()
        val props = houses(settled)
        assertTrue(props.isNotEmpty())
        val probe = props.first()
        var previous: androidx.compose.ui.geometry.Offset? = null
        for (step in 0..6) {
            val cam = base.copy(lon = base.lon + step * 0.01)
            val p = geometry.of(map, cam, size, MapDetail.High, 0.0).p
            val at = p.ground(probe.lon * PI / 180, probe.lat * PI / 180)
            // Projecting the same fixed world point twice for one camera gives one answer.
            assertEquals(at, p.ground(probe.lon * PI / 180, probe.lat * PI / 180))
            if (previous != null) assertTrue(at != previous, "the house moves on screen with the camera")
            previous = at
        }
    }
}
