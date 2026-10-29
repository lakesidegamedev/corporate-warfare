package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import uniffi.cw_ffi.CompanyPageDto
import uniffi.cw_ffi.LinkedFirmDto
import uniffi.cw_ffi.firmLogo

/** A firm's generated 16 by 16 logo, identical on every platform: the engine composes it from the name. */
object FirmLogos {
    private val cache = HashMap<String, PixelIcons.Icon>()

    fun of(name: String): PixelIcons.Icon = synchronized(cache) {
        cache.getOrPut(name) {
            val logo = firmLogo(name)
            val colours = HashMap<String, String>()
            colours["p"] = logo.plate
            colours["e"] = logo.edge
            colours["s"] = logo.symbol
            PixelIcons.Icon(colours, logo.rows)
        }
    }
}

/** A firm's logo as exact pixels. */
@Composable
fun FirmLogo(name: String, modifier: Modifier = Modifier, size: Dp = 32.dp) {
    val art = remember(name) { FirmLogos.of(name) }
    Canvas(modifier.size(size).clip(Pixel.small)) { drawPixelIcon(art, null) }
}

/** A company's page: who runs it, how big it is and, by what the investor may see, the rest. */
@Composable
fun CompanyPageBody(store: GameStore, p: CompanyPageDto, onOpenFirm: (ULong) -> Unit, onTrade: (() -> Unit)?) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            FirmLogo(p.name, size = 56.dp)
            Column(Modifier.weight(1f)) {
                Text(p.name, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 16.sp)
                Text("${Sectors.name(p.industry)} · ${p.homeRegion}", color = Palette.secondary, fontSize = 12.sp)
                Text(
                    when (p.kind) {
                        "yours" -> "Your company · ${p.sizeBand}"
                        "listed" -> "Listed · ${p.sizeBand}"
                        else -> "Private · ${p.sizeBand}"
                    },
                    color = Palette.gold, fontSize = 12.sp, fontWeight = FontWeight.SemiBold,
                )
            }
        }
        val ceo = p.ceo
        if (ceo != null) {
            Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                PixelPortrait(ceo.id, size = 72.dp, mood = ceo.mood, office = ceo.officePortrait)
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(ceo.name, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 15.sp)
                    Text("${ceo.age} · ${ceo.home}", color = Palette.secondary, fontSize = 12.sp)
                    Text("${temperamentLabel(ceo.temperament)} · ${p.style}", color = Palette.gold, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                    Text("Chief executive for ${p.ceoWeeks} weeks", color = Palette.secondary, fontSize = 11.sp)
                }
            }
            Text(temperamentNote(ceo.temperament), color = Palette.primaryText, fontSize = 12.sp)
            Text(ceo.background, color = Palette.secondary, fontSize = 12.sp)
        } else if (p.ceoIsYou) {
            Text("You run this company.", color = Palette.secondary, fontSize = 12.sp)
        }
        p.stock?.let { s ->
            Eyebrow("Shares")
            StatGroup(
                Stat("Share price", Format.price(s.priceCents), note = "${changeText(s.changeWeekBps)} this week"),
                listOf(
                    Stat("Quarter", changeText(s.changeQuarterBps), changeTint(s.changeQuarterBps)),
                    Stat("Market cap", Format.compact(s.marketCapCents)),
                    Stat("Fair value", Format.price(s.fairCents)),
                    Stat("Free float", "%.1f%%".format(s.floatBps / 100.0)),
                    Stat("You hold", Format.count(s.yourShares), note = Format.compact(s.yourShares * s.priceCents)),
                ),
            )
            StockHistoryChart(store, p.firm)
            if (onTrade != null) TextButton(onClick = onTrade) { Text("Buy or sell shares") }
        }
        if (p.finance != null || p.kind == "listed") {
            Eyebrow("Financials")
            rememberFinancials(store, p.firm, p.name, p.industry)?.let { CompanyFinancials(it) }
        }
        p.finance?.let { f ->
            Eyebrow("Financial summary (trailing year)")
            StatGroup(
                null,
                listOf(
                    Stat("Revenue", Format.compact(f.revenueTrailingCents)),
                    Stat("EBITDA", Format.compact(f.ebitdaTrailingCents), if (f.ebitdaTrailingCents < 0) Palette.negative else Palette.primaryText),
                    Stat("Net income", Format.compact(f.netIncomeTrailingCents), if (f.netIncomeTrailingCents < 0) Palette.negative else Palette.primaryText),
                    Stat("Cash", Format.compact(f.cashCents)),
                    Stat("Assets", Format.compact(f.assetsCents)),
                    Stat("Liabilities", Format.compact(f.liabilitiesCents)),
                    Stat("Equity", Format.compact(f.equityCents)),
                ),
            )
        }
        if (p.regions.isNotEmpty()) {
            Eyebrow("Where it operates")
            p.regions.forEach { r ->
                Text("${r.regionName}: ${r.plants} plants, ${r.blocks} blocks", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        if (p.divisions.isNotEmpty()) {
            Eyebrow("Divisions")
            p.divisions.forEach { d ->
                // Your own company: the row opens the live division's detail.
                val live = if (p.kind == "yours" && p.firm == store.selectedFirm) {
                    store.divisions.firstOrNull { it.regionName == d.regionName && it.industry == d.industry }
                } else null
                Text(
                    "${d.regionName} · ${Sectors.name(d.industry)} · ${d.strategyName}: ${d.plants} plants, ${d.blocks} blocks",
                    color = if (live != null) Palette.primaryText else Palette.secondary, fontSize = 12.sp,
                    modifier = if (live != null) Modifier.fillMaxWidth().clickable { store.showDivision(live) }.padding(vertical = 4.dp) else Modifier,
                )
            }
        }
        if (p.holders.isNotEmpty()) {
            Eyebrow("Holders")
            p.controller?.let { Text("Controlled by $it", color = Palette.gold, fontSize = 12.sp) }
            p.holders.take(8).forEach { h ->
                Text("${h.holder}: ${Format.count(h.quantity)} shares, ${"%.1f%%".format(h.votesBps / 100.0)} of votes", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        LinkedRows("Parent", listOfNotNull(p.parent), onOpenFirm)
        LinkedRows("Subsidiaries", p.subsidiaries, onOpenFirm)
        LinkedRows("Joint ventures", p.ventures, onOpenFirm)
        p.grudges.forEach { line ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                PixelIcon("target", size = 12.dp, tint = Palette.negative)
                Text(line, color = Palette.primaryText, fontSize = 12.sp)
            }
        }
        p.allies.forEach { line ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                PixelIcon("people", size = 12.dp, tint = Palette.positive)
                Text(line, color = Palette.primaryText, fontSize = 12.sp)
            }
        }
        if (p.moves.isNotEmpty()) {
            Eyebrow("Recent moves")
            p.moves.take(6).forEach { m ->
                Text("${ResultBuckets.weekLabel(m.week.toInt())}: ${m.text}", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        if (p.news.isNotEmpty()) {
            Eyebrow("In the news")
            p.news.forEach { n ->
                Text("${ResultBuckets.weekLabel(n.week.toInt())}: ${n.text}", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        if (p.kind != "yours") RaidMarketsList(store, p.firm)
    }
}

@Composable
private fun LinkedRows(title: String, rows: List<LinkedFirmDto>, onOpen: (ULong) -> Unit) {
    if (rows.isEmpty()) return
    Eyebrow(title)
    rows.forEach { r ->
        Row(Modifier.fillMaxWidth().clickable { onOpen(r.firm) }, verticalAlignment = Alignment.CenterVertically) {
            FirmLogo(r.name, size = 20.dp)
            Spacer(Modifier.width(8.dp))
            Text("${r.name}: ${r.note}", color = Palette.primaryText, fontSize = 12.sp)
        }
    }
}

/** The full company page for any firm, opened from the stock list, a dialog, a dossier, a region board or the news. */
@Composable
fun CompanyPageDialog(store: GameStore, firm: ULong, onClose: () -> Unit) {
    var current by remember(firm) { mutableStateOf(firm) }
    // Companies opened from this page, so back steps to the previous one before closing.
    val history = remember(firm) { mutableListOf<ULong>() }
    var page by remember { mutableStateOf<CompanyPageDto?>(null) }
    var trade by remember { mutableStateOf(false) }
    LaunchedEffect(current) { page = null; store.companyPage(current) { page = it } }
    AlertDialog(
        onDismissRequest = { if (history.isNotEmpty()) current = history.removeAt(history.lastIndex) else onClose() },
        containerColor = Palette.panel,
        title = { Text(page?.name ?: "Company") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                page?.let { CompanyPageBody(store, it, onOpenFirm = { f -> history += current; current = f }, onTrade = if (it.stock != null) ({ trade = true }) else null) }
                    ?: Text("Loading", color = Palette.secondary)
            }
        },
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
    val stock = page?.stock
    if (trade && stock != null) StockDialog(store, stock, showPage = false) { trade = false }
}

/** A name that opens its company page when tapped, with the logo beside it. */
@Composable
fun FirmChip(store: GameStore, name: String, firm: ULong, size: Dp = 20.dp) {
    var open by remember { mutableStateOf(false) }
    Row(Modifier.clickable { open = true }, verticalAlignment = Alignment.CenterVertically) {
        FirmLogo(name, size = size)
        Spacer(Modifier.width(6.dp))
        Text(name, color = Palette.primaryText, fontSize = 12.sp)
    }
    if (open) CompanyPageDialog(store, firm) { open = false }
}

/** Just the logo, tapping through to the company page. */
@Composable
fun FirmChipLogo(store: GameStore, name: String, firm: ULong, size: Dp = 28.dp) {
    var open by remember { mutableStateOf(false) }
    FirmLogo(name, Modifier.clickable { open = true }, size)
    if (open) CompanyPageDialog(store, firm) { open = false }
}
