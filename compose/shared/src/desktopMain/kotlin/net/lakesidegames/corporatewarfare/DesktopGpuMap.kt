package net.lakesidegames.corporatewarfare

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import com.sun.jna.Library
import com.sun.jna.Native
import org.jetbrains.skia.ColorAlphaType
import org.jetbrains.skia.ColorType
import org.jetbrains.skia.Image
import org.jetbrains.skia.ImageInfo
import java.io.File
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.thread
import kotlin.concurrent.withLock

/**
 * The engine's GPU map entry points (crates/cw-ffi/src/gpu_map.rs), a plain C interface next to the
 * uniffi one in the same library. `size_t` is a Long: the desktop builds are 64-bit only.
 */
@Suppress("FunctionName")
internal interface CwGpu : Library {
    fun cw_gpu_open(assets: String, scenario: String): Long
    fun cw_gpu_last_error(buf: ByteArray, len: Long): Int
    fun cw_gpu_info(id: Long, buf: ByteArray, len: Long): Int
    fun cw_gpu_update(id: Long, json: String): Int
    fun cw_gpu_render(id: Long, cam: DoubleArray, width: Int, height: Int, scale: Int, out: ByteArray, outLen: Long, timing: DoubleArray): Int
    fun cw_gpu_project(id: Long, cam: DoubleArray, width: Float, height: Float, pts: DoubleArray, n: Long, out: FloatArray): Int
    fun cw_gpu_pick(id: Long, cam: DoubleArray, width: Float, height: Float, px: Float, py: Float, buf: ByteArray, len: Long): Int
    fun cw_gpu_buildings(id: Long, buf: ByteArray, len: Long): Int
    fun cw_gpu_building_rects(id: Long, cam: DoubleArray, width: Float, height: Float, out: FloatArray, max: Long): Int
    fun cw_gpu_close(id: Long)

    companion object {
        val instance: CwGpu by lazy { Native.load("cw_ffi", CwGpu::class.java) }
    }
}

/** Files the renderer reads by name, copied once from the app's resources into a folder it can open. */
private fun gpuAssetFolder(scenario: String): File {
    val dir = File(System.getProperty("java.io.tmpdir"), "cw-gpu-map-${BuildInfo.VERSION}")
    dir.mkdirs()
    val names = listOf(
        "world-map.json", "voxel-models.json", "earth-$scenario.jpg", "clouds-$scenario.webp",
        "heightmap-$scenario.cwh", "urban-$scenario.json",
    )
    val loader = Thread.currentThread().contextClassLoader ?: GpuMapHost::class.java.classLoader
    for (name in names) {
        val target = File(dir, name)
        val bytes = loader.getResourceAsStream(name)?.use { it.readBytes() }
        if (bytes == null) {
            // The city outlines are optional; any other file missing means this build cannot draw the map.
            if (name.startsWith("urban-")) continue
            throw IllegalStateException("the map file $name is missing")
        }
        if (!target.exists() || target.length() != bytes.size.toLong()) target.writeBytes(bytes)
    }
    return dir
}

private fun CwGpu.text(call: (ByteArray) -> Int): String? {
    var buf = ByteArray(256)
    var n = call(buf)
    if (n < 0 && -n > buf.size) { buf = ByteArray(-n); n = call(buf) }
    return if (n >= 0) String(buf, 0, n, Charsets.UTF_8) else null
}

/** Opens the GPU map for a scenario, or throws why it cannot (no adapter, a driver fault, missing files). */
internal fun openDesktopGpuMap(scenario: String): GpuMapSession {
    val lib = CwGpu.instance
    val id = lib.cw_gpu_open(gpuAssetFolder(scenario).absolutePath, scenario)
    if (id == 0L) throw IllegalStateException(lib.text { lib.cw_gpu_last_error(it, it.size.toLong()) } ?: "no graphics adapter")
    val info = lib.text { lib.cw_gpu_info(id, it, it.size.toLong()) }?.split('|') ?: listOf("unknown", "", "0")
    return DesktopGpuSession(lib, id, info.getOrElse(0) { "unknown" }, info.getOrElse(2) { "0" } == "1")
}

/**
 * One open map. A dedicated thread draws the newest requested view on the graphics card and reads it back into one of two
 * reusable buffers; a second thread turns the previous buffer into the picture the UI draws, so that copy never holds up the next
 * frame. Every frame is the view plus an overscan margin on each side (so a pan or a small zoom out shows real map), at the view's
 * real pixel size when the camera is still and at three quarters of it (or full size on small views) while it moves; the full-size
 * frame follows within [SETTLE_NANOS] of the last change. The UI warps whichever frame is newest to the live camera.
 */
