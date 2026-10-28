package net.lakesidegames.corporatewarfare

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.math.roundToInt

/**
 * The player's divisions as cel-shaded voxel buildings, one model per industry in three sizes,
 * baked by tools/voxel_sprites.py into atlases at 1, 2 and 3 pixels per half voxel. The map picks
 * the scale for the zoom and screen density and draws the sprite one to one, so it stays crisp.
 */
internal object Voxels {
    @Serializable
    data class Frame(val x: Int, val y: Int, val w: Int, val h: Int, val ax: Double, val ay: Double)

    @Serializable
    private data class Atlas(val scales: List<Int>, val frames: Map<String, Map<String, Frame>>)

    private val atlas: Atlas? by lazy {
        runCatching {
            val text = Voxels::class.java.getResourceAsStream("/voxels.json")?.use { it.readBytes().decodeToString() } ?: return@runCatching null
            Json { ignoreUnknownKeys = true }.decodeFromString(Atlas.serializer(), text)
        }.getOrNull()
    }

    /** Small up to three plants, medium up to eight, large beyond. */
    fun size(plants: Int): Int = when {
        plants <= 3 -> 0
        plants <= 8 -> 1
        else -> 2
    }

    /**
     * The pixel scale for a map drawn at [zoomScale] on a screen of [density]: the player's
     * buildings start at the middle scale at the scenario's framing so they read as the stars.
     */
    fun scale(zoomScale: Float, density: Float): Int = (zoomScale * density * 1.6f).roundToInt().coerceIn(1, 3)

    fun frame(sector: String, size: Int, scale: Int): Frame? = atlas?.frames?.get("$sector/$size")?.get("$scale")

    /**
     * Draws a building with its ground anchor at [ground]; returns the top of the sprite on
     * screen, or null when there is no sprite for it (the caller falls back to a plain box).
     */
    fun DrawScope.drawBuilding(sector: String, plants: Int, ground: Offset, scale: Int): Float? {
        val f = frame(sector, size(plants), scale) ?: return null
        val image = Images.load("voxels-$scale") ?: return null
        val left = (ground.x - f.ax).roundToInt()
        val top = (ground.y - f.ay).roundToInt()
        drawImage(
            image, srcOffset = IntOffset(f.x, f.y), srcSize = IntSize(f.w, f.h),
            dstOffset = IntOffset(left, top), dstSize = IntSize(f.w, f.h), filterQuality = FilterQuality.None,
        )
        return top.toFloat()
    }
}
