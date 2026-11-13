package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.IOException
import java.net.HttpURLConnection
import java.net.InetAddress
import java.net.ServerSocket
import java.net.SocketException
import java.net.SocketTimeoutException
import java.net.URL
import java.net.URLDecoder
import java.net.URLEncoder
import java.security.SecureRandom

/** How a sign-in ended. The reason is plain words for the player and never contains the token or the code. */
sealed interface SignInResult {
    data class Done(val name: String) : SignInResult
    data class Failed(val reason: String) : SignInResult
    data object Cancelled : SignInResult
}

/**
 * Signing in to Lakeside so the in-game updater can download a paid build. Everything the updater
 * needs from sign-in sits behind this, so a later Keycloak flow is a one-file swap. The game never
 * needs it to play (docs/rules/LAUNCHER.md).
 */
interface LakesideAuth {
    /** Who is signed in, or null. Reads recompose the screen when it changes. */
    val name: String?

    /** Load a saved sign-in from the secure store, if any. Blocking; call it off the main thread. */
    fun restore()

    /** Open the browser and wait for the player to finish. Blocking; call it off the main thread. */
    fun signIn(provider: String = PROVIDER_AHD): SignInResult

    /** Stop a sign-in that is waiting on the browser. */
    fun cancelSignIn()

    /** The session token, or null when signed out or past its 30 days. Never log it. */
    fun token(): String?

    /** Forget the sign-in here: removes it from the secure store and from memory. */
    fun signOut()

    companion object {
        const val PROVIDER_AHD = "ahd"
        const val PROVIDER_DISCORD = "discord"
    }
}

/** Where the sign-in secret is kept between runs: the operating system's secure store, never a plain file. */
interface TokenStore {
    fun load(): String?

    /** False when this machine has no secure store that works; the sign-in then lasts this session only. */
    fun save(secret: String): Boolean
    fun delete()
}

/** No secure store: nothing is kept past the session. */
object NoTokenStore : TokenStore {
    override fun load(): String? = null
    override fun save(secret: String): Boolean = false
    override fun delete() = Unit
}

/**
 * The loopback sign-in of the Lakeside account service: the system browser signs the player in and
 * hands a one-time code back to a port on this computer, and the code is exchanged at once for a
 * session token that lasts 30 days. There is no refresh token; after 30 days the player signs in again.
 */
