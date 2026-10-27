package net.lakesidegames.corporatewarfare

import java.text.NumberFormat
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToLong

/**
 * Money and number formatting shared by every screen. Amounts arrive in integer cents of some
 * currency. With one money the symbol is "$"; a multi-currency scenario sets the reporting
 * symbol and the symbols of every currency, and a figure in another currency names its own.
 */
object Format {
    private val whole = NumberFormat.getCurrencyInstance(Locale.US).apply { maximumFractionDigits = 0 }
    private val cents = NumberFormat.getCurrencyInstance(Locale.US).apply {
        minimumFractionDigits = 2
        maximumFractionDigits = 2
    }
    private val wholeNumber = NumberFormat.getIntegerInstance(Locale.US)
    private val centsNumber = NumberFormat.getNumberInstance(Locale.US).apply {
        minimumFractionDigits = 2
        maximumFractionDigits = 2
    }

    private var symbol = "$"
    private var symbols: Map<String, String> = emptyMap()

    /** Currency id that fair values and share marks are quoted in; empty with one money. */
    var numeraire: String = ""
        private set

    /** Set the reporting symbol and the id to symbol table. An empty [reporting] restores "$". */
    fun configure(reporting: String, symbols: Map<String, String>, numeraire: String) {
        this.symbol = reporting.ifEmpty { "$" }
        this.symbols = symbols
        this.numeraire = numeraire
    }

    /** Back to one money in US dollars. */
    fun reset() = configure("", emptyMap(), "")

    /** The symbol for a currency id; an empty or unknown id gives the reporting symbol. */
    fun symbolOf(currency: String): String = if (currency.isEmpty()) symbol else symbols[currency] ?: symbol

    /** "$" sits against the figure; longer symbols such as "DM" get a thin gap. */
    private fun lead(sym: String): String = if (sym.length > 1) "$sym " else sym

    private fun body(value: Long, sym: String, fractional: Boolean): String {
        if (sym == "$") return (if (fractional) cents else whole).format(value / 100.0)
        val sign = if (value < 0) "-" else ""
        val magnitude = abs(value) / 100.0
        val text = if (fractional) centsNumber.format(magnitude) else wholeNumber.format(magnitude.roundToLong())
        return sign + lead(sym) + text
    }

    fun money(value: Long, currency: String = ""): String = body(value, symbolOf(currency), false)

    fun price(value: Long, currency: String = ""): String = body(value, symbolOf(currency), true)

    fun compact(value: Long, currency: String = ""): String {
        val units = abs(value / 100.0)
        val sign = if (value < 0) "-" else ""
        val lead = lead(symbolOf(currency))
        return when {
            units >= 1e9 -> sign + lead + "%.1fB".format(units / 1e9)
            units >= 1e6 -> sign + lead + "%.1fM".format(units / 1e6)
            units >= 1e4 -> sign + lead + "%.0fK".format(units / 1e3)
            else -> money(value, currency)
        }
    }

    fun signed(value: Long, currency: String = ""): String = (if (value >= 0) "+" else "") + money(value, currency)

    fun bps(value: Long): String = "%+.1f%%".format(value / 100.0)

    fun count(value: Long): String = NumberFormat.getIntegerInstance(Locale.US).format(value)

    /** An exchange rate from millionths, four decimals. */
    fun rate(ppm: Long): String = "%.4f".format(ppm / 1_000_000.0)
}
