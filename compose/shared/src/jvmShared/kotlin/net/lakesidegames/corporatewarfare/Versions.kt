package net.lakesidegames.corporatewarfare

import java.io.File
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter

/** Which releases the updater follows: finished versions only, or tester previews as well. */
enum class UpdateChannel(val id: String, val title: String, val note: String) {
    Stable("stable", "Full releases", "Finished versions only."),
    Preview("preview", "Previews", "Tester builds that come before each release, and every full release. They may have rough edges.");

    companion object {
        /** The channel this build was made for, which new installs start on. */
        val build: UpdateChannel get() = of(BuildInfo.CHANNEL)

        fun of(id: String?): UpdateChannel = entries.firstOrNull { it.id == id } ?: Stable
    }
}

/** How a platform takes an older version over a newer one. */
enum class Downgrade {
    /** The updater installs the older version itself, after backing up the saves. */
    Direct,

    /** The system refuses: the player must uninstall first, which deletes local saves. */
    UninstallFirst,
}

/** Version order, the channel's list and the oldest version the picker offers. */
object Versions {
    /**
     * The oldest version the version picker offers. Older builds have no picker and cannot read
     * saves from newer ones, so a player who went below this could not get back by themselves.
     */
    const val MINIMUM = "1.8.0-preview.3"

    /** MAJOR.MINOR.PATCH, then a release beats its previews and preview.10 beats preview.3. */
    fun compare(a: String, b: String): Int {
        fun parts(v: String): List<Int> {
            val core = v.substringBefore('-').split('.').map { it.toIntOrNull() ?: 0 } + List(3) { 0 }
            val pre = if ('-' in v) v.substringAfterLast('.').toIntOrNull() ?: 0 else Int.MAX_VALUE
            return core.take(3) + pre
        }
        val x = parts(a)
        val y = parts(b)
        for (i in 0 until 4) if (x[i] != y[i]) return x[i].compareTo(y[i])
        return 0
    }

    fun isPreview(version: String): Boolean = '-' in version

    fun isDowngrade(target: String, installed: String): Boolean = compare(target, installed) < 0

    /** The versions a player on [channel] may pick, newest first, none below [MINIMUM]. */
    fun listed(releases: List<Release>, channel: UpdateChannel, minimum: String = MINIMUM): List<Release> =
        releases
            .filter { channel == UpdateChannel.Preview || !it.prerelease }
            .filter { compare(it.version, minimum) >= 0 }
            .sortedWith { a, b -> compare(b.version, a.version) }

    /**
     * The version to offer at a check. Previews offer anything newer. Full releases offer the newest
     * full release when it is newer, and also when the player is on a preview and switched to full
     * releases, even if that release's number is lower than the preview.
     */
    fun offer(releases: List<Release>, channel: UpdateChannel, installed: String): Release? {
        val newest = listed(releases, channel).firstOrNull() ?: return null
        return when {
            compare(newest.version, installed) > 0 -> newest
            channel == UpdateChannel.Stable && isPreview(installed) && newest.version != installed -> newest
            else -> null
        }
    }

    /** The first change listed in release notes, on one line, for the version list. */
    fun shortNote(notes: String, max: Int = 110): String {
        val lines = notes.lines()
        val start = lines.indexOfFirst { it.trimStart().startsWith("- ") }
        val text = if (start < 0) {
            lines.firstOrNull { it.isNotBlank() && !it.startsWith("#") }.orEmpty().trim()
        } else {
            val rest = lines.drop(start + 1).takeWhile { it.isNotBlank() && !it.trimStart().startsWith("- ") && !it.startsWith("#") }
            (listOf(lines[start].trimStart().removePrefix("- ")) + rest.map { it.trim() }).joinToString(" ")
        }
        return if (text.length <= max) text else text.take(max - 3).trimEnd() + "..."
    }
}

/**
 * Before going back to an older version: copy every save, the settings and custom scenarios into
 * `backups/<date>-from-<version>` beside them, where the player can find them again.
 */
object SaveBackup {
    private val stamp = DateTimeFormatter.ofPattern("yyyy-MM-dd-HHmm")

    /** Folders beside the saves that are not the player's data. */
    private val skipped = setOf("backups", "updates")

    fun target(saveDir: File, installed: String, now: LocalDateTime = LocalDateTime.now()): File =
        File(File(saveDir, "backups"), "${now.format(stamp)}-from-$installed")

    /** Copy the saves into [into]; returns the number of files copied. */
    fun copy(saveDir: File, into: File): Int {
        into.mkdirs()
        var count = 0
        saveDir.listFiles()?.filter { it.name !in skipped && !it.name.endsWith(".tmp") }?.forEach { source ->
            source.walkTopDown().filter { it.isFile }.forEach { file ->
                file.copyTo(File(into, file.relativeTo(saveDir).path), overwrite = true)
                count++
            }
        }
        return count
    }
}

