package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.clickable
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.BondLineDto
import uniffi.cw_ffi.CampaignDto
import uniffi.cw_ffi.StructureViewDto
import uniffi.cw_ffi.VentureDto

private fun pct(bps: Number) = "%.1f%%".format(bps.toDouble() / 100.0)

private fun reasonText(key: String) = when (key) {
    "falling_price" -> "the share price has fallen a long way from its peak"
    "missed_goals" -> "the board missed its goals twice running"
    "squeeze" -> "pricing inside the group is taking value from minority holders"
    "dividend_cut" -> "the dividend stopped while the company can afford it"
    "missed_guidance" -> "management missed the profit range it gave the analysts"
    "overvalued" -> "the share price is far above what the business is worth"
    else -> key
}

private fun demandText(key: String) = when (key) {
    "dividend" -> "a special dividend"
    "divest" -> "that the weakest part is floated or spun off"
    "ceo" -> "a new chief executive"
    "restitution" -> "restitution for the minority and prices back at market"
    else -> key
}

private fun outcomeText(key: String) = when (key) {
    "conceded" -> "conceded"
    "compromised" -> "settled"
    "activist_won" -> "the activist won"
    "activist_lost" -> "the activist lost"
    else -> key
}

@Composable
private fun SmallButton(text: String, tint: androidx.compose.ui.graphics.Color = Palette.gold, onClick: () -> Unit) {
    Text(
        text, color = Palette.ink, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(Pixel.small).background(tint).clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

/**
 * Groups, transfer prices, reputation, bonds, ventures and activists in one dense column. Set
 * once; the engine alerts only when a decision is needed.
 */
@Composable
internal fun StructureColumn(store: GameStore) {
    val view = store.structure
    Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (view == null) {
            Text("Nothing to show yet.", color = Palette.secondary, fontSize = 12.sp)
            return@Column
        }
        val open = view.campaigns.filter { it.mine && it.outcome == "open" }
        if (open.isNotEmpty()) Section("Activists") { open.forEach { CampaignCard(store, it) } }
        Section("Group supply and transfer prices") { Links(store, view) }
        Section("Reputation") { Reputation(view) }
        Section("Bonds") { Bonds(store, view) }
        Section("Joint ventures") { Ventures(store, view) }
        val settled = view.campaigns.filter { it.mine && it.outcome != "open" }.take(3)
        if (settled.isNotEmpty()) {
            Section("Past campaigns") {
                settled.forEach {
                    Text("${it.firmName}: ${outcomeText(it.outcome)} (${demandText(it.demand)})", color = Palette.secondary, fontSize = 11.sp)
                }
            }
        }
        val s = view.stats
        Text(
            "Bonds issued ${s.bondsIssued} · ventures ${s.venturesFormed} · listings ${s.ipos} · spin-offs ${s.spinOffs} · carve-outs ${s.carveOuts} · " +
                "activist campaigns ${s.activistCampaigns} (${s.activistWins} won) · tax moved by internal pricing ${Format.signed(s.taxShiftCents)}",
            color = Palette.secondary, fontSize = 11.sp,
        )
        Text(
            "Carve out, list, spin off, form a venture or issue a bond from Actions.",
            color = Palette.secondary, fontSize = 11.sp,
        )
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Eyebrow(title)
        content()
    }
}

@Composable
private fun CampaignCard(store: GameStore, c: CampaignDto) {
    Column(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised).padding(10.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text("${c.firmName}: ${reasonText(c.reason)}.", color = Palette.primaryText, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
        Text(
            "They want ${demandText(c.demand)}. They hold ${pct(c.stakeBps)}; in a fight they win ${pct(c.chanceBps)} of the time. " +
                "If you do nothing the board answers in week ${c.deadline}.",
            color = Palette.secondary, fontSize = 11.sp,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            SmallButton("Concede") { store.submit("{\"RespondActivist\":{\"campaign\":${c.id},\"stance\":0}}") }
            SmallButton("Negotiate") { store.submit("{\"RespondActivist\":{\"campaign\":${c.id},\"stance\":1}}") }
            SmallButton("Fight", Palette.negative) { store.submit("{\"RespondActivist\":{\"campaign\":${c.id},\"stance\":2}}") }
        }
    }
}

@Composable
private fun Links(store: GameStore, view: StructureViewDto) {
    val links = view.links.filter { it.mine }
    if (links.isEmpty()) {
        Text("No two of your companies trade with each other yet. Divisions in one group buy each other's output before the open market.", color = Palette.secondary, fontSize = 11.sp)
        return
    }
    val settings = remember { mutableStateMapOf<String, Double>() }
    links.forEach { link ->
        val key = "${link.seller}-${link.industryIndex}-${link.regionIndex}"
        val value = settings[key] ?: link.settingBps.toDouble()
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                "${link.sellerName} sells ${link.product} to ${link.buyerName}: ${Format.count(link.lotsWeekly)} lots a week at ${Format.price(link.priceCents)} " +
                    "(cost ${Format.price(link.costCents)}, market ${Format.price(link.marketCents)})",
                color = Palette.primaryText, fontSize = 12.sp,
            )
            ValueSlider(
                value, { settings[key] = it }, 0.0..12_000.0, 100.0,
                label = { "Transfer price ${(it / 100).toInt()}% of market" + if (it < 100.0) " (cost)" else "" },
                ticks = 12,
                onCommit = {
                    store.submit("{\"SetTransferPrice\":{\"firm\":${link.seller},\"region\":${link.regionIndex},\"industry\":${link.industryIndex},\"bps\":${it.toInt()}}}")
                },
            )
            if (link.squeezeCents > 0 && link.supplierMinorityBps > 0) {
                Text(
                    "Minority holders of ${link.sellerName} have lost ${Format.compact(link.squeezeCents)} over the past year. A challenge becomes likely.",
                    color = Palette.warning, fontSize = 11.sp,
                )
            }
            if (link.markdownBps > 0) {
                Text("The market marks ${link.sellerName} down ${pct(link.markdownBps)} for persistent losses.", color = Palette.warning, fontSize = 11.sp)
            }
        }
    }
}

