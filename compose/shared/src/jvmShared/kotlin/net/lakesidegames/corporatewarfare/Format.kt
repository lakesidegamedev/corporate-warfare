package net.lakesidegames.corporatewarfare

import java.text.NumberFormat
import java.util.Locale
import kotlin.math.abs

/** Money and number formatting shared by every screen. Amounts arrive in integer cents. */
object Format {
    private val whole = NumberFormat.getCurrencyInstance(Locale.US).apply { maximumFractionDigits = 0 }
    private val cents = NumberFormat.getCurrencyInstance(Locale.US).apply {
        minimumFractionDigits = 2
        maximumFractionDigits = 2
    }

    fun money(value: Long): String = whole.format(value / 100.0)

    fun price(value: Long): String = cents.format(value / 100.0)

    fun compact(value: Long): String {
        val dollars = abs(value / 100.0)
        val sign = if (value < 0) "-" else ""
        return when {
            dollars >= 1e9 -> sign + "$%.1fB".format(dollars / 1e9)
            dollars >= 1e6 -> sign + "$%.1fM".format(dollars / 1e6)
            dollars >= 1e4 -> sign + "$%.0fK".format(dollars / 1e3)
            else -> money(value)
        }
    }

    fun signed(value: Long): String = (if (value >= 0) "+" else "") + money(value)

    fun bps(value: Long): String = "%+.1f%%".format(value / 100.0)

    fun count(value: Long): String = NumberFormat.getIntegerInstance(Locale.US).format(value)
}
