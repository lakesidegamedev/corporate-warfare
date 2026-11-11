package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.StatementDto

/**
 * Company: Finance in the 2.0 shell. The statements are one grid with the statement lines down the
 * side and the periods across (newest first); the built-in views pick the income statement, the
 * balance sheet, the cash flow or the ratios. Beside it, debt (loans and your bonds) and who owns
 * the company, with the borrowing and share actions from the action catalogue.
 */

private enum class Kind { Money, Pct, Times, Count, Price }

private class StLine(val group: String, val title: String, val total: Boolean, val kind: Kind, val value: (StatementDto) -> Long?)

private fun st(group: String, title: String, total: Boolean = false, kind: Kind = Kind.Money, value: (StatementDto) -> Long?) = StLine(group, title, total, kind, value)

private val StatementLines: List<StLine> = listOf(
    st("Income statement", "Revenue") { it.revenueCents },
    st("Income statement", "Cost of goods") { -it.costOfGoodsCents },
    st("Income statement", "Gross profit", true) { it.grossProfitCents },
    st("Income statement", "Wages") { -it.wagesCents },
    st("Income statement", "Operating costs") { -it.operatingExpenseCents },
    st("Income statement", "Other income") { it.otherCents },
    st("Income statement", "Depreciation") { -it.depreciationCents },
    st("Income statement", "Operating profit", true) { it.ebitCents },
    st("Income statement", "Interest paid") { -it.interestExpenseCents },
    st("Income statement", "Interest earned") { it.interestIncomeCents },
    st("Income statement", "Tax") { -it.taxCents },
    st("Income statement", "Net income", true) { it.netIncomeCents },
    st("Balance sheet", "Cash") { it.cashCents },
    st("Balance sheet", "Inventory") { it.inventoryCents },
    st("Balance sheet", "Current assets", true) { it.currentAssetsCents },
    st("Balance sheet", "Plant and construction") { it.fixedAssetsCents },
    st("Balance sheet", "Total assets", true) { it.assetsCents },
    st("Balance sheet", "Current liabilities") { it.currentLiabilitiesCents },
    st("Balance sheet", "Debt") { it.debtCents },
    st("Balance sheet", "Total liabilities", true) { it.liabilitiesCents },
    st("Balance sheet", "Equity", true) { it.equityCents },
    st("Balance sheet", "Shares issued", kind = Kind.Count) { it.sharesIssued },
    st("Balance sheet", "Share price", kind = Kind.Price) { it.sharePriceCents },
    st("Cash flow", "Opening cash") { it.cashOpeningCents },
    st("Cash flow", "Operating") { it.cashOperatingCents },
    st("Cash flow", "Investing") { it.cashInvestingCents },
    st("Cash flow", "Financing") { it.cashFinancingCents },
    st("Cash flow", "Tax paid") { it.cashTaxCents },
    st("Cash flow", "Closing cash", true) { it.cashClosingCents },
    st("Ratios", "Gross margin", kind = Kind.Pct) { it.grossMarginBps },
    st("Ratios", "Net margin", kind = Kind.Pct) { it.netMarginBps },
    st("Ratios", "Return on equity", kind = Kind.Pct) { it.roeBps },
    st("Ratios", "Return on assets", kind = Kind.Pct) { it.roaBps },
    st("Ratios", "Debt to equity", kind = Kind.Times) { it.debtToEquityBps },
    st("Ratios", "Current ratio", kind = Kind.Times) { it.currentRatioBps },
    st("Ratios", "Interest cover", kind = Kind.Times) { it.interestCoverBps },
    st("Ratios", "Revenue growth", kind = Kind.Pct) { it.revenueGrowthBps },
)

private fun show(kind: Kind, v: Long?): String = when {
    v == null -> DASH
    kind == Kind.Money -> Format.compact(v)
    kind == Kind.Pct -> "%.1f%%".format(v / 100.0)
    kind == Kind.Times -> "%.2fx".format(v / 10_000.0)
    kind == Kind.Price -> Format.price(v)
    else -> Format.count(v)
}

