package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.nativeCanvas

import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.runtime.rememberCoroutineScope

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // System back closes the open screen, step or overlay one level at a time; with nothing
        // open it leaves the app as usual.
        onBackPressedDispatcher.addCallback(this, object : androidx.activity.OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (Back.press()) return
                isEnabled = false
                onBackPressedDispatcher.onBackPressed()
                isEnabled = true
            }
        })
        Pixel.font = androidPixelFont()
        MeshCanvas.draw = { canvas, positions, tex, colors, indices, vertices, indexCount, paint ->
            // Clouds go to the graphics chip as arrays in one call.
            canvas.nativeCanvas.drawVertices(
                android.graphics.Canvas.VertexMode.TRIANGLES, vertices * 2, positions, 0, tex, 0, colors, 0, indices, 0, indexCount,
                paint.asFrameworkPaint(),
            )
        }
        // androidCloudFlow (Android 13 and up) is ready but not installed: it needs checking on a device first.
        Images.decode = { bytes ->
            android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
        }
        setContent {
            val scope = rememberCoroutineScope()
            val store = remember { GameStore(filesDir, scope) }
            val settings = remember { AppSettings(java.io.File(filesDir, "settings.properties")) }
            // A Play install updates through Play; only the direct-download APK keeps the updater.
            val updater = remember {
                if (Updater.selfUpdateAllowed(installerPackage())) {
                    Updater(AndroidUpdates(this), scope, java.io.File(cacheDir, "updates"), filesDir, channel = { settings.updateChannel })
                } else {
                    null
                }
            }
            CorporateWarfareApp(store, settings, updater)
        }
    }

    /** Which package installed this build: the Play Store, a browser, or nothing (adb/sideload). */
    private fun installerPackage(): String? = runCatching {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            packageManager.getInstallSourceInfo(packageName).installingPackageName
        } else {
            @Suppress("DEPRECATION")
            packageManager.getInstallerPackageName(packageName)
        }
    }.getOrNull()
}

/**
 * Android opens the release's APK in the browser; the system installer takes it from there. It
 * refuses a lower version over the installed app, so going back needs an uninstall first.
 */
private class AndroidUpdates(private val activity: ComponentActivity) : UpdatePlatform {
    override val installerSuffix: String? = null
    override val downgrade = Downgrade.UninstallFirst

    override fun install(file: java.io.File, version: String) = Unit

    override fun open(url: String) {
        runCatching {
            activity.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)))
        }
    }
}

/** Silkscreen, the pixel font: copied from the app's resources to the cache once, then loaded from there. */
private fun android.content.Context.androidPixelFont(): androidx.compose.ui.text.font.FontFamily {
    fun load(name: String, weight: androidx.compose.ui.text.font.FontWeight): androidx.compose.ui.text.font.Font? {
        val file = java.io.File(cacheDir, name)
        if (!file.exists()) {
            val bytes = Pixel::class.java.getResourceAsStream("/$name")?.use { it.readBytes() } ?: return null
            file.writeBytes(bytes)
        }
        return androidx.compose.ui.text.font.Font(file, weight)
    }
    val fonts = listOfNotNull(
        load("Silkscreen-Regular.ttf", androidx.compose.ui.text.font.FontWeight.Normal),
        load("Silkscreen-Bold.ttf", androidx.compose.ui.text.font.FontWeight.Bold),
    )
    return if (fonts.isEmpty()) androidx.compose.ui.text.font.FontFamily.Default else androidx.compose.ui.text.font.FontFamily(fonts)
}
