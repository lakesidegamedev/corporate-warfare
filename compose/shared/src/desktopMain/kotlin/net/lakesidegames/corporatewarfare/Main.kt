package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPlacement
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.awt.Desktop
import java.io.File
import java.net.URI
import kotlin.system.exitProcess

/** Where the desktop game keeps its saves: the per-user application data folder of each OS. */
private fun saveDirectory(): File {
    val os = System.getProperty("os.name").lowercase()
    val home = System.getProperty("user.home")
    return when {
        os.contains("win") -> File(System.getenv("APPDATA") ?: "$home\\AppData\\Roaming", "Corporate Warfare")
        os.contains("mac") -> File(home, "Library/Application Support/Corporate Warfare")
        else -> File(System.getenv("XDG_DATA_HOME") ?: "$home/.local/share", "corporate-warfare")
    }
}

private val windows = System.getProperty("os.name").lowercase().contains("win")

/**
 * Windows installs a downloaded version itself: a short script waits until no copy of the game
 * runs (closing any that stay open), runs the setup program with a progress bar and no questions,
 * and the setup program starts the game again. The same setup program upgrades and downgrades
 * ([WindowsInstall.setupPlan]). A release that only has an MSI goes through the 1.8 route: the
 * MSI, with an uninstall and second try for an older version ([WindowsInstall.plan]). Linux and Mac open the release page, where the package or app is one
 * click.
 */
private object DesktopUpdates : UpdatePlatform {
    override val installerSuffix: String? = if (windows) WindowsInstall.SETUP_SUFFIX else null
    override val installerSuffixes: List<String> = if (windows) WindowsInstall.SUFFIXES else emptyList()

    override fun install(file: File, version: String) {
        val app = System.getProperty("jpackage.app-path")
        val script = File(file.parentFile, "install-update.cmd")
        script.writeText(WindowsInstall.planFor(file.absolutePath, version, BuildInfo.VERSION, app).joinToString("\r\n", postfix = "\r\n"))
        ProcessBuilder("cmd", "/c", "start", "\"\"", "/min", script.absolutePath).start()
        exitProcess(0)
    }

    override fun open(url: String) {
        runCatching { Desktop.getDesktop().browse(URI(url)) }
    }
}

/** The game window, once open, so a second launch can bring it to the front. */
@Volatile
private var frontWindow: java.awt.Window? = null

/** Show the open window above others; Windows only flashes the taskbar for a plain toFront. */
private fun bringToFront() = java.awt.EventQueue.invokeLater {
    val window = frontWindow ?: return@invokeLater
    if (window is java.awt.Frame && window.extendedState and java.awt.Frame.ICONIFIED != 0) {
        window.extendedState = window.extendedState and java.awt.Frame.ICONIFIED.inv()
    }
    window.isVisible = true
    window.isAlwaysOnTop = true
    window.toFront()
    window.requestFocus()
    window.isAlwaysOnTop = false
}

fun main() {
    if (System.getenv("CW_STARTUP_TIMELINE") != null) {
        StartupTimeline.origin = java.lang.management.ManagementFactory.getRuntimeMXBean().startTime
    }
    StartupTimeline.mark("main() entered")
    // One copy at a time: a second launch brings the open game forward and exits (SingleInstance).
    when (SingleInstance.start(saveDirectory(), onActivate = ::bringToFront)) {
        is SingleInstance.Start.First -> Unit
        SingleInstance.Start.Signalled -> exitProcess(0)
        SingleInstance.Start.Unresponsive -> {
            javax.swing.JOptionPane.showMessageDialog(
                null,
                "Corporate Warfare is already open but is not responding.\n" +
                    "Close it, or end it in Task Manager, then start the game again.",
                "Corporate Warfare",
                javax.swing.JOptionPane.INFORMATION_MESSAGE,
            )
            exitProcess(0)
        }
    }
    game()
}

