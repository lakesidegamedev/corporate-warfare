package net.lakesidegames.corporatewarfare

import com.sun.net.httpserver.HttpExchange
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import java.io.File
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.nio.file.Files
import java.security.MessageDigest
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The signed-in download of a paid build, against a local fake of the Lakeside account service. */
class LakesideDownloadTest {
    private var server: HttpServer? = null
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val dir: File = Files.createTempDirectory("cw-paid-test").toFile()
    private val payload = ByteArray(150_000) { (it * 17).toByte() }

    /** Authorization header of each request, by path, as the fake server saw it. */
    private val seen = CopyOnWriteArrayList<Pair<String, String?>>()

    @AfterTest
    fun stop() {
        server?.stop(0)
        dir.deleteRecursively()
    }

    private class Platform : UpdatePlatform {
        override val installerSuffix: String? = ".exe"
        override val paidKeys = listOf("windows-x86_64")
        val installed = CopyOnWriteArrayList<String>()
        val opened = CopyOnWriteArrayList<String>()
        override fun install(file: File, version: String) { installed += file.name }
        override fun open(url: String) { opened += url }
    }

    private class FakeAuth(var token: String?, private val signInTo: String? = null) : LakesideAuth {
        var signedOut = false
        override val name: String? get() = token?.let { "tester" }
        override fun restore() = Unit
        override fun signIn(provider: String): SignInResult {
            token = signInTo
            return if (signInTo != null) SignInResult.Done("tester") else SignInResult.Failed("no")
        }
        override fun cancelSignIn() = Unit
        override fun token(): String? = token
        override fun signOut() { token = null; signedOut = true }
    }

    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    private fun HttpExchange.send(code: Int, body: ByteArray = ByteArray(0), type: String = "application/json") {
        responseHeaders.add("Content-Type", type)
        sendResponseHeaders(code, if (body.isEmpty()) -1 else body.size.toLong())
        if (body.isNotEmpty()) responseBody.use { it.write(body) }
        close()
    }

    /** Starts the fake service; returns its base URL. [fileStatus] is what the file path answers when it does not redirect. */
    private fun serve(
        entitled: Boolean = true,
        advertisedSha: String = sha(payload),
        listingStatus: Int = 200,
        fileStatus: Int = 302,
        version: String = "9.9.9",
    ): String {
        val s = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        s.createContext("/account/api/downloads/corporate-warfare") { ex ->
            seen += "listing" to ex.requestHeaders.getFirst("Authorization")
            if (listingStatus != 200) return@createContext ex.send(listingStatus)
            val files = if (entitled) {
                """[{"platform":"windows-x86_64","name":"cw-9.9.9-setup.exe","size":${payload.size},"sha256":"$advertisedSha","installer":"inno","url":"/account/launcher/download/corporate-warfare/windows-x86_64"}]"""
            } else "[]"
            val v = if (entitled) "\"$version\"" else "null"
            ex.send(200, """{"product":"corporate-warfare","version":$v,"entitled":$entitled,"files":$files}""".toByteArray())
        }
        s.createContext("/account/launcher/download/corporate-warfare/windows-x86_64") { ex ->
            seen += "download" to ex.requestHeaders.getFirst("Authorization")
            if (fileStatus != 302) return@createContext ex.send(fileStatus)
            ex.responseHeaders.add("Location", "http://127.0.0.1:${s.address.port}/storage/file?sig=abc")
            ex.send(302)
        }
        s.createContext("/storage/file") { ex ->
            seen += "storage" to ex.requestHeaders.getFirst("Authorization")
            ex.send(200, payload, "application/octet-stream")
        }
        s.start()
        server = s
        return "http://127.0.0.1:${s.address.port}"
    }

    private fun updater(base: String, platform: Platform, auth: LakesideAuth?) =
        Updater(platform, scope, File(dir, "u"), File(dir, "saves").also { it.mkdirs() }, feed = { emptyList() }, current = "1.0.0", auth = auth, accountBase = base)

    private fun release() = Release("9.9.9", "", Updater.STORE_URL, null, hasFiles = false)

    private fun wait(u: Updater, until: (UpdateStatus) -> Boolean) {
        val end = System.currentTimeMillis() + 10_000
        while (System.currentTimeMillis() < end && !until(u.status)) Thread.sleep(20)
    }

    @Test
    fun anEntitledDownloadIsVerifiedAndInstalled() {
        val base = serve()
        val p = Platform()
        val u = updater(base, p, FakeAuth("tok-1"))
        u.update(release())
        wait(u) { p.installed.isNotEmpty() || it is UpdateStatus.Failed || it is UpdateStatus.Unavailable }
        assertEquals(listOf("cw-9.9.9-setup.exe"), p.installed.toList(), "status was ${u.status}")
        assertEquals("Bearer tok-1", seen.first { it.first == "listing" }.second)
        assertEquals("Bearer tok-1", seen.first { it.first == "download" }.second)
    }

    @Test
    fun theRedirectIsFollowedWithoutTheSignIn() {
        val base = serve()
        val p = Platform()
        val u = updater(base, p, FakeAuth("tok-2"))
        u.update(release())
        wait(u) { p.installed.isNotEmpty() || it is UpdateStatus.Failed }
        val storage = seen.filter { it.first == "storage" }
        assertEquals(1, storage.size, "the storage link was fetched once")
        assertNull(storage.single().second, "the storage link must never receive the token")
    }

