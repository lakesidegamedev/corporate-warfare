package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.CommodityDto
import uniffi.cw_ffi.ProductLineDto
import uniffi.cw_ffi.StockDto

private enum class Section(val title: String) { Stocks("Stocks"), Commodities("Commodities"), Regions("Regions"), Structure("Structure") }
private enum class StockSort(val title: String) { Movers("Top movers"), Size("Largest"), Yours("Yours") }

/** Markets: the corridor's stock exchange, its commodity board, and each region's own board. */
@Composable
fun MarketsScreen(store: GameStore) {
    var section by remember { mutableStateOf(Section.Stocks) }
    var sort by remember { mutableStateOf(StockSort.Movers) }
    var stock by remember { mutableStateOf<StockDto?>(null) }
    var commodity by remember { mutableStateOf<CommodityDto?>(null) }
    var pickingRegion by remember { mutableStateOf(false) }
    TabList {
        item { ScreenHeader("Markets", store.overview?.weekLabel) }
        item { EconomyCard(store, collapsible = true) }
        item {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                Section.entries.forEachIndexed { index, option ->
                    SegmentedButton(
                        selected = section == option,
                        onClick = { section = option },
                        shape = SegmentedButtonDefaults.itemShape(index, Section.entries.size),
                    ) { Text(option.title, fontSize = 13.sp) }
                }
            }
        }
        when (section) {
            Section.Stocks -> stocks(store, sort, { sort = it }) { stock = it }
            Section.Commodities -> commodities(store) { commodity = it }
            Section.Regions -> regionMarkets(store) { pickingRegion = true }
            Section.Structure -> item { StructureColumn(store) }
        }
    }
    stock?.let { StockDialog(store, it) { stock = null } }
    commodity?.let { CommodityDialog(store, it) { commodity = null } }
    if (pickingRegion) RegionPicker(store) { pickingRegion = false }
}

internal fun changeText(bps: Long) = "%+.1f%%".format(bps / 100.0)
internal fun changeTint(bps: Long) = if (bps > 0) Palette.positive else if (bps < 0) Palette.negative else Palette.secondary

/** A small line of closing prices, scaled to its own range. */
@Composable
fun Sparkline(values: List<Long>, tint: Color, modifier: Modifier) {
    Canvas(modifier) {
        if (values.size < 2) return@Canvas
        val low = values.min()
        val span = maxOf(values.max() - low, 1L).toFloat()
        val path = Path()
        values.forEachIndexed { index, value ->
            val point = Offset(size.width * index / (values.size - 1), size.height * (1f - (value - low) / span))
            if (index == 0) path.moveTo(point.x, point.y) else path.lineTo(point.x, point.y)
        }
        drawPath(path, tint, style = Stroke(width = 3f, cap = StrokeCap.Round))
    }
}

/** A market-cap weighted index of the companies with a full history, 1,000 at its start. */
internal fun corridorIndex(stocks: List<StockDto>): List<Long> {
    val length = stocks.maxOfOrNull { it.historyCents.size } ?: 0
    val members = stocks.filter { it.historyCents.size == length && length > 0 }
    val base = members.sumOf { it.historyCents[0] * it.sharesIssued }
    if (base <= 0) return emptyList()
    return (0 until length).map { week -> members.sumOf { it.historyCents[week] * it.sharesIssued } * 1_000 / base }
}

private fun LazyListScope.stocks(store: GameStore, sort: StockSort, onSort: (StockSort) -> Unit, onOpen: (StockDto) -> Unit) {
    val index = corridorIndex(store.stocks)
    if (index.isNotEmpty()) {
        item {
            val last = index.last()
            val week = if (index.size > 1) (last - index[index.size - 2]) * 10_000 / maxOf(index[index.size - 2], 1) else 0
            Panel {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Eyebrow("Market index")
                        Text(Format.count(last), color = Palette.primaryText, fontSize = 26.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                        Text("${changeText(week)} this week · ${store.stocks.size} companies", color = changeTint(week), fontSize = 12.sp)
                    }
                    Sparkline(index, changeTint(last - index.first()), Modifier.width(140.dp).height(44.dp))
                }
            }
        }
    }
    item {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            StockSort.entries.forEach { option ->
                Text(
                    option.title,
                    color = if (sort == option) Palette.ink else Palette.primaryText,
                    fontSize = 12.sp,
                    fontFamily = Pixel.font,
                    modifier = Modifier.clip(Pixel.small).background(if (sort == option) Palette.gold else Palette.panel)
                        .clickable { onSort(option) }.padding(horizontal = 12.dp, vertical = 6.dp),
                )
            }
        }
    }
    val rows = when (sort) {
        StockSort.Movers -> store.stocks.sortedByDescending { kotlin.math.abs(it.changeWeekBps) }
        StockSort.Size -> store.stocks.sortedByDescending { it.marketCapCents }
        StockSort.Yours -> store.stocks.filter { it.yourShares > 0 || it.controlledByYou }
    }
    item {
        Panel {
            if (rows.isEmpty()) Text("You hold no shares yet. Open any company to buy.", color = Palette.secondary)
            rows.forEachIndexed { position, line ->
                if (position > 0) HorizontalDivider(color = Palette.stroke)
                Row(Modifier.fillMaxWidth().clickable { onOpen(line) }.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    FirmLogo(line.name, size = 28.dp)
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(line.name + if (line.controlledByYou) " ★" else "", color = Palette.primaryText, fontWeight = FontWeight.Medium, maxLines = 1)
                        Text("${Sectors.name(line.industry)} · cap ${Format.compact(line.marketCapCents)}", color = Palette.secondary, fontSize = 11.sp, maxLines = 1)
                        // Companies reporting results this week are marked on the board.
                        if (line.earnings.reportsThisWeek) Text("Reports this week · analysts: ${ratingText(line.earnings.rating)}", color = Palette.gold, fontSize = 11.sp, maxLines = 1)
                    }
                    Sparkline(line.historyCents, changeTint(line.changeQuarterBps), Modifier.width(54.dp).height(22.dp))
                    Spacer(Modifier.width(8.dp))
                    Column(horizontalAlignment = Alignment.End) {
                        Text(Format.price(line.priceCents), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                        SignalPill(changeText(line.changeWeekBps), changeTint(line.changeWeekBps))
                    }
                }
            }
        }
    }
}

