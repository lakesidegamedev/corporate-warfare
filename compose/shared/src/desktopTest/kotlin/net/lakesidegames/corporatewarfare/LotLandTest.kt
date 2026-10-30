package net.lakesidegames.corporatewarfare

import kotlin.math.hypot
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * A building lot is never over water. Every lot of every region of every scenario (a division's
 * lot for each sector, and each of the eight rival slots) is checked against the country and lake
 * outlines, the same ones the map is drawn from: before the fix at the fixed offset from the hub,
 * after it where the lot now stands.
 */
class LotLandTest {
    private val scenarios = listOf("great-lakes-2000", "texas-1981", "silicon-valley-1995", "ruhr-rhine-1991")

    private class Lot(val region: String, val offset: Pair<Double, Double>, val radius: Double)

    private fun lots(map: WorldMap): List<Lot> = map.regions.filter { map.anchor(it.code) != null }.flatMap { r ->
        Sectors.order.map { Lot(r.code, WorldScale.playerLot(it), LotLand.PLAYER_RADIUS_M) } +
            (0 until 8).map { Lot(r.code, WorldScale.rivalLot(it), LotLand.RIVAL_RADIUS_M) }
    }

    @Test
    fun noLotStandsInTheWater() {
        val report = StringBuilder()
        for (id in scenarios) {
            val map = WorldMap.forScenario(id)!!
            val all = lots(map)
            val wetBefore = all.count { !LotLand.isDry(map, it.region, it.offset.first, it.offset.second, 0.0) }
            val footBefore = all.count { !LotLand.isDry(map, it.region, it.offset.first, it.offset.second, it.radius) }
            val outBefore = all.count { !LotLand.isInRegion(map, it.region, it.offset.first, it.offset.second) }
            var wetAfter = 0; var outAfter = 0; var moved = 0; var far = 0
            for (lot in all) {
                val after = LotLand.dry(map, lot.region, lot.offset, lot.radius)
                assertEquals(after, LotLand.dry(map, lot.region, lot.offset, lot.radius), "the same lot always lands in the same place")
                if (!LotLand.isDry(map, lot.region, after.first, after.second, lot.radius)) wetAfter++
                if (!LotLand.isInRegion(map, lot.region, after.first, after.second)) outAfter++
                if (after != lot.offset) moved++
                // Walking toward the hub and round it never takes a lot farther out than it began.
                if (hypot(after.first, after.second) > hypot(lot.offset.first, lot.offset.second) + 1.0) far++
            }
            report.append("$id: ${all.size} lots, centre over water before $wetBefore, footprint wet before $footBefore, outside region before $outBefore; after: footprint wet $wetAfter, outside region $outAfter, moved $moved\n")
            assertEquals(0, wetAfter, "$id: lots with water under them\n$report")
            assertEquals(0, outAfter, "$id: lots outside their region\n$report")
            assertEquals(0, far, "$id: lots moved outward")
            assertTrue(moved <= all.size)
        }
        println("LOTLAND\n$report")
    }
}
