package net.lakesidegames.corporatewarfare

import com.sun.net.httpserver.HttpServer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.net.InetSocketAddress
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The launcher handoff against a local fake of the account service (docs/rules/LAUNCHER.md). */
class LauncherTest {
    private var server: HttpServer? = null

    @AfterTest
    fun stop() {
        server?.stop(0)
    }

    private class Fake(val url: String, val hits: AtomicInteger, val bodies: MutableList<String>)

    private fun fake(status: Int = 200, reply: String = "", delayMs: Long = 0, redirectTo: String? = null): Fake {
        val hits = AtomicInteger()
        val bodies = mutableListOf<String>()
        val s = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        s.createContext("/redeem") { ex ->
            hits.incrementAndGet()
            synchronized(bodies) { bodies += ex.requestBody.readBytes().decodeToString() }
            if (delayMs > 0) Thread.sleep(delayMs)
            if (redirectTo != null) {
                ex.responseHeaders.add("Location", redirectTo)
                ex.sendResponseHeaders(302, -1)
            } else {
                val bytes = reply.toByteArray()
                ex.responseHeaders.add("Content-Type", "application/json")
                ex.sendResponseHeaders(status, if (bytes.isEmpty()) -1 else bytes.size.toLong())
                if (bytes.isNotEmpty()) ex.responseBody.use { it.write(bytes) }
            }
            ex.close()
        }
        s.start()
        server = s
        return Fake("http://127.0.0.1:${s.address.port}/redeem", hits, bodies)
    }

    private val good = """{"accountId":"acc-123","displayName":"Ada","entitlements":["corporate-warfare"]}"""

    @Test
    fun redeemSendsTheCodeAndClientIdAndReadsTheAccount() {
        val f = fake(reply = good)
        val account = LauncherHandoff.redeem("code-abc", f.url)
        assertEquals(LakesideAccount("acc-123", "Ada"), account, "failure: ${LauncherHandoff.lastFailure}")
        val sent = Json.parseToJsonElement(f.bodies.single()).jsonObject
        assertEquals("code-abc", sent["code"]!!.jsonPrimitive.content)
        assertEquals("corporate-warfare-desktop", sent["clientId"]!!.jsonPrimitive.content)
        assertEquals(setOf("code", "clientId"), sent.keys)
    }

    @Test
    fun everyFailureIsNotSignedIn() {
        assertNull(LauncherHandoff.redeem("c", fake(status = 401, reply = """{"error":"used"}""").url))
        server?.stop(0)
        assertNull(LauncherHandoff.redeem("c", fake(status = 500).url))
        server?.stop(0)
        assertNull(LauncherHandoff.redeem("c", fake(reply = "not json").url))
        server?.stop(0)
        assertNull(LauncherHandoff.redeem("c", fake(reply = """{"displayName":"No id"}""").url))
        server?.stop(0)
        assertNull(LauncherHandoff.redeem("c", "http://127.0.0.1:1/redeem"))
    }

    @Test
    fun aSlowServerIsGivenUpOnAtTheTimeout() {
        val f = fake(reply = good, delayMs = 3_000)
        val started = System.nanoTime()
        assertNull(LauncherHandoff.redeem("c", f.url, timeoutMs = 400))
        val tookMs = (System.nanoTime() - started) / 1_000_000
        assertTrue(tookMs < 2_000, "gave up after $tookMs ms")
    }

    @Test
    fun theRealTimeoutIsFiveSeconds() {
        assertEquals(5_000, LauncherHandoff.TIMEOUT_MS)
    }

    @Test
    fun aRedirectIsNotFollowed() {
        val target = fake(reply = good)
        val targetUrl = target.url
        val redirecting = fake(redirectTo = targetUrl)
        assertNull(LauncherHandoff.redeem("c", redirecting.url))
        assertEquals(0, target.hits.get())
    }

