package net.lakesidegames.corporatewarfare

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val scope = rememberCoroutineScope()
            val store = remember { GameStore(filesDir, scope) }
            val settings = remember { AppSettings(java.io.File(filesDir, "settings.properties")) }
            val updater = remember { Updater(AndroidUpdates(this), scope, java.io.File(cacheDir, "updates")) }
            CorporateWarfareApp(store, settings, updater)
        }
    }
}

/** Android opens the release's APK in the browser; the system installer takes it from there. */
private class AndroidUpdates(private val activity: ComponentActivity) : UpdatePlatform {
    override val installerSuffix: String? = null

    override fun install(file: java.io.File) = Unit

    override fun open(url: String) {
        runCatching {
            activity.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)))
        }
    }
}
