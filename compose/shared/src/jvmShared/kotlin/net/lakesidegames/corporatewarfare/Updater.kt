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

/** What each platform does with a new release: download and run its installer, or open a page. */
interface UpdatePlatform {
    /** The release file this platform installs itself, for example ".msi"; null opens the page. */
    val installerSuffix: String?

    /** How this platform goes back to an older version. */
    val downgrade: Downgrade get() = Downgrade.Direct

    /** Run a downloaded installer for [version] and close the game; the installer reopens it when done. */
    fun install(file: File, version: String)

    fun open(url: String)
}

/** A published release, from the public GitHub releases feed. [date] is the publish day, YYYY-MM-DD. */
data class Release(
    val version: String,
    val notes: String,
    val page: String,
    val installer: Asset?,
    val date: String = "",
    val prerelease: Boolean = false,
)

data class Asset(val name: String, val url: String, val bytes: Long)

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
private data class GitHubAsset(val name: String, @SerialName("browser_download_url") val url: String, val size: Long = 0)

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
) {
    var status by mutableStateOf<UpdateStatus>(UpdateStatus.Idle); private set
    var versions by mutableStateOf<VersionList>(VersionList.Idle); private set

    val downgrade: Downgrade get() = platform.downgrade

    fun check() {
        if (status is UpdateStatus.Checking || status is UpdateStatus.Downloading) return
        status = UpdateStatus.Checking
        scope.launch {
            val releases = withContext(Dispatchers.IO) { runCatching { fetch() }.getOrNull() }
            if (releases == null) {
                status = UpdateStatus.Failed("Could not reach the update server.")
            } else {
                show(releases)
                status = Versions.offer(releases, channel(), current)?.let { UpdateStatus.Available(it) } ?: UpdateStatus.Current
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
                confirm.release.installer == null || platform.installerSuffix == null -> {
                    platform.open(confirm.release.page)
                    status = if (backup != null) UpdateStatus.BackedUp(confirm.release, backup) else UpdateStatus.Idle
                }
                else -> update(confirm.release)
            }
        }
    }

    fun cancel() {
        status = UpdateStatus.Idle
    }

    /** Download and run the installer, or open the release page where the platform cannot. */
    fun update(release: Release) {
        val installer = release.installer
        if (installer == null || platform.installerSuffix == null) {
            platform.open(release.page)
            return
        }
        status = UpdateStatus.Downloading(release, 0f)
        scope.launch {
            val file = withContext(Dispatchers.IO) {
                runCatching { download(installer) { status = UpdateStatus.Downloading(release, it) } }.getOrNull()
            }
            if (file == null) {
                status = UpdateStatus.Failed("The download did not finish. Try again, or get it from the release page.")
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
            return parse(text, platform.installerSuffix)
        } finally {
            connection.disconnect()
        }
    }

    private fun download(asset: Asset, progress: (Float) -> Unit): File {
        cacheDir.mkdirs()
        val target = File(cacheDir, asset.name)
        val partial = File(cacheDir, asset.name + ".part")
        val connection = (URL(asset.url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 10_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "CorporateWarfare/$current")
        }
        try {
            val total = connection.contentLengthLong.takeIf { it > 0 } ?: asset.bytes
            connection.inputStream.use { input ->
                partial.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    var done = 0L
                    while (true) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        done += read
                        if (total > 0) progress((done.toFloat() / total).coerceIn(0f, 1f))
                    }
                }
            }
            if (asset.bytes > 0 && partial.length() != asset.bytes) error("short download")
            target.delete()
            partial.renameTo(target)
            return target
        } finally {
            connection.disconnect()
        }
    }

    companion object {
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

        /** The releases feed as releases, drafts left out, each with this platform's installer if it has one. */
        fun parse(text: String, installerSuffix: String?): List<Release> =
            json.decodeFromString(ListSerializer(GitHubRelease.serializer()), text)
                .filter { !it.draft }
                .map { release ->
                    val asset = installerSuffix?.let { s -> release.assets.firstOrNull { it.name.endsWith(s, ignoreCase = true) } }
                    Release(
                        version = release.tag.removePrefix("v"),
                        notes = release.body.orEmpty(),
                        page = release.page,
                        installer = asset?.let { Asset(it.name, it.url, it.size) },
                        date = release.published.orEmpty().take(10),
                        prerelease = release.prerelease,
                    )
                }

        /** True when [candidate] is later than [installed]: MAJOR.MINOR.PATCH, then a release beats
         * its previews and preview.3 beats preview.2. */
        fun isNewer(candidate: String, installed: String): Boolean = compare(candidate, installed) > 0

        fun compare(a: String, b: String): Int = Versions.compare(a, b)
    }
}