    @Test
    fun theNameIsCleanedAndCapped() {
        val a = LauncherHandoff.parse("""{"accountId":"x","displayName":"  Ada\u0007 Lovelace  "}""")
        assertEquals("Ada Lovelace", a?.displayName)
        val long = LauncherHandoff.parse("""{"accountId":"x","displayName":"${"n".repeat(100)}"}""")
        assertEquals(40, long?.displayName?.length)
        assertEquals("your Lakeside account", LauncherHandoff.parse("""{"accountId":"x"}""")?.label)
    }

    @Test
    fun noLauncherMeansNothingHappens() {
        val calls = AtomicInteger()
        val s = LauncherSession(emptyMap()) { calls.incrementAndGet(); null }
        assertFalse(s.managed)
        s.redeemOnce()
        assertNull(s.account)
        assertEquals(0, calls.get())
        // A code without the launcher flag is ignored.
        val lone = LauncherSession(mapOf("LAKESIDE_HANDOFF_CODE" to "abc")) { calls.incrementAndGet(); null }
        lone.redeemOnce()
        assertFalse(lone.managed)
        assertEquals(0, calls.get())
    }

    @Test
    fun launcherWithoutSignInIsManagedButHasNoAccountAndNoCall() {
        val calls = AtomicInteger()
        val s = LauncherSession(mapOf("LAKESIDE_LAUNCHER" to "1", "LAKESIDE_LAUNCHER_VERSION" to "0.5.0")) { calls.incrementAndGet(); null }
        assertTrue(s.managed)
        assertEquals("0.5.0", s.launcherVersion)
        s.redeemOnce()
        assertNull(s.account)
        assertEquals(0, calls.get())
    }

    @Test
    fun theCodeIsRedeemedOnceEvenWhenItFails() {
        val seen = mutableListOf<String>()
        val s = LauncherSession(mapOf("LAKESIDE_LAUNCHER" to "1", "LAKESIDE_HANDOFF_CODE" to "one-time")) { seen += it; null }
        s.redeemOnce()
        s.redeemOnce()
        s.redeemOnce()
        assertEquals(listOf("one-time"), seen)
        assertNull(s.account)
    }

    @Test
    fun aThrowingRedeemerIsNotSignedIn() {
        val s = LauncherSession(mapOf("LAKESIDE_LAUNCHER" to "1", "LAKESIDE_HANDOFF_CODE" to "c")) { error("boom") }
        s.redeemOnce()
        assertNull(s.account)
    }

    @Test
    fun aSuccessfulHandoffKeepsOnlyTheNameAndId() {
        val f = fake(reply = good)
        val s = LauncherSession(mapOf("LAKESIDE_LAUNCHER" to "1", "LAKESIDE_HANDOFF_CODE" to "c")) { LauncherHandoff.redeem(it, f.url) }
        s.redeemOnce()
        val account = assertNotNull(s.account)
        assertEquals("Ada", account.displayName)
        assertEquals("acc-123", account.accountId)
        assertEquals(1, f.hits.get())
        // The account type has no other field, so nothing else (entitlements, the code) is kept.
        assertEquals(setOf("accountId", "displayName"), LakesideAccount::class.java.declaredFields.map { it.name }.filter { !it.startsWith("\$") }.toSet())
    }

    @Test
    fun telemetryCarriesNoAccountEvenWhenSignedIn() {
        val f = fake(reply = good)
        val s = LauncherSession(mapOf("LAKESIDE_LAUNCHER" to "1", "LAKESIDE_HANDOFF_CODE" to "secret-code")) { LauncherHandoff.redeem(it, f.url) }
        s.redeemOnce()
        assertNotNull(s.account)
        val body = Telemetry.envelope("11111111-2222-3333-4444-555555555555", "2.0.0", "stable", """{"week":5,"cash":100}""")
        val keys = Json.parseToJsonElement(body).jsonObject.keys
        assertEquals(setOf("install_id", "app_version", "build", "snapshot"), keys)
        for (secret in listOf("acc-123", "Ada", "secret-code", "account", "Lakeside")) assertFalse(body.contains(secret, ignoreCase = true), "payload mentions $secret")
    }
}
