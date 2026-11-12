package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.sp
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** The Lakeside account behind a launcher handoff. Kept in memory for the session only. */
data class LakesideAccount(val accountId: String, val displayName: String) {
    /** What the player is called in the account row. */
    val label: String get() = displayName.ifBlank { "your Lakeside account" }
}

/**
 * Starting the game from the Lakeside launcher (docs/rules/LAUNCHER.md). The launcher sets
 * LAKESIDE_LAUNCHER=1 and LAKESIDE_LAUNCHER_VERSION; it adds LAKESIDE_HANDOFF_CODE only when the
 * player is signed in. The game never checks ownership and never needs this to play.
 */
class LauncherSession(
    env: Map<String, String> = System.getenv(),
    private val redeemer: (String) -> LakesideAccount? = { LauncherHandoff.redeem(it) },
) {
    /** True when the launcher started this copy, so the launcher owns its updates. */
    val managed: Boolean = env["LAKESIDE_LAUNCHER"] == "1"

    /** The launcher's version, for the player to read in Settings; empty when it did not say. */
    val launcherVersion: String = if (managed) env["LAKESIDE_LAUNCHER_VERSION"].orEmpty().trim().take(32) else ""

    private var code: String? = if (managed) env["LAKESIDE_HANDOFF_CODE"]?.trim()?.takeIf { it.isNotEmpty() } else null

    /** Set once a handoff succeeds; null otherwise. Display name and account id only, never written to disk. */
    var account by mutableStateOf<LakesideAccount?>(null); private set

    /**
     * Redeem the handoff code, once. Blocking, so call it off the main thread. The code is forgotten
     * as soon as it is taken, so a second call never sends it again, whatever the first one got.
     */
    fun redeemOnce() {
        val taken = code ?: return
        code = null
        account = runCatching { redeemer(taken) }.getOrNull()
    }

    companion object {
        /** The session of this process, read from the real environment. */
        val current: LauncherSession by lazy { LauncherSession() }
    }
}

/** The redeem call to the account service. Any failure, in any form, is "not signed in". */
object LauncherHandoff {
    const val CLIENT_ID = "corporate-warfare-desktop"
    const val REDEEM_URL = "https://lakesidegames.net/account/launcher/handoff/redeem"
    const val TIMEOUT_MS = 5_000
    private const val MAX_BODY = 16 * 1024
    private val json = Json { ignoreUnknownKeys = true }

    /** Why the last call failed, as an exception name only (never the code or the reply); for tests. */
    @Volatile
    internal var lastFailure: String? = null

    @Serializable
    private data class Request(val code: String, val clientId: String)

    @Serializable
    private data class Reply(val accountId: String? = null, val displayName: String? = null)

    /**
     * One POST with a hard 5 second limit for the whole call, no redirects and no retry. Returns
     * null on a refused, expired, used, slow or malformed answer. Nothing is logged.
     */
    fun redeem(code: String, url: String = REDEEM_URL, timeoutMs: Int = TIMEOUT_MS): LakesideAccount? {
        val executor = Executors.newSingleThreadExecutor { r -> Thread(r, "lakeside-handoff").apply { isDaemon = true } }
        var connection: HttpURLConnection? = null
        try {
            val body = json.encodeToString(Request.serializer(), Request(code, CLIENT_ID)).toByteArray()
            val call = executor.submit(
                Callable {
                    val c = (URL(url).openConnection() as HttpURLConnection).also { connection = it }
                    c.requestMethod = "POST"
                    c.connectTimeout = timeoutMs
                    c.readTimeout = timeoutMs
                    c.instanceFollowRedirects = false
                    c.doOutput = true
                    c.setRequestProperty("Content-Type", "application/json")
                    c.setRequestProperty("Accept", "application/json")
                    c.outputStream.use { it.write(body) }
                    if (c.responseCode != 200) return@Callable null
                    val bytes = c.inputStream.use { it.readNBytes(MAX_BODY + 1) }
                    if (bytes.size > MAX_BODY) null else parse(String(bytes, Charsets.UTF_8))
                },
            )
            return try {
                call.get(timeoutMs.toLong(), TimeUnit.MILLISECONDS)
            } catch (e: Exception) {
                lastFailure = e.toString().take(200)
                call.cancel(true)
                connection?.disconnect()
                null
            }
        } catch (e: Exception) {
            lastFailure = e.toString().take(200)
            return null
        } finally {
            connection?.disconnect()
            executor.shutdownNow()
        }
    }

    /** The reply as an account, or null when it has no account id. The name is trimmed and capped. */
    internal fun parse(text: String): LakesideAccount? {
        val reply = runCatching { json.decodeFromString(Reply.serializer(), text) }.getOrNull() ?: return null
        val id = reply.accountId?.trim()?.takeIf { it.isNotEmpty() && it.length <= 128 } ?: return null
        val name = reply.displayName.orEmpty().filter { !it.isISOControl() }.trim().take(40)
        return LakesideAccount(id, name)
    }
}

/**
 * The account row of the main menu and Settings. It draws only when the launcher handed this
 * session an account: with no launcher, or a failed handoff, nothing shows, because the player
 * has nothing to press.
 */
@Composable
internal fun AccountEntry(session: LauncherSession = LauncherSession.current) {
    val account = session.account ?: return
    Column(Modifier.fillMaxWidth()) {
        Text("Signed in as ${account.label}", color = Palette.primaryText, fontSize = 13.sp)
        Text("From the Lakeside launcher. Your game works the same without it.", color = Palette.secondary, fontSize = 11.sp)
    }
}

/** Settings, Updates when the launcher runs the game: plain words, no controls. */
@Composable
internal fun LauncherUpdatesSection(session: LauncherSession = LauncherSession.current) {
    Panel {
        Eyebrow("Updates")
        Text("The Lakeside launcher keeps this copy up to date.", color = Palette.primaryText)
        Text(
            "Updates and versions are chosen in the launcher. Nothing about you or your game is sent to look for them." +
                if (session.launcherVersion.isNotEmpty()) " Launcher ${session.launcherVersion}." else "",
            color = Palette.secondary, fontSize = 12.sp,
        )
    }
}
