package net.lakesidegames.corporatewarfare

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

/** Table row keys must be types an Android Bundle can hold, or the app crashes on its first table. */
class LazyKeyTest {
    @Test
    fun plainKeysPassThrough() {
        assertEquals("plant", lazyKey("plant"))
        assertEquals(12, lazyKey(12))
        assertEquals(12L, lazyKey(12L))
    }

    @Test
    fun unsignedKeysBecomeText() {
        assertEquals("k:12", lazyKey(12u))
        assertEquals("k:12", lazyKey(12uL))
        assertNotEquals(lazyKey(12u), lazyKey(13u))
    }
}
