package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class TelemetryTest {
    @Test
    fun singleWeekStepsSendOnTheBoundary() {
        assertFalse(Telemetry.crossed(3u, 4u))
        assertTrue(Telemetry.crossed(4u, 5u))
        assertFalse(Telemetry.crossed(5u, 6u))
    }

    @Test
    fun multiWeekAdvancesStillSendWhenTheyStepOverABoundary() {
        assertTrue(Telemetry.crossed(3u, 7u))
        assertTrue(Telemetry.crossed(5u, 12u))
        assertFalse(Telemetry.crossed(6u, 9u))
        assertFalse(Telemetry.crossed(8u, 8u))
    }
}
