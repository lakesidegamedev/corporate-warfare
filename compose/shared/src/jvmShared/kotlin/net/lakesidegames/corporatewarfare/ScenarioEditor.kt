@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import uniffi.cw_ffi.CustomLawsDto
import uniffi.cw_ffi.CustomScenarioDto
import uniffi.cw_ffi.RegionAdjustDto
import uniffi.cw_ffi.RivalDto
import uniffi.cw_ffi.customScenarioDefault
import uniffi.cw_ffi.customScenarioDelete
import uniffi.cw_ffi.customScenarioList
import uniffi.cw_ffi.customScenarioRanges
import uniffi.cw_ffi.customScenarioSave
import uniffi.cw_ffi.customScenarioSummary
import uniffi.cw_ffi.scenarios

/** The engine's error text with the wrapper words taken off. */
private fun plain(failure: Throwable): String {
    val text = failure.message ?: failure.toString()
    val at = text.indexOf("detail=")
    return (if (at >= 0) text.substring(at + 7).trimEnd(')') else text).ifBlank { "That did not work." }
}

private val Difficulties = listOf("easy" to "Easy", "standard" to "Standard", "hard" to "Hard")
private val Economies = listOf("historic" to "Historic", "calm" to "Calm", "boom" to "Boom", "downturn" to "Downturn")
private val Unions = listOf("weak" to "Weak", "standard" to "Standard", "strong" to "Strong")
private val Paths = listOf("historical" to "Historical", "historical_variance" to "Historical with variance", "random" to "Random")

/**
 * Edit an existing scenario's numbers and keep the result as a custom scenario. Everything is
 * optional: a scenario left alone plays exactly like the built-in one.
 */
