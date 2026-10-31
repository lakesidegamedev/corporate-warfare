package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import java.awt.Canvas
import java.awt.Color
import java.awt.event.ComponentAdapter
import java.awt.event.ComponentEvent
import java.awt.event.HierarchyEvent
import java.util.concurrent.atomic.AtomicLong
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.thread
import kotlin.concurrent.withLock

/**
 * The native window the GPU map draws into: a heavyweight AWT canvas, which has a window of its own (an HWND on Windows, an X11
 * window on Linux) that the renderer presents frames to. Compose draws its layers over it when the render API supports interop
 * blending (Direct3D on Windows); [onPaint] fires when the system asks for the window to be redrawn.
 */
internal class MapSurfaceCanvas : Canvas() {
    @Volatile var onPaint: (() -> Unit)? = null

    init {
        background = Color(0x0B, 0x16, 0x22)
        isFocusable = false
        // The engine presents to this window itself: AWT must never paint it (no background, no repaint over the swap chain).
        ignoreRepaint = true
    }

    // Neither paint nor update draws anything: the window is the engine's. A repaint request only asks for a new frame.
    override fun paint(g: java.awt.Graphics?) { onPaint?.invoke() }
    override fun update(g: java.awt.Graphics?) { onPaint?.invoke() }

    /** Device pixels per Swing unit: what the window's real size is, against the size Swing reports. */
    val pixelScale: Double get() = graphicsConfiguration?.defaultTransform?.scaleX ?: 1.0
}

@Suppress("FunctionName")
private interface X11 : Library {
    fun XOpenDisplay(name: String?): Pointer?
    fun XCloseDisplay(display: Pointer?): Int

    companion object {
        val instance: X11 by lazy { Native.load("libX11.so.6", X11::class.java) }
    }
}

private val osName = System.getProperty("os.name").lowercase()
internal val onWindows = osName.contains("windows")
internal val onLinux = osName.contains("linux")

/** The platforms the engine can present to from a canvas: Windows and X11 (Linux). */
internal fun nativeSurfaceSupported(): Boolean = (onWindows || onLinux) && !java.awt.GraphicsEnvironment.isHeadless()

/** One frame the direct session presented, for the frame counter and the tests. */
internal class DirectFrame(
    val seq: Long, val camera: GpuCamera, val width: Int, val height: Int,
    val renderMs: Double, val presentMs: Double, val totalMs: Double, val atNanos: Long,
)

/**
 * Opens the GPU map drawing into [canvas]'s native window, or throws why it cannot (the window does not exist yet, no adapter that
 * can present to it, a driver fault, missing files).
 */
internal fun openDirectGpuMap(scenario: String, canvas: Canvas): GpuMapSession {
    val lib = CwGpu.instance
    // The canvas gets its window when the game window shows it; give that a few seconds.
    val deadline = System.nanoTime() + 5_000_000_000L
    while ((!canvas.isDisplayable || canvas.width <= 0) && System.nanoTime() < deadline) Thread.sleep(25)
    if (!canvas.isDisplayable) throw IllegalStateException("the map window is not on screen")
    val handle = Native.getComponentID(canvas)
    if (handle == 0L) throw IllegalStateException("the map window has no native handle")
    val display: Pointer? = if (onLinux) X11.instance.XOpenDisplay(null) ?: throw IllegalStateException("no X11 display") else null
    val kind = if (onWindows) 1 else 2
    val id = lib.cw_gpu_open_surface(gpuAssetFolder(scenario).absolutePath, scenario, kind, handle, display)
    if (id == 0L) {
        display?.let { X11.instance.XCloseDisplay(it) }
        throw IllegalStateException(lib.text { lib.cw_gpu_last_error(it, it.size.toLong()) } ?: "no graphics adapter for the window")
    }
    val info = lib.text { lib.cw_gpu_info(id, it, it.size.toLong()) }?.split('|') ?: listOf("unknown", "", "0")
    val surfaceInfo = info.getOrElse(3) { "" }
    println("direct map: adapter=${info.getOrElse(0) { "unknown" }} backend=${info.getOrElse(1) { "" }} surface ${canvas.width}x${canvas.height} ($surfaceInfo)")
    return DirectGpuSession(lib, id, info.getOrElse(0) { "unknown" }, info.getOrElse(2) { "0" } == "1", canvas, display)
}

/**
 * One open map drawing straight into a native window. A render thread draws the newest requested view and presents it (the
 * present waits for the display's refresh, which paces the thread), so while the camera moves a frame reaches the screen every
 * refresh, each for the camera that was asked for when it started. When nothing changes the thread sleeps. Nothing is copied back.
 */
