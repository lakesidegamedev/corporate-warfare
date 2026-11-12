package net.lakesidegames.corporatewarfare

import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import java.io.File
import java.net.InetSocketAddress
import java.nio.file.Files
import java.security.MessageDigest
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The built-in updater's checksum rule, against a local fake download server. */
class UpdaterDigestTest {
    private var server: HttpServer? = null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val dir: File = Files.createTempDirectory("cw-updater-test").toFile()

    @AfterTest
    fun stop() {
        server?.stop(0)
        dir.deleteRecursively()
    }

    private class Platform(override val installerSuffix: String? = ".exe") : UpdatePlatform {
        val installed = mutableListOf<String>()
        val opened = mutableListOf<String>()
        override fun install(file: File, version: String) { installed += file.name }
        override fun open(url: String) { opened += url }
    }

    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    private fun serve(bytes: ByteArray): String {
        val s = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        s.createContext("/f") { ex ->
            ex.sendResponseHeaders(200, bytes.size.toLong())
            ex.responseBody.use { it.write(bytes) }
        }
        s.start()
        server = s
        return "http://127.0.0.1:${s.address.port}/f"
    }

    private fun waitFor(updater: Updater, done: (UpdateStatus) -> Boolean, platform: Platform? = null, until: () -> Boolean = { false }) {
        val end = System.currentTimeMillis() + 10_000
        while (System.currentTimeMillis() < end && !done(updater.status) && !until()) Thread.sleep(20)
    }

    private fun release(asset: Asset?, files: Boolean = true) =
        Release("9.9.9", "", "https://example.invalid/r", asset, hasFiles = files)

    private val payload = ByteArray(200_000) { (it * 31).toByte() }

    private fun run(asset: Asset, platform: Platform): Updater {
        val u = Updater(platform, scope, File(dir, "u"), current = "1.0.0")
        u.update(release(asset))
        waitFor(u, { it is UpdateStatus.Failed }, until = { platform.installed.isNotEmpty() })
        return u
    }

    @Test
    fun aMatchingDigestInstalls() {
        val url = serve(payload)
        val p = Platform()
        run(Asset("cw-setup.exe", url, payload.size.toLong(), sha(payload)), p)
        assertEquals(listOf("cw-setup.exe"), p.installed)
    }

    @Test
    fun aMismatchedDigestIsRefusedAndDeleted() {
        val url = serve(payload)
        val p = Platform()
        val u = run(Asset("cw-setup.exe", url, payload.size.toLong(), sha(ByteArray(8))), p)
        assertTrue(u.status is UpdateStatus.Failed, "status was ${u.status}")
        assertTrue((u.status as UpdateStatus.Failed).reason.contains("checksum"))
        assertTrue(p.installed.isEmpty())
        assertFalse(File(dir, "u/cw-setup.exe").exists())
        assertFalse(File(dir, "u/cw-setup.exe.part").exists())
    }

    @Test
    fun theDigestComparisonIgnoresCase() {
        val url = serve(payload)
        val p = Platform()
        run(Asset("cw-setup.exe", url, payload.size.toLong(), sha(payload).uppercase()), p)
        assertEquals(1, p.installed.size)
    }

    @Test
    fun noPublishedDigestFallsBackToTheByteCount() {
        val url = serve(payload)
        val p = Platform()
        run(Asset("cw-setup.exe", url, payload.size.toLong(), null), p)
        assertEquals(1, p.installed.size)
        val q = Platform()
        val u = run(Asset("cw-setup.exe", url, payload.size.toLong() + 5, null), q)
        assertTrue(u.status is UpdateStatus.Failed)
        assertTrue(q.installed.isEmpty())
    }

    @Test
    fun githubDigestsAreReadFromTheFeed() {
        val feed = """[{"tag_name":"v9.9.9","html_url":"https://example.invalid/r","assets":[
            {"name":"a-setup.exe","browser_download_url":"https://example.invalid/a","size":10,"digest":"sha256:${"AB".repeat(32)}"},
            {"name":"b.msi","browser_download_url":"https://example.invalid/b","size":10,"digest":"sha512:abc"},
            {"name":"c.deb","browser_download_url":"https://example.invalid/c","size":10}]}]"""
        fun installer(suffix: String) = Updater.parse(feed, suffix).single().installer
        assertEquals("ab".repeat(32), installer("-setup.exe")?.sha256)
        assertNull(installer(".msi")?.sha256, "another algorithm is not a SHA-256")
        assertNull(installer(".deb")?.sha256)
        assertNull(Updater.sha256Of("sha256:zz"))
        assertNull(Updater.sha256Of("ab".repeat(32)))
        assertNull(Updater.sha256Of(null))
    }

    @Test
    fun aReleaseWithNoFilesSaysItIsDeliveredElsewhere() {
        val feed = """[{"tag_name":"v9.9.9","html_url":"https://example.invalid/r","assets":[]}]"""
        val r = Updater.parse(feed, ".exe").single()
        assertFalse(r.hasFiles)
        assertNull(r.installer)
        for (suffix in listOf<String?>(".exe", null)) {
            val p = Platform(suffix)
            val u = Updater(p, scope, File(dir, "u"), current = "1.0.0")
            u.update(r)
            assertTrue(u.status is UpdateStatus.NoInstaller, "suffix $suffix gave ${u.status}")
            assertTrue(p.opened.isEmpty())
        }
    }

    @Test
    fun aWindowsReleaseWithoutItsInstallerIsAlsoNotAnError() {
        val p = Platform(".exe")
        val u = Updater(p, scope, File(dir, "u"), current = "1.0.0")
        u.update(release(null, files = true))
        assertTrue(u.status is UpdateStatus.NoInstaller)
    }

    @Test
    fun linuxWithFilesStillOpensThePage() {
        val p = Platform(null)
        val u = Updater(p, scope, File(dir, "u"), current = "1.0.0")
        u.update(release(null, files = true))
        assertEquals(listOf("https://example.invalid/r"), p.opened)
    }
}
