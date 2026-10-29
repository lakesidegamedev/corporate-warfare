package net.lakesidegames.corporatewarfare

import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** First-run setup: the preset advice, Skip, the saved state, and a picture of every step. Set CW_SETUP_RENDER_DIR to keep the pictures. */
class SetupWizardTest {
    init {
        Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
        MeshCanvas.draw = desktopMeshDraw
        MeshCanvas.shader = desktopLinearShader
        Pixel.font = desktopPixelFont()
    }

    private fun settingsFile(name: String, create: Boolean = false): File {
        val file = File(kotlin.io.path.createTempDirectory("cwsetup").toFile(), "$name.properties")
        if (create) file.writeText("autoUpdate=true\n")
        return file
    }

    private fun settings(name: String) = AppSettings(settingsFile(name))

    @Test
    fun presetFollowsTheFrameTime() {
        val hd = 1_000_000L
        assertEquals(GraphicsPreset.High, SetupAdvice.recommend(5.0, hd, false))
        assertEquals(GraphicsPreset.Medium, SetupAdvice.recommend(40.0, hd, false), "slower frames drop a step")
        assertEquals(GraphicsPreset.Low, SetupAdvice.recommend(200.0, hd, false), "very slow frames get Low")
        // The same frame time costs more on a bigger screen.
        assertEquals(GraphicsPreset.High, SetupAdvice.recommend(30.0, 400_000L, false))
        assertEquals(GraphicsPreset.Medium, SetupAdvice.recommend(30.0, 2_000_000L, false))
        // Software drawing and huge screens never get High; Ultra is never advised.
        assertEquals(GraphicsPreset.Medium, SetupAdvice.recommend(1.0, hd, true))
        assertEquals(GraphicsPreset.Medium, SetupAdvice.recommend(1.0, SUGGEST_MEDIUM_ABOVE + 1, false))
        assertEquals(GraphicsPreset.Low, SetupAdvice.recommend(500.0, hd, true))
        assertTrue(listOf(0.1, 1.0, 10.0).all { SetupAdvice.recommend(it, hd, false) != GraphicsPreset.Ultra })
        // No measurement falls back on the screen and backend alone.
        assertEquals(GraphicsPreset.High, SetupAdvice.recommend(SetupBenchmark.Unavailable, hd, false))
        assertEquals(GraphicsPreset.Medium, SetupAdvice.recommend(SetupBenchmark.Unavailable, hd, true))
    }

    @Test
    fun skipAppliesTheDefaultsAndMarksSetupDone() {
        val file = settingsFile("skip")
        val s = AppSettings(file)
        assertEquals(SetupState.Fresh, s.setupState)
        s.chooseAnalytics(false)
        s.chooseUpdateChannel(UpdateChannel.Preview)
        SetupDraft.defaults(1_000_000L, software = true).commit(s, includeChannel = true)
        assertEquals(SetupState.Done, s.setupState)
        assertEquals(GraphicsPreset.Medium, s.graphics.preset, "software drawing starts on Medium")
        assertEquals(UpdateChannel.build, s.updateChannel)
        assertTrue(s.analytics)
        assertFalse(s.tutorialHidden)
        // It is written to the existing keys, plus the new one.
        val reread = AppSettings(file)
        assertEquals(SetupState.Done, reread.setupState)
        assertEquals(GraphicsPreset.Medium, reread.graphics.preset)
    }

    @Test
    fun commitOmitsTheChannelWhereThereIsNoUpdater() {
        val s = settings("nochannel")
        s.chooseUpdateChannel(UpdateChannel.Stable)
        SetupDraft(GraphicsPreset.Low, UpdateChannel.Preview, analytics = false, tutorial = false).commit(s, includeChannel = false)
        assertEquals(UpdateChannel.Stable, s.updateChannel)
        assertEquals(GraphicsPreset.Low, s.graphics.preset)
        assertFalse(s.analytics)
        assertTrue(s.tutorialHidden)
    }

