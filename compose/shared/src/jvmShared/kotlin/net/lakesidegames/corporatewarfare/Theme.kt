package net.lakesidegames.corporatewarfare

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** The iPhone app's cartographic palette: dark navy panels, gold for money and actions. */
object Palette {
    val ink = Color(0xFF0B1622)
    val panel = Color(0xFF13212F)
    val panelRaised = Color(0xFF1A2B3C)
    val stroke = Color(0xFF22364A)
    val primaryText = Color(0xFFE8EEF5)
    val secondary = Color(0xFF93A4B8)
    val gold = Color(0xFFE2B04A)
    val positive = Color(0xFF3FBF8A)
    val negative = Color(0xFFE5605E)
    val warning = Color(0xFFF2B748)
}

/** Sector colours, matching the iPhone app's sector badges. */
object Sectors {
    val order = listOf(
        "agriculture", "extraction", "energy", "chemical_industries", "manufacturing",
        "construction", "logistics", "retail", "healthcare", "technology", "media",
        "telecommunications", "real_estate", "defense", "financial",
    )

    fun name(id: String): String = when (id) {
        "chemical_industries" -> "Chemicals"
        "telecommunications" -> "Telecom"
        "real_estate" -> "Real Estate"
        else -> id.replaceFirstChar { it.uppercase() }
    }

    fun color(id: String): Color = when (id) {
        "agriculture" -> Color(0xFF8BC34A)
        "extraction" -> Color(0xFFA1887F)
        "energy" -> Color(0xFFFFC107)
        "chemical_industries" -> Color(0xFF26C6DA)
        "manufacturing" -> Color(0xFF90A4AE)
        "construction" -> Color(0xFFFF8A65)
        "logistics" -> Color(0xFF7E57C2)
        "retail" -> Color(0xFFEC407A)
        "healthcare" -> Color(0xFFEF5350)
        "technology" -> Color(0xFF42A5F5)
        "media" -> Color(0xFFAB47BC)
        "telecommunications" -> Color(0xFF29B6F6)
        "real_estate" -> Color(0xFF66BB6A)
        "defense" -> Color(0xFF78909C)
        "financial" -> Color(0xFFFFD54F)
        else -> Palette.secondary
    }
}

@Composable
fun CorporateWarfareTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Palette.gold,
            onPrimary = Palette.ink,
            background = Palette.ink,
            onBackground = Palette.primaryText,
            surface = Palette.panel,
            onSurface = Palette.primaryText,
            surfaceVariant = Palette.panelRaised,
            onSurfaceVariant = Palette.secondary,
            outline = Palette.stroke,
            error = Palette.negative,
        ),
        content = content,
    )
}
