package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * The 2.0 grid views: the view state, typed search, filters, history windows and saved views. The
 * expected answers are worked out by hand from the rows in each test, not from the code.
 */
class GridViewTest {
    private data class Good(val name: String, val sector: String, val priceCents: Long, val mine: Boolean)

    private val goods = listOf(
        Good("Steel", "Manufacturing", 12_000, false),
        Good("Plastics", "Chemicals", 4_500, true),
        Good("Fertiliser", "Chemicals", 2_000, false),
        Good("Paint", "Chemicals", 9_900, true),
        Good("Grain", "Agriculture", 800, false),
    )

    private val reader = RowReader<Good>(
        text = { r, id -> when (id) { "name" -> r.name; "sector" -> r.sector; else -> null } },
        // Prices are read in whole money units, as a player types them.
        figure = { r, id -> if (id == "price") r.priceCents / 100.0 else null },
        mine = { it.mine },
        words = { "${it.name} ${it.sector}" },
    )

    private fun resolve(name: String): String? = when (name.lowercase()) {
        "sector" -> "sector"
        "price" -> "price"
        "name", "good" -> "name"
        else -> null
    }

    @Test
    fun typedSearchBecomesChipsAndWords() {
        val f = ViewQuery.parse("sector:chemicals price>10 mine pa", ::resolve)
        assertEquals(
            listOf(
                Filter(FilterOp.Is, "sector", "chemicals"),
                Filter(FilterOp.Above, "price", number = 10.0),
                Filter(FilterOp.Mine),
                Filter(FilterOp.Text, text = "pa"),
            ),
            f,
        )
        // A term that names no column stays a word; a quoted run stays one word; "not" with a minus.
        assertEquals(listOf(Filter(FilterOp.Text, text = "colour:red")), ViewQuery.parse("colour:red", ::resolve))
        assertEquals(listOf(Filter(FilterOp.Not, "sector", "real estate")), ViewQuery.parse("-sector:\"real estate\"", ::resolve))
        assertEquals(listOf(Filter(FilterOp.Below, "price", number = 2.5), Filter(FilterOp.Top, "price", number = 3.0)), ViewQuery.parse("price<2.5 top3:price", ::resolve))
    }

    @Test
    fun filtersKeepWhatTheySay() {
        // Chemicals over 10 a lot that are mine: Plastics (45) is under 10? No: 45 is over 10. Paint 99 too. Fertiliser is not mine.
        val kept = applyFilters(goods, ViewQuery.parse("sector:chemicals price>10 mine", ::resolve), reader).map { it.name }
        assertEquals(listOf("Plastics", "Paint"), kept)
        // Not chemicals: Steel and Grain.
        assertEquals(listOf("Steel", "Grain"), applyFilters(goods, listOf(Filter(FilterOp.Not, "sector", "Chemicals")), reader).map { it.name })
        // The two dearest of everything: Steel (120) and Paint (99), in their row order.
        assertEquals(listOf("Steel", "Paint"), applyFilters(goods, listOf(Filter(FilterOp.Top, "price", number = 2.0)), reader).map { it.name })
        // Top applies after the other chips: the cheapest chemical is Fertiliser.
        assertEquals(listOf("Fertiliser"), applyFilters(goods, listOf(Filter(FilterOp.Is, "sector", "chem"), Filter(FilterOp.Bottom, "price", number = 1.0)), reader).map { it.name })
        // Free words must all appear.
        assertEquals(listOf("Paint"), applyFilters(goods, listOf(Filter(FilterOp.Text, text = "chem pai")), reader).map { it.name })
    }

    @Test
    fun historyWindowsAndChanges() {
        // 53 weekly prices from 100 to 152: a year back reaches the first, so the change is 52 and 52 percent.
        val weekly = (100L..152L).toList()
        assertEquals(52L, seriesChange(weekly, Timeframe.Y1))
        assertEquals(5_200L, seriesChangeBps(weekly, Timeframe.Y1))
        // 13 weeks: 152 against 139.
        assertEquals(13L, seriesChange(weekly, Timeframe.W13))
        // Five years is longer than the year kept: no answer, never a made-up one.
        assertNull(seriesChange(weekly, Timeframe.Y5))
        // All uses whatever is kept.
        assertEquals(52L, seriesChange(weekly, Timeframe.All))
        // Four-week results: 13 weeks needs 4 periods back (ceil 13/4), so 5 points.
        val periods = listOf(10L, 20L, 30L, 40L, 50L, 60L)
        assertEquals(listOf(20L, 30L, 40L, 50L, 60L), seriesWindow(periods, Timeframe.W13, step = 4))
        assertNull(seriesWindow(periods, Timeframe.Y1, step = 4))
        // A start of zero has no percent.
        assertNull(seriesChangeBps(listOf(0L, 5L, 9L, 9L, 9L), Timeframe.W4))
    }