    @Test
    fun firstLaunchUpgradeAndFinishedAreTold() {
        val file = settingsFile("fresh")
        assertEquals(SetupState.Fresh, AppSettings(file).setupState)
        // Closing the game before finishing keeps the wizard coming.
        assertEquals(SetupState.Fresh, AppSettings(file).setupState)
        // A 1.8 settings file has no setupDone key and gets What's new; its choices stay as they are.
        val oldFile = settingsFile("old", create = true)
        val old = AppSettings(oldFile)
        assertEquals(SetupState.Upgrade, old.setupState)
        old.chooseAnalytics(false)
        assertEquals(SetupState.Upgrade, AppSettings(oldFile).setupState, "other changes do not end What's new")
        old.finishSetup()
        assertEquals(SetupState.Done, AppSettings(oldFile).setupState)
        assertFalse(AppSettings(oldFile).analytics, "dismissing What's new changes no choice")
    }

    @Test
    fun everyStepRendersAtDesktopAndPhoneSize() {
        val out = System.getenv("CW_SETUP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        val dir = kotlin.io.path.createTempDirectory("cwsetupui").toFile()
        val sizes = mapOf("desktop" to (1280 to 800), "phone" to (390 to 844))
        for ((label, size) in sizes) {
            for (index in 0 until SetupStep.entries.size) {
                val settings = AppSettings(File(dir, "$label-$index.properties"))
                lateinit var visuals: SetupVisuals
                val scene = ImageComposeScene(size.first, size.second, Density(1f)) {
                    CorporateWarfareTheme {
                        visuals = rememberSetupVisuals()
                        SetupWizard(settings, SetupUpdater.stub(dir), "/home/player/.local/share/corporate-warfare", firstRun = true, onDone = {}, visuals = visuals, startStep = index)
                    }
                }
                scene.render(0L)
                // The picture and the test run behind the screen; give them time to finish.
                val deadline = System.currentTimeMillis() + 30_000
                var t = 0L
                while (System.currentTimeMillis() < deadline) {
                    t += 50_000_000L
                    scene.render(t)
                    val ready = when (SetupStep.entries[index]) {
                        SetupStep.Welcome -> visuals.globe != null
                        SetupStep.Graphics -> visuals.benchmark is SetupBenchmark.Done && visuals.preview != null
                        else -> true
                    }
                    if (ready) break
                    Thread.sleep(50)
                }
                // Let the preset chosen by the test and the preview for it settle.
                repeat(40) { t += 50_000_000L; scene.render(t); Thread.sleep(25) }
                val image = scene.render(t + 50_000_000L)
                scene.close()
                val bytes = assertNotNull(image.encodeToData(EncodedImageFormat.PNG)).bytes
                val decoded = org.jetbrains.skia.Image.makeFromEncoded(bytes)
                val bitmap = org.jetbrains.skia.Bitmap.makeFromImage(decoded)
                for ((x, y) in listOf(0 to 0, size.first - 1 to 0, 0 to size.second - 1, size.first - 1 to size.second - 1, size.first / 2 to size.second / 2)) {
                    assertEquals(255, (bitmap.getColor(x, y) ushr 24) and 0xFF, "$label step $index is opaque at $x,$y")
                }
                out?.let { File(it, "setup-${SetupStep.entries[index].name.lowercase()}-$label.png").writeBytes(bytes) }
            }
        }
    }

    @Test
    fun whatsNewRenders() {
        val out = System.getenv("CW_SETUP_RENDER_DIR")?.let(::File)?.also { it.mkdirs() }
        for ((label, size) in mapOf("desktop" to (1280 to 800), "phone" to (390 to 844))) {
            val scene = ImageComposeScene(size.first, size.second, Density(1f)) { CorporateWarfareTheme { WhatsNewScreen({}, {}) } }
            scene.render(0L)
            val image = scene.render(100_000_000L)
            scene.close()
            out?.let { File(it, "setup-whatsnew-$label.png").writeBytes(image.encodeToData(EncodedImageFormat.PNG)!!.bytes) }
        }
    }
}

/** A stand-in updater so the Updates step shows in the pictures. */
private object SetupUpdater {
    fun stub(dir: File): Updater {
        val platform = object : UpdatePlatform {
            override val installerSuffix: String? = null
            override fun install(file: File, version: String) = Unit
            override fun open(url: String) = Unit
        }
        return Updater(platform, kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.Dispatchers.Default), File(dir, "updates"), File(dir, "saves"), { UpdateChannel.Stable }, { emptyList() }, "1.9.0")
    }
}