@Composable
internal fun ScenarioEditorScreen(store: GameStore, baseId: String?, existing: CustomScenarioDto?, onBack: () -> Unit) {
    BackAction(onBack = onBack)
    val ranges = remember { customScenarioRanges() }
    val builtIns = remember { scenarios().filter { it.available } }
    var saved by remember { mutableStateOf(customScenarioList(store.customDir)) }
    var message by remember { mutableStateOf<String?>(null) }
    var confirmDelete by remember { mutableStateOf(false) }
    var guided by remember { mutableStateOf(true) }
    var openRival by remember { mutableStateOf<String?>(null) }
    var openRegion by remember { mutableStateOf<String?>(null) }
    var draft by remember {
        mutableStateOf(
            existing ?: baseId?.let { id ->
                val title = builtIns.firstOrNull { it.id == id }?.name ?: id
                runCatching { customScenarioDefault(id, "My $title") }.getOrNull()
            },
        )
    }
    val current = draft
    val summary = remember(current) { current?.let { runCatching { customScenarioSummary(it) }.getOrNull() } }

    fun open(base: String, name: String) {
        draft = runCatching { customScenarioDefault(base, name) }.onFailure { message = plain(it) }.getOrNull()
    }

    fun save(asNew: Boolean) {
        val d = draft ?: return
        val toSave = if (asNew) d.copy(id = "", name = if (d.id.isNotEmpty() && d.name.isNotBlank()) d.name.trimEnd() + " copy" else d.name) else d
        runCatching { customScenarioSave(store.customDir, toSave) }
            .onSuccess {
                draft = it
                saved = customScenarioList(store.customDir)
                message = "Saved as ${it.name}."
            }
            .onFailure { message = plain(it) }
    }

    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val wide = maxWidth >= 900.dp
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Row(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 20.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = { if (current == null || baseId != null || existing != null) onBack() else draft = null }) { Text("‹ Back", color = Palette.secondary) }
                Spacer(Modifier.width(8.dp))
                Text("Scenario editor", color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
                if (current != null && wide) {
                    GoldButton("Save", enabled = summary?.problems.isNullOrEmpty()) { save(false) }
                }
            }
            if (current == null) {
                Column(
                    Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(if (wide) 28.dp else 16.dp),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Text("Start from a built-in scenario", color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        builtIns.forEach { s -> Card(s.name, "${s.region} · ${s.startYear}", false) { open(s.id, "My ${s.name}") } }
                    }
                    if (saved.isNotEmpty()) {
                        Text("Your scenarios", color = Palette.primaryText, fontSize = 20.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            saved.forEach { s -> Card(s.name, "Custom · ${s.difficulty}", false) { draft = s } }
                        }
                    }
                    message?.let { Text(it, color = Palette.negative, fontSize = 13.sp) }
                }
            } else {
                val d: CustomScenarioDto = current
                val body: @Composable (Boolean) -> Unit = { left ->
                    if (left) {
                        Panel {
                            Eyebrow("Name")
                            OutlinedTextField(
                                d.name, { draft = d.copy(name = it.take(ranges.nameMax.toInt())) }, singleLine = true, modifier = Modifier.textEntry().fillMaxWidth(),
                                colors = OutlinedTextFieldDefaults.colors(
                                    focusedTextColor = Palette.primaryText, unfocusedTextColor = Palette.primaryText,
                                    focusedBorderColor = Palette.gold, unfocusedBorderColor = Palette.stroke, cursorColor = Palette.gold,
                                ),
                            )
                            Text(
                                "Based on ${builtIns.firstOrNull { it.id == d.baseScenarioId }?.name ?: d.baseScenarioId}",
                                color = Palette.secondary, fontSize = 12.sp,
                            )
                        }
                        Panel {
                            Eyebrow("The start")
                            Text("Difficulty", color = Palette.secondary, fontSize = 12.sp)
                            Chips(Difficulties, d.difficulty) { draft = d.copy(difficulty = it) }
                            Text("The economy on day one", color = Palette.secondary, fontSize = 12.sp)
                            Chips(Economies, d.economy) { draft = d.copy(economy = it) }
                            Text("World price path", color = Palette.secondary, fontSize = 12.sp)
                            Chips(Paths, d.pricePath) { draft = d.copy(pricePath = it) }
                            Text("How oil, steel, grain and other world prices move. Historical is fixed to the real dates; variance shifts the timing and depth with the game seed; random stays neutral.", color = Palette.secondary, fontSize = 11.sp)
                        }
                        Panel {
                            Eyebrow("Starting capital")
                            val standard = summary?.startingCapitalCents ?: 0L
                            Chips(listOf("std" to "Standard for difficulty", "set" to "Set an amount"), if (d.startingCapitalCents == null) "std" else "set") {
                                draft = d.copy(startingCapitalCents = if (it == "std") null else standard.coerceIn(ranges.capitalMinCents, ranges.capitalMaxCents))
                            }
                            d.startingCapitalCents?.let { cents ->
                                val top = minOf(ranges.capitalMaxCents / 100.0, 50_000_000.0)
                                ValueSlider(
                                    (cents / 100.0).coerceAtMost(top), { draft = d.copy(startingCapitalCents = (it * 100).toLong()) },
                                    (ranges.capitalMinCents / 100.0)..top, 250_000.0,
                                    label = { Format.compact((it * 100).toLong()) }, ticks = 20,
                                )
                            } ?: Text(Format.compact(standard), color = Palette.primaryText, fontSize = 18.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                        }
                        val laws = d.laws
                        if (laws != null) {
                            fun set(next: CustomLawsDto) { draft = d.copy(laws = next) }
                            Panel {
                                Eyebrow("Laws in force at the start")
                                Text("Corporate tax", color = Palette.secondary, fontSize = 12.sp)
                                ValueSlider(
                                    laws.corporateTaxBps.toDouble(), { set(laws.copy(corporateTaxBps = it.toInt())) },
                                    ranges.taxMinBps.toDouble()..ranges.taxMaxBps.toDouble(), 100.0, label = { "%.0f%%".format(it / 100) }, ticks = 30,
                                )
                                Text("Wage floor, a week", color = Palette.secondary, fontSize = 12.sp)
                                ValueSlider(
                                    laws.wageFloorCents.toDouble(), { set(laws.copy(wageFloorCents = it.toLong())) },
                                    ranges.wageFloorMinCents.toDouble()..ranges.wageFloorMaxCents.toDouble(), 1_000.0, label = { Format.money(it.toLong()) }, ticks = 20,
                                )
                                Text("Merger review starts at this combined regional share", color = Palette.secondary, fontSize = 12.sp)
                                ValueSlider(
                                    laws.mergerReviewBps.toDouble(), { set(laws.copy(mergerReviewBps = it.toLong())) },
                                    ranges.mergerMinBps.toDouble()..ranges.mergerMaxBps.toDouble(), 100.0, label = { "%.0f%%".format(it / 100) }, ticks = 18,
                                )
                                Text("Unions", color = Palette.secondary, fontSize = 12.sp)
                                Chips(Unions, laws.unions) { set(laws.copy(unions = it)) }
                            }
                        }
                    } else {
                        Panel {
                            Eyebrow("Rivals (${d.rivals.count { it.enabled }} of ${d.rivals.size} start)")
                            d.rivals.forEach { r ->
                                val isOpen = openRival == r.firmId
                                fun upd(next: RivalDto) { draft = d.copy(rivals = d.rivals.map { if (it.firmId == r.firmId) next else it }) }
                                Row(
                                    Modifier.fillMaxWidth().clip(Pixel.shape).clickable { openRival = if (isOpen) null else r.firmId }.padding(vertical = 4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Column(Modifier.weight(1f)) {
                                        Text(r.name, color = if (r.enabled) Palette.primaryText else Palette.secondary, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                                        Text(
                                            Sectors.name(r.industry) + if (r.enabled) " · size ${r.sizePct}% · cash ${r.cashPct}%" else " · does not start",
                                            color = Palette.secondary, fontSize = 11.sp,
                                        )
                                    }
                                    TextButton(onClick = { upd(r.copy(enabled = !r.enabled)) }) { Text(if (r.enabled) "Remove" else "Add back", color = Palette.gold) }
                                }
                                if (isOpen && r.enabled) {
                                    Text("Size", color = Palette.secondary, fontSize = 12.sp)
                                    ValueSlider(
                                        r.sizePct.toDouble(), { upd(r.copy(sizePct = it.toInt().toUShort())) },
                                        ranges.rivalSizeMin.toDouble()..ranges.rivalSizeMax.toDouble(), 5.0,
                                        label = { "${it.toInt()}% of standard" }, origin = 100.0, ticks = 22,
                                    )
                                    Text("Opening cash", color = Palette.secondary, fontSize = 12.sp)
                                    ValueSlider(
                                        r.cashPct.toDouble(), { upd(r.copy(cashPct = it.toInt().toUShort())) },
                                        ranges.rivalCashMin.toDouble()..ranges.rivalCashMax.toDouble(), 5.0,
                                        label = { "${it.toInt()}% of standard" }, origin = 100.0, ticks = 25,
                                    )
                                }
                            }
                        }
                        Panel {
                            Eyebrow("Regions")
                            d.regions.forEach { g ->
                                val isOpen = openRegion == g.code
                                fun upd(next: RegionAdjustDto) { draft = d.copy(regions = d.regions.map { if (it.code == g.code) next else it }) }
                                Column(Modifier.fillMaxWidth().clip(Pixel.shape).clickable { openRegion = if (isOpen) null else g.code }.padding(vertical = 4.dp)) {
                                    Text(g.name, color = Palette.primaryText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                                    Text("Wages ${g.wagePct}% · demand ${g.demandPct}%", color = Palette.secondary, fontSize = 11.sp)
                                }
                                if (isOpen) {
                                    Text("Wages", color = Palette.secondary, fontSize = 12.sp)
                                    ValueSlider(
                                        g.wagePct.toDouble(), { upd(g.copy(wagePct = it.toInt().toUShort())) },
                                        ranges.regionPctMin.toDouble()..ranges.regionPctMax.toDouble(), 5.0,
                                        label = { "${it.toInt()}% of standard" }, origin = 100.0, ticks = 30,
                                    )
                                    Text("Household demand", color = Palette.secondary, fontSize = 12.sp)
                                    ValueSlider(
                                        g.demandPct.toDouble(), { upd(g.copy(demandPct = it.toInt().toUShort())) },
                                        ranges.regionPctMin.toDouble()..ranges.regionPctMax.toDouble(), 5.0,
                                        label = { "${it.toInt()}% of standard" }, origin = 100.0, ticks = 30,
                                    )
                                }
                            }
                        }
                    }
                }
                Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(if (wide) 24.dp else 12.dp)) {
                    if (wide) {
                        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) { body(true) }
                            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) { body(false) }
                        }
                    } else {
                        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                            body(true)
                            body(false)
                        }
                    }
                }
                Column(Modifier.fillMaxWidth().background(Palette.panel).padding(horizontal = 20.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    val problems = summary?.problems.orEmpty()
                    Text(
                        summary?.let {
                            "${d.name.ifBlank { "Unnamed" }} · ${d.difficulty} · ${d.economy} economy · ${Format.compact(it.startingCapitalCents)} · " +
                                "${it.rivalsStarted} of ${it.rivalsTotal} rivals · wages ${it.averageWagePct}% · demand ${it.averageDemandPct}%"
                        } ?: "",
                        color = Palette.secondary, fontSize = 13.sp,
                    )
                    (problems.firstOrNull() ?: message)?.let {
                        Text(it, color = if (problems.isNotEmpty()) Palette.negative else Palette.positive, fontSize = 12.sp)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Chips(listOf("g" to "Run a company", "f" to "Found your own"), if (guided) "g" else "f") { guided = it == "g" }
                        Spacer(Modifier.weight(1f))
                        if (d.id.isNotEmpty()) TextButton(onClick = { confirmDelete = true }) { Text("Delete", color = Palette.negative) }
                        if (!wide || d.id.isNotEmpty()) TextButton(onClick = { save(true) }, enabled = problems.isEmpty()) { Text("Save as", color = Palette.gold) }
                        if (!wide) TextButton(onClick = { save(false) }, enabled = problems.isEmpty()) { Text("Save", color = Palette.gold) }
                        GoldButton("Start game", enabled = problems.isEmpty() && !store.busy) { store.startCustom(d, guided, false) }
                    }
                }
                if (confirmDelete) {
                    AlertDialog(
                        onDismissRequest = { confirmDelete = false },
                        confirmButton = {
                            TextButton(onClick = {
                                confirmDelete = false
                                runCatching { customScenarioDelete(store.customDir, d.id) }
                                    .onSuccess { saved = customScenarioList(store.customDir); draft = null; message = null; if (baseId != null || existing != null) onBack() }
                                    .onFailure { message = plain(it) }
                            }) { Text("Delete", color = Palette.negative) }
                        },
                        dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Keep it") } },
                        title = { Text("Delete ${d.name}?") },
                        text = { Text("The saved scenario is removed. Games already started from it are not affected.") },
                        containerColor = Palette.panel,
                    )
                }
            }
        }
    }
}

@Composable
private fun Card(title: String, note: String, on: Boolean, onClick: () -> Unit) {
    val shape = Pixel.shape
    Column(
        Modifier.width(300.dp).clip(shape).background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.panel)
            .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke, shape).clickable(onClick = onClick).padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(title, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
        Text(note, color = Palette.secondary, fontSize = 12.sp)
    }
}

@Composable
private fun Chips(items: List<Pair<String, String>>, selected: String, onPick: (String) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        items.forEach { (key, title) ->
            val on = key == selected
            val shape = Pixel.shape
            Text(
                title, color = if (on) Palette.gold else Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
                modifier = Modifier.clip(shape).background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.ink)
                    .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke, shape)
                    .clickable { onPick(key) }.padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }
    }
}