private fun LazyListScope.commodities(store: GameStore, onOpen: (CommodityDto) -> Unit) {
    item {
        Text(
            "Market prices per lot: the median asking price of the companies that make each good, or the import price where nobody does.",
            color = Palette.secondary,
            fontSize = 12.sp,
        )
    }
    val bySector = store.commodities.groupBy { it.sector }
    (Sectors.order + "other").forEach { sector ->
        val rows = bySector[sector]?.sortedBy { it.name } ?: return@forEach
        item {
            Panel {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (sector != "other") {
                        SectorBadge(sector, 20)
                        Spacer(Modifier.width(8.dp))
                    }
                    Eyebrow(if (sector == "other") "Other goods" else Sectors.name(sector))
                }
                rows.forEachIndexed { position, line ->
                    if (position > 0) HorizontalDivider(color = Palette.stroke)
                    Column(Modifier.fillMaxWidth().clickable { onOpen(line) }.padding(vertical = 4.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(line.name + if (line.yours) " ★" else "", color = Palette.primaryText, modifier = Modifier.weight(1f), maxLines = 1)
                            Sparkline(line.historyCents, changeTint(line.changeQuarterBps), Modifier.width(54.dp).height(20.dp))
                            Spacer(Modifier.width(8.dp))
                            Text(Format.money(line.priceCents), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                            Spacer(Modifier.width(6.dp))
                            SignalPill(changeText(line.changeWeekBps), changeTint(line.changeWeekBps))
                        }
                        val parts = buildList {
                            add("made ${Format.count(line.madeLots)}")
                            add("sold ${Format.count(line.soldLots)}")
                            if (line.importedLots > 0) add("imported ${Format.count(line.importedLots)}")
                            if (line.exportedLots > 0) add("exported ${Format.count(line.exportedLots)}")
                        }
                        Text(parts.joinToString(" · ") + " a week", color = Palette.secondary, fontSize = 11.sp)
                    }
                }
            }
        }
    }
}

/** One region's board: every good's local price, sales, stock and imports. */
private fun LazyListScope.regionMarkets(store: GameStore, onPickRegion: () -> Unit) {
    val board = store.boards.firstOrNull { it.code == store.marketRegion }
    item {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(board?.let { "${it.stateName} · prices per lot" } ?: "", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.weight(1f))
            OutlinedButton(onClick = onPickRegion) { Text(board?.name ?: "Region", color = Palette.primaryText) }
        }
    }
    val rows = store.marketRows.filter { it.product.referencePriceCents > 0 }
    item {
        Panel {
            rows.forEachIndexed { index, row ->
                if (index > 0) HorizontalDivider(color = Palette.stroke)
                val market = row.market
                val price = market?.lastPriceCents ?: row.product.referencePriceCents
                val sold = market?.lastWeekSalesLots ?: 0
                val stock = market?.regionalStockLots ?: 0
                val imported = board?.room?.firstOrNull { it.product == market?.productId }?.importLots ?: 0
                Column(Modifier.padding(vertical = 4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(row.product.name, color = Palette.primaryText, modifier = Modifier.weight(1f), maxLines = 1)
                        when {
                            imported > 0 -> SignalPill("Imported", Palette.positive)
                            (sold == 0L && stock > 0) || (sold > 0 && stock > sold * 4) -> SignalPill("Glut", Palette.negative)
                            sold > 0 && stock < sold -> SignalPill("Tight", Palette.gold)
                            else -> {}
                        }
                        Spacer(Modifier.width(8.dp))
                        Text(Format.money(price), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                    }
                    val parts = buildList {
                        add(if (sold > 0) "${Format.count(sold)}/wk sold" else "nothing sold")
                        if (sold > 0) add("%.1f wk of stock".format(stock.toDouble() / sold))
                        if (imported > 0) add("${Format.count(imported)}/wk imported")
                    }
                    Text(parts.joinToString(" · "), color = Palette.secondary, fontSize = 12.sp)
                }
            }
        }
    }
}