private class DesktopGpuSession(
    private val lib: CwGpu,
    private val id: Long,
    override val adapter: String,
    override val software: Boolean,
) : GpuMapSession {
    private class Request(val camera: GpuCamera, val width: Int, val height: Int)

    private val lock = ReentrantLock()
    private val wake = lock.newCondition()
    private var wanted: Request? = null
    private var rendered: Request? = null
    private var renderedFactor = 0f
    private var lastChange = 0L
    private var dirty = false
    private var closed = false
    private val buffers = arrayOf(ByteArray(0), ByteArray(0))
    private val pending = arrayOfNulls<java.util.concurrent.Future<*>>(2)
    private val uploader = java.util.concurrent.Executors.newSingleThreadExecutor { r -> Thread(r, "cw-gpu-upload").apply { isDaemon = true } }

    @Volatile override var renderMs = 0.0
    @Volatile override var copyMs = 0.0
    @Volatile private var latest: GpuShown? = null
    override val frame: ImageBitmap? get() = latest?.image
    override val frameCamera: GpuCamera? get() = latest?.camera
    override val shown: GpuShown? get() = latest
    override var version by mutableLongStateOf(0L)
        private set

    private val worker = thread(name = "cw-gpu-map", isDaemon = true) { loop() }

    @Volatile private var placed: List<GpuBuilding> = emptyList()

    override fun update(json: String) {
        val code = lib.cw_gpu_update(id, json)
        if (code < 0) return
        placed = readBuildings()
        lock.withLock { dirty = true; wake.signalAll() }
    }

    override fun buildings(): List<GpuBuilding> = placed

    override fun buildingRects(camera: GpuCamera, width: Int, height: Int): FloatArray {
        val out = FloatArray(placed.size * 4) { Float.NaN }
        if (placed.isNotEmpty()) lib.cw_gpu_building_rects(id, cam(camera), width.toFloat(), height.toFloat(), out, placed.size.toLong())
        return out
    }

    private fun readBuildings(): List<GpuBuilding> {
        val text = lib.text { lib.cw_gpu_buildings(id, it, it.size.toLong()) } ?: return emptyList()
        return runCatching {
            kotlinx.serialization.json.Json.parseToJsonElement(text).let { it as kotlinx.serialization.json.JsonArray }.map { e ->
                val o = e as kotlinx.serialization.json.JsonObject
                fun str(k: String) = (o[k] as kotlinx.serialization.json.JsonPrimitive).content
                fun opt(k: String) = (o[k] as? kotlinx.serialization.json.JsonPrimitive)?.takeIf { it !is kotlinx.serialization.json.JsonNull }?.content
                GpuBuilding(
                    str("lon").toDouble(), str("lat").toDouble(), str("owner"), str("kind"), (0xFF000000L or str("colour").toLong(16)).toInt(),
                    region = opt("region"), industry = opt("industry"), firm = opt("firm")?.toLongOrNull(), plants = opt("plants")?.toIntOrNull() ?: 0,
                )
            }
        }.getOrDefault(emptyList())
    }

    override fun request(camera: GpuCamera, width: Int, height: Int, moving: Boolean) {
        if (width <= 0 || height <= 0) return
        lock.withLock {
            val old = wanted
            if (old == null || old.camera != camera || old.width != width || old.height != height) {
                wanted = Request(camera, width, height)
                lastChange = System.nanoTime()
                wake.signalAll()
            }
        }
    }

    override fun project(camera: GpuCamera, width: Int, height: Int, lonLat: DoubleArray): FloatArray {
        val out = FloatArray(lonLat.size)
        lib.cw_gpu_project(id, cam(camera), width.toFloat(), height.toFloat(), lonLat, (lonLat.size / 2).toLong(), out)
        return out
    }

    override fun pick(camera: GpuCamera, width: Int, height: Int, x: Float, y: Float): String? {
        val buf = ByteArray(64)
        val n = lib.cw_gpu_pick(id, cam(camera), width.toFloat(), height.toFloat(), x, y, buf, buf.size.toLong())
        return if (n > 0) String(buf, 0, n, Charsets.UTF_8) else null
    }

    override fun close() {
        lock.withLock { closed = true; wake.signalAll() }
        worker.join(2000)
        uploader.shutdown()
        lib.cw_gpu_close(id)
    }

    private fun cam(c: GpuCamera) = doubleArrayOf(c.lon, c.lat, c.widthKm, c.tiltDeg, c.headingDeg, c.fovDeg)

    /** Image pixels per view pixel: full size when settled or when the view is small, three quarters while it moves. */
    private fun factorFor(r: Request, settled: Boolean) = if (settled || r.width.toLong() * r.height <= SMALL_VIEW) 1f else MOVING_FACTOR

    private fun loop() {
        val timing = DoubleArray(6)
        var next = 0
        while (true) {
            val job: Request
            val factor: Float
            lock.withLock {
                while (true) {
                    if (closed) return
                    val w = wanted
                    val settled = System.nanoTime() - lastChange > SETTLE_NANOS
                    if (w != null) {
                        val f = factorFor(w, settled)
                        if (dirty || rendered !== w || f > renderedFactor) { job = w; factor = f; dirty = false; break }
                    }
                    // Still showing a reduced frame: wake when the camera has been still long enough to draw the full one.
                    val wait = if (renderedFactor in 0.01f..0.99f) (SETTLE_NANOS - (System.nanoTime() - lastChange)).coerceAtLeast(1_000_000L) else 1_000_000_000L
                    wake.awaitNanos(wait)
                }
            }
            // The margin shrinks on very large views so a frame never passes what a card takes (and stays quick).
            val margin = if (job.width.toLong() * job.height > BIG_VIEW) 0.1 else GPU_OVERSCAN
            val mx = Math.round(job.width * margin).toInt(); val my = Math.round(job.height * margin).toInt()
            val bigW = job.width + 2 * mx; val bigH = job.height + 2 * my
            val rw = Math.round(bigW * factor).coerceAtLeast(1); val rh = Math.round(bigH * factor).coerceAtLeast(1)
            val camera = overscanCamera(job.camera, job.width, job.height, mx, my)
            // The buffer the uploader may still be reading is waited for: the one rendered into now is the one used two frames ago.
            pending[next]?.let { runCatching { it.get() } }
            val need = rw * rh * 4
            if (buffers[next].size < need) buffers[next] = ByteArray(need)
            val pixels = buffers[next]
            val code = try {
                lib.cw_gpu_render(id, cam(camera), rw, rh, 1, pixels, need.toLong(), timing)
            } catch (_: Throwable) { -1 }
            if (code < 0) {
                GpuMapHost.failure = lib.text { lib.cw_gpu_last_error(it, it.size.toLong()) }?.take(80) ?: "the graphics card stopped drawing"
                lock.withLock { if (!closed) wake.awaitNanos(500_000_000L) }
                continue
            }
            renderMs = timing[0]
            val gpuCopy = timing[1]
            lock.withLock { rendered = job; renderedFactor = factor }
            val view = job.camera
            pending[next] = uploader.submit {
                val t0 = System.nanoTime()
                val image = Image.makeRaster(ImageInfo(rw, rh, ColorType.RGBA_8888, ColorAlphaType.OPAQUE), pixels, rw * 4).toComposeImageBitmap()
                copyMs = gpuCopy + (System.nanoTime() - t0) / 1e6
                latest = GpuShown(image, camera, view, job.width, job.height, mx, factor)
                version += 1
            }
            next = 1 - next
        }
    }

    private companion object {
        /** How long the camera must be still before the full-size frame replaces the quick one. */
        const val SETTLE_NANOS = 150_000_000L
        /** Views up to this many pixels are drawn at full size even while moving. */
        const val SMALL_VIEW = 1_500_000L
        const val MOVING_FACTOR = 0.75f
        /** Views above this many pixels (beyond 4K) get a smaller overscan margin. */
        const val BIG_VIEW = 8_000_000L
    }
}