    @Test
    fun viewStateEdits() {
        val v = ViewState("Prices", cols("name", "price", "price.Percent"), sort = "price.Percent")
        // A column switched off goes in every way it shows, and the sort with it.
        val off = v.withColumn("price", false)
        assertEquals(listOf("name"), off.columns.map { it.key })
        assertNull(off.sort)
        // Showing the percent column as a sparkline instead moves the sort with it.
        val spark = v.withMode("price.Percent", ColumnMode.Spark)
        assertEquals(listOf("name", "price", "price.Spark"), spark.columns.map { it.key })
        assertEquals("price.Spark", spark.sort)
        // Two ways that would collide are refused.
        assertEquals(v, v.withMode("price.Percent", ColumnMode.Value))
        // A second way of showing goes right after the first; the last way cannot go.
        val two = v.toggleMode("price", ColumnMode.Rank)
        assertEquals(listOf("name", "price", "price.Percent", "price.Rank"), two.columns.map { it.key })
        assertEquals(listOf("name", "price.Percent"), v.toggleMode("price", ColumnMode.Value).columns.map { it.key })
        val one = ViewState("x", cols("name", "cap"))
        assertEquals(one, one.toggleMode("cap", ColumnMode.Value))
        // The name is not part of the shape.
        assertTrue(v.sameShape(v.copy(name = "Mine")))
        assertFalse(v.sameShape(v.copy(groupBy = "sector")))
        // Adding the same Is chip twice keeps one; a different value adds another.
        val chips = v.withFilter(Filter(FilterOp.Is, "sector", "Chemicals")).withFilter(Filter(FilterOp.Is, "sector", "Chemicals")).withFilter(Filter(FilterOp.Is, "sector", "Energy"))
        assertEquals(2, chips.filters.size)
        // A second Above on one column replaces the first.
        val above = v.withFilter(Filter(FilterOp.Above, "price", number = 1.0)).withFilter(Filter(FilterOp.Above, "price", number = 5.0))
        assertEquals(listOf(Filter(FilterOp.Above, "price", number = 5.0)), above.filters)
    }

    @Test
    fun savedViewsRoundTripAndSurviveANewBook() {
        val prefs = MapViewPrefs()
        val built = listOf(ViewState("Overview", cols("name", "price")), ViewState("Prices", cols("name", "price.Percent")))
        val book = ViewBook("goods", built, prefs)
        assertEquals("Overview", book.activeName)
        // Save over a built-in view and as a new one, with every part of the shape set.
        val changed = built[0].copy(
            columns = cols("name", "price", "price.Spark"), sort = "price", descending = false, groupBy = "sector",
            filters = listOf(Filter(FilterOp.Is, "sector", "Chemicals"), Filter(FilterOp.Top, "price", number = 5.0)),
            timeframe = Timeframe.Y1, scope = listOf("mine", "sector:chemicals"),
        )
        book.save(changed)
        book.save(changed.copy(name = "Chem watch"))
        // A new book on the same settings (a new career, a restart) sees the same views, in order, and the last active one.
        val again = ViewBook("goods", built, prefs)
        assertEquals(listOf("Overview", "Prices", "Chem watch"), again.all.map { it.name })
        assertEquals(changed, again.named("Overview"))
        assertEquals("Chem watch", again.activeName)
        // Back to the built view drops only the saved change over it.
        again.restoreBuiltIn("Overview")
        assertEquals(built[0], again.named("Overview"))
        // A player's own view can be deleted; built-in ones cannot.
        again.delete("Chem watch")
        again.delete("Prices")
        assertEquals(listOf("Overview", "Prices"), again.all.map { it.name })
        // Export and import of one view as a small text file.
        val text = ViewJson.exportView(changed)
        assertEquals(changed, ViewJson.importView(text))
        assertNull(ViewJson.importView("not a view"))
        // Another screen's views are kept apart.
        assertEquals(listOf("Overview", "Prices"), ViewBook("league", built, prefs).all.map { it.name })
    }

    @Test
    fun builtInViewsNameRealColumns() {
        // Every built-in view only names columns its screen has, and sorts by a column it shows.
        val screens = mapOf(
            GoodsViews to setOf("name", "sector", "price", "vsnormal", "week", "made", "sold", "through", "stock", "imported", "exported", "yours"),
            LeagueViews to setOf("name", "sector", "revenue", "profit", "margin", "cash", "debt", "price", "week", "cap", "fair", "float", "divisions", "held", "overlap", "theirlots", "raids"),
            DivisionViews to setOf("name", "sector", "region", "strategy", "plants", "share", "stock", "serves", "made", "sold", "util", "price", "cost", "margin", "weekly"),
        )
        for ((views, ids) in screens) {
            assertTrue(views.size in 3..5)
            for (v in views) {
                assertTrue(v.columns.all { it.id in ids }, "${v.name} names a column its screen lacks")
                assertNotNull(v.columns.firstOrNull { it.key == v.sort }, "${v.name} sorts by a column it does not show")
            }
        }
    }

    @Test
    fun csvQuotesWhatNeedsIt() {
        val c = listOf(
            GridColumn<Good>("name", "Good", null, numeric = false, text = { it.name }),
            GridColumn<Good>("sector", "Sector, kind", text = { it.sector }),
        )
        assertEquals("Good,\"Sector, kind\"\nSteel,Manufacturing", gridCsv(goods.take(1), c))
    }
}
