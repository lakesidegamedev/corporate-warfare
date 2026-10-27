package net.lakesidegames.corporatewarfare

import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals

class FormatTest {
    @AfterTest
    fun restore() = Format.reset()

    @Test
    fun singleMoneyKeepsDollars() {
        Format.reset()
        assertEquals("$1,235", Format.money(123_456))
        assertEquals("-$1,235", Format.money(-123_456))
        assertEquals("$12.35", Format.price(1_235))
        assertEquals("$1.2M", Format.compact(120_000_000))
        assertEquals("-$21K", Format.compact(-2_100_000))
        assertEquals("+$5", Format.signed(500))
        assertEquals("$1,235", Format.money(123_456, ""))
    }

    @Test
    fun multiLetterSymbolsGetAThinGap() {
        Format.configure("DM", mapOf("DM" to "DM", "NLG" to "f", "BEF" to "BF", "FRF" to "F"), "DM")
        assertEquals("DM 1,235", Format.money(123_456))
        assertEquals("-DM 1,235", Format.money(-123_456))
        assertEquals("DM 1.2M", Format.compact(120_000_000))
        assertEquals("DM 12.35", Format.price(1_235))
    }

    @Test
    fun companyFiguresUseTheirOwnCurrency() {
        Format.configure("DM", mapOf("DM" to "DM", "NLG" to "f", "BEF" to "BF"), "DM")
        assertEquals("f1.2M", Format.compact(120_000_000, "NLG"))
        assertEquals("BF 1,235", Format.money(123_456, "BEF"))
        assertEquals("DM 1,235", Format.money(123_456, ""))
        assertEquals("DM", Format.symbolOf("XXX"))
        assertEquals("1.8500", Format.rate(1_850_000))
    }
}