/**
 * CW_GPU_PROBE=1 starts the game headless to try the new map: open it, draw one frame at 1920x1080
 * (a quick half-size one, then the full one), print the frame counter line and exit. The CI build
 * of the release app runs it, so the packaged libraries, the shrunk classes and the map files are
 * proven together; the exit code is 0 only when the GPU map drew.
 */
internal fun gpuProbe(): Int {
    return try {
        val session = openDesktopGpuMap("great-lakes-2000")
        try {
            session.update(gpuUpdateJson(emptyList(), emptyList(), MapLayer.Presence, emptyMap(), night = false, clouds = 2))
            val map = WorldMap.forScenario("great-lakes-2000") ?: error("the world map did not load")
            val size = androidx.compose.ui.geometry.Size(1920f, 1080f)
            val camera = gpuCamera(MapCamera.scenario(size, map), size.width, size.height, Math.toRadians(28.0))
            session.request(camera, 1920, 1080, true)
            val deadline = System.currentTimeMillis() + 120_000
            while ((session.frame?.width ?: 0) < 1920 && System.currentTimeMillis() < deadline) Thread.sleep(50)
            if ((session.frame?.width ?: 0) < 1920) error("no frame within 120 seconds")
            System.err.println(gpuMapLine(true, session))
            0
        } finally {
            session.close()
        }
    } catch (e: Throwable) {
        System.err.println("map: classic, no GPU (${(e.message ?: e.javaClass.simpleName).take(120)})")
        3
    }
}
