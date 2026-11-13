package net.lakesidegames.corporatewarfare

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** The What's new panel: which lines it shows, when it is due, and that a version is never shown twice. */
class WhatsNewTest {
    private val log = """
        # Changelog

        ## [Unreleased]

        ### Fixed
        - A fix that is not shown.

        ### Added
        - **Big thing.** It does a lot. More words here.
        - Second thing: with `code` and more text. Then more.
          A continuation line that is not a change.
        - Third thing
        - Fourth thing! Really.
        - Fifth thing.
        - Sixth thing.

        ### Changed
        - Not an addition.

        ## [1.9.0] - 2026-10-31

        ### Added
        - Old thing.
    """.trimIndent()

    @Test
    fun theFirstFiveAddedLinesAreShownAsOneSentenceEach() {
        val top = WhatsNew.topChanges(log, "2.0.0")
        assertEquals(listOf("Big thing", "Second thing: with code and more text", "Third thing", "Fourth thing!", "Fifth thing"), top)
    }

    @Test
    fun aVersionWithItsOwnHeadingShowsItsOwnSection() {
        assertEquals(listOf("Old thing"), WhatsNew.topChanges(log, "1.9.0"))
        assertEquals(listOf("Old thing"), WhatsNew.topChanges(log, "1.9.0-preview.3"), "a preview shows the version it leads up to")
    }

    @Test
    fun noChangelogSectionMeansNothingToShow() {
        assertTrue(WhatsNew.topChanges("# Changelog\n", "2.0.0").isEmpty())
    }

    @Test
    fun theBundledChangelogHasSomethingToShowForThisBuild() {
        assertTrue(BuildChangelog.text.startsWith("# Changelog"), BuildChangelog.text.take(40))
        assertTrue(BuildChangelog.text.contains("## [Unreleased]") || BuildChangelog.text.contains("## [${BuildInfo.VERSION.substringBefore('-')}]"))
        assertTrue(WhatsNew.topChanges(BuildChangelog.text, BuildInfo.VERSION).isNotEmpty())
    }

    @Test
    fun aFirstInstallNeverShowsItAndAnUpdateShowsItOnce() {
        val dir = kotlin.io.path.createTempDirectory("cwnews").toFile()
        // First install: no settings file yet.
        val fresh = File(dir, "fresh.properties")
        val first = AppSettings(fresh)
        assertFalse(first.whatsNewDue(BuildInfo.VERSION), "nothing on a first install")
        first.chooseAnalytics(true)
        assertFalse(AppSettings(fresh).whatsNewDue(BuildInfo.VERSION), "and not on the second launch either")
        // An update: a file written by an older version, with a different seen version.
        val old = File(dir, "old.properties")
        old.writeText("setupDone=true\nwhatsNewSeen=1.9.0\n")
        val upgraded = AppSettings(old)
        assertTrue(upgraded.whatsNewDue("2.0.0"))
        upgraded.markWhatsNewSeen("2.0.0")
        assertFalse(AppSettings(old).whatsNewDue("2.0.0"), "never again for the same version")
        assertTrue(AppSettings(old).whatsNewDue("2.0.1"), "the next version shows it again")
        // A file from before this feature has no seen version at all.
        val legacy = File(dir, "legacy.properties")
        legacy.writeText("setupDone=true\n")
        assertTrue(AppSettings(legacy).whatsNewDue("2.0.0"))
    }
}
