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

/** The character cast's 16 by 16 pixel portraits, shared with the Apple apps (cast-portraits.json). */
object CastPortraits {
    @Serializable private data class File(val size: Int = 16, val portraits: Map<String, PixelIcons.Icon>)

    private val file: File? by lazy {
        val stream = CastPortraits::class.java.getResourceAsStream("/cast-portraits.json") ?: return@lazy null
        Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), stream.bufferedReader().readText())
    }

    val all: Map<String, PixelIcons.Icon> get() = file?.portraits ?: emptyMap()
}

/** One cast member's portrait as exact pixels. */
@Composable
fun PixelPortrait(person: String, modifier: Modifier = Modifier, size: Dp = 40.dp) {
    val art = CastPortraits.all[person]
    Canvas(modifier.size(size).clip(Pixel.small).background(Palette.panelRaised)) {
        if (art != null) drawPixelIcon(art, null)
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
        PixelPortrait(person.id, size = portrait)
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
                PixelPortrait(ceo.id, size = 64.dp)
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
