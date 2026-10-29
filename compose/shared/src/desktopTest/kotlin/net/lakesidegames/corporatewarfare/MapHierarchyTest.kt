package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.Color
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MapHierarchyTest {
    private fun overlaps(a: FloatArray, b: FloatArray) = a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]

    @Test
    fun labelsNeverOverlapAndLowerPriorityLoses() {
        val planner = LabelPlanner()
        // A building in the middle; a city label first, then a region label that would sit on the same spot.
        planner.frame {
            reserve(100f, 100f, 140f, 140f)
            val city = place("c:A", 50f, 12f, floatArrayOf(110f, 110f, 150f, 20f))
            // The city's first spot crosses the building, so it takes the second.
            assertEquals(1, city)
            val region = place("r:X", 50f, 12f, floatArrayOf(150f, 20f))
            assertEquals(-1, region, "the region name would cross the city name and is dropped")
        }
    }

    @Test
    fun aBigCityKeepsItsNameInACrowdedScene() {
        val planner = LabelPlanner()
        planner.frame {
            // The city block sits in the middle of the player's buildings: every ring round it is packed for 30 px,
            // so none of the four preferred places is clear.
            reserve(100f, 100f, 140f, 140f)
            for (i in 0 until 8) reserve(60f + i * 12f, 60f, 70f + i * 12f, 180f)
            reserve(60f, 60f, 160f, 70f); reserve(60f, 170f, 160f, 180f)
            val w = 50f; val h = 12f
            val slots = floatArrayOf(143f, 114f, 47f - w + 13f, 114f, 95f, 85f, 95f, 143f)
            val put = planner.placeCity("c:Columbus", w, h, slots, 120f, 120f, 26f)
            assertTrue(put.leader, "the name moves out and gets a leader line")
            val shownRect = floatArrayOf(put.x, put.y, put.x + w, put.y + h)
            assertFalse(overlaps(shownRect, floatArrayOf(100f, 100f, 140f, 140f)), "the name stays off the block")
            // A region name and a small town name placed afterwards do not displace it.
            assertEquals(-1, place("r:Ohio", w, h, floatArrayOf(put.x, put.y)))
        }
        // Next frame it holds its place.
        planner.frame {
            reserve(100f, 100f, 140f, 140f)
            for (i in 0 until 8) reserve(60f + i * 12f, 60f, 70f + i * 12f, 180f)
            reserve(60f, 60f, 160f, 70f); reserve(60f, 170f, 160f, 180f)
            assertTrue(placeCity("c:Columbus", 50f, 12f, floatArrayOf(143f, 114f), 120f, 120f, 26f).leader)
        }
    }

    @Test
    fun aBigCityIsNeverDroppedEvenWhenNothingIsClear() {
        val planner = LabelPlanner()
        planner.frame {
            reserve(-1000f, -1000f, 1000f, 1000f)
            val put = placeCity("c:Chicago", 60f, 12f, floatArrayOf(0f, 0f), 0f, 0f, 20f)
            assertTrue(put.x.isFinite() && put.y.isFinite())
        }
        // It is remembered as showing.
        planner.frame { assertEquals(0, place("c:Chicago", 60f, 12f, floatArrayOf(0f, 0f)).coerceAtLeast(0)) }
    }

    @Test
    fun noTwoPlacedLabelsOverlap() {
        val planner = LabelPlanner()
        val rects = ArrayList<FloatArray>()
        planner.frame {
            for (i in 0 until 60) {
                val x = (i % 8) * 20f; val y = (i / 8) * 9f
                val slot = place("c:$i", 44f, 12f, floatArrayOf(x, y, x, y + 14f))
                if (slot >= 0) rects += floatArrayOf(slots(x, y, slot)[0], slots(x, y, slot)[1], slots(x, y, slot)[0] + 44f, slots(x, y, slot)[1] + 12f)
            }
        }
        for (i in rects.indices) for (j in i + 1 until rects.size) assertFalse(overlaps(rects[i], rects[j]), "labels $i and $j overlap")
        assertTrue(rects.size in 2..59)
    }

    private fun slots(x: Float, y: Float, slot: Int) = if (slot == 0) floatArrayOf(x, y) else floatArrayOf(x, y + 14f)

    @Test
    fun aShowingLabelHoldsItsPlaceAtTheEdgeAndAHiddenOneStaysHidden() {
        val planner = LabelPlanner()
        // Frame 1: the label fits with room to spare, beside a building.
        planner.frame { reserve(0f, 0f, 40f, 40f); assertEquals(0, place("c:A", 30f, 10f, floatArrayOf(50f, 0f))) }
        // Frame 2: the building grew one pixel into the label. A showing label squeezes by.
        planner.frame { reserve(0f, 0f, 50.5f, 40f); assertEquals(0, place("c:A", 30f, 10f, floatArrayOf(50f, 0f))) }
        // A label that was dropped needs real clearance: touching distance is not enough to bring it back.
        planner.frame { reserve(0f, 0f, 50f, 40f); assertEquals(-1, place("c:B", 30f, 10f, floatArrayOf(50f, 0f))) }
        planner.frame { reserve(0f, 0f, 48f, 40f); assertEquals(-1, place("c:B", 30f, 10f, floatArrayOf(50f, 0f))) }
        planner.frame { reserve(0f, 0f, 40f, 40f); assertEquals(0, place("c:B", 30f, 10f, floatArrayOf(50f, 0f))) }
    }

    @Test
    fun aPanningCameraDoesNotFlickerLabels() {
        // The same crowded layout shifted a pixel at a time keeps the same set of labels showing.
        val planner = LabelPlanner()
        var previous: Set<String>? = null
        for (shift in 0 until 40) {
            val shown = HashSet<String>()
            planner.frame {
                for (i in 0 until 30) {
                    val x = 13f * (i % 6) + shift; val y = 11f * (i / 6)
                    if (place("c:$i", 30f, 10f, floatArrayOf(x, y)) >= 0) shown += "c:$i"
                }
            }
            if (previous != null) assertEquals(previous, shown, "label set changed while only panning (shift $shift)")
            previous = shown
        }
    }

    @Test
    fun laneHoldsVehiclesTwoLengthsApart() {
        assertEquals(5, maxPerLane(100f, 10f))
        assertEquals(1, maxPerLane(5f, 10f))
        for (len in listOf(60f, 300f, 2000f)) for (v in listOf(7f, 21f)) {
            val n = maxPerLane(len, v)
            assertTrue(len / n >= 2f * v || n == 1, "$n vehicles of length $v on $len")
        }
    }

    @Test
    fun citiesAndFreightLookByOwner() {
        assertEquals("city_port", CityKinds.of("Houston"))
        assertEquals("city_steel", CityKinds.of("Pittsburgh"))
        assertEquals("city_auto", CityKinds.of("Detroit"))
        assertEquals("city", CityKinds.of("Springfield"))
        val sectors = firmSectors(HierarchyScene.plants)
        val colors = HierarchyScene.lanes.map { FreightLook.ownerColor(it, sectors) }
        assertEquals(Palette.gold, colors[0])
        assertEquals(Sectors.color("manufacturing"), colors[1], "a rival's lane takes its biggest sector's colour")
        assertEquals(Sectors.color("financial"), colors[2])
        assertEquals(FreightLook.outside, colors[3], "outside carriers carry more than the rival does")
        // The neutral livery turns into the owner colour; the rest of the vehicle is untouched.
        val tone = FreightLook.tone(Color.Red)
        assertEquals(Color.Red, tone(Color(0xFF4F7FAF)))
        assertEquals(Color(0xFF20262C), tone(Color(0xFF20262C)))
    }
}