internal class DirectGpuSession(
    lib: CwGpu,
    id: Long,
    adapter: String,
    software: Boolean,
    private val canvas: Canvas,
    private val display: Pointer?,
) : FfiGpuSession(lib, id, adapter, software) {
    private class Request(val camera: GpuCamera, val width: Int, val height: Int)

    private val lock = ReentrantLock()
    private val wake = lock.newCondition()
    private var wanted: Request? = null
    private var drawn: Request? = null
    private var dirty = true
    private var closed = false
    private var shut = false

    override val direct = true
    @Volatile override var renderMs = 0.0
    @Volatile override var copyMs = 0.0
    override val frame: ImageBitmap? get() = null
    override val frameCamera: GpuCamera? get() = null
    override val shown: GpuShown? get() = null
    override var version by mutableLongStateOf(0L)
        private set

    /** Frames put on the screen, and a listener called on the render thread for each one. */
    val presented = AtomicLong()
    @Volatile var frameListener: ((DirectFrame) -> Unit)? = null

    private val firstRequestAt = AtomicLong(0)
    private var slowRun = 0
    @Volatile private var slowWhy: String? = null

    /**
     * The watchdog's verdict: no frame reached the window within 2 s of the first request, or presents took over 250 ms ten
     * frames in a row. Read by the map screen, which then closes this session and draws copied frames instead.
     */
    override val trouble: String?
        get() {
            slowWhy?.let { return it }
            val first = firstRequestAt.get()
            if (presented.get() == 0L && first != 0L && System.nanoTime() - first > 2_000_000_000L) return "no frame reached the window in 2 seconds"
            return null
        }

    private val worker = thread(name = "cw-gpu-surface", isDaemon = true) { loop() }

    /**
     * Frames can be presented and still not be on screen (the window behind another layer, a compositor that does not show the
     * swap chain). Once a frame is presented, look at the window now and then: if it still shows a single colour after 12 s, that
     * is trouble. Where the screen cannot be captured, the check quietly ends.
     */
    private val screenWatch = thread(name = "cw-gpu-screen-watch", isDaemon = true) {
        try {
            val robot = java.awt.Robot()
            var firstFrameAt = 0L
            while (!closed && slowWhy == null) {
                Thread.sleep(1000)
                if (presented.get() == 0L || !canvas.isShowing) continue
                if (firstFrameAt == 0L) firstFrameAt = System.nanoTime()
                val at = canvas.locationOnScreen
                val k = (canvas as? MapSurfaceCanvas)?.pixelScale ?: 1.0
                val w = (canvas.width * k).toInt(); val h = (canvas.height * k).toInt()
                if (w < 16 || h < 16) continue
                val img = robot.createScreenCapture(java.awt.Rectangle(at.x, at.y, w, h))
                val seen = HashSet<Int>()
                for (gy in 1..12) for (gx in 1..12) seen += img.getRGB(gx * w / 13, gy * h / 13)
                if (seen.size > 1) return@thread
                if (System.nanoTime() - firstFrameAt > 12_000_000_000L) { slowWhy = "the map window stayed blank for 12 seconds"; return@thread }
            }
        } catch (_: Throwable) { }
    }

    private val redraw = Runnable { lock.withLock { dirty = true; wake.signalAll() } }
    private val resized = object : ComponentAdapter() {
        override fun componentResized(e: ComponentEvent?) = redraw.run()
        override fun componentShown(e: ComponentEvent?) = redraw.run()
    }
    private val hierarchy = java.awt.event.HierarchyListener { e ->
        // The window goes away with its canvas: stop drawing into it first.
        if (e.changeFlags and HierarchyEvent.DISPLAYABILITY_CHANGED.toLong() != 0L && !canvas.isDisplayable) shutdown()
        else redraw.run()
    }

    init {
        (canvas as? MapSurfaceCanvas)?.onPaint = redraw::run
        canvas.addComponentListener(resized)
        canvas.addHierarchyListener(hierarchy)
    }

    override fun update(json: String) {
        if (!push(json)) return
        redraw.run()
    }

    override fun request(camera: GpuCamera, width: Int, height: Int, moving: Boolean) {
        if (width <= 0 || height <= 0) return
        firstRequestAt.compareAndSet(0, System.nanoTime())
        lock.withLock {
            val old = wanted
            if (old == null || old.camera != camera || old.width != width || old.height != height) {
                wanted = Request(camera, width, height)
                wake.signalAll()
            }
        }
    }

    override fun close() = shutdown()

    private fun shutdown() {
        lock.withLock {
            if (shut) return
            shut = true
            closed = true
            wake.signalAll()
        }
        (canvas as? MapSurfaceCanvas)?.onPaint = null
        canvas.removeComponentListener(resized)
        canvas.removeHierarchyListener(hierarchy)
        if (Thread.currentThread() !== worker) worker.join(3000)
        lib.cw_gpu_close(id)
        display?.let { X11.instance.XCloseDisplay(it) }
    }

    private fun loop() {
        val timing = DoubleArray(6)
        var failures = 0
        while (true) {
            val job: Request
            lock.withLock {
                while (true) {
                    if (closed) return
                    val w = wanted
                    if (w != null && (dirty || drawn !== w)) { job = w; dirty = false; drawn = w; break }
                    wake.awaitNanos(1_000_000_000L)
                }
            }
            val t0 = System.nanoTime()
            val code = try {
                lib.cw_gpu_render_surface(id, cam(job.camera), job.width, job.height, timing)
            } catch (_: Throwable) { -1 }
            val total = (System.nanoTime() - t0) / 1e6
            if (code < 0) {
                GpuMapHost.failure = lib.text { lib.cw_gpu_last_error(it, it.size.toLong()) }?.take(80) ?: "the graphics card stopped drawing"
                if (++failures > 20) { GpuMapHost.surfaceNote = GpuMapHost.failure; return }
                lock.withLock { if (!closed) wake.awaitNanos(200_000_000L) }
                continue
            }
            failures = 0
            val skipped = code == 1
            if (skipped) {
                // The window could not take a frame (hidden, being resized): try again shortly.
                lock.withLock { if (!closed) { dirty = true; wake.awaitNanos(30_000_000L) } }
                continue
            }
            renderMs = timing[0]
            copyMs = timing[1]
            if (timing[1] > 250.0) { if (++slowRun >= 10) slowWhy = "frames took over 250 ms to present, ten in a row" } else slowRun = 0
            val seq = presented.incrementAndGet()
            frameListener?.invoke(DirectFrame(seq, job.camera, job.width, job.height, timing[0], timing[1], total, t0))
        }
    }
}

