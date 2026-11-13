package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/** What each platform does with a new release: download and run its installer, or open a page. */
interface UpdatePlatform {
    /** The release file this platform installs itself, for example ".msi"; null opens the page. */
    val installerSuffix: String?

    /**
     * Every release file ending this platform can install, best first. A release with several
     * gives the first that matches. Defaults to [installerSuffix] alone.
     */
    val installerSuffixes: List<String> get() = listOfNotNull(installerSuffix)

    /** How this platform goes back to an older version. */
    val downgrade: Downgrade get() = Downgrade.Direct

    /** Run a downloaded installer for [version] and close the game; the installer reopens it when done. */
    fun install(file: File, version: String)

    fun open(url: String)

    /**
     * The Lakeside download keys this computer can install from a signed-in download, best first
     * (for example "windows-x86_64"). Empty: this platform does not update from a sign-in.
     */
    val paidKeys: List<String> get() = emptyList()

    /**
     * Install a verified signed-in download of kind [key]. The default installs it the way a public
     * installer is installed. A platform that cannot close the game itself returns what the player
     * should read instead (where the file went, what to run); null means the game is closing.
     */
    fun installPaid(file: File, key: String, version: String): String? {
        install(file, version)
        return null
    }
}

/** A published release, from the public GitHub releases feed. [date] is the publish day, YYYY-MM-DD. */
data class Release(
    val version: String,
    val notes: String,
    val page: String,
    val installer: Asset?,
    val date: String = "",
    val prerelease: Boolean = false,
    /** False when the release has no files at all: its installers are delivered elsewhere (the Lakeside launcher or a store). */
    val hasFiles: Boolean = true,
)

/**
 * A release file. [sha256] is the lowercase hex SHA-256 GitHub publishes for it, or null when the
 * release predates published digests or lists another algorithm.
 */
data class Asset(val name: String, val url: String, val bytes: Long, val sha256: String? = null)

@Serializable
private data class GitHubRelease(
    @SerialName("tag_name") val tag: String,
    val body: String? = null,
    @SerialName("html_url") val page: String,
    val draft: Boolean = false,
    val prerelease: Boolean = false,
    @SerialName("published_at") val published: String? = null,
    val assets: List<GitHubAsset> = emptyList(),
)

@Serializable
private data class GitHubAsset(
    val name: String,
    @SerialName("browser_download_url") val url: String,
    val size: Long = 0,
    /** "sha256:<hex>" on assets GitHub has hashed; null or empty on older ones. */
    val digest: String? = null,
)

sealed interface UpdateStatus {
    data object Idle : UpdateStatus
    data object Checking : UpdateStatus
    data object Current : UpdateStatus
    data class Available(val release: Release) : UpdateStatus

    /** The player picked an older version: warn, and say where the saves will be copied. */
    data class ConfirmDowngrade(val release: Release, val backup: File?) : UpdateStatus

    /** The saves are copied; the older version's page is open for the player to install. */
    data class BackedUp(val release: Release, val backup: File) : UpdateStatus

    /** Android will not install an older version over a newer one. */
    data class NeedsUninstall(val release: Release) : UpdateStatus
    data class Downloading(val release: Release, val fraction: Float) : UpdateStatus
    data class Failed(val reason: String) : UpdateStatus

    /** This release has no installer to fetch here: it is delivered through the Lakeside launcher or a store. */
    data class NoInstaller(val release: Release) : UpdateStatus

    /** A signed-in download is needed: the player has not signed in to Lakeside. [freeAccess] is Lakeside's free access message, when one is on. */
    data class SignInNeeded(val release: Release, val freeAccess: String? = null) : UpdateStatus

    /** The browser is open and the game is waiting for the player to finish signing in. */
    data class SigningIn(val release: Release) : UpdateStatus

    /** The signed-in account does not own Corporate Warfare. */
    data class NotEntitled(val release: Release) : UpdateStatus

