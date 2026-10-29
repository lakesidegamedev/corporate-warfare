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

    /**
     * An upgrade runs the newer MSI, which replaces the old install itself. The MSI refuses to
     * install over a newer version, so a downgrade first removes every installed Corporate
     * Warfare (found by the upgrade code), then installs the older MSI into the same folder.
     * Saves live in the player's AppData folder, which uninstalling leaves alone.
     */
    fun plan(msi: String, target: String, installed: String, relaunch: String?): List<String> = buildList {
        val downgrade = Versions.isDowngrade(target, installed)
        add("@echo off")
        add("timeout /t 2 /nobreak >nul")
        if (downgrade) {
            add(
                "powershell -NoProfile -ExecutionPolicy Bypass -Command \"\$i = New-Object -ComObject WindowsInstaller.Installer; " +
                    "foreach (\$p in \$i.RelatedProducts('{${UPGRADE_CODE.uppercase()}}')) { " +
                    "Start-Process msiexec.exe -ArgumentList '/x', \$p, '/passive', '/norestart' -Wait }\"",
            )
        }
        val folder = relaunch?.let { File(it.replace('\\', '/')).parent?.replace('/', '\\') }
        add("msiexec /i \"$msi\" /passive /norestart" + if (downgrade && folder != null) " INSTALLDIR=\"$folder\"" else "")
        if (relaunch != null) add("start \"\" \"$relaunch\"")
    }
}
