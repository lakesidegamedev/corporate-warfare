package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.PricePointDto
import uniffi.cw_ffi.StatementDto

/** Everything the financial tabs show for one company, read from the engine in one go. */
data class FinancialData(
    val name: String,
    val weeks: List<StatementDto>,
    val quarters: List<StatementDto>,
    val years: List<StatementDto>,
    val share: List<PricePointDto>,
    val index: List<PricePointDto>,
    /** Up to two listed rivals in the same sector, by quarter, for the comparison chart. */
    val rivals: List<Pair<String, List<StatementDto>>>,
) {
    fun of(period: ChartPeriod): List<StatementDto> = when (period) {
        ChartPeriod.Weekly -> weeks
        ChartPeriod.Quarterly -> quarters
        ChartPeriod.Yearly -> years
    }
}

/** The financial tabs of a company page. */
enum class FinanceTab(val title: String) {
    Overview("Overview"), Income("Income statement"), Balance("Balance sheet"), Cash("Cash flow"), Divisions("Divisions"), Ratios("Ratios")
}

/** Load a company's statements, price history and rivals; refreshes every week the clock moves. */
@Composable
fun rememberFinancials(store: GameStore, firm: ULong, name: String, industry: String): FinancialData? {
    val revision = store.overview?.revision
    val rivals = store.stocks.filter { it.firm != firm && it.industry == industry }.sortedByDescending { it.marketCapCents }.take(2)
    val state = produceState<FinancialData?>(null, firm, revision) {
        value = store.read { h ->
            FinancialData(
                name, h.statements(firm, "week"), h.statements(firm, "quarter"), h.statements(firm, "year"),
                h.sharePriceHistory(firm), h.corridorIndexHistory(),
                rivals.map { it.name to h.statements(it.firm, "quarter") }.filter { it.second.isNotEmpty() },
            )
        }
    }
    return state.value
}

private fun List<StatementDto>.points(value: (StatementDto) -> Long) = map { ChartPoint(it.firstWeek.toInt(), value(it)) }
private fun List<PricePointDto>.points() = map { ChartPoint(it.week.toInt(), it.cents) }

private fun pct(bps: Long?): String = bps?.let { "%.1f%%".format(it / 100.0) } ?: "n/a"
private fun times(bps: Long?): String = bps?.let { "%.2fx".format(it / 10_000.0) } ?: "n/a"

/**
 * A company's statements in depth: an overview with charts, the income statement, balance sheet
 * and cash flow as tables over the last several periods, each division's results, and ratios.
 * [columns] is how many periods the tables show side by side.
 */
