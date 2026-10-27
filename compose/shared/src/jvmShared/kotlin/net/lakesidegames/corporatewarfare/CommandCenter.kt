package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.cw_ffi.ActionDto
import uniffi.cw_ffi.FieldDto
import uniffi.cw_ffi.PreviewDto

/** The engine's sector order: a sector field's value is the position in this list. */
private val EngineSectors = listOf(
    "agriculture", "extraction", "energy", "chemical_industries", "manufacturing",
    "construction", "logistics", "retail", "healthcare", "financial", "technology", "media",
    "telecommunications", "real_estate", "defense",
)

/**
 * Every action in the game, searchable. Pick one to get its form: pickers, sliders and switches
 * generated from the engine's catalogue, a live preview of what it would do, and Submit.
 */
@Composable
fun CommandCenterScreen(store: GameStore, initialQuery: String = "", onClose: (() -> Unit)? = null) {
    var query by remember { mutableStateOf(initialQuery) }
    var category by remember { mutableStateOf<String?>(null) }
    val all = store.actions
    val matches = all.filter { action ->
        (category == null || action.category == category) &&
            (query.isBlank() || query.trim().split(' ').all { word ->
                listOf(action.title, action.summary, action.keywords, action.category).any { it.contains(word, ignoreCase = true) }
            })
    }
    var chosen by remember { mutableStateOf(if (initialQuery.isNotBlank()) matches.firstOrNull() else null) }
    BoxWithConstraints(Modifier.fillMaxSize().background(Palette.ink)) {
        val wide = maxWidth >= 900.dp
        if (!wide && chosen != null) {
            ActionForm(store, chosen!!, Modifier.fillMaxSize()) { chosen = null }
            return@BoxWithConstraints
        }
        Row(Modifier.fillMaxSize().padding(10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Column(
                Modifier.then(if (wide) Modifier.width(440.dp) else Modifier.fillMaxWidth()).fillMaxHeight()
                    .clip(RoundedCornerShape(10.dp)).background(Palette.panel).border(1.dp, Palette.stroke, RoundedCornerShape(10.dp)),
            ) {
                Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Actions", color = Palette.primaryText, fontSize = 18.sp, fontWeight = FontWeight.Bold, modifier = Modifier.weight(1f))
                    Text("${matches.size} of ${all.size}", color = Palette.secondary, fontSize = 12.sp)
                    if (onClose != null) TextButton(onClick = onClose) { Text("Done", color = Palette.gold) }
                }
                OutlinedTextField(
                    query, { query = it },
                    placeholder = { Text("Search: IPO, loan, build, price, research…", color = Palette.secondary) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
                    colors = fieldColors(),
                )
                FlowRow(Modifier.padding(12.dp), horizontalArrangement = Arrangement.spacedBy(5.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
                    Chip("All", category == null) { category = null }
                    all.map { it.category }.distinct().forEach { c -> Chip(c, category == c, categoryIcon(c)) { category = if (category == c) null else c } }
                }
                Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
                    matches.groupBy { it.category }.forEach { (group, actions) ->
                        Row(Modifier.padding(start = 14.dp, top = 10.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            PixelIcon(categoryIcon(group), size = 14.dp)
                            Spacer(Modifier.width(6.dp))
                            Text(
                                group.uppercase() + "  ${actions.size}", color = Palette.secondary, fontSize = 10.sp, fontWeight = FontWeight.SemiBold,
                                letterSpacing = 0.8.sp,
                            )
                        }
                        actions.forEach { action ->
                            val on = chosen?.command == action.command
                            Column(
                                Modifier.fillMaxWidth().background(if (on) Palette.gold.copy(alpha = 0.14f) else androidx.compose.ui.graphics.Color.Transparent)
                                    .clickable { chosen = action }.padding(horizontal = 14.dp, vertical = 8.dp),
                            ) {
                                Text(action.title, color = if (on) Palette.gold else Palette.primaryText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                                Text(action.summary, color = Palette.secondary, fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                    if (matches.isEmpty()) Text("Nothing matches. Try another word.", color = Palette.secondary, fontSize = 13.sp, modifier = Modifier.padding(14.dp))
                }
            }
            if (wide) {
                val action = chosen
                if (action == null) {
                    Box(Modifier.weight(1f).fillMaxHeight(), contentAlignment = Alignment.Center) {
                        Text("Pick an action on the left to see its form.", color = Palette.secondary, fontSize = 14.sp)
                    }
                } else {
                    androidx.compose.runtime.key(action.command) {
                        ActionForm(store, action, Modifier.weight(1f).fillMaxHeight(), onBack = null)
                    }
                }
            }
        }
    }
}

@Composable
private fun Chip(text: String, on: Boolean, icon: String? = null, onClick: () -> Unit) {
    Row(
        Modifier.clip(CircleShape).background(if (on) Palette.gold else Palette.panelRaised).clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        if (icon != null) PixelIcon(icon, size = 14.dp)
        Text(text, color = if (on) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
    }
}

/** The pixel icon for a Command Center category. */
internal fun categoryIcon(category: String): String = "cat_" + category.lowercase().replace(' ', '_')

@Composable
private fun fieldColors() = OutlinedTextFieldDefaults.colors(
    focusedTextColor = Palette.primaryText,
    unfocusedTextColor = Palette.primaryText,
    focusedBorderColor = Palette.gold,
    unfocusedBorderColor = Palette.stroke,
    cursorColor = Palette.gold,
)

/** One option of a picker field: the value sent to the engine and what the player sees. */
private data class Pick(val value: String, val label: String)

/** The options a picker field offers, from the store. */
private fun picks(store: GameStore, field: FieldDto, pickers: Pickers, firm: ULong?): List<Pick> = when (field.kind) {
    "my_firm" -> store.firms.filter { it.controlled }.map { Pick("${it.handle}", it.name) }
    "any_firm" -> store.firms.sortedBy { !it.controlled }.map { Pick("${it.handle}", it.name + if (it.controlled) " (yours)" else "") }
    "bank", "insurer" -> store.firms.filter { it.industry == "financial" }.map { Pick("${it.handle}", it.name) }
    "region" -> store.boards.sortedBy { it.index }.map { Pick("${it.index}", "${it.name}, ${it.stateCode}") }
    "product" -> store.productCatalogue.filter { it.referencePriceCents > 0 }.map { Pick("${it.index}", it.name) }
    "recipe" -> store.strategyCatalogue.map { Pick("${it.index}", "${it.name} (${Sectors.name(it.industry)})") }
    "sector" -> EngineSectors.mapIndexed { i, s -> Pick("$i", Sectors.name(s)) }
    "my_facility" -> store.facilities.map { Pick("${it.handle}", "${store.strategyCatalogue.firstOrNull { s -> s.id == it.recipe }?.name ?: it.recipe} · ${it.region} · ${it.blocks} plants") }
    "my_loan" -> pickers.loans.map { Pick("${it.handle}", "${Format.compact(it.principalCents)} from ${it.lender} at ${"%.2f".format(it.rateBps / 100.0)}%") }
    "my_contract" -> pickers.contracts.map { Pick("${it.handle}", "${it.product}: ${it.seller} to ${it.buyer}, ${it.quantityWeekly} a week") }
    "my_policy" -> pickers.policies.map { Pick("${it.handle}", "${it.insurer} cover, ${Format.compact(it.limitCents)}") }
    "vote" -> pickers.votes.filter { !it.resolved }.map { Pick("${it.id}", "${it.subject}, closes week ${it.deadline}") }
    "my_deal" -> pickers.deals.map { Pick("${it.handle}", "${it.buyer} for ${it.target} (${it.state})") }
    "bond", "my_bond", "my_venture", "open_campaign" -> pickers.extra[field.kind].orEmpty().map { Pick(it.value, it.label) }
    else -> emptyList()
}.let { if (field.encoding == "optional") listOf(Pick("", "None")) + it else it }

/** The default raw value of a field. */
private fun defaultOf(store: GameStore, field: FieldDto): String = when {
    field.kind == "my_firm" -> store.selectedFirm?.toString() ?: ""
    field.encoding == "optional" -> ""
    field.kind == "bool" -> if (field.defaultValue != 0L) "true" else "false"
    field.kind == "choice" -> field.choices.firstOrNull()?.value ?: ""
    field.kind == "text" -> ""
    field.kind in setOf("money", "lots", "shares", "bps", "weeks", "count") -> "${field.defaultValue.coerceAtLeast(field.min)}"
    else -> ""
}

/** The upper end of a numeric slider when the catalogue leaves it open. */
private fun ceilingOf(store: GameStore, field: FieldDto): Long {
    if (field.max > 0) return field.max
    if (field.kind == "money" && field.key.contains("price")) return 100_000
    return when (field.kind) {
        "money" -> maxOf(store.selectedFirmDto?.cashCents ?: 0, store.overview?.investorCashCents ?: 0, 100_000_000L)
        "shares" -> 10_000_000
        "lots" -> 10_000
        "weeks" -> 520
        else -> 100
    }
}

/** JSON for the action from the raw field values; null while a required field is empty. */
private fun buildJson(action: ActionDto, values: Map<String, String>): String? {
    val parts = action.fields.map { field ->
        val raw = values[field.key].orEmpty()
        val encoded = when (field.encoding) {
            "bool" -> if (raw == "true") "true" else "false"
            "string" -> if (raw.isEmpty()) return null else "\"" + raw.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
            "optional" -> raw.ifEmpty { "null" }
            else -> raw.toLongOrNull()?.toString() ?: return null
        }
        "\"${field.key}\":$encoded"
    }
    return "{\"${action.command}\":{${parts.joinToString(",")}}}"
}

/** The generated form for one action. */
@Composable
private fun ActionForm(store: GameStore, action: ActionDto, modifier: Modifier, onBack: (() -> Unit)?) {
    val values = remember(action.command) { mutableStateMapOf<String, String>().apply { action.fields.forEach { put(it.key, defaultOf(store, it)) } } }
    var pickers by remember { mutableStateOf(Pickers()) }
    var preview by remember { mutableStateOf<PreviewDto?>(null) }
    var result by remember { mutableStateOf<Pair<Boolean, String>?>(null) }
    val firmField = action.fields.firstOrNull { it.kind == "my_firm" }
    val firm = firmField?.let { values[it.key]?.toULongOrNull() } ?: store.selectedFirm
    LaunchedEffect(firm) { store.pickers(firm) { pickers = it } }
    val json = buildJson(action, values)
    LaunchedEffect(json) {
        preview = null
        if (json == null) return@LaunchedEffect
        delay(300)
        store.preview(json) { preview = it }
    }
    Column(
        modifier.clip(RoundedCornerShape(10.dp)).background(Palette.panel).border(1.dp, Palette.stroke, RoundedCornerShape(10.dp))
            .verticalScroll(rememberScrollState()).padding(18.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (onBack != null) TextButton(onClick = onBack) {
                PixelIcon("back", size = 14.dp, tint = Palette.secondary)
                Spacer(Modifier.width(6.dp))
                Text("Actions", color = Palette.secondary)
            }
            Column(Modifier.weight(1f)) {
                Eyebrow(action.category)
                Text(action.title, color = Palette.primaryText, fontSize = 22.sp, fontWeight = FontWeight.Bold)
            }
        }
        Text(action.summary, color = Palette.secondary, fontSize = 14.sp)
        action.fields.forEach { field -> FieldControl(store, field, values, pickers, firm) }
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(10.dp)).background(Palette.ink).padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Eyebrow("What it would do")
            val p = preview
            when {
                json == null -> Text("Fill in every field to see the effect.", color = Palette.secondary, fontSize = 13.sp)
                p == null -> Text("Working it out…", color = Palette.secondary, fontSize = 13.sp)
                p.blockedKey != null -> Text("Not allowed: " + Messages.rejection(p.blockedKey ?: "", p.blockedDetail ?: ""), color = Palette.negative, fontSize = 13.sp)
                else -> {
                    if (p.cashChangeCents != 0L) {
                        Text("Cash ${Format.signed(p.cashChangeCents)}", color = if (p.cashChangeCents >= 0) Palette.positive else Palette.negative, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                    } else {
                        Text("No immediate cash change.", color = Palette.primaryText, fontSize = 13.sp)
                    }
                    p.warnings.forEach {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            PixelIcon("warning", size = 14.dp)
                            Text(dollars(it), color = Palette.warning, fontSize = 12.sp)
                        }
                    }
                    p.assumptions.forEach { Text("· ${dollars(it)}", color = Palette.secondary, fontSize = 12.sp) }
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            GoldButton("Submit", enabled = json != null && preview?.blockedKey == null) {
                val text = json ?: return@GoldButton
                result = null
                store.act(text) { ok, message -> result = ok to message }
            }
            Spacer(Modifier.width(12.dp))
            result?.let { (ok, message) -> Text(message, color = if (ok) Palette.positive else Palette.negative, fontSize = 13.sp) }
        }
    }
}

/** One field's control, chosen by its kind. */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun FieldControl(store: GameStore, field: FieldDto, values: MutableMap<String, String>, pickers: Pickers, firm: ULong?) {
    val raw = values[field.key].orEmpty()
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(field.label, color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        when (field.kind) {
            "bool" -> Switch(
                checked = raw == "true", onCheckedChange = { values[field.key] = it.toString() },
                colors = SwitchDefaults.colors(checkedTrackColor = Palette.gold),
            )
            "choice" -> FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                field.choices.forEach { option -> Chip(option.label, raw == option.value) { values[field.key] = option.value } }
            }
            "text" -> OutlinedTextField(
                raw, { values[field.key] = it }, singleLine = true, modifier = Modifier.fillMaxWidth(), colors = fieldColors(),
                placeholder = { Text(if (field.key == "name") "Your company's name" else "", color = Palette.secondary) },
            )
            "money", "lots", "shares", "weeks", "count", "bps" -> NumberControl(store, field, raw) { values[field.key] = it }
            else -> {
                val options = picks(store, field, pickers, firm)
                Dropdown(options, raw) { values[field.key] = it }
            }
        }
        if (field.help.isNotEmpty()) Text(field.help, color = Palette.secondary, fontSize = 12.sp)
    }
}

@Composable
private fun Dropdown(options: List<Pick>, selected: String, onPick: (String) -> Unit) {
    var open by remember { mutableStateOf(false) }
    val label = options.firstOrNull { it.value == selected }?.label ?: if (options.isEmpty()) "Nothing to choose yet" else "Choose…"
    Box {
        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(Palette.ink).border(1.dp, Palette.stroke, RoundedCornerShape(8.dp))
                .clickable(enabled = options.isNotEmpty()) { open = true }.padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(label, color = if (selected.isEmpty() && options.none { it.value.isEmpty() }) Palette.secondary else Palette.primaryText, fontSize = 14.sp, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
            PixelIcon("chevron_down", size = 14.dp, tint = Palette.secondary)
        }
        DropdownMenu(open, { open = false }, Modifier.heightIn(max = 420.dp).background(Palette.panelRaised)) {
            options.forEach { option ->
                DropdownMenuItem(
                    text = { Text(option.label, color = if (option.value == selected) Palette.gold else Palette.primaryText, fontSize = 13.sp) },
                    onClick = { onPick(option.value); open = false },
                )
            }
        }
    }
}

/** A slider with an exact box beside it. Money is shown in dollars, percentages in percent. */
@Composable
private fun NumberControl(store: GameStore, field: FieldDto, raw: String, onValue: (String) -> Unit) {
    val value = raw.toLongOrNull() ?: field.defaultValue
    val ceiling = ceilingOf(store, field)
    val (scale, label) = when (field.kind) {
        "money" -> 100.0 to { v: Double -> Format.money((v * 100).toLong()) }
        "bps" -> 100.0 to { v: Double -> "%.2f%%".format(v) }
        "weeks" -> 1.0 to { v: Double -> "${v.toLong()} weeks" }
        "shares" -> 1.0 to { v: Double -> "${Format.count(v.toLong())} shares" }
        "lots" -> 1.0 to { v: Double -> "${Format.count(v.toLong())} lots" }
        else -> 1.0 to { v: Double -> Format.count(v.toLong()) }
    }
    val low = field.min / scale
    val high = (ceiling / scale).coerceAtLeast(low + 1)
    val step = (field.step.coerceAtLeast(1) / scale).coerceAtLeast(if (field.kind == "money") 1.0 else 0.01)
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        ValueSlider(
            (value / scale).coerceIn(low, high), { onValue(((it * scale).toLong()).toString()) }, low..high, step,
            label = label, modifier = Modifier.weight(1f),
        )
        OutlinedTextField(
            if (field.kind == "money") (value / 100).toString() else if (field.kind == "bps") "%.2f".format(value / 100.0) else value.toString(),
            { text ->
                val number = text.replace(",", "").toDoubleOrNull() ?: return@OutlinedTextField
                onValue((number * if (field.kind == "money" || field.kind == "bps") 100 else 1).toLong().toString())
            },
            singleLine = true,
            modifier = Modifier.width(140.dp),
            colors = fieldColors(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            prefix = if (field.kind == "money") ({ Text("$", color = Palette.secondary) }) else null,
            suffix = if (field.kind == "bps") ({ Text("%", color = Palette.secondary) }) else null,
        )
    }
}

/** Engine notes give money as "N cents"; show it in dollars. */
internal fun dollars(text: String): String =
    Regex("(-?\\d+) cents").replace(text) { m -> Format.money(m.groupValues[1].toLong()) }