@Composable
private fun Reputation(view: StructureViewDto) {
    if (view.reputations.isEmpty()) {
        Text("Reputation builds from quality spending and marketing in each division.", color = Palette.secondary, fontSize = 11.sp)
    }
    view.reputations.take(10).forEach { r ->
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("${r.firmName} · ${Sectors.name(r.industry)} · ${r.region}", color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 1)
            Row(Modifier.width(90.dp).height(6.dp).clip(Pixel.small).background(Palette.panelRaised)) {
                Row(Modifier.fillMaxWidth(r.reputationBps / 10_000f).height(6.dp).background(Palette.gold)) {}
            }
            Text(
                (if (r.premiumBps >= 0) " +" else " ") + pct(r.premiumBps) + " price",
                color = if (r.premiumBps >= 0) Palette.positive else Palette.negative, fontSize = 11.sp, modifier = Modifier.width(86.dp),
            )
        }
    }
}

@Composable
private fun Bonds(store: GameStore, view: StructureViewDto) {
    Text("Priced off the ten-year yield, now ${pct(view.tenYearBps)}, plus a credit spread for each issuer.", color = Palette.secondary, fontSize = 11.sp)
    if (view.bonds.isEmpty()) Text("No bonds outstanding.", color = Palette.secondary, fontSize = 11.sp)
    view.bonds.forEach { b -> BondRow(store, b) }
}

@Composable
private fun BondRow(store: GameStore, b: BondLineDto) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            "${b.issuerName}: ${Format.compact(b.principalCents)} at ${pct(b.couponBps)}, ${b.weeksLeft} weeks, price ${pct(b.markBps)}, " +
                "yield ${pct(b.yieldBps)}, spread ${pct(b.spreadNowBps)}" + if (b.held > 0) " · you hold ${Format.compact(b.held)}" else "",
            color = if (b.mine) Palette.gold else Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f),
        )
        if (b.available > 0) SmallButton("Buy ${Format.compact(minOf(b.available, 10_000_000L))}") {
            store.submit("{\"PlaceBondOrder\":{\"bond\":${b.bond},\"qty\":${minOf(b.available, 10_000_000L)}}}")
        }
        if (b.held > 0) SmallButton("Sell", Palette.secondary) {
            store.submit("{\"SellBond\":{\"bond\":${b.bond},\"qty\":${b.held}}}")
        }
    }
}

@Composable
private fun Ventures(store: GameStore, view: StructureViewDto) {
    if (view.ventures.isEmpty()) Text("No joint ventures. Two groups can co-own a division, such as a pipeline or a refinery.", color = Palette.secondary, fontSize = 11.sp)
    view.ventures.forEach { v -> VentureRow(store, v) }
}

@Composable
private fun VentureRow(store: GameStore, v: VentureDto) {
    val mineIsA = store.firms.any { it.handle == v.a && it.controlled }
    val partner = if (mineIsA) v.a else v.b
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            "${v.name}: ${v.aName} ${pct(v.stakeABps)} and ${v.bName} ${pct(v.stakeBBps)}, formed week ${v.formed}, worth ${Format.compact(v.equityCents)}",
            color = if (v.mine) Palette.gold else Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f),
        )
        if (v.mine && v.openToExit) {
            SmallButton("Buy out") { store.submit("{\"BuyOutVenture\":{\"venture\":${v.venture},\"firm\":$partner}}") }
            SmallButton("Leave", Palette.secondary) { store.submit("{\"ExitVenture\":{\"venture\":${v.venture},\"firm\":$partner}}") }
        } else if (v.mine) {
            Text("locked up", color = Palette.secondary, fontSize = 11.sp)
        }
    }
}
