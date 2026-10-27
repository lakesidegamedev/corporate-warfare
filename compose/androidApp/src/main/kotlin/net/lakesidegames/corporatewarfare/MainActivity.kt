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
            CorporateWarfareApp(store, settings)
        }
    }
}
