package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.Color
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** The 8-bit sector icons shared with the iPhone app (sector-icons.json). */
object SectorIcons {
    @Serializable data class Icon(val palette: Map<String, String>, val rows: List<String>)
    @Serializable private data class File(val icons: Map<String, Icon>)

    val all: Map<String, Icon> by lazy {
        val stream = SectorIcons::class.java.getResourceAsStream("/sector-icons.json") ?: return@lazy emptyMap()
        Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), stream.bufferedReader().readText()).icons
    }

    fun color(hex: String): Color = Color(0xFF000000 or hex.removePrefix("#").toLong(16))
}

/** The Corporate Warfare logo (cw-logo.json), the same pixels as on iPhone. */
object PixelLogo {
    @Serializable data class File(val width: Int, val height: Int, val palette: Map<String, String>, val rows: List<String>)

    val file: File? by lazy {
        val stream = PixelLogo::class.java.getResourceAsStream("/cw-logo.json") ?: return@lazy null
        Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), stream.bufferedReader().readText())
    }
}
