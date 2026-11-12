package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import java.io.File
import kotlin.test.Test

/**
 * The main menu, load, new career, settings, first run and game menu at 1920 x 1080 and on a 390 x 844 phone.
 * Pictures go to CW_MAP_RENDER_DIR as menus-<tag>-<screen>-<desktop|phone>.png, tag from CW_MENUS_TAG.
 */
class MenusRenderTest {
    private val tag = System.getenv("CW_MENUS_TAG") ?: "after"

    private fun both(name: String, settings: AppSettings, settleMs: Long = 0L, content: @androidx.compose.runtime.Composable () -> Unit) {
        ProposalCareer.shoot("menus-$tag-$name-desktop", 1920, 1080, settings, settleMs = settleMs, content = content)
        ProposalCareer.shoot("menus-$tag-$name-phone", 390, 844, settings, density = 2f, settleMs = settleMs, content = content)
    }

    private fun stubUpdater(dir: File): Updater {
        val platform = object : UpdatePlatform {
            override val installerSuffix: String? = null
            override fun install(file: File, version: String) = Unit
            override fun open(url: String) = Unit
        }
        return Updater(platform, CoroutineScope(Dispatchers.Default), File(dir, "updates"), File(dir, "saves"), { UpdateChannel.Stable }, { emptyList() }, BuildInfo.VERSION)
    }

    @Test
    fun menusRender() {
        val (career, settings) = ProposalCareer.open()
        // Two saves of the week 156 career, so Continue and the load list have something in them.
        career.saveNow()
        Thread.sleep(1500)
        career.saveNow()
        Thread.sleep(1500)
        val fresh = GameStore(kotlin.io.path.createTempDirectory("cwmenus").toFile(), CoroutineScope(Dispatchers.Default))
        val dir = kotlin.io.path.createTempDirectory("cwmenusu").toFile()
        val updater = stubUpdater(dir)

        both("main-first", settings) { StartScreen(fresh, updater, onSettings = {}, onQuit = {}) }
        both("main", settings) { StartScreen(career, updater, onSettings = {}, onQuit = {}) }
        both("load", settings) { LoadGameScreen(career) {} }
        val news = WhatsNew.topChanges(BuildChangelog.text, BuildInfo.VERSION)
        both("whats-new", settings) { WhatsNewPanel(BuildInfo.VERSION, news, onClose = {}, onFull = {}) }
        both("changelog", settings) { ChangelogScreen(BuildChangelog.text) {} }
        both("scenarios", settings) { ScenariosScreen(fresh, {}, {}, {}, { _, _ -> }) }
        both("new-1-scenario", settings) { NewGameScreen(fresh, startStep = 0, onBack = {}) }
        both("new-2-start", settings, settleMs = 2500) { NewGameScreen(fresh, startStep = 1, startHelm = true, onBack = {}) }
        both("new-3-ambition", settings) { NewGameScreen(fresh, startStep = 2, startAmbition = "richest", onBack = {}) }
        both("new-4-world", settings) { NewGameScreen(fresh, startStep = 3, onBack = {}) }
        both("new-5-chief", settings) { NewGameScreen(fresh, startStep = 4, onBack = {}) }
        both("new-5-company", settings) { NewGameScreen(fresh, startStep = 4, startFounder = true, onBack = {}) }
        for (section in listOf(SettingsSection.Game, SettingsSection.Graphics, SettingsSection.Keys, SettingsSection.Data, SettingsSection.Updates)) {
            both("settings-${section.name.lowercase()}", settings) { SettingsScreen(career, settings, desktop = true, updater = updater, onSetupAgain = {}, startSection = section) {} }
        }
        both("settings-search", settings) { SettingsScreen(career, settings, desktop = true, updater = updater, onSetupAgain = {}, startQuery = "map") {} }
        for (step in 0 until SetupStep.entries.size) {
            val s = AppSettings(File(dir, "setup-$step.properties"))
            both("setup-${SetupStep.entries[step].name.lowercase()}", settings, settleMs = 1500) {
                SetupWizard(s, updater, "/home/player/.local/share/corporate-warfare", firstRun = true, onDone = {}, startStep = step)
            }
        }
        career.saveAs("Lakeside run, gold attempt")
        Thread.sleep(1500)
        ProposalCareer.shoot("menus-$tag-game-menu-desktop", 1920, 1080, settings) {
            Box(Modifier.fillMaxSize()) {
                TycoonShell(career, onSettings = {}, onMenu = {})
                GameMenuDialog(career, onResume = {}, onLoad = {}, onSettings = {}, onMainMenu = {}, onQuit = {})
            }
        }
        ProposalCareer.shoot("menus-$tag-game-menu-phone", 390, 844, settings, density = 2f) {
            Box(Modifier.fillMaxSize()) {
                PhoneShell(career, PhoneNav(), onSettings = {}, onMenu = {})
                GameMenuDialog(career, onResume = {}, onLoad = {}, onSettings = {}, onMainMenu = {}, onQuit = null)
            }
        }
        // The save browser after Save as, Rename and Duplicate.
        career.saveSlots().firstOrNull { !it.info.autosave }?.let { Saves.duplicate(it) }
        both("load-after-save-as", settings) { LoadGameScreen(career) {} }
        // The richer save description reads back, and an old one without the 2.0 fields still loads.
        val slot = career.saveSlots().first { it.info.name == "Lakeside run, gold attempt" }
        kotlin.test.assertTrue(slot.info.detailed && slot.info.netWorthCents != 0L && slot.info.scenario.isNotEmpty(), "the save carries its figures")
    }
}
