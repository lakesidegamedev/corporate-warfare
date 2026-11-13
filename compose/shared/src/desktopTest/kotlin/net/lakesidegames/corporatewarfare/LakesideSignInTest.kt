package net.lakesidegames.corporatewarfare

import com.sun.net.httpserver.HttpServer
import java.net.HttpURLConnection
import java.net.InetSocketAddress
import java.net.URL
import java.net.URLDecoder
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The loopback sign-in against a fake account service: state check, code exchange, secure storage, expiry. */
class LakesideSignInTest {
    private var server: HttpServer? = null
    private val posts = CopyOnWriteArrayList<String>()

    @AfterTest
    fun stop() { server?.stop(0) }

    private class MemoryStore : TokenStore {
        var secret: String? = null
        var deleted = false
        override fun load() = secret
        override fun save(secret: String): Boolean { this.secret = secret; return true }
        override fun delete() { secret = null; deleted = true }
    }

    private fun serve(status: Int = 200): String {
        val s = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        s.createContext("/account/launcher/session") { ex ->
            posts += String(ex.requestBody.readAllBytes())
            val body = if (status == 200) """{"token":"opaque-token","session":{"username":"Ada","email":"a@example.invalid","provider":"ahd"}}""".toByteArray() else ByteArray(0)
            ex.sendResponseHeaders(status, if (body.isEmpty()) -1 else body.size.toLong())
            if (body.isNotEmpty()) ex.responseBody.use { it.write(body) }
            ex.close()
        }
        s.start()
        server = s
        return "http://127.0.0.1:${s.address.port}"
    }

    private fun query(url: String) = URL(url).query.split('&').associate { URLDecoder.decode(it.substringBefore('='), "UTF-8") to URLDecoder.decode(it.substringAfter('='), "UTF-8") }

    private fun get(url: String): Int {
        val c = URL(url).openConnection() as HttpURLConnection
        return try { c.responseCode } finally { c.disconnect() }
    }

    /** A "browser": reads the start URL and calls back on its port with [stateOf] of the real state. */
    private fun browser(visited: MutableList<String>, stateOf: (String) -> String = { it }) = { url: String ->
        visited += url
        val q = query(url)
        Thread {
            // A stray request and a wrong state first: both must be ignored.
            runCatching { get("http://127.0.0.1:${q["port"]}/other") }
            runCatching { get("http://127.0.0.1:${q["port"]}/callback?code=bad&state=wrong") }
            runCatching { get("http://127.0.0.1:${q["port"]}/callback?code=one-time&state=${stateOf(q.getValue("state"))}") }
        }.start()
    }

    @Test
    fun signsInThroughTheBrowserAndKeepsTheTokenInTheSecureStore() {
        val base = serve()
        val store = MemoryStore()
        val visited = CopyOnWriteArrayList<String>()
        val auth = LakesideSignIn(browser(visited), store, base)
        val result = auth.signIn("discord")
        assertEquals(SignInResult.Done("Ada"), result)
        val q = query(visited.single())
        assertEquals("discord", q["provider"])
        assertTrue(q.getValue("port").toInt() in 47832..47841)
        assertTrue(q.getValue("state").matches(Regex("[A-Za-z0-9_-]{16,128}")))
        assertEquals("""{"code":"one-time"}""", posts.single())
        assertEquals("opaque-token", auth.token())
        assertEquals("Ada", auth.name)
        assertTrue(store.secret!!.contains("opaque-token"), "kept in the secure store")
        auth.signOut()
        assertNull(auth.token())
        assertNull(auth.name)
        assertTrue(store.deleted)
    }

    @Test
    fun aSavedSignInIsRestoredAndAnExpiredOneIsDropped() {
        val store = MemoryStore()
        store.secret = """{"token":"kept","name":"Ada","savedAt":1000}"""
        val fresh = LakesideSignIn({ }, store, "http://127.0.0.1:1", clock = { 1000 + 1000 })
        fresh.restore()
        assertEquals("kept", fresh.token())
        assertEquals("Ada", fresh.name)
        val expired = LakesideSignIn({ }, store, "http://127.0.0.1:1", clock = { 1000 + LakesideSignIn.LIFETIME_MS + 1 })
        expired.restore()
        assertNull(expired.token())
        assertNull(store.secret, "an expired sign-in is deleted")
    }

    @Test
    fun theTokenStaysInMemoryWhenThereIsNoSecureStore() {
        val base = serve()
        val auth = LakesideSignIn(browser(CopyOnWriteArrayList()), NoTokenStore, base)
        assertEquals(SignInResult.Done("Ada"), auth.signIn())
        assertEquals("opaque-token", auth.token())
    }

    @Test
    fun aRefusedCodeIsAPlainFailureWithNoRetry() {
        val base = serve(status = 429)
        val auth = LakesideSignIn(browser(CopyOnWriteArrayList()), MemoryStore(), base)
        val result = auth.signIn()
        assertTrue(result is SignInResult.Failed && result.reason.contains("Wait a few minutes"), "result was $result")
        assertEquals(1, posts.size, "a 429 is not retried")
        assertNull(auth.token())
    }

    @Test
    fun cancellingStopsTheWait() {
        val base = serve()
        val auth = LakesideSignIn({ Thread { Thread.sleep(300); auth0?.cancelSignIn() }.start() }, MemoryStore(), base).also { auth0 = it }
        assertEquals(SignInResult.Cancelled, auth.signIn())
        assertNotNull(auth)
    }

    private var auth0: LakesideSignIn? = null
}
