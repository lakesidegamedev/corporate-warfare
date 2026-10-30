package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import kotlin.math.roundToInt
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import uniffi.cw_ffi.DossierDto
import uniffi.cw_ffi.PersonDto

/**
 * The character cast's portraits, shared with the Apple apps (cast-portraits.json): a 32 by 32 bust
 * with three moods (neutral, pleased, displeased) and its 16 by 16 reduction for every person, named
 * or generated, and a 64 by 64 office portrait for the named chiefs.
 */
object CastPortraits {
    @Serializable class Bust(val palette: Map<String, String>, val expressions: Map<String, List<String>>, val small: Map<String, List<String>>)
    @Serializable class Office(val palette: Map<String, String>, val rows: List<String>)
    @Serializable private class File(val portraits: Map<String, Bust>, val office: Map<String, Office> = emptyMap())

    private val file: File? by lazy {
        val stream = CastPortraits::class.java.getResourceAsStream("/cast-portraits.json") ?: return@lazy null
        Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), stream.bufferedReader().readText())
    }

    val all: Map<String, Bust> get() = file?.portraits ?: emptyMap()
    val office: Map<String, Office> get() = file?.office ?: emptyMap()

    /** The mood to draw: the atlas has neutral, pleased and displeased, and anything else reads neutral. */
    fun mood(key: String): String = if (key == "pleased" || key == "displeased") key else "neutral"
}

/** Portraits narrower than this draw the 16 by 16 bust, the rest the 32 by 32 one. */
private val SmallBelow = 32.dp

private fun bustBitmap(person: String, mood: String, small: Boolean): ImageBitmap? {
    val art = CastPortraits.all[person] ?: return null
    val m = CastPortraits.mood(mood)
    val rows = (if (small) art.small[m] else art.expressions[m]) ?: return null
    return PixelBitmaps.of("cast:$person:$m:${if (small) 16 else 32}", rows) { ch -> art.palette[ch.toString()]?.let { SectorIcons.color(it) } }
}

private fun officeBitmap(person: String): ImageBitmap? {
    val art = CastPortraits.office[person] ?: return null
    return PixelBitmaps.of("cast:$person:office", art.rows) { ch -> art.palette[ch.toString()]?.let { SectorIcons.color(it) } }
}

/**
 * One cast member's portrait as exact pixels. [mood] is neutral, pleased or displeased. Below 32 dp it
 * is the 16 by 16 bust for rows and chips; with [office] set and a 64 by 64 office portrait on file
 * (chiefs on company pages, dossiers and the CEO Office) that is drawn instead.
 */
@Composable
fun PixelPortrait(person: String, modifier: Modifier = Modifier, size: Dp = 40.dp, mood: String = "neutral", office: Boolean = false, forceSmall: Boolean = false) {
    val bitmap = (if (office) officeBitmap(person) else null) ?: bustBitmap(person, mood, forceSmall || size < SmallBelow)
    Canvas(modifier.size(size).clip(Pixel.small).background(Palette.panelRaised)) {
        if (bitmap != null) {
            val side = minOf(this.size.width, this.size.height).roundToInt()
            drawPixelBitmap(bitmap, width = side, height = side)
        }
    }
}

internal fun temperamentLabel(key: String): String = when (key) {
    "integrator" -> "Integrator"
    "raider" -> "Raider"
    "financier" -> "Financier"
    "speculator" -> "Speculator"
    "builder" -> "Builder"
    "cautious" -> "Cautious"
    else -> key.replaceFirstChar { it.uppercase() }
}

internal fun temperamentNote(key: String): String = when (key) {
    "integrator" -> "Ties supply chains together and buys suppliers."
    "raider" -> "Takes rivals' plants whenever they are weak."
    "financier" -> "Leans on credit, deals and the balance sheet."
    "speculator" -> "Bets on new technology and big swings."
    "builder" -> "Grows organically, plant by plant."
    "cautious" -> "Keeps cash and avoids risk."
    else -> ""
}

