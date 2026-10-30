package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import kotlinx.coroutines.delay

/** How long the pointer rests on a figure before its explanation shows. Tests set it to zero. */
object ExplainTiming {
    var delayMs: Long = 350
}

/**
 * Hover a figure and a short plain-English note says what it is and how it is worked out. The note
 * shows after a short rest and goes when the pointer leaves; with no text there is no tooltip. The
 * wrapper is a plain Box, so a weight or width goes on [modifier].
 */
@Composable
fun Explain(text: String?, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    if (text.isNullOrBlank()) {
        Box(modifier) { content() }
        return
    }
    val source = remember { MutableInteractionSource() }
    val hovered by source.collectIsHoveredAsState()
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(hovered) {
        if (hovered) delay(ExplainTiming.delayMs)
        shown = hovered
    }
    Box(modifier.hoverable(source)) {
        content()
        if (shown) {
            Popup(alignment = Alignment.BottomStart, offset = IntOffset(0, 8), properties = PopupProperties(focusable = false)) {
                Text(
                    text, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
                    modifier = Modifier.widthIn(max = 280.dp).clip(Pixel.shape).background(Palette.panelRaised)
                        .border(1.dp, Palette.gold.copy(alpha = 0.5f), Pixel.shape).padding(horizontal = 10.dp, vertical = 8.dp),
                )
            }
        }
    }
}

/**
 * What each figure on the CEO Office, Financials, Markets and Stats screens means, in plain words.
 * Where the engine already explains itself (the CFO's reasons, the brief's lines) the screens use
 * the engine's own text; this table covers the rest, looked up by the figure's label.
 */
