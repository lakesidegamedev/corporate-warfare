package net.lakesidegames.corporatewarfare

import android.content.Intent
import android.graphics.Bitmap
import android.os.SystemClock
import android.view.MotionEvent
import android.view.WindowInsets
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertNull
import org.junit.Assume.assumeTrue
import org.junit.Test
import uniffi.cw_ffi.NewGameDto

/**
 * Store listing screenshots, taken from the real app on the emulator. It plays the same career as the
 * App Store set (Great Lakes 2000, guided, seed 7, ten weeks), then opens each tab of the phone shell
 * and saves a picture of the screen. It only runs when started with `-e screenshots true` (the
 * Play screenshots workflow); the ordinary smoke run skips it. Debug build only, never shipped.
 */
class StoreScreenshotsTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    private fun waitFor(what: String, timeoutMs: Long = 90_000, done: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (!done()) {
            if (System.currentTimeMillis() > end) throw AssertionError("timed out waiting for $what")
            Thread.sleep(50)
        }
    }

    private fun tap(x: Float, y: Float) {
        val now = SystemClock.uptimeMillis()
        instrumentation.sendPointerSync(MotionEvent.obtain(now, now, MotionEvent.ACTION_DOWN, x, y, 0))
        instrumentation.sendPointerSync(MotionEvent.obtain(now, now + 60, MotionEvent.ACTION_UP, x, y, 0))
    }

    @Test
    fun capturePhoneTabs() {
        assumeTrue("not a screenshot run", InstrumentationRegistry.getArguments().getString("screenshots") == "true")
        val context = instrumentation.targetContext
        val out = File(context.getExternalFilesDir(null), "shots").apply { deleteRecursively(); mkdirs() }
        val store = GameStore(File(context.cacheDir, "shots-saves").apply { deleteRecursively(); mkdirs() }, CoroutineScope(Dispatchers.Default))
        store.startCareer(
            NewGameDto(
                seed = 7uL,
                scenarioId = "great-lakes-2000",
                guided = true,
                fiveYear = false,
                difficulty = "standard",
                economy = "historic",
                startingCapitalCents = null,
                pricePath = "",
                timeline = "",
            ),
        )
        waitFor("the career to start") { store.ready || store.error != null }
        assertNull("starting failed: ${store.error}", store.error)
        repeat(10) {
            val before = store.overview!!.tick
            store.advance()
            waitFor("week ${before + 1u}") { store.error != null || (store.overview?.tick ?: 0u) > before }
            assertNull("advancing failed: ${store.error}", store.error)
        }
        store.overview?.primaryFirm?.let { store.selectFirm(it) }

        // A returning player's settings: setup finished, this version's notes seen, tutorial hidden and no
        // telemetry from the emulator, so every shot shows the game rather than a first-run panel.
        val settingsFile = File(context.cacheDir, "shots-settings.properties").apply {
            writeText("setupDone=true\nwhatsNewSeen=${BuildInfo.VERSION}\ntutorial.hidden=true\nanalytics=false\n")
        }
        val settings = AppSettings(settingsFile)
        val intent = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val activity = instrumentation.startActivitySync(intent) as ComponentActivity
        instrumentation.runOnMainSync { activity.setContent { CorporateWarfareApp(store, settings) } }
        Thread.sleep(6000)

        val metrics = activity.resources.displayMetrics
        var bottomInset = 0
        instrumentation.runOnMainSync {
            val insets = activity.window.decorView.rootWindowInsets
            bottomInset = insets?.getInsets(WindowInsets.Type.systemBars())?.bottom ?: 0
        }
        // The bottom bar is 58 dp tall with five equal entries: Map, Company, Markets, Rivals, More.
        val barY = metrics.heightPixels - bottomInset - 29f * metrics.density
        val tabs = listOf("map", "company", "markets", "rivals", "more")
        tabs.forEachIndexed { i, name ->
            tap(metrics.widthPixels * (i + 0.5f) / tabs.size, barY)
            Thread.sleep(3500)
            val shot = instrumentation.uiAutomation.takeScreenshot()
            File(out, "${i + 1}-$name.png").outputStream().use { shot.compress(Bitmap.CompressFormat.PNG, 100, it) }
        }
    }
}
