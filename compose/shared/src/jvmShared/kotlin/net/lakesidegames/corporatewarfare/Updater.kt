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
import kotlinx.serialization.json.Json
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/** What each platform does with a new release: download and run its installer, or open a page. */
interface UpdatePlatform {
    /** The release file this platform installs itself, for example ".msi"; null opens the page. */
    val installerSuffix: String?

    /** Run a downloaded installer and close the game; the installer reopens it when done. */
    fun install(file: File)

    fun open(url: String)
}

/** The newest published release, from the public GitHub releases feed. */
data class Release(val version: String, val notes: String, val page: String, val installer: Asset?)

data class Asset(val name: String, val url: String, val bytes: Long)

@Serializable
private data class GitHubRelease(
    @SerialName("tag_name") val tag: String,
    val body: String? = null,
    @SerialName("html_url") val page: String,
    val draft: Boolean = false,
    val prerelease: Boolean = false,
    val assets: List<GitHubAsset> = emptyList(),
)

@Serializable
private data class GitHubAsset(val name: String, @SerialName("browser_download_url") val url: String, val size: Long = 0)

sealed interface UpdateStatus {
    data object Idle : UpdateStatus
    data object Checking : UpdateStatus
    data object Current : UpdateStatus
    data class Available(val release: Release) : UpdateStatus
    data class Downloading(val release: Release, val fraction: Float) : UpdateStatus
    data class Failed(val reason: String) : UpdateStatus
}

/**
 * Checks for a newer Corporate Warfare release and installs it where the platform allows. The
 * only network call is a read of the public releases feed and, when the player chooses to
 * update, the installer download.
 */
class Updater(private val platform: UpdatePlatform, private val scope: CoroutineScope, private val cacheDir: File) {
    var status by mutableStateOf<UpdateStatus>(UpdateStatus.Idle); private set

    val current: String = runCatching { uniffi.cw_ffi.engineInfo().version }.getOrDefault("0.0.0")

    fun check() {
        if (status is UpdateStatus.Checking || status is UpdateStatus.Downloading) return
        status = UpdateStatus.Checking
        scope.launch {
            status = withContext(Dispatchers.IO) {
                runCatching { latest() }.fold(
                    onSuccess = { release ->
                        if (release != null && isNewer(release.version, current)) UpdateStatus.Available(release) else UpdateStatus.Current
                    },
                    onFailure = { UpdateStatus.Failed("Could not reach the update server.") },
                )
            }
        }
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
                platform.install(file)
            }
        }
    }

    fun openPage(release: Release) = platform.open(release.page)

    private fun latest(): Release? {
        val connection = (URL(FEED).openConnection() as HttpURLConnection).apply {
            connectTimeout = 6_000
            readTimeout = 6_000
            setRequestProperty("Accept", "application/vnd.github+json")
            setRequestProperty("User-Agent", "CorporateWarfare/$current")
        }
        try {
            if (connection.responseCode != 200) return null
            val release = json.decodeFromString(GitHubRelease.serializer(), connection.inputStream.bufferedReader().readText())
            if (release.draft || release.prerelease) return null
            val suffix = platform.installerSuffix
            val asset = suffix?.let { s -> release.assets.firstOrNull { it.name.endsWith(s, ignoreCase = true) } }
            return Release(
                version = release.tag.removePrefix("v"),
                notes = release.body.orEmpty(),
                page = release.page,
                installer = asset?.let { Asset(it.name, it.url, it.size) },
            )
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
        const val FEED = "https://api.github.com/repos/lakesidegamedev/corporate-warfare/releases/latest"
        private val json = Json { ignoreUnknownKeys = true }

        /** True when [candidate] is a later MAJOR.MINOR.PATCH than [installed]. */
        fun isNewer(candidate: String, installed: String): Boolean {
            fun parts(v: String) = v.split('.', '-').take(3).map { it.toIntOrNull() ?: 0 } + List(3) { 0 }
            val a = parts(candidate)
            val b = parts(installed)
            for (i in 0 until 3) if (a[i] != b[i]) return a[i] > b[i]
            return false
        }
    }
}