/**
 * The map area's native surface for the Compose tree: a heavyweight canvas the engine draws into, laid out like any Compose
 * element. Compose draws over it (interop blending), so names, badges, controls and panels sit on top. [onSurface] gets the canvas
 * once it exists; [onMouse] gets the pointer events the canvas receives itself, in map pixels.
 */
@androidx.compose.runtime.Composable
internal fun MapSurfacePanel(modifier: androidx.compose.ui.Modifier, onSurface: (Any) -> Unit, onMouse: (SurfaceMouse) -> Unit) {
    val canvas = androidx.compose.runtime.remember { MapSurfaceCanvas() }
    val mouse = androidx.compose.runtime.rememberUpdatedState(onMouse)
    androidx.compose.runtime.DisposableEffect(canvas) {
        val adapter = object : java.awt.event.MouseAdapter() {
            private fun send(e: java.awt.event.MouseEvent, kind: SurfaceMouse.Kind, wheel: Float = 0f) {
                val k = canvas.pixelScale.toFloat()
                mouse.value(SurfaceMouse(kind, e.x * k, e.y * k, e.button, wheel, e.clickCount))
            }
            override fun mousePressed(e: java.awt.event.MouseEvent) = send(e, SurfaceMouse.Kind.Press)
            override fun mouseReleased(e: java.awt.event.MouseEvent) = send(e, SurfaceMouse.Kind.Release)
            override fun mouseDragged(e: java.awt.event.MouseEvent) = send(e, SurfaceMouse.Kind.Drag)
            override fun mouseMoved(e: java.awt.event.MouseEvent) = send(e, SurfaceMouse.Kind.Move)
            override fun mouseExited(e: java.awt.event.MouseEvent) = send(e, SurfaceMouse.Kind.Exit)
            override fun mouseWheelMoved(e: java.awt.event.MouseWheelEvent) = send(e, SurfaceMouse.Kind.Wheel, e.preciseWheelRotation.toFloat())
        }
        canvas.addMouseListener(adapter)
        canvas.addMouseMotionListener(adapter)
        canvas.addMouseWheelListener(adapter)
        onDispose {
            canvas.removeMouseListener(adapter)
            canvas.removeMouseMotionListener(adapter)
            canvas.removeMouseWheelListener(adapter)
        }
    }
    androidx.compose.ui.awt.SwingPanel(
        factory = { canvas },
        modifier = modifier,
        update = { onSurface(it) },
    )
}

/**
 * Whether the map can be drawn straight into a native surface here: the engine can present to the platform's windows, and Compose
 * draws over a native panel (interop blending, which Compose supports on Direct3D and Metal only, so Windows with the Direct3D
 * renderer). `CW_NO_DIRECT_MAP` turns it off; `CW_DIRECT_MAP=1` forces it on (for a machine under test).
 */
internal fun directSurfaceUsable(): Boolean {
    if (!nativeSurfaceSupported() || System.getenv("CW_NO_DIRECT_MAP") != null) return false
    if (System.getenv("CW_DIRECT_MAP") == "1") return true
    return onWindows && RenderInfo.backend == "DIRECT3D" && System.getProperty("compose.interop.blending") == "true"
}

/** Installs the native surface for the desktop map. Must run before the first window is made (Compose reads the blending switch once). */
internal fun installDirectSurface() {
    if (onWindows && System.getProperty("compose.interop.blending") == null) System.setProperty("compose.interop.blending", "true")
    GpuMapHost.surfaceHost = { modifier, onSurface, onMouse -> MapSurfacePanel(modifier, onSurface, onMouse) }
    GpuMapHost.surfaceOpener = { scenario, surface -> openDirectGpuMap(scenario, surface as Canvas) }
    GpuMapHost.surfaceUsable = ::directSurfaceUsable
    GpuMapHost.directDefault = { System.getenv("CW_DIRECT_MAP") == "1" || (onWindows && RenderInfo.backend == "DIRECT3D") }
}
