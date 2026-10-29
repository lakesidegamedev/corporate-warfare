package net.lakesidegames.corporatewarfare

import java.io.File
import java.io.RandomAccessFile
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.nio.channels.FileChannel
import java.nio.channels.FileLock
import java.nio.channels.OverlappingFileLockException
import java.util.UUID

/**
 * One copy of the desktop game at a time. The first copy holds an OS lock on `instance.lock` in
 * the save folder, which the OS frees when the process ends, even on a crash, so a stale file never
 * blocks a start. It also listens on a loopback port named in `instance.port` (a separate file,
 * because Windows locks are mandatory and would hide the port from readers). A second launch finds
 * the lock taken, asks the first copy over that port to come to the front, and exits. If the first
 * copy does not answer, the second says so instead of opening, so two copies never write the same
 * saves.
 */
class SingleInstance private constructor(
    private val channel: FileChannel,
    private val lock: FileLock,
    private val server: ServerSocket,
) : AutoCloseable {

    /** What a launch should do. */
    sealed interface Start {
        /** This is the only copy: open the game and keep [instance] until exit. */
        class First(val instance: SingleInstance) : Start

        /** Another copy is running and was asked to come to the front: exit quietly. */
        data object Signalled : Start

        /** Another copy holds the lock but did not answer: tell the player, then exit. */
        data object Unresponsive : Start
    }

    override fun close() {
        runCatching { server.close() }
        runCatching { lock.release() }
        runCatching { channel.close() }
    }

    companion object {
        const val LOCK_FILE = "instance.lock"
        const val PORT_FILE = "instance.port"
        private const val ACTIVATE = "activate"
        private const val OK = "ok"

        /**
         * Take the lock in [dir], or signal the copy holding it. [onActivate] runs on a background
         * thread whenever a later launch asks this copy to come to the front.
         */
        fun start(dir: File, timeoutMs: Int = 3_000, onActivate: () -> Unit): Start {
            dir.mkdirs()
            val channel = RandomAccessFile(File(dir, LOCK_FILE), "rw").channel
            val lock = try {
                channel.tryLock()
            } catch (_: OverlappingFileLockException) {
                null // Held by this same process: another start in-process counts as a second copy.
            } catch (_: java.io.IOException) {
                null
            }
            if (lock == null) {
                channel.close()
                return if (signal(dir, timeoutMs)) Start.Signalled else Start.Unresponsive
            }
            val server = ServerSocket(0, 8, InetAddress.getLoopbackAddress())
            val token = UUID.randomUUID().toString()
            val portFile = File(dir, PORT_FILE)
            val temp = File(dir, "$PORT_FILE.tmp")
            temp.writeText("${server.localPort} $token\n")
            if (!temp.renameTo(portFile)) {
                portFile.delete()
                temp.renameTo(portFile)
            }
            Thread({ listen(server, token, onActivate) }, "single-instance").apply { isDaemon = true }.start()
            return Start.First(SingleInstance(channel, lock, server))
        }

        private fun listen(server: ServerSocket, token: String, onActivate: () -> Unit) {
            while (!server.isClosed) {
                val client = runCatching { server.accept() }.getOrNull() ?: continue
                runCatching {
                    client.use { socket ->
                        socket.soTimeout = 2_000
                        val line = socket.getInputStream().bufferedReader().readLine()
                        if (line == "$ACTIVATE $token") {
                            socket.getOutputStream().write("$OK\n".toByteArray())
                            socket.getOutputStream().flush()
                            onActivate()
                        }
                    }
                }
            }
        }

        /**
         * Ask the copy named in the port file to come to the front; true when it answered. Tries
         * until [timeoutMs] passes, since a copy that has just started may not have written its
         * port yet.
         */
        private fun signal(dir: File, timeoutMs: Int): Boolean {
            val deadline = System.nanoTime() + timeoutMs * 1_000_000L
            while (true) {
                if (signalOnce(dir, timeoutMs.coerceAtMost(1_000))) return true
                if (System.nanoTime() >= deadline) return false
                Thread.sleep(200)
            }
        }

        private fun signalOnce(dir: File, timeoutMs: Int): Boolean = runCatching {
            val (port, token) = File(dir, PORT_FILE).readText().trim().split(' ', limit = 2)
            Socket().use { socket ->
                socket.connect(InetSocketAddress(InetAddress.getLoopbackAddress(), port.toInt()), timeoutMs)
                socket.soTimeout = timeoutMs
                socket.getOutputStream().write("$ACTIVATE $token\n".toByteArray())
                socket.getOutputStream().flush()
                socket.getInputStream().bufferedReader().readLine() == OK
            }
        }.getOrDefault(false)
    }
}