@Composable
fun CompanyFinancials(data: FinancialData, columns: Int = 4, initialTab: FinanceTab = FinanceTab.Overview, initialPeriod: ChartPeriod = ChartPeriod.Quarterly) {
    var tab by remember { mutableStateOf(initialTab) }
    var period by remember { mutableStateOf(initialPeriod) }
    val rows = data.of(period)
    val periods = ChartPeriod.entries
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        androidx.compose.foundation.layout.FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            FinanceTab.entries.forEach { t -> ChartChips(listOf(t.title), if (t == tab) t.title else "") { tab = t } }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Period", color = Palette.secondary, fontSize = 11.sp)
            ChartChips(periods.map { it.title }, period.title) { t -> period = periods.first { it.title == t } }
        }
        if (rows.isEmpty()) {
            Callout("Statements build up as weeks close. A career saved before this version starts its history now.", Palette.secondary)
            return@Column
        }
        val recent = rows.takeLast(columns).reversed()
        val pick: (ChartPeriod) -> Unit = { period = it }
        when (tab) {
            FinanceTab.Overview -> {
                val latest = rows.last()
                StatGroup(
                    Stat("Revenue", Format.compact(latest.revenueCents), note = "${latest.label}, growth ${pct(latest.revenueGrowthBps)}"),
                    listOf(
                        Stat("Net income", Format.compact(latest.netIncomeCents), if (latest.netIncomeCents < 0) Palette.negative else Palette.positive),
                        Stat("Net margin", pct(latest.netMarginBps)),
                        Stat("Cash", Format.compact(latest.cashCents)),
                        Stat("Equity", Format.compact(latest.equityCents)),
                        Stat("Debt", Format.compact(latest.debtCents)),
                        Stat("Return on equity", pct(latest.roeBps)),
                    ),
                )
                Panel {
                    Chart(
                        listOf(
                            ChartSeries("Revenue", Palette.gold, rows.points { it.revenueCents }),
                            ChartSeries("Net income", Palette.positive, rows.points { it.netIncomeCents }),
                        ),
                        title = "Revenue and profit", initialKind = ChartKind.Bar, period = period, periods = periods, onPeriod = pick,
                    )
                }
                if (data.rivals.isNotEmpty()) {
                    val colours = listOf(Palette.gold, Color(0xFF6EC6FF), Color(0xFFC792EA))
                    Panel {
                        Chart(
                            (listOf(data.name to data.quarters) + data.rivals).mapIndexed { i, (n, s) -> ChartSeries(n, colours[i % colours.size], s.points { it.revenueCents }) },
                            title = "Revenue against rivals, by quarter", period = ChartPeriod.Quarterly,
                        )
                    }
                }
                if (data.share.size > 1) {
                    // The share and the corridor index on one scale: both rebased to 100 at the first close shown.
                    val base = data.share.first()
                    val indexBase = data.index.firstOrNull { it.week >= base.week }
                    val share = data.share.map { ChartPoint(it.week.toInt(), it.cents * 10_000 / maxOf(base.cents, 1)) }
                    val index = if (indexBase == null) emptyList() else data.index.filter { it.week >= base.week }.map { ChartPoint(it.week.toInt(), it.cents * 10_000 / maxOf(indexBase.cents, 1)) }
                    var sharePeriod by remember { mutableStateOf(ChartPeriod.Weekly) }
                    Panel {
                        Chart(
                            listOf(ChartSeries("Share", Palette.gold, GameDates.lastClose(share, sharePeriod)), ChartSeries("Corridor index", Palette.secondary, GameDates.lastClose(index, sharePeriod))),
                            title = "Share price against the index (100 at start)", period = sharePeriod, periods = periods, onPeriod = { sharePeriod = it },
                            format = { "%.1f".format(it / 100.0) },
                        )
                    }
                }
            }
            FinanceTab.Income -> {
                StatementTable(
                    recent,
                    listOf(
                        Line("Revenue") { it.revenueCents },
                        Line("Cost of goods") { -it.costOfGoodsCents },
                        Line("Gross profit", total = true) { it.grossProfitCents },
                        Line("Wages") { -it.wagesCents },
                        Line("Operating costs") { -it.operatingExpenseCents },
                        Line("Other income") { it.otherCents },
                        Line("Depreciation") { -it.depreciationCents },
                        Line("Operating profit", total = true) { it.ebitCents },
                        Line("Interest paid") { -it.interestExpenseCents },
                        Line("Interest earned") { it.interestIncomeCents },
                        Line("Tax") { -it.taxCents },
                        Line("Net income", total = true) { it.netIncomeCents },
                    ),
                )
                Panel {
                    Chart(
                        listOf(
                            ChartSeries("Revenue", Palette.gold, rows.points { it.revenueCents }),
                            ChartSeries("Gross profit", Color(0xFF6EC6FF), rows.points { it.grossProfitCents }),
                            ChartSeries("Net income", Palette.positive, rows.points { it.netIncomeCents }),
                        ),
                        title = "Income", period = period, periods = periods, onPeriod = pick,
                    )
                }
            }
            FinanceTab.Balance -> {
                StatementTable(
                    recent,
                    listOf(
                        Line("Cash") { it.cashCents },
                        Line("Inventory") { it.inventoryCents },
                        Line("Current assets", total = true) { it.currentAssetsCents },
                        Line("Plant and construction") { it.fixedAssetsCents },
                        Line("Total assets", total = true) { it.assetsCents },
                        Line("Current liabilities") { it.currentLiabilitiesCents },
                        Line("Debt") { it.debtCents },
                        Line("Total liabilities", total = true) { it.liabilitiesCents },
                        Line("Equity", total = true) { it.equityCents },
                        Line("Shares issued", money = false) { it.sharesIssued },
                        Line("Share price", money = false) { it.sharePriceCents },
                    ),
                )
                Panel {
                    Chart(
                        listOf(
                            ChartSeries("Assets", Palette.gold, rows.points { it.assetsCents }),
                            ChartSeries("Liabilities", Palette.negative, rows.points { it.liabilitiesCents }),
                            ChartSeries("Equity", Palette.positive, rows.points { it.equityCents }),
                        ),
                        title = "Balance sheet at each close", initialKind = ChartKind.Area, period = period, periods = periods, onPeriod = pick,
                    )
                }
            }
            FinanceTab.Cash -> {
                StatementTable(
                    recent,
                    listOf(
                        Line("Opening cash") { it.cashOpeningCents },
                        Line("Operating") { it.cashOperatingCents },
                        Line("Investing") { it.cashInvestingCents },
                        Line("Financing") { it.cashFinancingCents },
                        Line("Tax") { it.cashTaxCents },
                        Line("Closing cash", total = true) { it.cashClosingCents },
                    ),
                )
                Text("Operating covers sales, wages, materials and interest. Investing is plants, construction and holdings. Financing is loans, deposits, shares and dividends.", color = Palette.secondary, fontSize = 11.sp)
                Panel {
                    Chart(
                        listOf(
                            ChartSeries("Operating", Palette.positive, rows.points { it.cashOperatingCents }),
                            ChartSeries("Investing", Palette.negative, rows.points { it.cashInvestingCents }),
                            ChartSeries("Financing", Color(0xFF6EC6FF), rows.points { it.cashFinancingCents }),
                            ChartSeries("Tax", Palette.warning, rows.points { it.cashTaxCents }),
                        ),
                        title = "Cash flow by activity", initialKind = ChartKind.Bar, period = period, periods = periods, onPeriod = pick,
                    )
                }
            }
            FinanceTab.Divisions -> {
                val source = if (period == ChartPeriod.Weekly) data.quarters else rows
                val latest = source.lastOrNull()
                if (latest == null || latest.divisions.isEmpty()) {
                    Callout("Division results are kept by quarter once a quarter has closed weeks.", Palette.secondary)
                } else {
                    Text("${latest.label}. Estimated from each division's plants; weekly view shows quarters.", color = Palette.secondary, fontSize = 11.sp)
                    val header = listOf("Division", "Revenue", "Costs", "Profit", "Plants", "Share")
                    Row(Modifier.fillMaxWidth()) {
                        header.forEachIndexed { i, h -> Text(h, color = Palette.secondary, fontSize = 10.sp, fontFamily = Pixel.font, modifier = Modifier.weight(if (i == 0) 2f else 1f)) }
                    }
                    latest.divisions.sortedByDescending { it.revenueCents }.forEach { d ->
                        Row(Modifier.fillMaxWidth().padding(vertical = 2.dp)) {
                            Text("${Sectors.name(d.industry)} · ${d.regionName}", color = Palette.primaryText, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(2f))
                            Text(Format.compact(d.revenueCents), color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f))
                            Text(Format.compact(d.costsCents), color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f))
                            Text(Format.compact(d.profitCents), color = if (d.profitCents < 0) Palette.negative else Palette.positive, fontSize = 12.sp, modifier = Modifier.weight(1f))
                            Text("${d.plants}", color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f))
                            Text(pct(d.shareBps), color = Palette.gold, fontSize = 12.sp, modifier = Modifier.weight(1f))
                        }
                    }
                    val names = latest.divisions.sortedByDescending { it.revenueCents }.take(4).map { it.industry to it.regionName }
                    val colours = listOf(Palette.gold, Palette.positive, Color(0xFF6EC6FF), Color(0xFFC792EA))
                    Panel {
                        Chart(
                            names.mapIndexed { i, (industry, region) ->
                                ChartSeries(
                                    "${Sectors.name(industry)} · $region", colours[i],
                                    source.mapNotNull { s -> s.divisions.firstOrNull { it.industry == industry && it.regionName == region }?.let { ChartPoint(s.firstWeek.toInt(), it.profitCents) } },
                                )
                            },
                            title = "Division profit", initialKind = ChartKind.Bar,
                            period = if (period == ChartPeriod.Weekly) ChartPeriod.Quarterly else period,
                        )
                    }
                }
            }
            FinanceTab.Ratios -> {
                StatementTable(
                    recent,
                    listOf(
                        Line("Gross margin", text = { pct(it.grossMarginBps) }),
                        Line("Net margin", text = { pct(it.netMarginBps) }),
                        Line("Return on equity, a year", text = { pct(it.roeBps) }),
                        Line("Return on assets, a year", text = { pct(it.roaBps) }),
                        Line("Debt to equity", text = { times(it.debtToEquityBps) }),
                        Line("Current ratio", text = { times(it.currentRatioBps) }),
                        Line("Interest cover", text = { times(it.interestCoverBps) }),
                        Line("Revenue growth", text = { pct(it.revenueGrowthBps) }),
                    ),
                )
                Panel {
                    Chart(
                        listOf(
                            ChartSeries("Gross margin", Palette.gold, rows.mapNotNull { s -> s.grossMarginBps?.let { ChartPoint(s.firstWeek.toInt(), it) } }),
                            ChartSeries("Net margin", Palette.positive, rows.mapNotNull { s -> s.netMarginBps?.let { ChartPoint(s.firstWeek.toInt(), it) } }),
                            ChartSeries("Return on equity", Color(0xFF6EC6FF), rows.mapNotNull { s -> s.roeBps?.let { ChartPoint(s.firstWeek.toInt(), it) } }),
                        ),
                        title = "Margins and returns", period = period, periods = periods, onPeriod = pick, format = { pct(it) },
                    )
                }
            }
        }
    }
}

