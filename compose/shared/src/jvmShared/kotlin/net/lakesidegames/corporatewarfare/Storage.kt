package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
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
import uniffi.cw_ffi.CommodityDto
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.StorageCellDto

/** The company's space in a region, one dense line per class: used of capacity, and any overflow. */
@Composable
internal fun StorageLines(cells: List<StorageCellDto>, modifier: Modifier = Modifier) {
    if (cells.isEmpty()) return
    Column(modifier, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        cells.forEach { cell ->
            val over = cell.outsideLots > 0
            val tint = if (over) Palette.warning else Palette.primaryText
            Row(verticalAlignment = Alignment.CenterVertically) {
                PixelIcon("storage", size = 12.dp, tint = if (over) Palette.warning else Palette.secondary, modifier = Modifier.padding(end = 6.dp))
                val outside = if (over) " · ${Format.count(cell.outsideLots)} outside at ${Format.money(cell.outsideFeeCentsWeekly)} a week" else ""
                Text(
                    "${cell.classLabel} ${Format.count(cell.usedLots)} of ${Format.count(cell.capacityLots)} lots$outside",
                    color = tint, fontSize = 12.sp,
                )
            }
        }
    }
}

/** The stock target of one division: weeks of inputs to keep, one slider. Zero keeps the company default. */
@Composable
internal fun DivisionStock(store: GameStore, division: DivisionDto) {
    var weeks by remember(division.region, division.industry, division.stockWeeks) { mutableStateOf(division.stockWeeks.toDouble()) }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("storage", size = 12.dp, tint = Palette.secondary, modifier = Modifier.padding(end = 6.dp))
            Eyebrow("Stock target · ${division.regionName}")
        }
        ValueSlider(
            weeks, { weeks = it }, 0.0..26.0, 1.0,
            label = { if (it < 0.5) "Company default: ${division.stockWeeksDefault} weeks" else "${it.toInt()} weeks of inputs" },
            detail = { "How many weeks of inputs this division keeps on hand. Perishable inputs keep at most 2 weeks unless you set a target. Grain is also bought ahead in the harvest quarter." },
            onCommit = { store.setStockTarget(division.plants, it.toInt(), division.seasonalBuild) },
        )
        StorageLines(division.storage)
    }
}

private fun classLabel(key: String): String = when (key) {
    "tank" -> "tank farm"
    "silo" -> "silo"
    "cold" -> "cold store"
    else -> "warehouse"
}

/** Buy a good on the exchange into storage, and release what is held, from the good's market card. */
@Composable
internal fun StoreGoodSection(store: GameStore, line: CommodityDto) {
    val info = store.productCatalogue.firstOrNull { it.index == line.product } ?: return
    if (info.storageClass.isEmpty() || store.selectedFirm == null) return
    val places = store.divisions.map { it.region to it.regionName }.distinct()
    if (places.isEmpty()) return
    var where by remember { mutableStateOf(places.first().first) }
    var lots by remember { mutableStateOf(100.0) }
    var limit by remember { mutableStateOf(100.0) }
    val held = store.held.firstOrNull { it.product == line.product && it.region == where }
    HorizontalDivider(color = Palette.stroke)
    Row(verticalAlignment = Alignment.CenterVertically) {
        PixelIcon("storage", size = 14.dp, tint = Palette.secondary, modifier = Modifier.padding(end = 6.dp))
        Eyebrow("Store")
    }
    Text(
        "Needs ${classLabel(info.storageClass)} space. Holding costs ${"%.2f".format(info.holdingBpsWeekly.toInt() / 100.0)}% a week beyond a plant's yard" +
            if (info.spoilageBpsWeekly > 0u) "; ${"%.1f".format(info.spoilageBpsWeekly.toInt() / 100.0)}% spoils each week." else ".",
        color = Palette.secondary, fontSize = 11.sp,
    )
    if (places.size > 1) {
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            places.forEach { (code, name) ->
                Text(
                    name, color = if (code == where) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.clip(Pixel.small).background(if (code == where) Palette.gold else Palette.panelRaised)
                        .clickable { where = code }.padding(horizontal = 10.dp, vertical = 4.dp),
                )
            }
        }
    }
    if (held != null && (held.heldLots > 0 || held.orderLotsLeft > 0)) {
        val order = if (held.orderLotsLeft > 0) " · ${Format.count(held.orderLotsLeft)} lots still on order" else ""
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Stored ${Format.count(held.heldLots)} lots$order", color = Palette.gold, fontSize = 12.sp, modifier = Modifier.weight(1f))
            TextButton(onClick = { store.releaseStored(line.product, store.regionIndex(where), 0) }) { Text("Release all") }
        }
    }
    ValueSlider(lots, { lots = it }, 10.0..2_000.0, 10.0, label = { "${Format.count(it.toLong())} lots" })
    ValueSlider(
        limit, { limit = it }, 50.0..200.0, 5.0,
        label = { "Pay up to ${it.toInt()}% of reference" },
        detail = { "Fills over the following weeks from the cheapest sellers, never above this price a lot." },
    )
    Row(verticalAlignment = Alignment.CenterVertically) {
        Spacer(Modifier.weight(1f))
        GoldButton("Buy to store", Modifier.width(150.dp)) {
            store.buyToStore(line.product, store.regionIndex(where), lots.toLong(), (limit * 100).toInt())
        }
    }
}