private fun statementColumns(rows: List<StatementDto>, step: Int, count: Int): List<GridColumn<StLine>> {
    val newest = rows.asReversed().take(count)
    val name = GridColumn<StLine>("name", "Line", null, numeric = false, text = { it.title }, minWidth = 170.dp, tone = { if (it.total) Palette.gold else Palette.primaryText }, topic = "About")
    val group = GridColumn<StLine>("group", "Statement", 120.dp, numeric = false, text = { it.group }, tone = { Palette.secondary }, shown = false, topic = "About")
    val periods = newest.mapIndexed { i, s ->
        val open = s.weeks.toInt() < step
        GridColumn<StLine>(
            "p$i", s.label + if (open) " *" else "", 90.dp,
            text = { l -> show(l.kind, l.value(s)) },
            sort = { l -> l.value(s) ?: Long.MIN_VALUE },
            tone = { l ->
                val v = l.value(s)
                if (l.total && v != null && l.kind == Kind.Money) (if (v >= 0) Palette.positive else Palette.negative) else Palette.primaryText
            },
            // The newest period carries the whole history, so its change, percent and sparkline follow the timeframe.
            series = if (i == 0) ({ l -> if (l.kind == Kind.Money) rows.mapNotNull { l.value(it) } else emptyList() }) else null,
            step = step,
            fmt = { Format.compact(it) },
            scale = 100.0,
            dropOrder = i,
            topic = "Periods",
            tip = if (open) "The period still running, so far." else "Closed period.",
        )
    }
    return listOf(name, group) + periods
}

private fun lineCols(vararg extra: String) = cols("name", *Array(8) { "p$it" }, *extra)

internal val FinanceViews = listOf(
    ViewState("Income statement", lineCols(), filters = listOf(Filter(FilterOp.Is, "group", "Income")), sort = null),
    ViewState("Balance sheet", lineCols(), filters = listOf(Filter(FilterOp.Is, "group", "Balance"))),
    ViewState("Cash flow", lineCols(), filters = listOf(Filter(FilterOp.Is, "group", "Cash flow"))),
    ViewState("Ratios", lineCols(), filters = listOf(Filter(FilterOp.Is, "group", "Ratios"))),
    ViewState("Trend", cols("name", "p0", "p0.Change", "p0.Percent", "p0.Spark"), filters = listOf(Filter(FilterOp.Is, "group", "Income"))),
)

private class DebtRow(val key: String, val kind: String, val name: String, val principal: Long, val rateBps: Int, val weeksLeft: Long, val status: String, val handle: ULong?)

internal val DebtViews = listOf(
    ViewState("Debt", cols("name", "kind", "principal", "rate", "left", "status"), sort = "principal"),
    ViewState("Cost of debt", cols("name", "principal", "rate", "left"), sort = "rate"),
)