object Words {
    private val table: Map<String, String> = buildMap {
        fun add(labels: List<String>, text: String) = labels.forEach { put(it, text) }
        // The status bar and the CEO Office strip.
        add(listOf("company cash", "cash"), "Everything the company holds in the bank right now, in its own currency.")
        add(listOf("profit, quarter", "profit"), "What the company earned this quarter so far: sales minus every cost, interest and tax. A negative number is a loss.")
        add(listOf("cash in 4 wk"), "Where the engine expects the company's cash to be in four weeks, from this week's sales, costs, wages and bills already due.")
        add(listOf("net worth"), "Everything you own, added up: your cash, your shares in companies and the value of the companies you control.")
        add(listOf("rate", "central bank rate"), "What the central bank charges banks to borrow. Loans for everyone else cost this plus a margin, so when it rises borrowing gets dearer.")
        add(listOf("inflation"), "How fast prices are rising over a year. Wages, input prices and the central bank all react to it.")
        add(listOf("equity"), "What the company owns minus what it owes: its assets less its debts and bills.")
        add(listOf("fair value"), "What the engine thinks one share is worth, from what the company owns and owes. The share price drifts about an eighth of the way toward it each week.")
        add(listOf("share price"), "The price of one share on the exchange now.")
        add(listOf("plants"), "How many plants the company runs.")
        add(listOf("your votes", "votes"), "Your share of the company's voting power. Over half means you control it.")
        // Tables.
        add(listOf("share"), "The part of this market your plants supply, out of everything sold there.")
        add(listOf("division"), "A line of business in one region.")
        add(listOf("posture"), "How you treat the price of this product: follow the market, undercut or charge a premium.")
        add(listOf("cost"), "What it costs to make one lot of this product.")
        add(listOf("margin"), "The price minus the cost of one lot. Positive means you make money on each lot.")
        add(listOf("made"), "Lots your plants made last week.")
        add(listOf("sold"), "Lots sold last week, shown as a share of what you made. Low means stock is piling up.")
        add(listOf("stock"), "Lots in the warehouse waiting for a buyer.")
        add(listOf("price"), "What one lot sells for now (for a share, what one share costs).")
        add(listOf("week"), "How much the price moved over the last week.")
        add(listOf("quarter"), "How much the price moved over the last three months.")
        add(listOf("market cap"), "The price of one share times all the shares issued: what the market says the whole company is worth.")
        add(listOf("float", "free float"), "The part of the shares anyone can buy. The rest is held by owners who are not selling.")
        add(listOf("you hold"), "How many shares you hold, and the share of the company that makes.")
        add(listOf("sector"), "The company's sector, which decides what it makes and who it competes with.")
        add(listOf("good"), "Lots made and sold in the whole economy last week for this good.")
        add(listOf("imported"), "Lots brought in from outside the corridor last week to cover a shortage.")
        add(listOf("exported"), "Lots sent out of the corridor last week because buyers inside did not need them.")
        add(listOf("shares"), "How many shares this owner holds.")
        add(listOf("stake"), "The part of all issued shares this owner holds.")
        add(listOf("hq"), "Where the company is based.")
        add(listOf("credit"), "How safe lenders think the company is, from A (safest) down. A lower grade means loans cost more.")
        add(listOf("value"), "What your shares are worth at today's price.")
        add(listOf("wages"), "Pay in this region compared with the country's average.")
        add(listOf("skill"), "How skilled the local workers are. Skilled workers make more per plant.")
        add(listOf("roads"), "How good the roads are. Better roads mean cheaper, faster deliveries.")
        add(listOf("power"), "How reliable the electricity supply is.")
        add(listOf("lots"), "Lots a region has to bring in each week because its own plants cannot supply them.")
        add(listOf("import price"), "What an importer pays for a lot here, which is the most a new local plant can expect to charge.")
        // Stats and statements.
        add(listOf("revenue"), "What the company sold, before any cost.")
        add(listOf("net income"), "What the company earned after every cost, interest and tax.")
        add(listOf("net margin"), "Net income as a share of revenue: how much of each unit sold is kept as profit.")
        add(listOf("debt"), "Money the company owes lenders, including loans and bonds.")
        add(listOf("return on equity"), "What the owners earn in a year on what they have in the company: net income scaled up to a year, divided by equity.")
        add(listOf("cost of goods"), "The cost of the goods sold: inputs, materials and fuel used up.")
        add(listOf("gross profit"), "Revenue minus the cost of goods: what is left to pay for everything else.")
        add(listOf("wages"), "Pay for the company's workers.")
        add(listOf("operating costs"), "Rent, upkeep, marketing and other running costs.")
        add(listOf("other income"), "Other money that came in, such as sales of old plant.")
        add(listOf("depreciation"), "The yearly wear on plants and buildings, spread across the weeks.")
        add(listOf("operating profit"), "Gross profit minus wages, running costs and depreciation, before interest and tax.")
        add(listOf("interest paid"), "Interest the company paid on its debts.")
        add(listOf("interest earned"), "Interest the company earned on its cash.")
        add(listOf("tax"), "Profit tax paid to the state.")
        add(listOf("inventory"), "Goods made and not yet sold, at what they cost to make.")
        add(listOf("current assets"), "Cash, stock and money owed to the company, all things it will turn into cash within a year.")
        add(listOf("plant and construction"), "Plants and works under construction, at cost less wear.")
        add(listOf("total assets"), "Everything the company owns.")
        add(listOf("current liabilities"), "Bills and debts due within a year.")
        add(listOf("total liabilities"), "Everything the company owes.")
        add(listOf("shares issued"), "How many shares exist.")
        add(listOf("opening cash"), "Cash at the start of the period.")
        add(listOf("operating"), "Cash made or used by running the business.")
        add(listOf("investing"), "Cash spent on, or got from, plants and other long-lived things.")
        add(listOf("financing"), "Cash raised or paid back to lenders and shareholders.")
        add(listOf("closing cash"), "Cash at the end of the period.")
        add(listOf("gross margin"), "Gross profit as a share of revenue.")
        add(listOf("debt to equity"), "Debt divided by equity. Higher means more borrowing against what owners put in.")
        add(listOf("current ratio"), "Current assets divided by current liabilities. Below 1 means near-term bills are bigger than near-term money.")
        add(listOf("interest cover"), "Operating profit divided by interest paid: how many times over the company covers its interest.")
        add(listOf("revenue growth"), "How much revenue rose or fell compared with the same period a year earlier.")
        add(listOf("price to earnings"), "The share price divided by a year of earnings. High means buyers pay a lot for each dollar earned.")
        add(listOf("earnings a year"), "This quarter's earnings scaled to a year.")
        add(listOf("market value"), "The share price times all shares issued.")
        // The economy card.
        add(listOf("growth, last year"), "How much the whole economy grew over the last year.")
        add(listOf("output vs trend"), "How far output is above or below its normal path. Below zero is slack, above zero is a hot economy.")
        add(listOf("prime lending rate"), "What banks charge their best borrowers: the central bank rate plus a margin.")
        add(listOf("10-year bond"), "What the state pays to borrow for ten years. It moves with what people expect rates to do.")
        // The profit bars.
        add(listOf("inputs and goods"), "What was spent on materials, fuel and parts to make the goods sold.")
                add(listOf("overheads"), "Rent, upkeep, marketing and the other costs of running the company.")
        add(listOf("depreciation"), "Wear on plants, spread across the weeks.")
        add(listOf("interest"), "Interest paid minus interest earned.")
    }

    /** The note for a figure's label, or null when there is none. A "Profit, 5 wk" style label counts as Profit. */
    fun about(label: String): String? {
        val key = label.trim().lowercase()
        return table[key] ?: if (key.startsWith("profit,")) table["profit, quarter"] else null
    }

    /** The labels the table covers, for the test that checks the plain-English rules. */
    internal fun all(): Map<String, String> = table
}