    @Test
    fun aShaMismatchIsRefusedAndNothingIsInstalled() {
        val base = serve(advertisedSha = sha(ByteArray(8)))
        val p = Platform()
        val u = updater(base, p, FakeAuth("tok-3"))
        u.update(release())
        wait(u) { p.installed.isNotEmpty() || it is UpdateStatus.Failed }
        val status = u.status
        assertTrue(status is UpdateStatus.Failed && status.reason.contains("checksum"), "status was $status")
        assertTrue(p.installed.isEmpty())
        assertFalse(File(dir, "u/cw-9.9.9-setup.exe").exists())
        assertFalse(File(dir, "u/cw-9.9.9-setup.exe.part").exists())
    }

    @Test
    fun aListingWithNoChecksumIsRefused() {
        val base = serve(advertisedSha = "")
        val p = Platform()
        val u = updater(base, p, FakeAuth("tok-4"))
        u.update(release())
        wait(u) { it is UpdateStatus.Unavailable || p.installed.isNotEmpty() }
        assertTrue(u.status is UpdateStatus.Unavailable, "status was ${u.status}")
        assertTrue(p.installed.isEmpty())
    }

    @Test
    fun anAccountThatDoesNotOwnTheGameSaysSoAndOffersBuy() {
        val base = serve(entitled = false)
        val p = Platform()
        val u = updater(base, p, FakeAuth("tok-5"))
        u.update(release())
        wait(u) { it is UpdateStatus.NotEntitled }
        assertTrue(u.status is UpdateStatus.NotEntitled, "status was ${u.status}")
        u.openStore()
        assertEquals(listOf(Updater.STORE_URL), p.opened.toList())
        assertTrue(p.installed.isEmpty())
    }

    @Test
    fun aForbiddenDownloadIsTheSameNotOwnedAnswer() {
        val base = serve(fileStatus = 403)
        val p = Platform()
        val u = updater(base, p, FakeAuth("tok-6"))
        u.update(release())
        wait(u) { it is UpdateStatus.NotEntitled || p.installed.isNotEmpty() }
        assertTrue(u.status is UpdateStatus.NotEntitled, "status was ${u.status}")
        assertTrue(p.installed.isEmpty())
    }

    @Test
    fun anExpiredSignInIsDroppedAndAsksAgain() {
        val base = serve(listingStatus = 401)
        val p = Platform()
        val auth = FakeAuth("old-token")
        val u = updater(base, p, auth)
        u.update(release())
        wait(u) { it is UpdateStatus.SignInNeeded }
        assertTrue(u.status is UpdateStatus.SignInNeeded, "status was ${u.status}")
        assertTrue(auth.signedOut)
        assertNull(auth.token())
    }

    @Test
    fun offlineGivesAPlainMessageAndTheGameKeepsWorking() {
        val closed = ServerSocket(0).use { it.localPort }
        val p = Platform()
        val u = updater("http://127.0.0.1:$closed", p, FakeAuth("tok-7"))
        u.update(release())
        wait(u) { it is UpdateStatus.Unavailable }
        val status = u.status
        assertTrue(status is UpdateStatus.Unavailable && status.reason.contains("Could not reach Lakeside"), "status was $status")
        assertTrue(p.installed.isEmpty())
        u.openStore()
        assertEquals(listOf(Updater.STORE_URL), p.opened.toList())
    }

    @Test
    fun signedOutAsksToSignInThenDownloads() {
        val base = serve()
        val p = Platform()
        val auth = FakeAuth(null, signInTo = "fresh-token")
        val u = updater(base, p, auth)
        u.update(release())
        wait(u) { it is UpdateStatus.SignInNeeded }
        assertTrue(u.status is UpdateStatus.SignInNeeded, "status was ${u.status}")
        assertTrue(seen.isEmpty(), "nothing is sent to Lakeside before sign-in")
        u.signIn((u.status as UpdateStatus.SignInNeeded).release)
        wait(u) { p.installed.isNotEmpty() || it is UpdateStatus.Failed || it is UpdateStatus.Unavailable }
        assertEquals(1, p.installed.size, "status was ${u.status}")
        assertEquals("Bearer fresh-token", seen.first { it.first == "listing" }.second)
    }

    @Test
    fun aSignedInCheckFindsANewerPaidBuildWhenGithubHasNone() {
        val base = serve()
        val u = updater(base, Platform(), FakeAuth("tok-8"))
        u.check()
        wait(u) { it is UpdateStatus.Available || it is UpdateStatus.Current || it is UpdateStatus.Failed }
        val status = u.status
        assertTrue(status is UpdateStatus.Available && status.release.version == "9.9.9", "status was $status")
    }

    @Test
    fun aSignedInCheckThatIsCurrentStaysCurrent() {
        val base = serve(version = "1.0.0")
        val u = updater(base, Platform(), FakeAuth("tok-9"))
        u.check()
        wait(u) { it is UpdateStatus.Available || it is UpdateStatus.Current || it is UpdateStatus.Failed }
        assertEquals(UpdateStatus.Current, u.status)
    }
}