    /** The signed-in download could not finish; the game keeps working and the page is still there. */
    data class Unavailable(val release: Release, val reason: String) : UpdateStatus

    /** The download is verified and saved, but this platform cannot restart the game itself; [note] says what to do. */
    data class Saved(val release: Release, val note: String) : UpdateStatus
}

/** The version picker's list, loaded only when the player opens it. */
sealed interface VersionList {
    data object Idle : VersionList
    data object Loading : VersionList
    data class Loaded(val releases: List<Release>) : VersionList
    data class Failed(val reason: String) : VersionList
}

/**
 * Checks for a newer Corporate Warfare release on the player's chosen channel, lists the versions
 * they can move to, and installs one where the platform allows. The only network calls are reads
 * of the public releases feed and, when the player picks a version, its installer download.
 * [saveDir] is where the saves live, for the backup before going back to an older version.
 */
class Updater(
    private val platform: UpdatePlatform,
    private val scope: CoroutineScope,
    private val cacheDir: File,
    private val saveDir: File? = null,
    private val channel: () -> UpdateChannel = { UpdateChannel.build },
    private val feed: () -> List<Release>? = { null },
    val current: String = BuildInfo.VERSION,
    /** Signs the player in to Lakeside for downloads of a paid build; null leaves the updater public-only. */
    val auth: LakesideAuth? = null,
    /** The Lakeside account service; tests point it at a local server. */
    private val accountBase: String = LakesideSignIn.ACCOUNT_BASE,
) {
    var status by mutableStateOf<UpdateStatus>(UpdateStatus.Idle); private set
    var versions by mutableStateOf<VersionList>(VersionList.Idle); private set

    val downgrade: Downgrade get() = platform.downgrade

    /** True when this computer can update a paid build through a Lakeside sign-in. */
    val signInUpdates: Boolean get() = auth != null && platform.paidKeys.isNotEmpty()

    fun check() {
        if (status is UpdateStatus.Checking || status is UpdateStatus.Downloading) return
        status = UpdateStatus.Checking
        scope.launch {
            val releases = withContext(Dispatchers.IO) { runCatching { fetch() }.getOrNull() }
            if (releases != null) show(releases)
            var offer = releases?.let { Versions.offer(it, channel(), current) }
            // Signed in: the Lakeside download list knows the paid build's version, so it settles "is there an update".
            if (offer == null && signInUpdates) offer = withContext(Dispatchers.IO) { runCatching { paidOffer() }.getOrNull() }
            status = when {
                offer != null -> UpdateStatus.Available(offer)
                releases == null -> UpdateStatus.Failed("Could not reach the update server.")
                else -> UpdateStatus.Current
            }
        }
    }

    /** Load the versions the player can pick from, on their channel. */
    fun loadVersions() {
        if (versions is VersionList.Loading) return
        versions = VersionList.Loading
        scope.launch {
            val releases = withContext(Dispatchers.IO) { runCatching { fetch() }.getOrNull() }
            if (releases == null) versions = VersionList.Failed("Could not reach the update server.") else show(releases)
        }
    }

    /** Show a fetched release list in the picker, filtered to the channel and the minimum version. */
    internal fun show(releases: List<Release>) {
        all = releases
        versions = VersionList.Loaded(Versions.listed(releases, channel()))
    }

    /** After the player changes channel: re-filter the list and check again. */
    fun channelChanged() {
        all?.let { versions = VersionList.Loaded(Versions.listed(it, channel())) }
        if (status !is UpdateStatus.Downloading) {
            status = UpdateStatus.Idle
            check()
        }
    }

    private var all: List<Release>? = null

    /**
     * The player picked a version. Newer installs straight away; older first warns and backs up
     * the saves, or on Android explains that it needs an uninstall.
     */
    fun choose(release: Release) {
        if (!Versions.isDowngrade(release.version, current)) {
            update(release)
            return
        }
        status = when (platform.downgrade) {
            Downgrade.UninstallFirst -> UpdateStatus.NeedsUninstall(release)
            Downgrade.Direct -> UpdateStatus.ConfirmDowngrade(release, saveDir?.let { SaveBackup.target(it, current) })
        }
    }

    /** The player read the warning: copy the saves, then install the older version. */
    fun confirmDowngrade(confirm: UpdateStatus.ConfirmDowngrade) {
        val backup = confirm.backup
        scope.launch {
            val copied = if (backup != null && saveDir != null) {
                withContext(Dispatchers.IO) { runCatching { SaveBackup.copy(saveDir, backup) }.isSuccess }
            } else {
                true
            }
            when {
                !copied -> status = UpdateStatus.Failed("Could not copy your saves, so nothing was installed.")
                !confirm.release.hasFiles || (platform.installerSuffix != null && confirm.release.installer == null) ->
                    status = UpdateStatus.NoInstaller(confirm.release)
                confirm.release.installer == null || platform.installerSuffix == null -> {
                    platform.open(confirm.release.page)
                    status = if (backup != null) UpdateStatus.BackedUp(confirm.release, backup) else UpdateStatus.Idle
                }
                else -> update(confirm.release)
            }
        }
    }

    fun cancel() {
        if (status is UpdateStatus.SigningIn) auth?.cancelSignIn()
        status = UpdateStatus.Idle
    }

    /** Sign out of Lakeside on this computer. */
    fun signOut() {
        auth?.signOut()
    }

    /** Open the Corporate Warfare page in the Lakeside store, for buying or downloading by hand. */
    fun openStore() = platform.open(STORE_URL)

    /** Download and run the installer, or open the release page where the platform cannot. */
    fun update(release: Release) {
        val installer = release.installer
        if (!release.hasFiles || (platform.installerSuffix != null && installer == null)) {
            if (signInUpdates) startPaid(release) else status = UpdateStatus.NoInstaller(release)
            return
        }
        if (installer == null || platform.installerSuffix == null) {
            platform.open(release.page)
            return
        }
        status = UpdateStatus.Downloading(release, 0f)
        scope.launch {
            val file = withContext(Dispatchers.IO) {
                try {
                    download(installer) { status = UpdateStatus.Downloading(release, it) }
                } catch (e: ChecksumMismatch) {
                    MISMATCH
                } catch (e: Exception) {
                    null
                }
            }
            if (file == null) {
                status = UpdateStatus.Failed("The download did not finish. Try again, or get it from the release page.")
            } else if (file === MISMATCH) {
                status = UpdateStatus.Failed("The download did not match the checksum published for it, so it was thrown away and nothing was installed.")
            } else {
                platform.install(file, release.version)
            }
        }
    }

    fun openPage(release: Release) = platform.open(release.page)

    /** Every published release, previews included; the channel filter applies afterwards. */
    private fun fetch(): List<Release>? {
        feed()?.let { return it }
        val connection = (URL(LIST).openConnection() as HttpURLConnection).apply {
            connectTimeout = 6_000
            readTimeout = 6_000
            setRequestProperty("Accept", "application/vnd.github+json")
            setRequestProperty("User-Agent", "CorporateWarfare/$current")
        }
        try {
            if (connection.responseCode != 200) return null
            val text = connection.inputStream.bufferedReader().readText()
            return parse(text, platform.installerSuffixes)
        } finally {
            connection.disconnect()
        }
    }

    // ---- Signed-in downloads of a paid build (installers are not public) ----

    /** Pick a sign-in provider and start the browser flow for [release]; on success the download follows. */
    fun signIn(release: Release, provider: String = LakesideAuth.PROVIDER_AHD) {
        val a = auth ?: return
        status = UpdateStatus.SigningIn(release)
        scope.launch {
            withContext(Dispatchers.IO) {
                val result = a.signIn(provider)
                if (status !is UpdateStatus.SigningIn) return@withContext
                when (result) {
                    is SignInResult.Done -> a.token()?.let { paidDownload(release, it) } ?: run { status = UpdateStatus.SignInNeeded(release) }
                    SignInResult.Cancelled -> status = UpdateStatus.SignInNeeded(release)
                    is SignInResult.Failed -> status = UpdateStatus.Unavailable(release, result.reason)
                }
            }
        }
    }

    /** Download the paid build: straight away when signed in, otherwise ask the player to sign in first. */
    private fun startPaid(release: Release) {
        val a = auth ?: return
        status = UpdateStatus.Downloading(release, 0f)
        scope.launch {
            withContext(Dispatchers.IO) {
                val token = a.token()
                if (token == null) status = UpdateStatus.SignInNeeded(release, runCatching { freeAccessMessage() }.getOrNull())
                else paidDownload(release, token)
            }
        }
    }

    private class HttpStatus(val code: Int) : Exception("HTTP $code")

    @Serializable
    private data class PaidFile(val platform: String = "", val name: String = "", val size: Long = 0, val sha256: String? = null, val url: String = "")

    @Serializable
    private data class PaidListing(val version: String? = null, val entitled: Boolean = false, val files: List<PaidFile> = emptyList())

    @Serializable
    private data class FreeAccess(val on: Boolean = false, val message: String? = null)

    private fun get(url: String, token: String?): HttpURLConnection =
        (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 10_000
            readTimeout = 30_000
            instanceFollowRedirects = false
            setRequestProperty("Accept", "application/json")
            setRequestProperty("User-Agent", "CorporateWarfare/$current")
            if (token != null) setRequestProperty("Authorization", "Bearer $token")
        }

    private fun listing(token: String): PaidListing {
        val c = get(accountBase + LISTING_PATH, token)
        try {
            val code = c.responseCode
            if (code != 200) throw HttpStatus(code)
            val text = String(c.inputStream.use { it.readNBytes(1024 * 1024) }, Charsets.UTF_8)
            return json.decodeFromString(PaidListing.serializer(), text)
        } finally {
            c.disconnect()
        }
    }

    /** Lakeside's free access message when free access is on, for the sign-in prompt; null otherwise. */
    private fun freeAccessMessage(): String? {
        val c = get(accountBase + FREE_ACCESS_PATH, null)
        c.connectTimeout = 4_000
        c.readTimeout = 4_000
        try {
            if (c.responseCode != 200) return null
            val reply = json.decodeFromString(FreeAccess.serializer(), String(c.inputStream.use { it.readNBytes(16 * 1024) }, Charsets.UTF_8))
            return reply.message?.filter { !it.isISOControl() }?.trim()?.take(200)?.takeIf { reply.on && it.isNotEmpty() }
        } finally {
            c.disconnect()
        }
    }

    /** A signed-in check for a newer paid build: the release to offer, or null. A 401 drops the stale sign-in. */
    private fun paidOffer(): Release? {
        val a = auth ?: return null
        val token = a.token() ?: return null
        val list = try {
            listing(token)
        } catch (e: HttpStatus) {
            if (e.code == 401) a.signOut()
            return null
        }
        val version = list.version?.takeIf { list.entitled && Versions.compare(it, current) > 0 } ?: return null
        return Release(version, "", STORE_URL, null, hasFiles = false)
    }

    private fun paidDownload(release: Release, token: String) {
        val a = auth ?: return
        val list = try {
            listing(token)
        } catch (e: HttpStatus) {
            status = if (e.code == 401) {
                a.signOut()
                UpdateStatus.SignInNeeded(release)
            } else {
                UpdateStatus.Unavailable(release, "Lakeside could not list the download (error ${e.code}). Try again later, or use Get it.")
            }
            return
        } catch (e: Exception) {
            status = UpdateStatus.Unavailable(release, OFFLINE)
            return
        }
        if (!list.entitled) {
            status = UpdateStatus.NotEntitled(release)
            return
        }
        val (key, file) = platform.paidKeys.firstNotNullOfOrNull { k -> list.files.firstOrNull { it.platform == k }?.let { k to it } }
            ?: run {
                status = UpdateStatus.Unavailable(release, "There is no build for this computer in your Lakeside library yet.")
                return
            }
        val sha = file.sha256?.trim()?.lowercase()
        if (sha == null || !sha.matches(Regex("[0-9a-f]{64}"))) {
            status = UpdateStatus.Unavailable(release, "Lakeside did not publish a checksum for this download, so nothing was installed.")
            return
        }
        if (!file.name.matches(Regex("[A-Za-z0-9._-]{1,120}")) || !file.url.startsWith("/") || file.url.startsWith("//")) {
            status = UpdateStatus.Unavailable(release, "Lakeside sent a download this game does not trust, so nothing was installed.")
            return
        }
        val version = list.version ?: release.version
        val shown = release.copy(version = version)
        status = UpdateStatus.Downloading(shown, 0f)
        try {
            cacheDir.mkdirs()
            val saved = downloadPaid(token, Asset(file.name, accountBase + file.url, file.size, sha)) { status = UpdateStatus.Downloading(shown, it) }
            // Saves are copied aside first, as for going back a version. The installer never touches them.
            if (saveDir != null) runCatching { SaveBackup.copy(saveDir, SaveBackup.target(saveDir, current)) }
            val note = platform.installPaid(saved, key, version)
            if (note != null) status = UpdateStatus.Saved(shown, note)
        } catch (e: ChecksumMismatch) {
            status = UpdateStatus.Failed("The download did not match the checksum Lakeside published for it, so it was thrown away and nothing was installed.")
        } catch (e: HttpStatus) {
            status = when (e.code) {
                401 -> { a.signOut(); UpdateStatus.SignInNeeded(release) }
                403 -> UpdateStatus.NotEntitled(release)
                404 -> UpdateStatus.Unavailable(release, "There is no build for this computer in your Lakeside library yet.")
                else -> UpdateStatus.Unavailable(release, "The download failed (error ${e.code}). Try again later, or use Get it.")
            }
        } catch (e: Exception) {
            status = UpdateStatus.Unavailable(release, OFFLINE)
        }
    }

    /** Ask Lakeside for the file with the sign-in, follow its redirect to the storage link without it, and verify. */
    private fun downloadPaid(token: String, asset: Asset, progress: (Float) -> Unit): File {
        val home = URL(accountBase)
        var url = asset.url
        var bearer: String? = token
        for (@Suppress("UNUSED_VARIABLE") hop in 0..MAX_HOPS) {
            val c = get(url, bearer)
            try {
                val code = c.responseCode
                if (code in setOf(301, 302, 303, 307, 308)) {
                    val next = URL(URL(url), c.getHeaderField("Location") ?: throw java.io.IOException("redirect without a place"))
                    // Only the storage link (https) or Lakeside itself; and the sign-in never goes with it.
                    if (next.protocol != "https" && !(next.protocol == home.protocol && next.host == home.host)) throw java.io.IOException("unsafe redirect")
                    url = next.toString()
                    bearer = null
                    continue
                }
                if (code != 200) throw HttpStatus(code)
                return stream(c, asset, progress)
            } finally {
                c.disconnect()
            }
        }
        throw java.io.IOException("too many redirects")
    }

    private fun download(asset: Asset, progress: (Float) -> Unit): File {
        cacheDir.mkdirs()
        val connection = (URL(asset.url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 10_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "CorporateWarfare/$current")
        }
        try {
            return stream(connection, asset, progress)
        } finally {
            connection.disconnect()
        }
    }

    /** Copy the response to a partial file, check size and digest, and move it into place. */
    private fun stream(connection: HttpURLConnection, asset: Asset, progress: (Float) -> Unit): File {
        val target = File(cacheDir, asset.name)
        val partial = File(cacheDir, asset.name + ".part")
        val total = connection.contentLengthLong.takeIf { it > 0 } ?: asset.bytes
        val hash = MessageDigest.getInstance("SHA-256")
        connection.inputStream.use { input ->
            partial.outputStream().use { output ->
                val buffer = ByteArray(64 * 1024)
                var done = 0L
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    output.write(buffer, 0, read)
                    hash.update(buffer, 0, read)
                    done += read
                    if (total > 0) progress((done.toFloat() / total).coerceIn(0f, 1f))
                }
            }
        }
        if (asset.bytes > 0 && partial.length() != asset.bytes) {
            partial.delete()
            error("short download")
        }
        // A published digest must match. With none published (older releases) the byte count is all there is.
        if (asset.sha256 != null && !asset.sha256.equals(hash.digest().toHex(), ignoreCase = true)) {
            partial.delete()
            throw ChecksumMismatch()
        }
        target.delete()
        partial.renameTo(target)
        return target
    }

    private class ChecksumMismatch : Exception()

    companion object {
        /** Stands for "the download failed its checksum" between the IO thread and the status update. */
        private val MISMATCH = File("checksum-mismatch")

        private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it) }

        /** The SHA-256 in a GitHub "sha256:<64 hex>" digest, lowercased; null for anything else. */
        internal fun sha256Of(digest: String?): String? =
            digest?.trim()?.removePrefix("sha256:")?.takeIf { digest.trim().startsWith("sha256:") && it.matches(Regex("[0-9a-fA-F]{64}")) }?.lowercase()

        const val LIST = "https://api.github.com/repos/lakesidegamedev/corporate-warfare/releases?per_page=100"

        /** The Play Store's installer package. */
        const val PLAY_STORE = "com.android.vending"

        /**
         * Play policy: an app installed from Google Play updates through Play, never by sending the
         * player to a download page. Every other distribution, and a null installer for platforms
         * without one, keeps the self-updater.
         */
        fun selfUpdateAllowed(installerPackage: String?): Boolean = installerPackage != PLAY_STORE
        private val json = Json { ignoreUnknownKeys = true }

        /** Where owners and members get a build the in-game updater cannot fetch. */
        const val STORE_URL = "https://lakesidegames.net/store/corporate-warfare/"
        const val LISTING_PATH = "/account/api/downloads/corporate-warfare"
        const val FREE_ACCESS_PATH = "/account/api/free-access"
        private const val MAX_HOPS = 4
        private const val OFFLINE = "Could not reach Lakeside. Your game works as before; try again later, or use Get it."

        /** The releases feed as releases, drafts left out, each with this platform's installer if it has one. */
        fun parse(text: String, installerSuffix: String?): List<Release> = parse(text, listOfNotNull(installerSuffix))

        /** The first asset ending with the earliest suffix in [suffixes] that any asset matches; null if none. */
        internal fun <T> pick(assets: List<T>, suffixes: List<String>, name: (T) -> String): T? =
            suffixes.firstNotNullOfOrNull { s -> assets.firstOrNull { name(it).endsWith(s, ignoreCase = true) } }

        /** As above, for suffixes in order of preference (the setup program before the MSI). */
        fun parse(text: String, installerSuffixes: List<String>): List<Release> =
            json.decodeFromString(ListSerializer(GitHubRelease.serializer()), text)
                .filter { !it.draft }
                .map { release ->
                    val asset = pick(release.assets, installerSuffixes) { it.name }
                    Release(
                        version = release.tag.removePrefix("v"),
                        notes = release.body.orEmpty(),
                        page = release.page,
                        installer = asset?.let { Asset(it.name, it.url, it.size, sha256Of(it.digest)) },
                        date = release.published.orEmpty().take(10),
                        prerelease = release.prerelease,
                        hasFiles = release.assets.isNotEmpty(),
                    )
                }

        /** True when [candidate] is later than [installed]: MAJOR.MINOR.PATCH, then a release beats
         * its previews and preview.3 beats preview.2. */
        fun isNewer(candidate: String, installed: String): Boolean = compare(candidate, installed) > 0

        fun compare(a: String, b: String): Int = Versions.compare(a, b)
    }
}
