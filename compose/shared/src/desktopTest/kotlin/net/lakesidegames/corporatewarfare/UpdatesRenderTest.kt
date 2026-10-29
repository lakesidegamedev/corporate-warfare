package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

/** The Updates settings section and its version list. Set CW_MAP_RENDER_DIR to keep the pictures. */
class UpdatesRenderTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private class Platform(override val downgrade: Downgrade, override val installerSuffix: String?) : UpdatePlatform {
        val opened = mutableListOf<String>()
        override fun install(file: File, version: String) = Unit
        override fun open(url: String) { opened += url }
    }

    private val feed = listOf(
        "1.9.0-preview.1" to "2026-11-20", "1.8.1" to "2026-11-12", "1.8.0" to "2026-11-05",
        "1.8.0-preview.4" to "2026-10-30", "1.8.0-preview.3" to "2026-10-29", "1.8.0-preview.2" to "2026-10-28", "1.7.1" to "2026-10-28",
    ).map { (v, d) ->
        Release(v, "Tester preview.\n\n### Added\n\n- What changed in $v, written for players, about one line\n  long.", "https://example.invalid/$v", null, d, '-' in v)
    }

    private fun waitFor(updater: Updater) {
        val deadline = System.currentTimeMillis() + 10_000
        while ((updater.status is UpdateStatus.Checking || updater.versions is VersionList.Loading) && System.currentTimeMillis() < deadline) Thread.sleep(20)
    }

    @Test
    fun updatesSectionRenders() {
        val out = System.getenv("CW_MAP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val dir = kotlin.io.path.createTempDirectory("cwupdates").toFile()
        val scope = CoroutineScope(Dispatchers.Default)
        fun case(name: String, channel: UpdateChannel, current: String, downgrade: Downgrade = Downgrade.Direct, act: (Updater) -> Unit = {}): Updater {
            val settings = AppSettings(File(dir, "$name.properties")).apply { chooseUpdateChannel(channel) }
            val updater = Updater(Platform(downgrade, null), scope, File(dir, "updates"), File(dir, "saves"), { settings.updateChannel }, { feed }, current)
            updater.check()
            waitFor(updater)
            updater.loadVersions()
            waitFor(updater)
            act(updater)
            val scene = ImageComposeScene(720, 1100, Density(1f)) {
                CorporateWarfareTheme {
                    Box(Modifier.fillMaxSize().background(Palette.ink).padding(16.dp)) {
                        Column(Modifier.width(688.dp)) { UpdatesSection(settings, updater, showVersions = true) }
                    }
                }
            }
            scene.render(0L)
            val image = scene.render(1_000_000_000L)
            scene.close()
            out?.let { File(it, "updates-$name.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
            return updater
        }

        val stable = case("full-releases", UpdateChannel.Stable, "1.8.0")
        assertEquals("1.8.1", (stable.status as UpdateStatus.Available).release.version)
        assertEquals(listOf("1.8.1", "1.8.0"), (stable.versions as VersionList.Loaded).releases.map { it.version })

        val previews = case("previews", UpdateChannel.Preview, "1.8.0-preview.4")
        assertEquals("1.9.0-preview.1", (previews.status as UpdateStatus.Available).release.version)
        assertEquals(5, (previews.versions as VersionList.Loaded).releases.size)

        // On a preview newer than every full release, switching to full releases offers 1.8.1 as a step back.
        val switched = case("switched-to-full", UpdateChannel.Stable, "1.9.0-preview.1")
        assertEquals("1.8.1", (switched.status as UpdateStatus.Available).release.version)

        val confirm = case("go-back-warning", UpdateChannel.Preview, "1.8.1") { it.choose(feed.first { r -> r.version == "1.8.0-preview.3" }) }
        val warning = assertIs<UpdateStatus.ConfirmDowngrade>(confirm.status)
        assertTrue(warning.backup!!.path.contains("backups"))

        val android = case("android-go-back", UpdateChannel.Preview, "1.8.1", Downgrade.UninstallFirst) { it.choose(feed.first { r -> r.version == "1.8.0" }) }
        assertIs<UpdateStatus.NeedsUninstall>(android.status)

        // Confirming on a platform that opens the page copies the saves first, then shows where.
        File(dir, "saves").mkdirs()
        File(dir, "saves/autosave.cwsave").writeBytes(byteArrayOf(9))
        val linux = Updater(Platform(Downgrade.Direct, null), scope, File(dir, "updates"), File(dir, "saves"), { UpdateChannel.Preview }, { feed }, "1.8.1")
        linux.choose(feed.first { it.version == "1.8.0" })
        linux.confirmDowngrade(linux.status as UpdateStatus.ConfirmDowngrade)
        val deadline = System.currentTimeMillis() + 10_000
        while (linux.status !is UpdateStatus.BackedUp && System.currentTimeMillis() < deadline) Thread.sleep(20)
        val done = assertIs<UpdateStatus.BackedUp>(linux.status)
        assertTrue(File(done.backup, "autosave.cwsave").readBytes().contentEquals(byteArrayOf(9)))
        dir.deleteRecursively()
    }
}
