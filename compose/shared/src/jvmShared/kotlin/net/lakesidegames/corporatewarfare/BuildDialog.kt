package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.BuildQuoteDto
import uniffi.cw_ffi.RegionBoardDto
import uniffi.cw_ffi.StrategyDto

/**
 * Build plants in a region: choose a sector and strategy, the number of plants, see the quote.
 * With [initialRecipe] (a strategy index) the dialog opens on that strategy with [initialBlocks]
 * plants already chosen, as when a freight suggestion hands over its build.
 */
@Composable
fun BuildDialog(store: GameStore, board: RegionBoardDto, initialRecipe: Int? = null, initialBlocks: Int = 1, onClose: () -> Unit) {
    val preset = initialRecipe?.let { index -> store.strategyCatalogue.firstOrNull { it.index == index.toUInt() } }
    var sector by remember { mutableStateOf(preset?.industry) }
    var strategy by remember { mutableStateOf(preset) }
    var blocks by remember { mutableIntStateOf(initialBlocks.coerceIn(1, 20)) }
    var quote by remember { mutableStateOf<BuildQuoteDto?>(null) }

    LaunchedEffect(strategy, blocks) {
        quote = null
        strategy?.let { chosen -> store.quote(board.index, chosen.index, blocks.toUInt()) { quote = it } }
    }

    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Build in ${board.name}") },
        text = {
            LazyColumn(Modifier.height(460.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val chosenSector = sector
                val chosenStrategy = strategy
                when {
                    chosenSector == null -> {
                        item { Text("Choose a sector. A division runs one strategy; specialized states make it cheaper.", color = Palette.secondary, fontSize = 12.sp) }
                        val sectors = Sectors.order.filter { id -> store.strategyCatalogue.any { it.industry == id } }
                        items(sectors.size) { index ->
                            val id = sectors[index]
                            Row(
                                Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised)
                                    .clickable { sector = id }.padding(10.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                SectorBadge(id, 24)
                                Spacer(Modifier.width(10.dp))
                                Text(Sectors.name(id), color = Palette.primaryText, modifier = Modifier.weight(1f))
                                if (id in board.specialization) SignalPill("Specialized", Palette.positive)
                            }
                        }
                    }
                    chosenStrategy == null -> {
                        item { TextButton(onClick = { sector = null }) { BackLabel("Sectors", Palette.gold) } }
                        val options = store.strategyCatalogue.filter { it.industry == chosenSector }
                        items(options.size) { index ->
                            val option = options[index]
                            Column(
                                Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised)
                                    .clickable { strategy = option }.padding(10.dp),
                            ) {
                                Text(option.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                                Text(
                                    "${option.weeklyOutputLots} lots of ${option.outputName}${if (option.byProductsText.isEmpty()) "" else " " + option.byProductsText} a week · ${Format.compact(option.capexCents)} a plant · ${option.buildWeeks} weeks · ${option.referenceMarginBps / 100}% margin",
                                    color = Palette.secondary,
                                    fontSize = 12.sp,
                                )
                            }
                        }
                    }
                    else -> {
                        item { TextButton(onClick = { strategy = null }) { BackLabel("Strategies", Palette.gold) } }
                        item { Text(chosenStrategy.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold) }
                        item {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text("Plants", color = Palette.secondary, modifier = Modifier.weight(1f))
                                OutlinedButton(onClick = { if (blocks > 1) blocks-- }) { PixelIcon("minus", size = 16.dp, tint = Palette.primaryText) }
                                Text("  $blocks  ", color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                                OutlinedButton(onClick = { if (blocks < 20) blocks++ }) { PixelIcon("plus", size = 16.dp, tint = Palette.primaryText) }
                            }
                        }
                        val current = quote
                        if (current == null) {
                            item { Text("Pricing the build…", color = Palette.secondary) }
                        } else {
                            item {
                                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                    if (current.opensDivision) QuoteRow("Entry fee", Format.money(current.entryFeeCents))
                                    QuoteRow(if (current.opensDivision) "Plants (founding plant at half price)" else "Plants", Format.money(current.capexCents))
                                    QuoteRow("Total", Format.money(current.totalCents))
                                    QuoteRow("Pay now", Format.money(current.entryFeeCents + current.capexCents * 40 / 100))
                                    QuoteRow("On delivery", Format.money(current.capexCents - current.capexCents * 40 / 100))
                                    QuoteRow("Your share after", "${current.shareAfterBps / 100}%")
                                    QuoteRow("Online in", "${chosenStrategy.buildWeeks} weeks")
                                    current.lockedStrategyName?.let {
                                        Text("Your division here runs $it; a new plant joins that strategy.", color = Palette.warning, fontSize = 12.sp)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            val chosen = strategy
            GoldButton("Build", enabled = chosen != null && quote != null && !store.busy) {
                if (chosen != null) store.build(board.index, chosen.index, blocks.toUInt())
                onClose()
            }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
    )
}

@Composable
private fun QuoteRow(label: String, value: String) {
    Row {
        Text(label, color = Palette.secondary, fontSize = 13.sp, modifier = Modifier.weight(1f))
        Text(value, color = Palette.primaryText, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
    }
}