/** The script lines Windows runs to install a downloaded MSI while the game is closed. */
object WindowsInstall {
    /** The upgrade code in shared/build.gradle.kts; every Corporate Warfare MSI carries it. */
    const val UPGRADE_CODE = "5b6f6b2e-3c1d-4b8e-9f1a-0c7d2e4a9b31"

    /** The game's process on Windows: the jpackage launcher, which hosts the JVM itself. */
    const val EXE = "Corporate Warfare.exe"

    /** Seconds to wait for every copy to close by itself, then after asking, before forcing. */
    const val WAIT_SECONDS = 60
    const val GRACE_SECONDS = 15

    /** Shown if a copy of the game would not close even when ended; nothing is installed then. */
    const val STILL_RUNNING = "Corporate Warfare is still running, so the new version was not installed. " +
        "Close the game, or end it in Task Manager, and try again."

    /**
     * Every Windows install first waits until no copy of the game runs ([closeAll]), so msiexec
     * never meets the game's files in use. An upgrade then runs the newer MSI, which replaces the
     * old install itself. A downgrade runs the older MSI into the same folder: its Upgrade table
     * finds the newer install and removes it first (jpackage MSIs carry no downgrade block; see the
     * 1.8 versions report). Should that install still fail, for any reason but the player
     * cancelling, the script removes every installed Corporate Warfare by the upgrade code and
     * installs the older MSI again, checking again that the game is closed before each step. Saves
     * live in the player's AppData folder, which neither step touches.
     */
    fun plan(msi: String, target: String, installed: String, relaunch: String?): List<String> = buildList {
        add("@echo off")
        add("call :closeall")
        add("if errorlevel 1 goto running")
        if (!Versions.isDowngrade(target, installed)) {
            add("msiexec /i \"$msi\" /passive /norestart")
        } else {
            val folder = relaunch?.let { File(it.replace('\\', '/')).parent?.replace('/', '\\') }
            val install = "msiexec /i \"$msi\" /passive /norestart" + if (folder != null) " INSTALLDIR=\"$folder\"" else ""
            add(install)
            // 0 done, 3010 done but wants a restart, 1602 the player cancelled.
            add("if %errorlevel%==0 goto started")
            add("if %errorlevel%==3010 goto started")
            add("if %errorlevel%==1602 goto started")
            add("call :closeall")
            add("if errorlevel 1 goto running")
            add(
                "powershell -NoProfile -ExecutionPolicy Bypass -Command \"\$i = New-Object -ComObject WindowsInstaller.Installer; " +
                    "foreach (\$p in \$i.RelatedProducts('{${UPGRADE_CODE.uppercase()}}')) { " +
                    "Start-Process msiexec.exe -ArgumentList '/x', \$p, '/passive', '/norestart' -Wait }\"",
            )
            add("call :closeall")
            add("if errorlevel 1 goto running")
            add(install)
            add(":started")
        }
        if (relaunch != null) add("start \"\" \"$relaunch\"")
        // Plain exit closes the script's window, whichever way cmd started it.
        add("exit 0")
        add(":running")
        add(
            "powershell -NoProfile -Command \"Add-Type -AssemblyName PresentationFramework; " +
                "[System.Windows.MessageBox]::Show('$STILL_RUNNING', 'Corporate Warfare')\"",
        )
        add("exit 1")
        addAll(closeAll())
    }

    /**
     * The `:closeall` subroutine: wait up to [WAIT_SECONDS] for every copy of the game to close,
     * then ask the rest to close (taskkill without /f sends the window a close request, which
     * quits the game normally), wait [GRACE_SECONDS] more, then end any copy still running. Exits
     * 0 once none remains, 1 if one survives even that. `ping` paces the loop because `timeout`
     * refuses to run when the script has no console input.
     */
    fun closeAll(): List<String> {
        val running = "tasklist /fi \"imagename eq $EXE\" /nh 2>nul | find /i \"$EXE\" >nul"
        val pause = "ping -n 2 127.0.0.1 >nul"
        return listOf(
            ":closeall",
            "set /a cw_wait=0",
            ":closeall_wait",
            running,
            "if errorlevel 1 exit /b 0",
            "if %cw_wait% geq $WAIT_SECONDS goto closeall_ask",
            pause,
            "set /a cw_wait+=1",
            "goto closeall_wait",
            ":closeall_ask",
            "taskkill /im \"$EXE\" >nul 2>&1",
            "set /a cw_wait=0",
            ":closeall_grace",
            pause,
            running,
            "if errorlevel 1 exit /b 0",
            "set /a cw_wait+=1",
            "if %cw_wait% lss $GRACE_SECONDS goto closeall_grace",
            "taskkill /f /im \"$EXE\" >nul 2>&1",
            pause,
            pause,
            running,
            "if errorlevel 1 exit /b 0",
            "exit /b 1",
        )
    }
}
