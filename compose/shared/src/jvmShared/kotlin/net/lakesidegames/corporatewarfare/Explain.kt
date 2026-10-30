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
        fun add(text: String, vararg labels: String) = labels.forEach { put(it, text) }
        // The status bar and the CEO Office strip.
        add("Everything the company holds in the bank right now, in its own currency.", "company cash", "cash")
        add("What the company earned this quarter so far: sales minus every cost, interest and tax. A negative number is a loss.", "profit, quarter", "profit")
        add("Where the engine expects the company's cash to be in four weeks, from this week's sales, costs, wages and bills already due.", "cash in 4 wk")
        add("Everything you own, added up: your cash, your shares in companies and the value of the companies you control.", "net worth")
        add("What the central bank charges banks to borrow. Loans for everyone else cost this plus a margin, so when it rises borrowing gets dearer.", "rate", "central bank rate")
        add("How fast prices are rising over a year. Wages, input prices and the central bank all react to it.", "inflation")
        add("What the company owns minus what it owes: its assets less its debts and bills.", "equity")
        add("What the engine thinks one share is worth, from what the company owns and owes. The share price drifts about an eighth of the way toward it each week.", "fair value")
        add("The price of one share on the exchange now.", "share price")
        add("How many plants the company runs.", "plants")
        add("Your share of the company's voting power. Over half means you control it.", "your votes", "votes")
        // Tables.
        add("The part of this market your plants supply, out of everything sold there.", "share")
        add("A line of business in one region.", "division")
        add("How you treat the price of this product: follow the market, undercut or charge a premium.", "posture")
        add("What it costs to make one lot of this product.", "cost")
        add("The price minus the cost of one lot. Positive means you make money on each lot.", "margin")
        add("Lots your plants made last week.", "made")
        add("Lots sold last week, shown as a share of what you made. Low means stock is piling up.", "sold")
        add("Lots in the warehouse waiting for a buyer.", "stock")
        add("What one lot sells for now (for a share, what one share costs).", "price")
        add("How much the price moved over the last week.", "week")
        add("How much the price moved over the last three months.", "quarter")
        add("The price of one share times all the shares issued: what the market says the whole company is worth.", "market cap")
        add("The part of the shares anyone can buy. The rest is held by owners who are not selling.", "float", "free float")
        add("How many shares you hold, and the share of the company that makes.", "you hold")
        add("The company's sector, which decides what it makes and who it competes with.", "sector")
        add("Lots made and sold in the whole economy last week for this good.", "good")
        add("Lots brought in from outside the corridor last week to cover a shortage.", "imported")
        add("Lots sent out of the corridor last week because buyers inside did not need them.", "exported")
        add("How many shares this owner holds.", "shares")
        add("The part of all issued shares this owner holds.", "stake")
        add("Where the company is based.", "hq")
        add("How safe lenders think the company is, from A (safest) down. A lower grade means loans cost more.", "credit")
        add("What your shares are worth at today's price.", "value")
        add("Pay in this region compared with the country's average.", "wages")
        add("How skilled the local workers are. Skilled workers make more per plant.", "skill")
        add("How good the roads are. Better roads mean cheaper, faster deliveries.", "roads")
        add("How reliable the electricity supply is.", "power")
        add("Lots a region has to bring in each week because its own plants cannot supply them.", "lots")
        add("What an importer pays for a lot here, which is the most a new local plant can expect to charge.", "import price")
        // Stats and statements.
        add("What the company sold, before any cost.", "revenue")
        add("What the company earned after every cost, interest and tax.", "net income")
        add("Net income as a share of revenue: how much of each unit sold is kept as profit.", "net margin")
        add("Money the company owes lenders, including loans and bonds.", "debt")
        add("What the owners earn in a year on what they have in the company: net income scaled up to a year, divided by equity.", "return on equity")
        add("The cost of the goods sold: inputs, materials and fuel used up.", "cost of goods")
        add("Revenue minus the cost of goods: what is left to pay for everything else.", "gross profit")
        add("Pay for the company's workers.", "wages")
        add("Rent, upkeep, marketing and other running costs.", "operating costs")
        add("Other money that came in, such as sales of old plant.", "other income")
        add("The yearly wear on plants and buildings, spread across the weeks.", "depreciation")
        add("Gross profit minus wages, running costs and depreciation, before interest and tax.", "operating profit")
        add("Interest the company paid on its debts.", "interest paid")
        add("Interest the company earned on its cash.", "interest earned")
        add("Profit tax paid to the state.", "tax")
        add("Goods made and not yet sold, at what they cost to make.", "inventory")
        add("Cash, stock and money owed to the company, all things it will turn into cash within a year.", "current assets")
        add("Plants and works under construction, at cost less wear.", "plant and construction")
        add("Everything the company owns.", "total assets")
        add("Bills and debts due within a year.", "current liabilities")
        add("Everything the company owes.", "total liabilities")
        add("How many shares exist.", "shares issued")
        add("Cash at the start of the period.", "opening cash")
        add("Cash made or used by running the business.", "operating")
        add("Cash spent on, or got from, plants and other long-lived things.", "investing")
        add("Cash raised or paid back to lenders and shareholders.", "financing")
        add("Cash at the end of the period.", "closing cash")
        add("Gross profit as a share of revenue.", "gross margin")
        add("Debt divided by equity. Higher means more borrowing against what owners put in.", "debt to equity")
        add("Current assets divided by current liabilities. Below 1 means near-term bills are bigger than near-term money.", "current ratio")
        add("Operating profit divided by interest paid: how many times over the company covers its interest.", "interest cover")
        add("How much revenue rose or fell compared with the same period a year earlier.", "revenue growth")
        add("The share price divided by a year of earnings. High means buyers pay a lot for each dollar earned.", "price to earnings")
        add("This quarter's earnings scaled to a year.", "earnings a year")
        add("The share price times all shares issued.", "market value")
        // The economy card.
        add("How much the whole economy grew over the last year.", "growth, last year")
        add("How far output is above or below its normal path. Below zero is slack, above zero is a hot economy.", "output vs trend")
        add("What banks charge their best borrowers: the central bank rate plus a margin.", "prime lending rate")
        add("What the state pays to borrow for ten years. It moves with what people expect rates to do.", "10-year bond")
        // The profit bars.
        add("What was spent on materials, fuel and parts to make the goods sold.", "inputs and goods")
                add("Rent, upkeep, marketing and the other costs of running the company.", "overheads")
        add("Wear on plants, spread across the weeks.", "depreciation")
        add("Interest paid minus interest earned.", "interest")
    }

    /** The note for a figure's label, or null when there is none. A "Profit, 5 wk" style label counts as Profit. */
    fun about(label: String): String? {
        val key = label.trim().lowercase()
        return table[key] ?: if (key.startsWith("profit,")) table["profit, quarter"] else null
    }

    /** The labels the table covers, for the test that checks the plain-English rules. */
    internal fun all(): Map<String, String> = table
}
