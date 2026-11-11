package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.BondLineDto
import uniffi.cw_ffi.FirmDto
import uniffi.cw_ffi.StockDto

/**
 * You: Portfolio in the 2.0 shell. What you own outside your own companies (shares, with the
 * price over the timeframe, and bonds), the companies you control, and a side pane for the
 * holding you pick with Buy and Sell going through the action catalogue.
 */

internal val HoldingViews = listOf(
    ViewState("Holdings", cols("name", "sector", "shares", "stake", "price", "value", "quarter"), sort = "value"),
    ViewState("Performance", cols("name", "price", "price.Change", "price.Percent", "price.Spark", "quarter", "fair"), sort = "price.Percent"),
    ViewState("Control", cols("name", "shares", "stake", "float", "value"), sort = "stake"),
    ViewState("Cheap or dear", cols("name", "price", "fair", "gap", "value"), sort = "gap"),
)

private fun holdingColumns(): List<GridColumn<StockDto>> = listOf(
    GridColumn("name", "Company", null, numeric = false, text = { it.name + if (it.controlledByYou) " (yours)" else "" }, minWidth = 160.dp, bold = true, topic = "About"),
    GridColumn("sector", "Sector", 110.dp, numeric = false, text = { Sectors.name(it.industry) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("shares", "Shares", 84.dp, text = { Format.count(it.yourShares) }, sort = { it.yourShares }, total = true, topic = "Holding"),
    GridColumn("stake", "Stake", 64.dp, text = { if (it.sharesIssued > 0) "%.1f%%".format(it.yourShares * 100.0 / it.sharesIssued) else DASH },
        sort = { if (it.sharesIssued > 0) it.yourShares * 10_000 / it.sharesIssued else 0L }, bar = { if (it.sharesIssued > 0) (it.yourShares.toFloat() / it.sharesIssued).coerceIn(0f, 1f) else 0f },
        scale = 100.0, topic = "Holding"),
    GridColumn("float", "Free float", 76.dp, text = { "%.0f%%".format(it.floatBps / 100.0) }, sort = { it.floatBps }, scale = 100.0, topic = "Holding", tip = "Share of the company's shares that outside investors can trade."),
    GridColumn("price", "Price", 76.dp, text = { Format.price(it.priceCents) }, sort = { it.priceCents }, series = { it.historyCents }, step = 1, fmt = { Format.price(it) }, scale = 100.0, topic = "Money"),
    GridColumn("fair", "Fair value", 84.dp, text = { Format.price(it.fairCents) }, sort = { it.fairCents }, scale = 100.0, topic = "Money", tip = "What the engine's valuation of the company comes to per share."),
    GridColumn("gap", "Price vs fair", 84.dp, text = { if (it.fairCents > 0) changeText((it.priceCents - it.fairCents) * 10_000 / it.fairCents) else DASH },
        sort = { if (it.fairCents > 0) (it.priceCents - it.fairCents) * 10_000 / it.fairCents else 0L }, tone = { if (it.fairCents > 0) changeTint(it.fairCents - it.priceCents) else Palette.secondary },
        scale = 100.0, topic = "Money", tip = "Below zero means the price is under the valuation."),
    GridColumn("value", "Worth", 84.dp, text = { Format.compact(it.yourShares * it.priceCents) }, sort = { it.yourShares * it.priceCents }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Money"),
    GridColumn("quarter", "Quarter", 72.dp, text = { changeText(it.changeQuarterBps) }, sort = { it.changeQuarterBps }, tone = { changeTint(it.changeQuarterBps) }, scale = 100.0, topic = "Money"),
)

internal val ControlledViews = listOf(
    ViewState("Companies", cols("name", "sector", "hq", "cash", "equity", "fair", "price", "plants", "votes", "credit"), sort = "equity"),
    ViewState("Money", cols("name", "cash", "equity", "fair", "price", "credit"), sort = "cash"),
)

private fun controlledColumns(): List<GridColumn<FirmDto>> = listOf(
    GridColumn("name", "Company", null, numeric = false, text = { it.name }, minWidth = 150.dp, bold = true, topic = "About"),
    GridColumn("sector", "Sector", 100.dp, numeric = false, text = { Sectors.name(it.industry) }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("hq", "HQ", 100.dp, numeric = false, text = { it.region }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("cash", "Cash", 80.dp, text = { Format.compact(it.cashCents, it.currency) }, sort = { it.cashCents }, scale = 100.0, topic = "Money"),
    GridColumn("equity", "Equity", 80.dp, text = { Format.compact(it.equityCents, it.currency) }, sort = { it.equityCents }, scale = 100.0, topic = "Money"),
    GridColumn("fair", "Fair value", 84.dp, text = { Format.compact(it.fairValueCents, Format.numeraire) }, sort = { it.fairValueCents }, scale = 100.0, topic = "Money"),
    GridColumn("price", "Price", 70.dp, text = { Format.price(it.markCents, Format.numeraire) }, sort = { it.markCents }, scale = 100.0, topic = "Money"),
    GridColumn("plants", "Plants", 60.dp, text = { "${it.facilities}" }, sort = { it.facilities.toLong() }, total = true, topic = "Operations"),
    GridColumn("votes", "Votes", 64.dp, text = { "%.0f%%".format(it.investorVotesBps / 100.0) }, sort = { it.investorVotesBps.toLong() }, scale = 100.0, tone = { Palette.gold }, topic = "Holding"),
    GridColumn("credit", "Credit", 60.dp, numeric = false, text = { it.grade }, topic = "Money"),
)

internal val BondHeldViews = listOf(ViewState("Bonds", cols("name", "face", "coupon", "left", "mark", "yield"), sort = "face"))

private fun bondColumns(): List<GridColumn<BondLineDto>> = listOf(
    GridColumn("name", "Issuer", null, numeric = false, text = { it.issuerName }, minWidth = 130.dp, bold = true, topic = "About"),
    GridColumn("face", "Face held", 84.dp, text = { Format.compact(it.held) }, sort = { it.held }, total = true, scale = 100.0, topic = "Holding"),
    GridColumn("coupon", "Coupon", 66.dp, text = { "%.2f%%".format(it.couponBps / 100.0) }, sort = { it.couponBps.toLong() }, scale = 100.0, topic = "Money"),
    GridColumn("left", "Weeks left", 76.dp, text = { "${it.weeksLeft}" }, sort = { it.weeksLeft.toLong() }, topic = "Money"),
    GridColumn("mark", "Price", 66.dp, text = { "%.1f%%".format(it.markBps / 100.0) }, sort = { it.markBps }, scale = 100.0, topic = "Money", tip = "Market price as a share of face value."),
    GridColumn("yield", "Yield", 66.dp, text = { "%.2f%%".format(it.yieldBps / 100.0) }, sort = { it.yieldBps }, scale = 100.0, topic = "Money"),
)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PortfolioScreen(store: GameStore) {
    val overview = store.overview ?: return
    val act = LocalActions.current
    val held = store.stocks.filter { it.yourShares > 0 }
    val controlled = store.firms.filter { it.controlled }
    val bonds = rememberEngine(store, store.overview?.tick) { h -> h.structureView().bonds.filter { it.held > 0 } }.orEmpty()
    var pickFirm by remember { mutableStateOf<ULong?>(null) }
    val chosen = held.firstOrNull { it.firm == pickFirm } ?: held.firstOrNull()
    val stocksValue = held.sumOf { it.yourShares * it.priceCents }
    Column(Modifier.fillMaxSize().padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FigureLine(
            Triple("Net worth", Format.compact(overview.investorNetWorthCents), Palette.gold),
            Triple("Your cash", Format.compact(overview.investorCashCents), Palette.primaryText),
            Triple("Shares held", Format.compact(stocksValue), Palette.primaryText),
            Triple("Company cash", Format.compact(overview.firmCashCents, overview.primaryCurrency), Palette.primaryText),
            Triple("Cash in 4 wk", Format.compact(overview.forecastFourWeekCents, overview.primaryCurrency), if (overview.forecastFourWeekCents < 0) Palette.negative else Palette.primaryText),
        )
        SplitOrStack(Modifier.fillMaxWidth().weight(1f), 1.5f, 1f, 640.dp, 560.dp, left = { m ->
            Column(m, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                DataGrid(
                    "Your shareholdings", held, holdingColumns(), key = { it.firm },
                    views = GridViews("holdings", HoldingViews, mine = { it.controlledByYou }),
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    filterText = { "${it.name} ${Sectors.name(it.industry)}" },
                    selected = chosen?.firm, onSelect = { pickFirm = it.firm },
                    watchKey = { "firm:" + it.name },
                    breakdown = { s -> "${s.name}: ${Format.count(s.yourShares)} shares at ${Format.price(s.priceCents)}, worth ${Format.compact(s.yourShares * s.priceCents)}; fair value ${Format.price(s.fairCents)}" },
                    initialSort = "value",
                )
                DataGrid(
                    "Companies you control", controlled, controlledColumns(), key = { it.handle },
                    views = GridViews("controlled", ControlledViews, mine = { true }),
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(controlled.size.coerceAtLeast(3)) + 30.dp),
                    filterText = { "${it.name} ${Sectors.name(it.industry)} ${it.region}" },
                    selected = store.selectedFirm, onSelect = { store.selectFirm(it.handle) },
                    breakdown = { f -> "${f.name}: cash ${Format.compact(f.cashCents, f.currency)}, equity ${Format.compact(f.equityCents, f.currency)}, ${f.facilities} plants, you hold ${"%.1f".format(f.investorVotesBps / 100.0)}% of the votes. Click to manage it." },
                    initialSort = "equity",
                )
            }
        }, right = { m ->
            Column(m, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                DetailPane(chosen?.name ?: "Holding", Modifier.fillMaxWidth().weight(1f)) {
                    if (chosen == null) {
                        Text("You hold no shares outside your companies. Pick a company on the League table to buy in.", color = Palette.secondary, fontSize = 12.sp)
                        return@DetailPane
                    }
                    FigureLine(
                        Triple("Price", Format.price(chosen.priceCents), Palette.primaryText),
                        Triple("Fair value", Format.price(chosen.fairCents), Palette.primaryText),
                        Triple("Quarter", changeText(chosen.changeQuarterBps), changeTint(chosen.changeQuarterBps)),
                    )
                    GridSpark(chosen.historyCents, Modifier.fillMaxWidth().height(70.dp))
                    Text(
                        "You hold ${Format.count(chosen.yourShares)} shares, ${if (chosen.sharesIssued > 0) "%.1f%%".format(chosen.yourShares * 100.0 / chosen.sharesIssued) else DASH} of the company, " +
                            "worth ${Format.compact(chosen.yourShares * chosen.priceCents)}.",
                        color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
                    )
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        GoldButton("Buy more") { act("PlaceShareOrder", mapOf("firm" to "${chosen.firm}", "buy" to "1")) }
                        GridButton("Sell", false) { act("PlaceShareOrder", mapOf("firm" to "${chosen.firm}", "buy" to "0")) }
                    }
                }
                if (bonds.isNotEmpty()) DataGrid(
                    "Bonds you hold", bonds, bondColumns(), key = { it.bond },
                    views = GridViews("bonds", BondHeldViews),
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(bonds.size.coerceAtLeast(3)) + 30.dp),
                    phoneBelow = 380.dp,
                    filterText = { it.issuerName },
                    onOpen = { b -> act("SellBond", mapOf("bond" to "${b.bond}")) },
                    breakdown = { b -> "${b.issuerName}: ${Format.compact(b.held)} face, coupon ${"%.2f".format(b.couponBps / 100.0)}%, ${b.weeksLeft} weeks left. Press Enter to sell." },
                    initialSort = "face",
                )
            }
        })
    }
}
