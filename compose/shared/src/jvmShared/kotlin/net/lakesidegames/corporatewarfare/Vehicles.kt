package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.roundToInt

/**
 * Trucks, cars, trains, cargo ships and cargo planes as tiny cel-shaded voxel sprites, baked by
 * tools/voxel_vehicles.py for eight headings and two liveries (neutral, and the player's gold) at
 * 1, 2 and 3 pixels per half voxel. Drawn one to one, facing the way they move on screen, so they
 * stay right when the map is turned or tilted.
 */
internal object Vehicles {
    @Serializable
    private data class Atlas(val scales: List<Int>, val headings: List<String>, val frames: Map<String, Map<String, Voxels.Frame>>)

    private val atlas: Atlas? by lazy {
        runCatching {
            val text = Vehicles::class.java.getResourceAsStream("/vehicles.json")?.use { it.readBytes().decodeToString() } ?: return@runCatching null
            Json { ignoreUnknownKeys = true }.decodeFromString(Atlas.serializer(), text)
        }.getOrNull()
    }

    private val Headings = listOf("N", "NE", "E", "SE", "S", "SW", "W", "NW")

    /**
     * The nearest of the eight headings for a screen direction ([dx], [dy], y down). The sprites
     * see the ground at about two to one, so the vertical part is doubled back before the angle.
     */
    fun heading(dx: Float, dy: Float): String {
        val angle = atan2(dx.toDouble(), -2.0 * dy)
        val index = ((angle / (PI / 4)).roundToInt() % 8 + 8) % 8
        return Headings[index]
    }

    fun frame(type: String, mine: Boolean, heading: String, scale: Int): Voxels.Frame? =
        atlas?.frames?.get("$type/${if (mine) "player" else "neutral"}/$heading")?.get("$scale")

    /** Draws a vehicle with its ground anchor at [ground], facing screen direction [dir]; false when there is no sprite. */
    fun DrawScope.drawVehicle(
        type: String, mine: Boolean, ground: Offset, dir: Offset, scale: Int, alpha: Float = 1f, filter: ColorFilter? = null,
    ): Boolean {
        val f = frame(type, mine, heading(dir.x, dir.y), scale) ?: return false
        val image = Images.load("vehicles-$scale") ?: return false
        drawImage(
            image, srcOffset = IntOffset(f.x, f.y), srcSize = IntSize(f.w, f.h),
            dstOffset = IntOffset((ground.x - f.ax).roundToInt(), (ground.y - f.ay).roundToInt()), dstSize = IntSize(f.w, f.h),
            alpha = alpha, filterQuality = FilterQuality.None, colorFilter = filter,
        )
        return true
    }
}

/** One vehicle to draw this frame; gathered first so they can be drawn back to front. */
internal class VehicleSpot(
    val type: String, val mine: Boolean,
    /** Where it stands as a ground offset from the view centre, and its direction of travel on the ground. */
    val g: Offset, val dir: Offset, val height: Double = 0.0,
) {
    /** For drawing back to front. */
    var depth = 0f
}