/** One row of a statement table: a money line, a plain count or a preformatted ratio. */
private class Line(val title: String, val total: Boolean = false, val money: Boolean = true, val text: ((StatementDto) -> String)? = null, val value: ((StatementDto) -> Long)? = null)

/** Statement lines down the side, the latest periods across, newest first. */
@Composable
private fun StatementTable(columns: List<StatementDto>, lines: List<Line>) {
    Panel {
        Row(Modifier.fillMaxWidth()) {
            Spacer(Modifier.weight(1.6f))
            columns.forEach { Text(it.label, color = Palette.secondary, fontSize = 10.sp, fontFamily = Pixel.font, modifier = Modifier.weight(1f)) }
        }
        lines.forEach { line ->
            Row(Modifier.fillMaxWidth().padding(vertical = 1.dp)) {
                val weight = if (line.total) FontWeight.SemiBold else FontWeight.Normal
                Text(line.title, color = Palette.primaryText, fontSize = 12.sp, fontWeight = weight, modifier = Modifier.weight(1.6f))
                columns.forEach { s ->
                    val v = line.value?.invoke(s)
                    val text = line.text?.invoke(s) ?: when {
                        v == null -> ""
                        line.money -> Format.compact(v)
                        line.title == "Share price" -> Format.price(v)
                        else -> Format.count(v)
                    }
                    val tone = if (line.total && v != null) (if (v >= 0) Palette.positive else Palette.negative) else Palette.primaryText
                    Text(text, color = tone, fontSize = 12.sp, fontWeight = weight, modifier = Modifier.weight(1f))
                }
            }
        }
    }
}
