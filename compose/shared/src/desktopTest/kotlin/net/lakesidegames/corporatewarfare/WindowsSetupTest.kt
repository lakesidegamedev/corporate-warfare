package net.lakesidegames.corporatewarfare

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The Windows setup program: which release file the updater picks, what it runs, and that the script agrees with the code. */
class WindowsSetupTest {
    private fun feed(vararg names: String): String {
        val assets = names.joinToString(",") { """{"name":"$it","browser_download_url":"https://x/$it","size":9}""" }
        return """[{"tag_name":"v1.9.0","body":"","html_url":"https://x/r","published_at":"2026-11-01T00:00:00Z","assets":[$assets]}]"""
    }

    @Test
    fun aReleaseWithBothFilesGivesTheSetupProgram() {
        val both = Updater.parse(feed("CorporateWarfare-1.9.0-windows.msi", "CorporateWarfare-1.9.0-setup.exe"), WindowsInstall.SUFFIXES)
        assertEquals("CorporateWarfare-1.9.0-setup.exe", both.single().installer?.name)
        // The asset order in the feed does not matter.
        val reversed = Updater.parse(feed("CorporateWarfare-1.9.0-setup.exe", "CorporateWarfare-1.9.0-windows.msi"), WindowsInstall.SUFFIXES)
        assertEquals("CorporateWarfare-1.9.0-setup.exe", reversed.single().installer?.name)
    }

    @Test
    fun anMsiOnlyReleaseFallsBackToTheMsi() {
        val old = Updater.parse(feed("CorporateWarfare-1.8.0-windows.msi", "CorporateWarfare-1.8.0-linux.deb"), WindowsInstall.SUFFIXES)
        assertEquals("CorporateWarfare-1.8.0-windows.msi", old.single().installer?.name)
        assertNull(Updater.parse(feed("CorporateWarfare-1.8.0-linux.deb"), WindowsInstall.SUFFIXES).single().installer)
        // A platform with no installer, or the single-suffix form, still behaves as before.
        assertNull(Updater.parse(feed("CorporateWarfare-1.9.0-setup.exe"), emptyList()).single().installer)
        assertNotNull(Updater.parse(feed("CorporateWarfare-1.9.0-setup.exe"), "-setup.exe").single().installer)
    }

    @Test
    fun otherExeFilesAreNotTakenForTheSetupProgram() {
        assertNull(Updater.parse(feed("tool.exe", "CorporateWarfare-1.9.0-portable.exe"), WindowsInstall.SUFFIXES).single().installer)
        assertTrue(WindowsInstall.isSetup("C:\\u\\CorporateWarfare-1.9.0-preview.2-SETUP.EXE"))
        assertFalse(WindowsInstall.isSetup("C:\\u\\CorporateWarfare-1.9.0-windows.msi"))
    }

    @Test
    fun theSetupPlanWaitsForTheGameThenRunsSetupAndOffersNoDowngradeDetour() {
        val app = "C:\\Users\\p\\AppData\\Local\\Programs\\Corporate Warfare\\Corporate Warfare.exe"
        val setup = "C:\\u\\CorporateWarfare-1.8.0-setup.exe"
        // Newer and older take the same route: one setup program run, no msiexec, no uninstall step.
        for ((target, installed) in listOf("1.9.0" to "1.8.0", "1.8.0" to "1.9.0", "1.9.0" to "1.9.0-preview.2")) {
            val plan = WindowsInstall.planFor(setup, target, installed, app)
            assertEquals(listOf("@echo off", "call :closeall", "if errorlevel 1 goto running"), plan.take(3))
            assertEquals("start \"\" /wait \"$setup\" ${WindowsInstall.SETUP_ARGS}", plan[3])
            assertEquals("if %errorlevel%==0 exit 0", plan[4])
            assertFalse(plan.any { "msiexec" in it || "RelatedProducts" in it })
            assertEquals(WindowsInstall.closeAll(), plan.takeLast(WindowsInstall.closeAll().size))
            // Only a failed setup reopens the version that was there.
            assertEquals(1, plan.count { it == "start \"\" \"$app\"" })
            assertTrue(plan.indexOf("start \"\" \"$app\"") > plan.indexOf("if %errorlevel%==0 exit 0"))
        }
        assertTrue(plan(null).none { it.startsWith("start \"\" \"") && "wait" !in it })
    }

    private fun plan(app: String?) = WindowsInstall.setupPlan("s-setup.exe", app)

    @Test
    fun setupArgumentsAreSilentCloseTheGameAndRelaunch() {
        val args = WindowsInstall.SETUP_ARGS.split(' ')
        assertTrue("/SILENT" in args && "/VERYSILENT" !in args) // a progress window, not nothing
        assertTrue("/CLOSEAPPLICATIONS" in args)
        assertTrue("/NORESTART" in args)
        assertTrue("/SUPPRESSMSGBOXES" in args)
        assertTrue("/RELAUNCH" in args)
    }

    @Test
    fun anMsiRouteIsUnchangedForAnMsiFile() {
        val app = "C:\\Program Files\\Corporate Warfare\\Corporate Warfare.exe"
        assertEquals(
            WindowsInstall.plan("C:\\u\\old.msi", "1.8.0-preview.3", "1.8.0", app),
            WindowsInstall.planFor("C:\\u\\old.msi", "1.8.0-preview.3", "1.8.0", app),
        )
    }

    @Test
    fun theSetupScriptAgreesWithTheCode() {
        val iss = File("packaging/windows/setup.iss").readText()
        // The mutex the game holds is the one setup and uninstall check.
        assertTrue("#define Mutex \"${WindowsInstall.MUTEX}\"" in iss)
        assertTrue("AppMutex={#Mutex}" in iss)
        // The MSI it removes is the one the Gradle config builds.
        assertTrue("{${WindowsInstall.UPGRADE_CODE.uppercase()}}" in iss)
        assertTrue("OutputBaseFilename=CorporateWarfare-{#DisplayVersion}-setup" in iss)
        // Per user by default; the updater's /RELAUNCH is understood.
        assertTrue("PrivilegesRequired=lowest" in iss)
        assertTrue("CmdLineParamExists('/RELAUNCH')" in iss)
        // Program files are cleared on a version change; the saves folder is only ever deleted on an explicit answer.
        assertTrue("Name: \"{app}\\app\"" in iss)
        assertEquals(1, Regex("DelTree\\(").findAll(iss).count())
        assertTrue("(CurUninstallStep = usPostUninstall) and DeleteSaves and (not UninstallSilent)" in iss)
        assertTrue("if UninstallSilent then Exit;" in iss)
        assertTrue("{userappdata}\\{#AppName}" in iss)
        assertFalse("{app}\\.." in iss)
    }

    @Test
    fun theSaveFolderIsNeverAnInstallFolder() {
        // The game saves under %APPDATA%\Corporate Warfare (Main.kt) and the script's uninstall question names the same one.
        val main = File("src/desktopMain/kotlin/net/lakesidegames/corporatewarfare/Main.kt").readText()
        assertTrue("\"Corporate Warfare\"" in main && "APPDATA" in main)
        val iss = File("packaging/windows/setup.iss").readText()
        assertTrue("ExpandConstant('{userappdata}\\{#AppName}')" in iss && "#define AppName \"Corporate Warfare\"" in iss)
    }
}
