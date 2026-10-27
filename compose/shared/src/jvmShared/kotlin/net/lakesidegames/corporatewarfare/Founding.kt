package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.cw_ffi.PreviewDto

private val SuggestedNames = listOf(
    "Lakeshore Industries", "Keystone Holdings", "Prairie Works", "Iron Valley Group", "Northgate Enterprises",
    "Riverbend Company", "Summit Manufacturing", "Harbor Point Holdings",
)

/**
 * Found a company: name, headquarters, first strategy, capital. Shown wherever a screen needs a
 * company and the player has none yet.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun FoundCompanyPanel(store: GameStore) {
    val cash = store.overview?.investorCashCents ?: 0
    var name by remember { mutableStateOf(SuggestedNames[(cash % SuggestedNames.size).toInt().coerceAtLeast(0)]) }
    var region by remember { mutableStateOf(store.boards.maxByOrNull { b -> b.room.sumOf { it.importLots * it.importPriceCents } }?.index) }
    var sector by remember { mutableStateOf(store.strategyCatalogue.firstOrNull()?.industry ?: "agriculture") }
    var recipe by remember { mutableStateOf(store.strategyCatalogue.firstOrNull { it.industry == sector }?.index) }
    var capital by remember { mutableStateOf((cash * 0.6 / 100).coerceAtLeast(1_000_000.0)) }
    var preview by remember { mutableStateOf<PreviewDto?>(null) }
    var result by remember { mutableStateOf<Pair<Boolean, String>?>(null) }
    val json = if (name.isNotBlank() && region != null && recipe != null) {
        CommandJson.command("FoundFirm", "name" to name.trim(), "region" to region!!, "capital_cents" to (capital * 100).toLong(), "recipe" to recipe!!)
            .replace("}}", ",\"secondary\":null}}")
    } else {
        null
    }
    LaunchedEffect(json) {
        preview = null
        if (json == null) return@LaunchedEffect
        delay(300)
        store.preview(json) { preview = it }
    }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = 1000.dp).fillMaxWidth().verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Found your company", color = Palette.primaryText, fontSize = 24.sp, fontWeight = FontWeight.Bold)
            Text("You have ${Format.compact(cash)} of your own. Put some of it into a company, pick where it is based and what it makes first.", color = Palette.secondary, fontSize = 14.sp)
            Panel {
                Eyebrow("Name")
                OutlinedTextField(
                    name, { name = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = Palette.primaryText, unfocusedTextColor = Palette.primaryText,
                        focusedBorderColor = Palette.gold, unfocusedBorderColor = Palette.stroke, cursorColor = Palette.gold,
                    ),
                )
            }
            Panel {
                Eyebrow("Headquarters")
                Text("Strong demand and room to build bring buyers; cheaper wages cut costs.", color = Palette.secondary, fontSize = 12.sp)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    store.boards.sortedWith(compareByDescending<uniffi.cw_ffi.RegionBoardDto> { b -> b.room.sumOf { it.importLots * it.importPriceCents } }.thenByDescending { it.demandIndexBps }).forEach { b ->
                        val on = b.index == region
                        val room = b.room.sumOf { it.importLots * it.importPriceCents }
                        Column(
                            Modifier.width(210.dp).clip(RoundedCornerShape(10.dp)).background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.ink)
                                .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke, RoundedCornerShape(10.dp))
                                .clickable { region = b.index }.padding(10.dp),
                        ) {
                            Text("${b.name}, ${b.stateCode}", color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
                            Text(
                                "Wages ${b.wageIndexBps / 100}% · " + if (room > 0) "room ${Format.compact(room)} a week" else "demand ${b.demandIndexBps / 100}%",
                                color = Palette.secondary, fontSize = 11.sp,
                            )
                        }
                    }
                }
            }
            Panel {
                Eyebrow("First strategy")
                FlowRow(horizontalArrangement = Arrangement.spacedBy(5.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                    store.strategyCatalogue.map { it.industry }.distinct().forEach { s ->
                        Text(
                            Sectors.name(s), color = if (s == sector) Palette.ink else Palette.primaryText, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.clip(RoundedCornerShape(50)).background(if (s == sector) Palette.gold else Palette.panelRaised)
                                .clickable { sector = s; recipe = store.strategyCatalogue.firstOrNull { it.industry == s }?.index }
                                .padding(horizontal = 10.dp, vertical = 5.dp),
                        )
                    }
                }
                store.strategyCatalogue.filter { it.industry == sector }.forEach { st ->
                    val on = st.index == recipe
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(if (on) Palette.gold.copy(alpha = 0.14f) else androidx.compose.ui.graphics.Color.Transparent)
                            .clickable { recipe = st.index }.padding(10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        SectorBadge(st.industry, 22)
                        Spacer(Modifier.width(10.dp))
                        Column(Modifier.weight(1f)) {
                            Text(st.name, color = if (on) Palette.gold else Palette.primaryText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                            Text("Makes ${st.outputName} · ${st.weeklyOutputLots} lots a week per plant", color = Palette.secondary, fontSize = 12.sp)
                        }
                        Column(horizontalAlignment = Alignment.End) {
                            Text(Format.compact(st.capexCents) + " a plant", color = Palette.primaryText, fontSize = 12.sp)
                            Text("${st.buildWeeks} weeks · margin ${st.referenceMarginBps / 100}%", color = Palette.secondary, fontSize = 11.sp)
                        }
                    }
                }
            }
            Panel {
                Eyebrow("Capital to put in")
                ValueSlider(
                    capital, { capital = it }, 1_000_000.0..(cash / 100.0).coerceAtLeast(1_000_001.0), 50_000.0,
                    label = { Format.compact((it * 100).toLong()) },
                    detail = { "You keep ${Format.compact(cash - (it * 100).toLong())} of your own to invest elsewhere." },
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                GoldButton("Found the company", enabled = json != null && preview?.blockedKey == null) {
                    val text = json ?: return@GoldButton
                    store.act(text) { ok, message ->
                        result = ok to message
                        if (ok) store.firms.lastOrNull { it.controlled }?.let { store.selectFirm(it.handle) }
                    }
                }
                Spacer(Modifier.width(12.dp))
                val p = preview
                when {
                    result != null -> Text(result!!.second, color = if (result!!.first) Palette.positive else Palette.negative, fontSize = 13.sp)
                    p?.blockedKey != null -> Text(Messages.rejection(p.blockedKey ?: "", p.blockedDetail ?: ""), color = Palette.negative, fontSize = 13.sp)
                    p != null -> Text("Your cash ${Format.signed(p.cashChangeCents)}", color = Palette.secondary, fontSize = 13.sp)
                }
            }
        }
    }
}