private fun debtColumns(): List<GridColumn<DebtRow>> = listOf(
    GridColumn("name", "Lender", null, numeric = false, text = { it.name }, minWidth = 120.dp, topic = "About"),
    GridColumn("kind", "Kind", 60.dp, numeric = false, text = { it.kind }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("principal", "Owed", 84.dp, text = { Format.compact(it.principal) }, sort = { it.principal }, fmt = { Format.compact(it) }, scale = 100.0, total = true, topic = "Money"),
    GridColumn("rate", "Rate", 60.dp, text = { "%.2f%%".format(it.rateBps / 100.0) }, sort = { it.rateBps.toLong() }, scale = 100.0, topic = "Money", tip = "Yearly rate, or the bond's coupon."),
    GridColumn("left", "Weeks left", 80.dp, text = { "${it.weeksLeft}" }, sort = { it.weeksLeft }, topic = "Money"),
    GridColumn("status", "Status", 84.dp, numeric = false, text = { it.status }, tone = { if (it.status == "In default") Palette.negative else Palette.secondary }, topic = "About"),
)

private class HolderRow(val name: String, val shares: Long, val votesBps: Int, val cost: Long, val issued: Long, val price: Long)

internal val OwnerViews = listOf(
    ViewState("Owners", cols("name", "shares", "stake", "votes", "value"), sort = "shares"),
    ViewState("What they paid", cols("name", "shares", "cost", "value"), sort = "cost"),
)

private fun holderColumns(): List<GridColumn<HolderRow>> = listOf(
    GridColumn("name", "Holder", null, numeric = false, text = { it.name }, minWidth = 120.dp, topic = "About"),
    GridColumn("shares", "Shares", 80.dp, text = { Format.count(it.shares) }, sort = { it.shares }, total = true, topic = "Holding"),
    GridColumn("stake", "Stake", 64.dp, text = { if (it.issued > 0) "%.1f%%".format(it.shares * 100.0 / it.issued) else DASH }, sort = { if (it.issued > 0) it.shares * 10_000 / it.issued else 0L },
        bar = { if (it.issued > 0) (it.shares.toFloat() / it.issued).coerceIn(0f, 1f) else 0f }, scale = 100.0, topic = "Holding"),
    GridColumn("votes", "Votes", 64.dp, text = { "%.1f%%".format(it.votesBps / 100.0) }, sort = { it.votesBps.toLong() }, scale = 100.0, topic = "Holding", tip = "Share of the votes at a shareholder meeting."),
    GridColumn("cost", "Paid", 84.dp, text = { Format.compact(it.cost) }, sort = { it.cost }, scale = 100.0, topic = "Money"),
    GridColumn("value", "Worth now", 84.dp, text = { Format.compact(it.shares * it.price) }, sort = { it.shares * it.price }, scale = 100.0, total = true, topic = "Money", tip = "Shares times the share price today."),
)

@OptIn(ExperimentalLayoutApi::class, ExperimentalFoundationApi::class)
@Composable
fun FinanceScreen(store: GameStore) {
    val firm = store.selectedFirmDto ?: run { Box(Modifier.fillMaxSize().padding(16.dp)) { FoundCompanyPanel(store) }; return }
    val tf = GridPrefs.timeframe
    // Short windows read week by week, a year by quarter, longer by year; a button can pick one by hand.
    var manual by remember { mutableStateOf<String?>(null) }
    val code = manual ?: when (tf) { Timeframe.W4, Timeframe.W13 -> "week"; Timeframe.Y1 -> "quarter"; else -> "year" }
    val step = when (code) { "week" -> 1; "quarter" -> 13; else -> 52 }
    val count = when (code) { "week" -> tf.weeks?.coerceAtMost(13) ?: 13; "quarter" -> 8; else -> 8 }
    val statements = rememberEngine(store, firm.handle, code, store.overview?.tick) { h -> h.statements(firm.handle, code) }.orEmpty()
    val loans = rememberEngine(store, firm.handle, store.overview?.tick) { h -> h.loans(firm.handle) }.orEmpty()
    val bonds = rememberEngine(store, firm.handle, store.overview?.tick) { h -> h.structureView().bonds.filter { it.mine && it.issuer == firm.handle } }.orEmpty()
    val own = rememberEngine(store, firm.handle, store.overview?.tick) { h -> h.ownership(firm.handle) }
    val act = LocalActions.current
    var loanPick by remember { mutableStateOf<String?>(null) }
    val cur = firm.currency
    val last = statements.lastOrNull()
    val debtRows = loans.filter { it.principalCents > 0 }.map { DebtRow("loan${it.handle}", "Loan", it.lender, it.principalCents, it.rateBps, it.remainingWeeks.toLong(), if (it.defaulted) "In default" else if (it.secured) "Secured" else "Open", it.handle) } +
        bonds.map { DebtRow("bond${it.bond}", "Bond", "Bond holders", it.principalCents, it.couponBps, it.weeksLeft.toLong(), "Market ${"%.1f".format(it.markBps / 100.0)}%", null) }
    val holders = own?.holders.orEmpty().map { HolderRow(it.holder, it.quantity, it.votesBps, it.costCents, own?.issued ?: 0, firm.markCents) }
    val chosen = debtRows.firstOrNull { it.key == loanPick }
    Column(Modifier.fillMaxSize().padding(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FigureLine(
            Triple("Cash", Format.compact(firm.cashCents, cur), Palette.primaryText),
            Triple("Equity", Format.compact(firm.equityCents, cur), Palette.primaryText),
            Triple("Owed", Format.compact(debtRows.sumOf { it.principal }, cur), if (debtRows.isEmpty()) Palette.primaryText else Palette.warning),
            Triple("Credit", firm.grade, Palette.primaryText),
            Triple("Share price", Format.price(firm.markCents, Format.numeraire), Palette.primaryText),
            Triple("Your votes", "%.1f%%".format(firm.investorVotesBps / 100.0), Palette.gold),
            Triple("Net income, ${last?.label ?: "last period"}", last?.let { Format.compact(it.netIncomeCents, cur) } ?: DASH, last?.let { if (it.netIncomeCents < 0) Palette.negative else Palette.positive } ?: Palette.secondary),
        )
        SplitOrStack(Modifier.fillMaxWidth().weight(1f), 1.5f, 1f, 560.dp, 760.dp, left = { m ->
            DataGrid(
                "Statements", StatementLines, statementColumns(statements, step, count), key = { it.group + it.title },
                views = GridViews("finance", FinanceViews),
                modifier = m,
                filterText = { "${it.group} ${it.title}" },
                breakdown = { l -> "${l.group}: ${l.title}" + (statements.lastOrNull()?.let { s -> ", ${s.label}: ${show(l.kind, l.value(s))}" } ?: "") },
                trailing = {
                    listOf("Auto" to null, "Weeks" to "week", "Quarters" to "quarter", "Years" to "year").forEach { (label, c) ->
                        GridButton(label, manual == c) { manual = c }
                    }
                },
            )
        }, right = { m ->
            Column(m, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (debtRows.isNotEmpty()) DataGrid(
                    "Debt", debtRows, debtColumns(), key = { it.key }, phoneBelow = 380.dp,
                    views = GridViews("debt", DebtViews),
                    modifier = Modifier.fillMaxWidth().height(gridNaturalHeight(debtRows.size.coerceAtLeast(3)) + 30.dp),
                    filterText = { "${it.kind} ${it.name}" },
                    selected = loanPick, onSelect = { loanPick = it.key },
                    breakdown = { d -> "${d.kind} from ${d.name}: ${Format.compact(d.principal)} at ${"%.2f".format(d.rateBps / 100.0)}%, ${d.weeksLeft} weeks left" },
                    initialSort = "principal",
                )
                DataGrid(
                    "Who owns the company", holders, holderColumns(), key = { it.name }, phoneBelow = 380.dp,
                    views = GridViews("owners", OwnerViews, mine = { it.name.equals("You", true) || it.name == firm.name }),
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    filterText = { it.name },
                    breakdown = { h -> "${h.name}: ${Format.count(h.shares)} shares, ${"%.1f".format(h.votesBps / 100.0)}% of the votes" },
                    initialSort = "shares",
                )
                DetailPane("Borrow and shares", Modifier.fillMaxWidth().height(150.dp)) {
                    if (debtRows.isEmpty()) Text("No debt. The banks will quote you a loan, or you can sell bonds to the market.", color = Palette.secondary, fontSize = 12.sp)
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        GridButton("Borrow", false) { act("RequestLoan", emptyMap()) }
                        GridButton("Issue a bond", false) { act("IssueBond", emptyMap()) }
                        if (chosen?.handle != null) GridButton("Repay this loan", false) { act("RepayLoan", mapOf("loan" to "${chosen.handle}")) }
                        GridButton("Issue shares", false) { act("IssueShares", emptyMap()) }
                        GridButton("Buy back shares", false) { act("RepurchaseShares", emptyMap()) }
                        GridButton("Pay a dividend", false) { act("PayDividend", emptyMap()) }
                    }
                    own?.controller?.let { Text("Controlled by $it. Next shareholder meeting: week ${own.nextMeeting}.", color = Palette.secondary, fontSize = 11.sp) }
                }
            }
        })
    }
}