/** A portrait, a name and one line, for posts and chiefs. */
@Composable
fun PersonLine(person: PersonDto, role: String, portrait: Dp = 36.dp) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        PixelPortrait(person.id, size = portrait, mood = person.mood)
        Column {
            Text(person.name, color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
            Text("$role · ${person.home}", color = Palette.secondary, fontSize = 11.sp, maxLines = 1)
        }
    }
}

/** A rival's chief executive, their style, background and what the firm has done lately. */
@Composable
fun DossierBody(d: DossierDto, currency: String = "") {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        val ceo = d.ceo
        if (ceo == null) {
            Text("${d.firmName} is run by its management. Style: ${d.style}.", color = Palette.secondary, fontSize = 12.sp)
        } else {
            Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                PixelPortrait(ceo.id, size = 72.dp, mood = ceo.mood, office = ceo.officePortrait)
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(ceo.name, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                    Text("${ceo.age} · ${ceo.home}", color = Palette.secondary, fontSize = 12.sp)
                    Text(
                        "${temperamentLabel(ceo.temperament)} · ${d.style}",
                        color = Palette.gold, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                    )
                    Text("Chief executive for ${d.ceoWeeks} weeks", color = Palette.secondary, fontSize = 11.sp)
                }
            }
            Text(temperamentNote(ceo.temperament), color = Palette.primaryText, fontSize = 12.sp)
            Text(ceo.background, color = Palette.secondary, fontSize = 12.sp)
        }
        RivalExecutives(d.executives, currency)
        d.grudges.forEach { line ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                PixelIcon("target", size = 12.dp, tint = Palette.negative)
                Text(line, color = Palette.primaryText, fontSize = 12.sp)
            }
        }
        d.allies.forEach { line ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                PixelIcon("people", size = 12.dp, tint = Palette.positive)
                Text(line, color = Palette.primaryText, fontSize = 12.sp)
            }
        }
        if (d.moves.isNotEmpty()) {
            Eyebrow("Recent moves")
            d.moves.take(5).forEach { m ->
                Text("${ResultBuckets.weekLabel(m.week.toInt())}: ${m.text}", color = Palette.secondary, fontSize = 12.sp)
            }
        }
    }
}

/** The dossier of one company, loaded from the engine. */
@Composable
fun DossierSection(store: GameStore, firm: ULong) {
    var dossier by remember(firm) { mutableStateOf<DossierDto?>(null) }
    LaunchedEffect(firm) { store.dossier(firm) { dossier = it } }
    dossier?.let { d ->
        if (d.ceo != null || d.moves.isNotEmpty() || d.grudges.isNotEmpty() || d.allies.isNotEmpty()) {
            Eyebrow("Who runs it")
            DossierBody(d, store.firms.firstOrNull { it.handle == firm }?.currency ?: store.selectedFirmDto?.currency.orEmpty())
        }
    }
    RaidMarketsList(store, firm)
}

/** A rival's company page. */
@Composable
fun DossierDialog(store: GameStore, firm: ULong, onClose: () -> Unit) = CompanyPageDialog(store, firm, onClose)

@Composable
fun RivalLeaderRow(store: GameStore, name: String, firm: ULong, detail: String, region: String? = null, industry: String? = null) {
    var open by remember { mutableStateOf(false) }
    Row(Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Column(Modifier.weight(1f).clickable { open = true }, verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FirmLogo(name, size = 22.dp)
                Spacer(Modifier.width(6.dp))
                Text(name, color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 1)
                Spacer(Modifier.width(6.dp))
                Text(detail, color = Palette.secondary, fontSize = 12.sp)
                PixelIcon("chevron_down", size = 10.dp, tint = Palette.secondary, modifier = Modifier.padding(start = 6.dp))
            }
            RivalIdentity(store, firm, 24.dp)
        }
        if (region != null && industry != null) RaidMarketButton(store, firm, region, industry)
    }
    if (open) DossierDialog(store, firm) { open = false }
}
