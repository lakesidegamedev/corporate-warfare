package net.lakesidegames.corporatewarfare

import java.io.File
import java.net.ServerSocket
import java.nio.file.Files
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

class SingleInstanceTest {
    private fun tempDir(): File = Files.createTempDirectory("cw-single").toFile()

    @Test
    fun secondLaunchSignalsTheFirstAndDoesNotStart() {
        val dir = tempDir()
        val activated = CountDownLatch(2)
        val first = SingleInstance.start(dir) { activated.countDown() }
        assertIs<SingleInstance.Start.First>(first)
        // A second attempt finds the lock held, wakes the first and is told to exit.
        assertEquals(SingleInstance.Start.Signalled, SingleInstance.start(dir) { error("second copy must not listen") })
        assertEquals(SingleInstance.Start.Signalled, SingleInstance.start(dir) { error("third copy must not listen") })
        assertTrue(activated.await(5, TimeUnit.SECONDS))
        first.instance.close()
        // Closed (the game exited): the next launch is the only copy again.
        val again = SingleInstance.start(dir) {}
        assertIs<SingleInstance.Start.First>(again)
        again.instance.close()
        dir.deleteRecursively()
    }

    @Test
    fun staleFilesFromACrashDoNotBlockAStart() {
        val dir = tempDir()
        // A crashed copy leaves both files behind, but the OS has freed its lock.
        val dead = ServerSocket(0).use { it.localPort }
        File(dir, SingleInstance.LOCK_FILE).writeText("")
        File(dir, SingleInstance.PORT_FILE).writeText("$dead old-token\n")
        val start = SingleInstance.start(dir) {}
        assertIs<SingleInstance.Start.First>(start)
        // The port file now names the live copy, not the dead one.
        assertTrue(!File(dir, SingleInstance.PORT_FILE).readText().startsWith("$dead "))
        assertEquals(SingleInstance.Start.Signalled, SingleInstance.start(dir) {})
        start.instance.close()
        dir.deleteRecursively()
    }

    @Test
    fun lockHeldByACopyThatDoesNotAnswerIsReportedNotOpened() {
        val dir = tempDir()
        val first = SingleInstance.start(dir) {}
        assertIs<SingleInstance.Start.First>(first)
        // The holder stops answering (hung), but still holds the lock.
        val dead = ServerSocket(0).use { it.localPort }
        File(dir, SingleInstance.PORT_FILE).writeText("$dead token\n")
        assertEquals(SingleInstance.Start.Unresponsive, SingleInstance.start(dir, timeoutMs = 600) {})
        // Garbage in the port file reads the same way.
        File(dir, SingleInstance.PORT_FILE).writeText("not a port")
        assertEquals(SingleInstance.Start.Unresponsive, SingleInstance.start(dir, timeoutMs = 600) {})
        first.instance.close()
        dir.deleteRecursively()
    }

    @Test
    fun aWrongTokenDoesNotWakeTheGame() {
        val dir = tempDir()
        var woken = false
        val first = SingleInstance.start(dir) { woken = true }
        assertIs<SingleInstance.Start.First>(first)
        val port = File(dir, SingleInstance.PORT_FILE).readText().substringBefore(' ')
        File(dir, SingleInstance.PORT_FILE).writeText("$port someone-else\n")
        assertEquals(SingleInstance.Start.Unresponsive, SingleInstance.start(dir, timeoutMs = 600) {})
        assertTrue(!woken)
        first.instance.close()
        dir.deleteRecursively()
    }
}
