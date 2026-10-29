package net.lakesidegames.corporatewarfare

import java.io.File
import java.time.LocalDateTime
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class VersionsTest {
    private fun release(version: String, date: String = "2026-10-28") =
        Release(version, "- Change in $version", "https://example.invalid/$version", null, date, prerelease = '-' in version)

    /** The public feed as it stood on 2026-10-28, plus the releases this branch leads to. */
    private val feed = listOf(
        "1.7.0-preview.6", "1.7.0", "1.7.1", "1.8.0-preview.1", "1.8.0-preview.2",
        "1.8.0-preview.3", "1.8.0-preview.10", "1.8.0-preview.4", "1.8.0", "1.8.1", "1.9.0-preview.1",
    ).map { release(it) }

    @Test
    fun previewsSortByNumberNotText() {
        // Read as text, "preview.10" would sort below "preview.3".
        assertTrue(Versions.compare("1.8.0-preview.3", "1.8.0-preview.10") < 0)
        assertTrue(Versions.compare("1.8.0-preview.10", "1.8.0") < 0)
        assertTrue(Versions.compare("1.8.0-preview.3", "1.8.0") < 0)
        assertTrue(Versions.compare("1.7.1", "1.8.0-preview.1") < 0)
        assertEquals(0, Versions.compare("1.8.0-preview.3", "1.8.0-preview.3"))
        val shuffled = listOf("1.8.0", "1.8.0-preview.10", "1.8.0-preview.3", "1.8.0-preview.4").shuffled(kotlin.random.Random(7))
        assertEquals(listOf("1.8.0-preview.3", "1.8.0-preview.4", "1.8.0-preview.10", "1.8.0"), shuffled.sortedWith(Versions::compare))
    }

    @Test
    fun fullReleasesHideEveryPreview() {
        val stable = Versions.listed(feed, UpdateChannel.Stable).map { it.version }
        assertEquals(listOf("1.8.1", "1.8.0"), stable)
        val previews = Versions.listed(feed, UpdateChannel.Preview).map { it.version }
        assertEquals(
            listOf("1.9.0-preview.1", "1.8.1", "1.8.0", "1.8.0-preview.10", "1.8.0-preview.4", "1.8.0-preview.3"),
            previews,
        )
    }

    @Test
    fun nothingBelowThirdPreviewOfOnePointEightIsOffered() {
        assertEquals("1.8.0-preview.3", Versions.MINIMUM)
        val all = Versions.listed(feed, UpdateChannel.Preview).map { it.version }
        assertFalse("1.8.0-preview.2" in all)
        assertFalse("1.7.1" in all)
        assertTrue(all.all { Versions.compare(it, Versions.MINIMUM) >= 0 })
        assertTrue(Versions.MINIMUM in all)
    }

    @Test
    fun downgradeIsAnyLowerVersion() {
        assertTrue(Versions.isDowngrade("1.8.0-preview.3", "1.8.0-preview.4"))
        assertTrue(Versions.isDowngrade("1.8.0", "1.9.0-preview.1"))
        assertTrue(Versions.isDowngrade("1.8.0-preview.10", "1.8.0"))
        assertFalse(Versions.isDowngrade("1.8.0", "1.8.0-preview.10"))
        assertFalse(Versions.isDowngrade("1.8.0", "1.8.0"))
    }

    @Test
    fun offersFollowTheChosenChannel() {
        // A preview player on previews gets the newest preview.
        assertEquals("1.9.0-preview.1", Versions.offer(feed, UpdateChannel.Preview, "1.8.0-preview.4")?.version)
        // A full-release player is offered the newest full release only.
        assertEquals("1.8.1", Versions.offer(feed, UpdateChannel.Stable, "1.8.0")?.version)
        assertNull(Versions.offer(feed, UpdateChannel.Stable, "1.8.1"))
        // Switching to full releases on a preview offers the newest full release, even when lower.
        assertEquals("1.8.1", Versions.offer(feed, UpdateChannel.Stable, "1.9.0-preview.1")?.version)
        // A preview player who stays on previews is not pushed back.
        assertNull(Versions.offer(feed, UpdateChannel.Preview, "1.9.0-preview.1"))
        // Before any full release at or above the minimum, a preview player switching stays put.
        val early = feed.filter { Versions.compare(it.version, "1.8.0-preview.4") <= 0 }
        assertNull(Versions.offer(early, UpdateChannel.Stable, "1.8.0-preview.4"))
    }

    @Test
    fun channelDefaultsToTheBuildsOwn() {
        val dir = kotlin.io.path.createTempDirectory("cwchannel").toFile()
        val settings = AppSettings(File(dir, "settings.properties"))
        assertEquals(UpdateChannel.of(BuildInfo.CHANNEL), settings.updateChannel)
        settings.chooseUpdateChannel(UpdateChannel.Preview)
        assertEquals(UpdateChannel.Preview, AppSettings(File(dir, "settings.properties")).updateChannel)
        settings.chooseUpdateChannel(UpdateChannel.Stable)
        assertEquals(UpdateChannel.Stable, AppSettings(File(dir, "settings.properties")).updateChannel)
        assertEquals(UpdateChannel.Stable, UpdateChannel.of("nonsense"))
        dir.deleteRecursively()
    }

    @Test
    fun backupCopiesSavesSettingsAndScenariosButNotDownloads() {
        val dir = kotlin.io.path.createTempDirectory("cwbackup").toFile()
        File(dir, "autosave.cwsave").writeBytes(byteArrayOf(1, 2, 3))
        File(dir, "autosave.json").writeText("{}")
        File(dir, "save-1.cwsave").writeBytes(ByteArray(2048) { it.toByte() })
        File(dir, "settings.properties").writeText("autoUpdate=true")
        File(dir, "custom-scenarios").mkdirs()
        File(dir, "custom-scenarios/mine.json").writeText("{\"id\":\"mine\"}")
        File(dir, "updates").mkdirs()
        File(dir, "updates/Corporate.Warfare-1.8.0.msi").writeBytes(ByteArray(10))
        File(dir, "half.tmp").writeText("x")
        val target = SaveBackup.target(dir, "1.8.0-preview.4", LocalDateTime.of(2026, 10, 29, 14, 5))
        assertEquals(File(dir, "backups/2026-10-29-1405-from-1.8.0-preview.4"), target)

        assertEquals(5, SaveBackup.copy(dir, target))
        assertTrue(File(target, "autosave.cwsave").readBytes().contentEquals(byteArrayOf(1, 2, 3)))
        assertTrue(File(target, "save-1.cwsave").readBytes().contentEquals(File(dir, "save-1.cwsave").readBytes()))
        assertEquals("{\"id\":\"mine\"}", File(target, "custom-scenarios/mine.json").readText())
        assertFalse(File(target, "updates").exists())
        assertFalse(File(target, "half.tmp").exists())
        // A second backup leaves the first alone and does not copy it into itself.
        val second = SaveBackup.target(dir, "1.8.0-preview.4", LocalDateTime.of(2026, 10, 30, 9, 0))
        assertEquals(5, SaveBackup.copy(dir, second))
        assertFalse(File(second, "backups").exists())
        dir.deleteRecursively()
    }

    @Test
    fun windowsUpgradeRunsTheMsiAndDowngradeUninstallsFirst() {
        val app = "C:\\Program Files\\Corporate Warfare\\Corporate Warfare.exe"
        val up = WindowsInstall.plan("C:\\u\\new.msi", "1.8.0", "1.8.0-preview.4", app)
        assertEquals(
            listOf(
                "@echo off",
                "timeout /t 2 /nobreak >nul",
                "msiexec /i \"C:\\u\\new.msi\" /passive /norestart",
                "start \"\" \"$app\"",
            ),
            up,
        )
        val down = WindowsInstall.plan("C:\\u\\old.msi", "1.8.0-preview.3", "1.8.0-preview.4", app)
        assertEquals(5, down.size)
        val uninstall = down[2]
        assertTrue(uninstall.startsWith("powershell "))
        assertTrue("RelatedProducts('{5B6F6B2E-3C1D-4B8E-9F1A-0C7D2E4A9B31}')" in uninstall)
        assertTrue("'/x', \$p, '/passive'" in uninstall)
        assertEquals("msiexec /i \"C:\\u\\old.msi\" /passive /norestart INSTALLDIR=\"C:\\Program Files\\Corporate Warfare\"", down[3])
        assertEquals("start \"\" \"$app\"", down[4])
        // Same version: a repair-style reinstall, never an uninstall.
        assertFalse(WindowsInstall.plan("x.msi", "1.8.0", "1.8.0", app).any { it.startsWith("powershell") })
    }

    @Test
    fun upgradeCodeMatchesThePackagingConfig() {
        val gradle = File("build.gradle.kts").readText()
        assertTrue("upgradeUuid = \"${WindowsInstall.UPGRADE_CODE}\"" in gradle)
    }

    @Test
    fun feedParsingKeepsDatePreviewFlagAndInstaller() {
        val text = """
            [{"tag_name":"v1.8.0-preview.3","body":"Tester preview.\n\n### Added\n\n- Choose previews or full releases, and\n  install any version.\n- Second","html_url":"https://x/p3",
              "prerelease":true,"published_at":"2026-10-29T10:00:00Z",
              "assets":[{"name":"CorporateWarfare-1.8.0-preview.3-windows.msi","browser_download_url":"https://x/a.msi","size":5}]},
             {"tag_name":"v1.7.1","body":"","html_url":"https://x/171","draft":true}]
        """.trimIndent()
        val parsed = Updater.parse(text, ".msi")
        assertEquals(1, parsed.size)
        val p3 = parsed.single()
        assertEquals("1.8.0-preview.3", p3.version)
        assertEquals("2026-10-29", p3.date)
        assertTrue(p3.prerelease)
        assertEquals("https://x/a.msi", p3.installer?.url)
        assertEquals("Choose previews or full releases, and install any version.", Versions.shortNote(p3.notes))
        assertNull(Updater.parse(text, null).single().installer)
    }
}