@Composable
internal fun RegionPicker(store: GameStore, onClose: () -> Unit) {
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Region") },
        text = {
            LazyColumn(Modifier.height(420.dp)) {
                items(store.boards.size) { index ->
                    val item = store.boards[index]
                    Text(
                        "${item.name} · ${item.stateName}",
                        color = Palette.primaryText,
                        modifier = Modifier.fillMaxWidth().clickable {
                            store.selectRegion(item.code)
                            onClose()
                        }.padding(vertical = 10.dp),
                    )
                }
            }
        },
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
}

/** One stock: valuation, your position, and an order ticket. */
@Composable
internal fun StockDialog(store: GameStore, line: StockDto, showPage: Boolean = true, onClose: () -> Unit) {
    var page by remember { mutableStateOf(false) }
    var buy by remember { mutableStateOf(true) }
    var quantity by remember { mutableStateOf("1000") }
    var limit by remember { mutableStateOf("") }
    val annual = if (line.earningsWeeks > 0u) line.earningsCents * 52 / line.earningsWeeks.toLong() else 0L
    val pe = if (annual <= 0) "Loss" else (line.marketCapCents.toDouble() / annual).let { if (it > 200) "n/m" else "%.1f".format(it) }
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                FirmLogo(line.name, size = 32.dp)
                Text(line.name)
            }
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                StatGroup(
                    Stat("Share price", Format.price(line.priceCents), note = "${changeText(line.changeWeekBps)} this week"),
                    listOf(
                        Stat("Quarter", changeText(line.changeQuarterBps), changeTint(line.changeQuarterBps)),
                        Stat("Market cap", Format.compact(line.marketCapCents)),
                        Stat("P/E", pe),
                        Stat("Fair value", Format.price(line.fairCents)),
                        Stat("Free float", "%.1f%%".format(line.floatBps / 100.0)),
                        Stat("You hold", Format.count(line.yourShares), note = Format.compact(line.yourShares * line.priceCents)),
                    ),
                )
                StockHistoryChart(store, line.firm)
                EarningsSection(line.earnings, null)
                DossierSection(store, line.firm)
                if (showPage) TextButton(onClick = { page = true }) { Text("Open company page") }
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    SegmentedButton(selected = buy, onClick = { buy = true }, shape = SegmentedButtonDefaults.itemShape(0, 2)) { Text("Buy") }
                    SegmentedButton(selected = !buy, onClick = { buy = false }, shape = SegmentedButtonDefaults.itemShape(1, 2)) { Text("Sell") }
                }
                OutlinedTextField(quantity, { quantity = it.filter(Char::isDigit) }, label = { Text("Shares") },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true)
                OutlinedTextField(limit, { limit = it }, label = { Text("Limit per share ($), default %.2f".format(line.priceCents / 100.0)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal), singleLine = true)
                Text("Orders fill at the weekly close, best price first, up to 2% of the freely held shares a week.", color = Palette.secondary, fontSize = 11.sp)
            }
        },
        confirmButton = {
            val shares = quantity.toLongOrNull() ?: 0
            GoldButton(if (buy) "Place buy order" else "Place sell order", enabled = shares > 0 && !store.busy) {
                // Default limits sit two percent through the price so the order is marketable.
                val cents = limit.toDoubleOrNull()?.let { (it * 100).toLong() }
                    ?: if (buy) line.priceCents * 102 / 100 else line.priceCents * 98 / 100
                store.orderShares(line.firm, buy, shares, maxOf(cents, 1))
                onClose()
            }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
    if (page) CompanyPageDialog(store, line.firm) { page = false }
}

@Composable
internal fun CommodityDialog(store: GameStore, line: CommodityDto, onClose: () -> Unit) {
    val mine: ProductLineDto? = store.productLines.firstOrNull { it.product == line.product }
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        modifier = Modifier.widthIn(min = 360.dp, max = 980.dp),
        title = { Text(line.name) },
        text = {
            Column(Modifier.heightIn(max = 640.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                StatGroup(
                    Stat("Market price per lot", Format.money(line.priceCents), note = "${changeText(line.changeWeekBps)} this week"),
                    listOf(
                        Stat("Quarter", changeText(line.changeQuarterBps), changeTint(line.changeQuarterBps)),
                        Stat("Reference", Format.money(line.referenceCents)),
                        Stat("Import at", if (line.importCents > 0) Format.money(line.importCents) else "n/a"),
                        Stat("Made / wk", Format.count(line.madeLots)),
                        Stat("Sold / wk", Format.count(line.soldLots)),
                        Stat("In stock", Format.count(line.stockLots)),
                    ),
                )
                GoodHistoryChart(store, line.product, line.referenceCents)
                GoodDepthSection(store, line.product)
                if (mine != null) Text("You make this: ${Format.money(mine.priceCents)} a lot. Change it from the CEO Office pricing panel.", color = Palette.gold, fontSize = 12.sp)
                StoreGoodSection(store, line)
            }
        },
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
}
