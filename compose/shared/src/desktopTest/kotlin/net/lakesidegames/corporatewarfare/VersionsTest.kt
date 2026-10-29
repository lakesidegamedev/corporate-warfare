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
    fun windowsUpgradeRunsTheMsiAndDowngradeFallsBackToUninstall() {
        val app = "C:\\Program Files\\Corporate Warfare\\Corporate Warfare.exe"
        val up = WindowsInstall.plan("C:\\u\\new.msi", "1.8.0", "1.8.0-preview.4", app)
        val close = listOf("call :closeall", "if errorlevel 1 goto running")
        assertEquals(
            listOf("@echo off") + close + listOf(
                "msiexec /i \"C:\\u\\new.msi\" /passive /norestart",
                "start \"\" \"$app\"",
                "exit 0",
                ":running",
            ),
            up.take(7),
        )
        assertTrue(up[7].startsWith("powershell ") && WindowsInstall.STILL_RUNNING in up[7])
        assertEquals("exit 1", up[8])
        assertEquals(WindowsInstall.closeAll(), up.drop(9))
        assertFalse(up.any { "RelatedProducts" in it })

        val down = WindowsInstall.plan("C:\\u\\old.msi", "1.8.0-preview.3", "1.8.0-preview.4", app)
        val install = "msiexec /i \"C:\\u\\old.msi\" /passive /norestart INSTALLDIR=\"C:\\Program Files\\Corporate Warfare\""
        // The older MSI goes into the same folder first; success, restart-pending or cancel stop there.
        assertEquals(close, down.subList(1, 3))
        assertEquals(install, down[3])
        assertEquals(
            listOf("if %errorlevel%==0 goto started", "if %errorlevel%==3010 goto started", "if %errorlevel%==1602 goto started"),
            down.subList(4, 7),
        )
        // Any other failure removes the installed game by its upgrade code and installs again,
        // checking before each step that no copy of the game has been started meanwhile.
        assertEquals(close, down.subList(7, 9))
        val uninstall = down[9]
        assertTrue(uninstall.startsWith("powershell "))
        assertTrue("RelatedProducts('{5B6F6B2E-3C1D-4B8E-9F1A-0C7D2E4A9B31}')" in uninstall)
        assertTrue("'/x', \$p, '/passive'" in uninstall)
        assertEquals(close, down.subList(10, 12))
        assertEquals(install, down[12])
        assertEquals(":started", down[13])
        assertEquals("start \"\" \"$app\"", down[14])
        assertEquals("exit 0", down[15])

        // Same version: an ordinary install, never an uninstall.
        assertFalse(WindowsInstall.plan("x.msi", "1.8.0", "1.8.0", app).any { "RelatedProducts" in it })
        // No known app path (a dev run): no folder and no relaunch.
        val bare = WindowsInstall.plan("x.msi", "1.8.0", "1.8.1", null)
        assertEquals("msiexec /i \"x.msi\" /passive /norestart", bare[3])
        assertFalse(bare.any { it.startsWith("start") })
    }

    @Test
    fun windowsScriptNeverRunsAnInstallerBeforeEveryCopyIsClosed() {
        val app = "C:\\Program Files\\Corporate Warfare\\Corporate Warfare.exe"
        for (plan in listOf(
            WindowsInstall.plan("new.msi", "1.8.1", "1.8.0", app),
            WindowsInstall.plan("old.msi", "1.8.0-preview.3", "1.8.0", app),
        )) {
            // Every msiexec, direct or through the uninstall, comes straight after a passed check.
            plan.forEachIndexed { i, line ->
                if ("msiexec" in line) assertEquals(listOf("call :closeall", "if errorlevel 1 goto running"), plan.subList(i - 2, i), line)
            }
            // The main part ends before the subroutine, so nothing falls through into it.
            assertTrue(plan.indexOf("exit 0") < plan.indexOf(":closeall"))
        }

        val sub = WindowsInstall.closeAll()
        val running = "tasklist /fi \"imagename eq Corporate Warfare.exe\" /nh 2>nul | find /i \"Corporate Warfare.exe\" >nul"
        fun at(line: String, from: Int = 0) = sub.drop(from).indexOf(line).also { assertTrue(it >= 0, line) } + from
        // Wait: a check, done when none is found, give up waiting after 60 one-second rounds.
        val wait = at(":closeall_wait")
        assertEquals(running, sub[wait + 1])
        assertEquals("if errorlevel 1 exit /b 0", sub[wait + 2])
        assertEquals("if %cw_wait% geq 60 goto closeall_ask", sub[wait + 3])
        // Then ask politely (no /f), wait up to 15 more seconds, and only then force.
        val ask = at("taskkill /im \"Corporate Warfare.exe\" >nul 2>&1")
        val grace = at("if %cw_wait% lss 15 goto closeall_grace", ask)
        val force = at("taskkill /f /im \"Corporate Warfare.exe\" >nul 2>&1", grace)
        assertTrue(wait < ask && ask < grace && grace < force)
        assertTrue(sub.subList(ask, grace).contains(running))
        // After forcing, a last check; still running means exit 1 and no install.
        assertEquals(listOf(running, "if errorlevel 1 exit /b 0", "exit /b 1"), sub.takeLast(3))
        assertEquals(1, sub.count { it.startsWith("taskkill /f") })
    }

    @Test
    fun windowsInstallerTemplateClosesTheGame() {
        val wxs = File("packaging/windows/main.wxs").readText()
        assertTrue("xmlns:util=\"http://schemas.microsoft.com/wix/UtilExtension\"" in wxs)
        val close = wxs.substringAfter("<util:CloseApplication").substringBefore("/>")
        assertTrue("Target=\"${WindowsInstall.EXE}\"" in close)
        assertTrue("CloseMessage=\"yes\"" in close)
        assertTrue("TerminateProcess=" in close)
        assertTrue("RebootPrompt=\"no\"" in close)
        // The rest is jpackage's own template: upgrades and downgrades behave as before.
        assertTrue("<RemoveExistingProducts Before=\"CostInitialize\"/>" in wxs)
        assertTrue("Property=\"JP_DOWNGRADABLE_FOUND\"" in wxs)
        val gradle = File("build.gradle.kts").readText()
        assertTrue("project.file(\"packaging/windows\")" in gradle)
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