class LakesideSignIn(
    private val openBrowser: (String) -> Unit,
    private val store: TokenStore = NoTokenStore,
    private val base: String = ACCOUNT_BASE,
    private val ports: IntRange = PORTS,
    private val clock: () -> Long = System::currentTimeMillis,
    private val waitMs: Long = 5 * 60_000L,
) : LakesideAuth {
    @Serializable
    private data class Saved(val token: String, val name: String = "", val savedAt: Long = 0)

    @Serializable
    private data class SessionBody(val code: String)

    @Serializable
    private data class SessionInfo(val username: String? = null)

    @Serializable
    private data class SessionReply(val token: String? = null, val session: SessionInfo? = null)

    private val json = Json { ignoreUnknownKeys = true }
    private var saved: Saved? = null
    private var loaded = false

    @Volatile
    private var listening: ServerSocket? = null

    @Volatile
    private var cancelled = false

    override var name: String? by mutableStateOf(null); private set

    @Synchronized
    override fun restore() {
        if (loaded) return
        loaded = true
        val text = runCatching { store.load() }.getOrNull() ?: return
        val s = runCatching { json.decodeFromString(Saved.serializer(), text) }.getOrNull() ?: return
        if (s.token.isBlank() || clock() - s.savedAt > LIFETIME_MS) {
            runCatching { store.delete() }
            return
        }
        saved = s
        name = s.name.ifBlank { FALLBACK_NAME }
    }

    @Synchronized
    override fun token(): String? {
        restore()
        val s = saved ?: return null
        if (clock() - s.savedAt > LIFETIME_MS) {
            signOut()
            return null
        }
        return s.token
    }

    @Synchronized
    override fun signOut() {
        saved = null
        name = null
        runCatching { store.delete() }
    }

    override fun cancelSignIn() {
        cancelled = true
        runCatching { listening?.close() }
    }

    override fun signIn(provider: String): SignInResult {
        cancelled = false
        val server = bind() ?: return SignInResult.Failed("Could not open a local port for the sign-in. Close other Lakeside apps and try again.")
        listening = server
        try {
            val state = randomState()
            val query = "provider=${URLEncoder.encode(provider, "UTF-8")}&port=${server.localPort}&state=$state"
            runCatching { openBrowser("$base/account/launcher/start?$query") }
                .onFailure { return SignInResult.Failed("Could not open your browser. Open $base/store/corporate-warfare/ and sign in there, or try again.") }
            val code = when (val got = awaitCode(server, state)) {
                is Wait.Code -> got.value
                Wait.Cancelled -> return SignInResult.Cancelled
                Wait.TimedOut -> return SignInResult.Failed("The sign-in was not finished in time. Try again.")
            }
            return exchange(code)
        } finally {
            listening = null
            runCatching { server.close() }
        }
    }

    private fun bind(): ServerSocket? {
        val loopback = InetAddress.getByName("127.0.0.1")
        for (port in ports) {
            try {
                return ServerSocket(port, 4, loopback)
            } catch (e: IOException) {
                continue
            }
        }
        return null
    }

    private sealed interface Wait {
        data class Code(val value: String) : Wait
        data object Cancelled : Wait
        data object TimedOut : Wait
    }

    private fun awaitCode(server: ServerSocket, state: String): Wait {
        val end = clock() + waitMs
        while (true) {
            val left = end - clock()
            if (left <= 0) return Wait.TimedOut
            server.soTimeout = left.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
            val socket = try {
                server.accept()
            } catch (e: SocketTimeoutException) {
                return Wait.TimedOut
            } catch (e: SocketException) {
                return if (cancelled) Wait.Cancelled else Wait.TimedOut
            }
            socket.use { s ->
                s.soTimeout = 5_000
                val line = runCatching { readRequestLine(s.getInputStream()) }.getOrNull()
                val target = line?.split(' ')?.getOrNull(1).orEmpty()
                val path = target.substringBefore('?')
                if (path != "/callback") {
                    reply(s, 404, "Not found.")
                    return@use
                }
                val params = parseQuery(target.substringAfter('?', ""))
                val code = params["code"].orEmpty()
                if (params["state"] != state || code.isBlank() || code.length > 2048) {
                    reply(s, 400, "This sign-in link does not match. Go back to Corporate Warfare and try again.")
                    return@use
                }
                reply(s, 200, "You are signed in to Lakeside. You can close this window and go back to Corporate Warfare.")
                return Wait.Code(code)
            }
        }
    }

    private fun readRequestLine(input: java.io.InputStream): String {
        val out = StringBuilder()
        while (out.length < 4096) {
            val b = input.read()
            if (b < 0 || b == '\n'.code) break
            if (b != '\r'.code) out.append(b.toChar())
        }
        return out.toString()
    }

    private fun reply(socket: java.net.Socket, status: Int, text: String) {
        runCatching {
            val body = "<!doctype html><meta charset=utf-8><title>Corporate Warfare</title>" +
                "<body style=\"font-family:sans-serif;background:#111;color:#eee;text-align:center;padding-top:20vh\"><p>$text</p>"
            val bytes = body.toByteArray(Charsets.UTF_8)
            val head = "HTTP/1.1 $status ${if (status == 200) "OK" else "Error"}\r\nContent-Type: text/html; charset=utf-8\r\n" +
                "Content-Length: ${bytes.size}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n"
            socket.getOutputStream().apply { write(head.toByteArray()); write(bytes); flush() }
        }
    }

    private fun parseQuery(query: String): Map<String, String> =
        query.split('&').filter { it.contains('=') }.associate {
            URLDecoder.decode(it.substringBefore('='), "UTF-8") to URLDecoder.decode(it.substringAfter('='), "UTF-8")
        }

    /** Exchange the one-time code at once. One attempt, no retry: a code is single use, and 429 means wait. */
    private fun exchange(code: String): SignInResult {
        var connection: HttpURLConnection? = null
        return try {
            val c = (URL("$base/account/launcher/session").openConnection() as HttpURLConnection).also { connection = it }
            c.requestMethod = "POST"
            c.connectTimeout = 10_000
            c.readTimeout = 15_000
            c.instanceFollowRedirects = false
            c.doOutput = true
            c.setRequestProperty("Content-Type", "application/json")
            c.setRequestProperty("Accept", "application/json")
            c.outputStream.use { it.write(json.encodeToString(SessionBody.serializer(), SessionBody(code)).toByteArray()) }
            when (val status = c.responseCode) {
                200 -> {
                    val text = String(c.inputStream.use { it.readNBytes(MAX_BODY + 1) }, Charsets.UTF_8)
                    val reply = text.takeIf { it.length <= MAX_BODY }?.let { runCatching { json.decodeFromString(SessionReply.serializer(), it) }.getOrNull() }
                    val token = reply?.token?.trim().orEmpty()
                    if (token.isEmpty()) {
                        SignInResult.Failed("Lakeside sent back something unexpected. Try again.")
                    } else {
                        val who = reply?.session?.username.orEmpty().filter { !it.isISOControl() }.trim().take(40).ifBlank { FALLBACK_NAME }
                        keep(Saved(token, who, clock()))
                        SignInResult.Done(who)
                    }
                }
                400 -> SignInResult.Failed("That sign-in link expired. Try again.")
                401 -> SignInResult.Failed("Lakeside did not accept the sign-in. Try again.")
                429 -> SignInResult.Failed("Too many sign-in attempts. Wait a few minutes and try again.")
                502, 503 -> SignInResult.Failed("Lakeside sign-in is not available right now. Try again later.")
                else -> SignInResult.Failed("Lakeside sign-in failed (error $status). Try again later.")
            }
        } catch (e: IOException) {
            SignInResult.Failed("Could not reach Lakeside to finish signing in.")
        } finally {
            connection?.disconnect()
        }
    }

    @Synchronized
    private fun keep(s: Saved) {
        saved = s
        loaded = true
        name = s.name
        // No working secure store means the sign-in lasts this session only; that is the promise.
        runCatching { store.save(json.encodeToString(Saved.serializer(), s)) }
    }

    private fun randomState(): String {
        val alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        val random = SecureRandom()
        return buildString { repeat(32) { append(alphabet[random.nextInt(alphabet.length)]) } }
    }

    companion object {
        const val ACCOUNT_BASE = "https://lakesidegames.net"
        val PORTS = 47832..47841
        const val LIFETIME_MS = 30L * 24 * 60 * 60 * 1000
        const val FALLBACK_NAME = "your Lakeside account"
        private const val MAX_BODY = 16 * 1024
    }
}