private fun game() = application {
    Images.decode = { bytes -> org.jetbrains.skia.Image.makeFromEncoded(bytes).toComposeImageBitmap() }
    MeshCanvas.draw = desktopMeshDraw
    MapBake.enabled = true
    // The one-pass cloud shader is measured slower than the meshes while the layers are baked on the CPU, so it is opt-in.
    if (System.getenv("CW_CLOUD_SHADER") != null) MeshCanvas.cloudFlow = desktopCloudFlow
    Pixel.font = desktopPixelFont()
    MeshCanvas.shader = desktopLinearShader
    val settings = remember { AppSettings(File(saveDirectory(), "settings.properties")) }
    var loaded by remember { mutableStateOf(false) }
    var progress by remember { mutableFloatStateOf(0f) }
    var stage by remember { mutableStateOf("Starting") }

    if (!loaded) {
        // The launcher: a small branded window while the engine and the world map load.
        Window(
            onCloseRequest = ::exitApplication,
            title = "Corporate Warfare",
            undecorated = true,
            resizable = false,
            state = rememberWindowState(position = WindowPosition(Alignment.Center), size = DpSize(560.dp, 380.dp)),
        ) {
            frontWindow = window
            Splash(progress, stage)
            LaunchedEffect(Unit) {
                StartupTimeline.mark("launcher window shown")
                withContext(Dispatchers.Default) {
                    stage = "Starting the engine"
                    runCatching { uniffi.cw_ffi.engineInfo() }
                    StartupTimeline.mark("engine loaded")
                    progress = 0.35f
                    stage = "Loading the world map"
                    WorldMap.shared
                    StartupTimeline.mark("world map loaded")
                    progress = 0.85f
                    stage = "Drawing the icons"
                    SectorIcons.all
                    PixelLogo.file
                    StartupTimeline.mark("icons loaded")
                    progress = 1f
                }
                loaded = true
            }
        }
        return@application
    }

    val state = rememberWindowState(
        size = DpSize(settings.resolution.first.dp, settings.resolution.second.dp),
        placement = settings.windowMode.placement(),
        position = WindowPosition(Alignment.Center),
    )
    // Window settings apply as soon as they change on the settings page.
    LaunchedEffect(settings.windowMode, settings.resolution) {
        state.placement = settings.windowMode.placement()
        if (settings.windowMode == WindowMode.Windowed) {
            state.size = DpSize(settings.resolution.first.dp, settings.resolution.second.dp)
        }
    }
    Window(
        onCloseRequest = ::exitApplication,
        title = "Corporate Warfare",
        state = state,
    ) {
        frontWindow = window
        StartupTimeline.mark("game window composing")
        // The frame counter shows which Skia backend draws this window.
        androidx.compose.runtime.DisposableEffect(window) {
            RenderInfo.backend = window.renderApi.name
            window.onRenderApiChanged { RenderInfo.backend = window.renderApi.name }
            onDispose { }
        }
        // With no saved graphics choice, a big window or software drawing starts at Medium.
        LaunchedEffect(window) {
            kotlinx.coroutines.delay(1500)
            val scale = window.graphicsConfiguration?.defaultTransform
            val pixels = window.width * (scale?.scaleX ?: 1.0) * window.height * (scale?.scaleY ?: 1.0)
            settings.suggestGraphics(pixels.toLong(), RenderInfo.software)
        }
        val scope = rememberCoroutineScope()
        val store = remember { GameStore(saveDirectory(), scope) }
        val updater = remember {
            Updater(DesktopUpdates, scope, File(saveDirectory(), "updates"), saveDirectory(), channel = { settings.updateChannel })
        }
        CorporateWarfareApp(store, settings, updater, onQuit = ::exitApplication, saveFolder = saveDirectory().path)
    }
}

@Composable
private fun Splash(progress: Float, stage: String) {
    CorporateWarfareTheme {
        Column(
            Modifier.fillMaxSize().background(Palette.ink).border(1.dp, Palette.stroke).padding(28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
        ) {
            LogoArt(Modifier.width(230.dp))
            Spacer(Modifier.height(22.dp))
            LinearProgressIndicator(
                progress = { progress },
                modifier = Modifier.fillMaxWidth(0.8f).height(6.dp),
                color = Palette.gold,
                trackColor = Palette.panelRaised,
            )
            Spacer(Modifier.height(10.dp))
            Text(stage, color = Palette.secondary, fontSize = 12.sp)
            Spacer(Modifier.height(4.dp))
            Text(
                "Version " + BuildInfo.VERSION,
                color = Palette.secondary.copy(alpha = 0.6f),
                fontSize = 11.sp,
                modifier = Modifier.fillMaxWidth(),
                textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
        }
    }
}

private fun WindowMode.placement(): WindowPlacement = when (this) {
    WindowMode.Windowed -> WindowPlacement.Floating
    WindowMode.Maximized -> WindowPlacement.Maximized
    WindowMode.Fullscreen -> WindowPlacement.Fullscreen
}

/** Clouds go to Skia as arrays in one call. */
internal val desktopMeshDraw: (androidx.compose.ui.graphics.Canvas, FloatArray, FloatArray, IntArray, ShortArray, Int, Int, androidx.compose.ui.graphics.Paint) -> Unit =
    { canvas, positions, tex, colors, indices, vertices, indexCount, paint ->
        canvas.nativeCanvas.drawVertices(
            org.jetbrains.skia.VertexMode.TRIANGLES, positions.copyOf(vertices * 2), colors.copyOf(vertices), tex.copyOf(vertices * 2),
            indices.copyOf(indexCount), org.jetbrains.skia.BlendMode.MODULATE, paint.asFrameworkPaint(),
        )
    }

/** Skia shaders made by Compose sample the nearest texel; clouds need smooth sampling when magnified. */
internal val desktopLinearShader: (androidx.compose.ui.graphics.ImageBitmap, Boolean) -> androidx.compose.ui.graphics.Shader = { image, repeatX ->
    org.jetbrains.skia.Image.makeFromBitmap(image.asSkiaBitmap()).makeShader(
        if (repeatX) org.jetbrains.skia.FilterTileMode.REPEAT else org.jetbrains.skia.FilterTileMode.MIRROR,
        if (repeatX) org.jetbrains.skia.FilterTileMode.CLAMP else org.jetbrains.skia.FilterTileMode.MIRROR,
        org.jetbrains.skia.SamplingMode.LINEAR, null,
    )
}

/** Silkscreen, the pixel font, from the app's resources. */
internal fun desktopPixelFont(): androidx.compose.ui.text.font.FontFamily {
    fun load(name: String, weight: androidx.compose.ui.text.font.FontWeight) =
        Pixel::class.java.getResourceAsStream("/$name")?.use { it.readBytes() }
            ?.let { androidx.compose.ui.text.platform.Font(name, it, weight) }
    val fonts = listOfNotNull(
        load("Silkscreen-Regular.ttf", androidx.compose.ui.text.font.FontWeight.Normal),
        load("Silkscreen-Bold.ttf", androidx.compose.ui.text.font.FontWeight.Bold),
    )
    return if (fonts.isEmpty()) androidx.compose.ui.text.font.FontFamily.Default else androidx.compose.ui.text.font.FontFamily(fonts)
}
