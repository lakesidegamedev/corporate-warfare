package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.geometry.isSpecified
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.graphicsLayer
import kotlin.math.PI
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.sin
import kotlin.math.sqrt
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.cw_ffi.MapDivisionDto
import uniffi.cw_ffi.RegionBoardDto
import kotlin.math.exp
import kotlin.math.max
import kotlin.math.min

enum class MapLayer(val title: String) { Presence("Your divisions"), Room("Room to build"), Wages("Wages"), Freight("Freight") }

/** Seconds since the app started, shared by every moving thing so they stay in step. */
internal object MapClock {
    private val start = System.nanoTime()
    fun now(): Double = (System.nanoTime() - start) / 1e9
}

/**
 * The time for moving effects, updated on display frames but no faster than [fps] (0 for every
 * frame). Nothing runs while [active] is false.
 */
@Composable
internal fun rememberFrameSeconds(active: Boolean, fps: Int): State<Double> {
    val seconds = remember { mutableDoubleStateOf(MapClock.now()) }
    LaunchedEffect(active, fps) {
        if (!active) return@LaunchedEffect
        var last = 0L
        val gap = if (fps > 0) (1e9 / fps * 0.9).toLong() else 0L
        while (true) {
            withFrameNanos { now ->
                if (now - last >= gap) {
                    last = now
                    seconds.doubleValue = MapClock.now()
                }
            }
        }
    }
    return seconds
}

/**
 * Time for the world on the map: it stands still while the game is paused and runs one, two or
 * four times as fast at the three speeds, so moving things follow the game rather than the wall.
 */
internal class WorldClock {
    private var last = Double.NaN
    private var seconds = 0.0

    fun advance(wall: Double, running: Boolean, speed: Int): Double {
        if (!last.isNaN() && running) seconds += (wall - last).coerceIn(0.0, 0.5) * when (speed) { 1 -> 1.0; 2 -> 2.0; else -> 4.0 }
        last = wall
        return seconds
    }
}

/** Region heights ease to a new layer's values instead of jumping. */
@Composable
private fun animatedHeights(target: Map<String, Double>, animate: Boolean): Map<String, Double> {
    var from by remember { mutableStateOf(target) }
    var to by remember { mutableStateOf(target) }
    val progress = remember { Animatable(1f) }
    LaunchedEffect(target) {
        if (target == to) return@LaunchedEffect
        from = blendHeights(from, to, progress.value)
        to = target
        if (!animate) {
            progress.snapTo(1f)
            return@LaunchedEffect
        }
        progress.snapTo(0f)
        progress.animateTo(1f, tween(600, easing = FastOutSlowInEasing))
    }
    val f = progress.value
    return if (f >= 1f) to else blendHeights(from, to, f)
}

private fun blendHeights(a: Map<String, Double>, b: Map<String, Double>, f: Float): Map<String, Double> =
    (a.keys + b.keys).associateWith { (a[it] ?: 0.0) + ((b[it] ?: 0.0) - (a[it] ?: 0.0)) * f }

/**
 * The world map with its own camera and zoom buttons: a globe when zoomed out that flattens onto
 * the scenario as you zoom in, tilting into 3D. Tapping a region reports it through [onSelect].
 * The still parts are drawn in their own layer and only redrawn when the view changes; moving
 * effects are drawn over them each frame.
 */
@Composable
fun WorldMapView(
    store: GameStore, layer: MapLayer, selected: String?, onSelect: (String?) -> Unit, modifier: Modifier = Modifier,
    pickedLane: String? = null, onLane: ((String?) -> Unit)? = null, controlsAtTop: Boolean = false,
) {
    val map = WorldMap.forRegions(store.boards.map { it.code })
    var camera by remember(map) { mutableStateOf<MapCamera?>(null) }
    var viewSize by remember { mutableStateOf(Size.Zero) }
    val scope = rememberCoroutineScope()
    var flight by remember { mutableStateOf<Job?>(null) }
    // Big enough for every label on screen, so a moving camera never lays text out again.
    val measurer = rememberTextMeasurer(cacheSize = 160)
    val graphics = LocalAppSettings.current?.graphics ?: GraphicsConfig.Default
    var tilted by remember { mutableStateOf(true) }
    var pitch by remember { mutableDoubleStateOf(graphics.pitch) }
    val pitchTarget = if (tilted) graphics.pitch else 0.0
    LaunchedEffect(pitchTarget) {
        if (!graphics.animations) {
            pitch = pitchTarget
            return@LaunchedEffect
        }
        val start = pitch
        val frames = 30
        for (frame in 1..frames) {
            val x = frame / frames.toDouble()
            pitch = start + (pitchTarget - start) * x * x * (3 - 2 * x)
            delay(16)
        }
    }
    val geometry = remember(map) { MapGeometryCache() }
    val shaded = remember(layer, store.boards, store.mapDivisions) { shading(store, layer) }
    val heights = animatedHeights(if (layer == MapLayer.Freight) emptyMap() else shaded, graphics.animations)
    val clock = rememberFrameSeconds(graphics.moves || graphics.showFps, graphics.frameRate)
    // The world's own clock: traffic, planes, clouds and the sun stop when the game is paused and
    // run faster at higher speeds. Only interface feedback (the selection glow) uses the wall clock.
    val world = remember { WorldClock() }
    val gameSeconds by remember { derivedStateOf { world.advance(clock.value, store.running, store.speed) } }
    val sunStep by remember(graphics.dayNight) { derivedStateOf { if (graphics.dayNight) floor(gameSeconds * 3) else 0.0 } }
    val cloudTick by remember { derivedStateOf { floor(gameSeconds * 8) / 8 } }
    val week = store.overview?.weekLabel
    var rippleAt by remember { mutableDoubleStateOf(-10.0) }
    LaunchedEffect(week) { if (week != null) rippleAt = MapClock.now() }
    val fps = remember { FpsMeter() }
    val heightsNow by rememberUpdatedState(heights)

    fun current(): MapCamera? = camera ?: map?.let { if (viewSize.width > 0) MapCamera.scenario(viewSize, it) else null }
    fun fly(target: MapCamera) {
        val start = current() ?: return
        flight?.cancel()
        if (!graphics.animations) {
            camera = target
            return
        }
        flight = scope.launch {
            val frames = 42
            for (frame in 1..frames) {
                val x = frame / frames.toDouble()
                camera = MapCamera.interpolate(start, target, x * x * (3 - 2 * x))
                delay(16)
            }
        }
    }
    fun toggleGlobe() {
        val now = current() ?: return
        fly(if (now.zoom > MapCamera.FLAT_START) now.copy(zoom = 1.0) else MapCamera.scenario(viewSize, map ?: return))
    }

    Box(modifier.clipToBounds().background(Palette.ink)) {
        if (map == null) {
            Text("The map file is missing from this build.", color = Palette.secondary, modifier = Modifier.padding(24.dp))
        } else {
            val sun = if (graphics.dayNight) Sun.moving(sunStep / 3.0) else null
            Canvas(
                Modifier.fillMaxSize().graphicsLayer()
                    .onSizeChanged { viewSize = Size(it.width.toFloat(), it.height.toFloat()) }
                    .pointerInput(Unit) {
                        detectTransformGestures { _, panChange, zoomChange, rotationChange ->
                            flight?.cancel()
                            val now = current() ?: return@detectTransformGestures
                            // A two-finger twist turns the map; drags go any way, almost to the poles.
                            camera = now.zoomed(zoomChange.toDouble()).rotated(rotationChange.toDouble()).panned(panChange, viewSize, pitch)
                        }
                    }
                    .pointerInput(Unit) {
                        awaitPointerEventScope {
                            while (true) {
                                val event = awaitPointerEvent()
                                // On desktop, dragging with the right button turns the map.
                                if (event.type == PointerEventType.Move && event.buttons.isSecondaryPressed) {
                                    val dx = event.changes.firstOrNull()?.let { it.position.x - it.previousPosition.x } ?: 0f
                                    val now = current()
                                    if (now != null && dx != 0f) {
                                        flight?.cancel()
                                        camera = now.rotated(dx * 0.4)
                                    }
                                    event.changes.forEach { it.consume() }
                                }
                                if (event.type == PointerEventType.Scroll) {
                                    val dy = event.changes.firstOrNull()?.scrollDelta?.y ?: 0f
                                    val now = current()
                                    if (now != null && dy != 0f) {
                                        flight?.cancel()
                                        camera = now.zoomed(exp(-dy * 0.15))
                                    }
                                    event.changes.forEach { it.consume() }
                                }
                            }
                        }
                    }
                    .pointerInput(layer, pickedLane, selected, pitch) {
                        detectTapGestures(
                            onDoubleTap = { toggleGlobe() },
                            onTap = { tap ->
                                val now = current() ?: return@detectTapGestures
                                val g = geometry.of(map, now, viewSize, graphics.detail, pitch)
                                val projection = g.p
                                if (layer == MapLayer.Freight && onLane != null) {
                                    val lane = laneAt(map, projection, store.lanes, tap)
                                    if (lane != null) {
                                        onLane(if (laneKey(lane) == pickedLane) null else laneKey(lane))
                                        return@detectTapGestures
                                    }
                                    onLane(null)
                                }
                                onSelect(regionAt(map, g, graphics, heightsNow, tap).takeIf { it != selected })
                            },
                        )
                    },
            ) {
                drawWorldMap(
                    map, camera ?: MapCamera.scenario(size, map), layerTint(layer), shaded, selected, store.mapDivisions, measurer,
                    lanes = store.lanes, freightLayer = layer == MapLayer.Freight, pickedLane = pickedLane,
                    geometry = geometry, graphics = graphics, pitch = pitch, sun = sun, heights = heights, phase = MapPhase.Ground,
                )
            }
            // Layers, bottom to top: the ground, traffic on it, clouds, buildings and labels, then
            // effects. Traffic runs under the buildings; buildings stand clear of the clouds.
            if (graphics.moves) {
                Canvas(Modifier.fillMaxSize()) {
                    val cam = camera ?: MapCamera.scenario(size, map)
                    val g = geometry.of(map, cam, size, graphics.detail, pitch)
                    val o = g.overlay(store.lanes, store.mapDivisions, selected, heights, graphics, layer == MapLayer.Freight, density)
                    val detailFade = if (graphics.detail != MapDetail.High) 0f else ((cam.zoom - 7) / 5).coerceIn(0.0, 1.0).toFloat()
                    drawMapOverlay(
                        o, graphics, gameSeconds, layer == MapLayer.Freight, sun ?: Sun.fixed(g.p), map.playBounds, cam.zoom,
                        clock.value - rippleAt, detailFade, map.scenarioId, part = OverlayPart.Traffic,
                    )
                }
            }
            // Clouds drift a fraction of a degree a second, so their layer is redrawn eight times a
            // second rather than every frame.
            if (graphics.clouds > 0) {
                Canvas(Modifier.fillMaxSize().graphicsLayer()) {
                    val seconds = cloudTick
                    val cam = camera ?: MapCamera.scenario(size, map)
                    val g = geometry.of(map, cam, size, graphics.detail, pitch)
                    with(Weather) {
                        drawWeather(g.p, sun ?: Sun.fixed(g.p), seconds, graphics.clouds, map.playBounds, 46.0 * g.scale * g.p.t, cam.zoom, graphics.lighting > 0, map.scenarioId)
                    }
                }
            }
            Canvas(Modifier.fillMaxSize().graphicsLayer()) {
                drawWorldMap(
                    map, camera ?: MapCamera.scenario(size, map), layerTint(layer), shaded, selected, store.mapDivisions, measurer,
                    lanes = store.lanes, freightLayer = layer == MapLayer.Freight, pickedLane = pickedLane,
                    geometry = geometry, graphics = graphics, pitch = pitch, sun = sun, heights = heights, phase = MapPhase.Structures,
                    plants = store.mapPlants,
                )
            }
            Canvas(Modifier.fillMaxSize()) {
                val cam = camera ?: MapCamera.scenario(size, map)
                val g = geometry.of(map, cam, size, graphics.detail, pitch)
                if (graphics.moves) {
                    val o = g.overlay(store.lanes, store.mapDivisions, selected, heights, graphics, layer == MapLayer.Freight, density)
                    val detailFade = if (graphics.detail != MapDetail.High) 0f else ((cam.zoom - 7) / 5).coerceIn(0.0, 1.0).toFloat()
                    drawMapOverlay(
                        o, graphics, gameSeconds, layer == MapLayer.Freight, sun ?: Sun.fixed(g.p), map.playBounds, cam.zoom,
                        clock.value - rippleAt, detailFade, map.scenarioId, part = OverlayPart.Effects, wallSeconds = clock.value,
                    )
                } else {
                    g.overlay(store.lanes, store.mapDivisions, selected, heights, graphics, layer == MapLayer.Freight, density).selectedTop?.let {
                        drawPath(it, Palette.gold, style = Stroke(2.5f))
                    }
                }
                if (graphics.showFps) {
                    val rate = fps.tick(System.nanoTime())
                    val label = measurer.measure("${rate.toInt()} fps", TextStyle(color = Palette.positive, fontSize = 11.sp, fontWeight = FontWeight.Bold))
                    drawRect(Color.Black.copy(alpha = 0.55f), Offset(6f, size.height - label.size.height - 14f), Size(label.size.width + 12f, label.size.height + 8f))
                    drawText(label, topLeft = Offset(12f, size.height - label.size.height - 10f))
                }
            }
            // On desktop the tutorial card sits in the bottom corner, so the controls go to the top.
            Column(
                Modifier.align(if (controlsAtTop) Alignment.TopEnd else Alignment.BottomEnd)
                    .padding(end = 12.dp, top = if (controlsAtTop) 12.dp else 0.dp, bottom = if (controlsAtTop) 0.dp else 24.dp)
                    .clip(Pixel.shape).background(Palette.panel.copy(alpha = 0.94f)),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val zoomedIn = (current()?.zoom ?: 1.0) > MapCamera.FLAT_START
                MapButton("zoom_in") { current()?.let { fly(it.zoomed(1.8)) } }
                MapButton("zoom_out") { current()?.let { fly(it.zoomed(1 / 1.8)) } }
                MapButton(if (zoomedIn) "globe" else "scope") { toggleGlobe() }
                // Clouds in one tap: heavy, light, thin, off, and round again.
                val settings = LocalAppSettings.current
                if (settings != null) {
                    Box(
                        Modifier.size(40.dp).clickable { settings.chooseGraphics(graphics.copy(clouds = (graphics.clouds + 3) % 4)) },
                        contentAlignment = Alignment.Center,
                    ) {
                        PixelIcon(if (graphics.clouds == 0) "cloud_off" else "cloud", size = 22.dp, tint = if (graphics.clouds == 0) null else Palette.primaryText.copy(alpha = 0.4f + 0.2f * graphics.clouds))
                    }
                }
                // Turned away from north: one tap turns it back.
                val bearing = current()?.bearing ?: 0.0
                if (bearing > 0.5 && bearing < 359.5) {
                    Box(Modifier.size(40.dp).clickable { current()?.let { fly(it.copy(bearing = 0.0)) } }, contentAlignment = Alignment.Center) {
                        Text("N", color = Palette.gold, fontSize = 13.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
                    }
                }
                if (graphics.tilt > 0) {
                    Box(Modifier.size(40.dp).clickable { tilted = !tilted }, contentAlignment = Alignment.Center) {
                        Text("3D", color = if (tilted) Palette.gold else Palette.secondary, fontSize = 13.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

/** The region under a tap: raised tiles nearest the viewer first, so a tall tile catches its top. */
private fun regionAt(map: WorldMap, g: MapGeometry, graphics: GraphicsConfig, heights: Map<String, Double>, tap: Offset): String? {
    if (graphics.extrude && g.p.t > 0.05) {
        g.raised(heights).asReversed().forEach { solid ->
            val region = map.regions.firstOrNull { it.code == solid.code } ?: return@forEach
            if (g.p.contains(region.rings, tap, solid.h)) return solid.code
        }
    }
    return map.regions.firstOrNull { g.p.contains(it.rings, tap) }?.code
}

/** The phone map: the world map with the company HUD and layer chips over it. */
@Composable
fun MapScreen(store: GameStore) {
    var layer by remember { mutableStateOf(MapLayer.Presence) }
    var selected by remember { mutableStateOf<String?>(null) }
    var pickedLane by remember { mutableStateOf<String?>(null) }
    var building by remember { mutableStateOf<RegionBoardDto?>(null) }
    Box(Modifier.fillMaxSize().background(Palette.ink)) {
        WorldMapView(store, layer, selected, { selected = it }, Modifier.fillMaxSize(), pickedLane, { pickedLane = it })

        Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            MapHud(store)
            Row(
                Modifier.horizontalScroll(androidx.compose.foundation.rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                MapLayer.entries.forEach { option ->
                    Text(
                        option.title,
                        color = if (layer == option) Palette.ink else Palette.primaryText,
                        fontSize = 12.sp,
                        fontFamily = Pixel.font,
                        modifier = Modifier.clip(Pixel.small)
                            .background(if (layer == option) Palette.gold else Palette.panel)
                            .clickable { layer = option }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }
            }
        }
    }

    if (layer == MapLayer.Freight) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
            Column(
                Modifier.fillMaxWidth().padding(8.dp).clip(Pixel.shape).background(Palette.panel.copy(alpha = 0.96f)),
            ) { FreightPanel(store, selected, pickedLane, { pickedLane = it }) }
        }
    }

    selected?.let { code ->
        val board = store.boards.firstOrNull { it.code == code }
        if (board != null && layer != MapLayer.Freight) RegionSheet(store, board, onBuild = { building = board; selected = null }, onClose = { selected = null })
    }
    building?.let { board -> BuildDialog(store, board) { building = null } }
}

/**
 * Draws the still parts of the world through the blended, tilted projection: space, the lit
 * globe, land, the raised scenario, night, buildings, cities and labels. Moving effects are drawn
 * over it by [drawMapOverlay].
 */
internal fun DrawScope.drawWorldMap(
    map: WorldMap,
    cam: MapCamera,
    tint: Color,
    shading: Map<String, Double>,
    selected: String?,
    divisions: List<MapDivisionDto>,
    measurer: TextMeasurer,
    space: Boolean = true,
    lanes: List<uniffi.cw_ffi.LaneDto> = emptyList(),
    freightLayer: Boolean = lanes.isNotEmpty(),
    pickedLane: String? = null,
    geometry: MapGeometryCache = MapGeometryCache(),
    graphics: GraphicsConfig = GraphicsConfig.Default,
    pitch: Double = graphics.pitch,
    sun: Sun? = null,
    heights: Map<String, Double> = shading,
    phase: MapPhase = MapPhase.All,
    plants: List<uniffi.cw_ffi.MapPlantDto> = emptyList(),
) {
    MapProfiler.start()
    val detailLevel = graphics.detail
    val g = geometry.of(map, cam, size, detailLevel, pitch)
    val p = g.p
    val t = p.t
    val light = sun ?: Sun.fixed(p)
    val lit = graphics.lighting > 0
    val detail = if (detailLevel != MapDetail.High) 0f else ((cam.zoom - 7) / 5).coerceIn(0.0, 1.0).toFloat()
    val raised = graphics.extrude && t > 0.05
    val regionPaths = g.regionPaths
    // How far up the base of the raised scenario sits on screen, for lines drawn on it.
    val plinth = if (raised) g.plinth else 0.0
    val up = p.lift(0.0, 0.0, plinth).y - p.lift(0.0, 0.0, 0.0).y
    val water = Color(0xFF16354F)
    val ground = if (raised) heights else FlatGround
    // The ground: everything that lies flat, under the traffic and the buildings.
    if (phase != MapPhase.Structures) {
        if (space) {
            drawRect(Color(0xFF05090E))
            if (graphics.lighting == 2 && t < 1) SharedStars.draw(this, 0.0, 0f, false)
        }
        if (lit && t < 1) geometry.back.draw(this, listOf(g, light, graphics.lighting)) { drawAtmosphere(p, light, graphics) }
        drawGlobeBack(p, light, graphics)
        MapProfiler.mark("back")
        if (t < 1) drawPath(g.graticule, Palette.primaryText.copy(alpha = (0.06 * (1 - t)).toFloat()), style = Stroke(0.6f))
        // Outside the scenario the world is muted: climate-tinted land on the globe that greys as the
        // map flattens, grey borders and roads. The scenario's regions are drawn over it in colour.
        if (lit) {
            // Shallow water: a lighter band of sea hugging every coast, under the land.
            val shallows = (2 + cam.zoom * 0.35).coerceIn(2.0, 9.0).toFloat()
            g.biomes.forEach { drawPath(it, Color(0xFF2F6F9A).copy(alpha = 0.30f), style = Stroke(shallows)) }
            drawBiomes(g.biomes, (0.25 + 0.6 * t).toFloat())
        } else drawPath(g.land, OutsideLand)
        MapProfiler.mark("landfill")
        val provinces = if (detailLevel == MapDetail.Low) 0f else ((cam.zoom - 1.4) / 1.2).coerceIn(0.0, 1.0).toFloat()
        if (provinces > 0) drawPath(g.provinces, OutsideBorder.copy(alpha = 0.7f * provinces), style = Stroke(0.5f))
        val roads = if (detailLevel != MapDetail.High) 0f else ((cam.zoom - 2.4) / 1.6).coerceIn(0.0, 1.0).toFloat()
        if (roads > 0) drawPath(g.roads, OutsideRoad.copy(alpha = 0.6f * roads), style = Stroke(0.8f))
        // Lit, the climate paths already hold every country, so the plain land outline is never built.
        if (lit) g.biomes.forEach { drawPath(it, OutsideBorder, style = Stroke(0.8f)) } else drawPath(g.land, OutsideBorder, style = Stroke(0.8f))
        MapProfiler.mark("landstroke")
        // Lit and flat, the world outside the scenario falls into shade so the scenario stands out.
        if (lit && t > 0.5) drawRect(Color.Black.copy(alpha = (0.18 * t).toFloat()))
        MapProfiler.mark("veil")
        if (raised) {
            drawPath(g.lakes, water)
            val solids = g.raised(heights)
            // Shadows first, so no tile's shadow falls across a tile drawn before it.
            if (lit) drawPath(g.shadows(heights), Color.Black.copy(alpha = 0.22f))
            solids.forEach { solid ->
                val intensity = (shading[solid.code] ?: 0.0).toFloat()
                val top = if (intensity > 0) lerp(RegionLand, tint, (0.15f + 0.6f * intensity).coerceAtMost(1f)) else RegionLand
                drawPath(solid.walls[0], top.shade(0.50f))
                drawPath(solid.walls[1], top.shade(0.68f))
                drawPath(solid.walls[2], top.shade(0.88f))
                drawPath(solid.top, top)
                // Tops catch the light from the upper left and fall off away from it.
                if (lit) {
                    val b = solid.bounds
                    drawPath(solid.top, Brush.linearGradient(0f to Color.White.copy(alpha = 0.09f), 0.5f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.12f), start = b.topLeft, end = b.bottomRight))
                }
                if (solid.code == selected) drawPath(solid.top, Palette.gold.copy(alpha = 0.35f))
                drawPath(solid.top, if (lit) Color(0xFF9FB4C8).copy(alpha = 0.55f) else Palette.stroke, style = Stroke(1f))
            }
            if (detail > 0) translate(top = up) { drawPath(g.counties, Palette.stroke.copy(alpha = 0.45f * detail), style = Stroke(0.5f)) }
        } else {
            regionPaths.forEach { (code, path) ->
                drawPath(path, RegionLand)
                val intensity = shading[code] ?: 0.0
                if (intensity > 0) drawPath(path, tint.copy(alpha = (0.15 + 0.6 * intensity).toFloat()))
            }
            if (detail > 0) drawPath(g.counties, Palette.stroke.copy(alpha = 0.45f * detail), style = Stroke(0.5f))
            drawPath(g.lakes, water)
        }
        MapProfiler.mark("regions")
        translate(top = up) {
            if (detail > 0) {
                drawPath(g.rivers, water.copy(alpha = 0.9f * detail), style = Stroke(1.4f))
                drawPath(g.interstates, Color(0xFFC68A3D).copy(alpha = 0.35f * detail), style = Stroke(1f))
            }
            if (!raised) {
                regionPaths.forEach { (code, path) ->
                    if (code == selected) drawPath(path, Palette.gold.copy(alpha = 0.35f))
                    drawPath(path, Palette.stroke, style = Stroke(0.9f))
                }
            }
            g.playStates.forEach {
                // A cool rim light along the scenario's states when lit.
                if (lit && t > 0.5) drawPath(it, Color(0xFF8EC5FF).copy(alpha = 0.10f), style = Stroke(4f))
                drawPath(it, Color(0xFFD2DEEA).copy(alpha = 0.85f), style = Stroke(if (cam.zoom > 3) 2f else 1f))
            }
        }
        // Night falls on the globe whenever it is lit, and on the flat map with day and night on.
        // With the globe's shading, the haze and the vignette it is soft, so it all goes in one small
        // light layer, clipped to the globe while there is one so its edge stays sharp.
        if (lit) {
            val night = t < 1 || sun != null
            val drawLight = {
                geometry.front.draw(this, listOf(g, light, graphics.lighting, night)) { full ->
                    if (night) drawNight(g.night(light))
                    drawGlobeFront(p, light, graphics)
                    if (t > 0.5) drawFlatLight(p, full)
                }
            }
            if (t < 0.5) clipPath(Path().apply { addOval(Rect(Offset(p.cx.toFloat(), p.cy.toFloat()), p.r.toFloat())) }) { drawLight() } else drawLight()
        }
        MapProfiler.mark("night")
        MapProfiler.mark("front")
        if (t < 1 && !lit) {
            // The globe's rim, fading as it unrolls.
            val center = Offset(p.cx.toFloat(), p.cy.toFloat())
            drawCircle(water.copy(alpha = (0.9 * (1 - t)).toFloat()), radius = p.r.toFloat(), center = center, style = Stroke(1.5f))
            drawCircle(water.copy(alpha = (0.3 * (1 - t)).toFloat()), radius = p.r.toFloat() + 4f, center = center, style = Stroke(6f))
        }
        translate(top = up) {
            if (freightLayer && lanes.isNotEmpty()) drawLanes(map, p, lanes, selected, pickedLane)
            else if (graphics.traffic > 0 && lanes.isNotEmpty() && t > 0.5) drawFaintLanes(map, p, lanes)
        }
    }
    MapProfiler.mark("lanes")
    // The structures: buildings, skylines and labels, drawn in their own layer over the traffic.
    if (phase == MapPhase.Ground) return
    if (cam.zoom > 3) {
        val close = cam.zoom > 14
        val nightNow = sun != null
        // City skylines, then the player's divisions as buildings, nearest drawn last.
        val cityTops = HashMap<String, Float>()
        if (graphics.buildings >= 2 && g.closeUp && t > 0.5) {
            // Each big city on screen is a voxel city block sized by its population, a step smaller
            // than the player's buildings so those stay the stars.
            val voxelScale = (Voxels.scale(g.scale, density) - 1).coerceAtLeast(1)
            g.cityBlocks(ground).forEach { block ->
                val base = p.lift(block.gx.toDouble(), block.gy.toDouble(), block.floor)
                val top = with(Voxels) { drawBuilding("city", block.plants, base, voxelScale) }
                if (top != null) cityTops[block.name] = top
            }
        }
        if (graphics.buildings > 0 && g.closeUp && t > 0.5) {
            // Rivals' plants first, a size smaller and muted, so the player's own stand over them.
            val rivalScale = (Voxels.scale(g.scale, density) - 1).coerceAtLeast(1)
            g.rivalBuildings(plants, ground).forEach { b ->
                with(Voxels) { drawBuilding(b.sector, b.plants, p.lift(b.gx.toDouble(), b.gy.toDouble(), b.floor), rivalScale, Voxels.Muted) }
            }
            val voxelScale = Voxels.scale(g.scale, density)
            g.buildings(divisions, ground).forEach { b ->
                // Each division is a voxel building for its industry; a plain box if the sprite is missing.
                val base = p.lift(b.gx.toDouble(), b.gy.toDouble(), b.floor)
                val spriteTop = with(Voxels) { drawBuilding(b.sector, b.plants, base, voxelScale) }
                val topAt = if (spriteTop != null) Offset(base.x, spriteTop) else {
                    val windows = if (nightNow && light.facing(b.lon, b.lat) < 0) 0.85f else if (graphics.lighting == 2) 0.30f else 0f
                    drawBox(p, b.gx, b.gy, b.half, b.floor, b.top, Sectors.color(b.sector), windows, Palette.gold.copy(alpha = 0.9f))
                    p.lift(b.gx.toDouble(), b.gy.toDouble(), b.top)
                }
                val label = measurer.measure("${b.plants}", TextStyle(color = Palette.ink, fontSize = 10.sp, fontWeight = FontWeight.Bold))
                val badge = topAt - Offset(0f, 12f)
                drawCircle(Palette.gold, radius = max(label.size.width, label.size.height) / 2f + 3f, center = badge)
                drawText(label, topLeft = badge - Offset(label.size.width / 2f, label.size.height / 2f))
            }
        } else if (t > 0.9 && g.zoomRatio >= 0.7) {
            // Around the scenario's framing: a small badge per division. Further out, nothing.
            val grouped = divisions.groupBy { it.region to it.industry }
            val perRegion = mutableMapOf<String, Int>()
            grouped.forEach { (key, items) ->
                val anchor = map.anchor(key.first)?.let { p.project(it, plinth) } ?: return@forEach
                val index = perRegion.merge(key.first, 1, Int::plus)!! - 1
                val dot = anchor + Offset(index * 26f, 19f)
                drawCircle(Sectors.color(key.second), radius = 11f, center = dot)
                drawCircle(Palette.ink, radius = 11f, center = dot, style = Stroke(2f))
                val plants = items.sumOf { it.blocks.toInt() }
                val label = measurer.measure("$plants", TextStyle(color = Palette.ink, fontSize = 10.sp, fontWeight = FontWeight.Bold))
                drawText(label, topLeft = dot - Offset(label.size.width / 2f, label.size.height / 2f))
            }
        }
        // City names last, so no building ever covers them.
        val cityLift = plinth
        map.cities.take(if (close) 40 else 14).forEach { (city, point) ->
            if (!close && city.country != "USA") return@forEach
            val floor = g.cityFloor(city.name, ground)
            val at = p.project(point, floor) ?: return@forEach
            val above = cityTops[city.name]?.let { Offset(at.x, it) }
            drawCircle(Palette.primaryText.copy(alpha = 0.85f), radius = 2.5f, center = at)
            val label = measurer.measure(city.name, TextStyle(color = Palette.primaryText.copy(alpha = 0.9f), fontSize = 10.sp, fontWeight = FontWeight.SemiBold, shadow = LabelShadow))
            val anchorY = above?.y?.let { min(it, at.y) } ?: at.y
            drawText(label, topLeft = Offset(at.x + 6f, anchorY - label.size.height / 2f))
        }
        if (graphics.cityLights && lit && (sun != null || t < 1)) drawCityLights(map.cities, p, light, cityLift, 1f)
    } else if (graphics.cityLights && lit && t < 1) {
        drawCityLights(map.cities, p, light, 0.0, 0.7f)
    }
    MapProfiler.mark("cities+buildings")
    if (cam.zoom > 14) {
        map.regions.forEach { region ->
            val at = map.anchor(region.code)?.let { p.project(it, g.regionLift(region.code, ground)) } ?: return@forEach
            val label = measurer.measure(region.name.uppercase(), TextStyle(color = Palette.secondary.copy(alpha = 0.8f), fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.8.sp, shadow = LabelShadow))
            drawText(label, topLeft = at + Offset(-label.size.width / 2f, 30f))
        }
    }
    MapProfiler.mark("labels+vignette")
}

/** Which part of the still map to draw: the ground, the structures over the traffic, or both. */
internal enum class MapPhase { Ground, Structures, All }

/** Heights that put everything on the ground, for a map drawn without raised regions. */
internal val FlatGround: Map<String, Double> = HashMap()

/** Haze towards the horizon when tilted, and a light vignette that holds the eye in the middle. */
private fun DrawScope.drawFlatLight(p: MapProjection, full: Size) {
    // Kept faint: depth without washing the map out.
    val haze = (0.07 * sin(p.pitch) / sin(48 * PI / 180)).toFloat()
    if (haze > 0.01f) {
        drawRect(Brush.verticalGradient(0f to Color(0xFF9CC3E6).copy(alpha = haze), 1f to Color.Transparent, endY = full.height * 0.25f), size = Size(full.width, full.height * 0.25f))
    }
    drawRect(
        Brush.radialGradient(
            0f to Color.Transparent, 0.8f to Color.Transparent, 1f to Color.Black.copy(alpha = 0.18f),
            center = Offset(full.width / 2, full.height / 2), radius = max(full.width, full.height) * 0.75f,
        ),
        size = full,
    )
}

/** Section timings for the benchmark; off in the game. */
internal object MapProfiler {
    var on = false
    val totals = LinkedHashMap<String, Double>()
    private var last = 0L
    fun start() { if (on) last = System.nanoTime() }
    fun mark(name: String) {
        if (!on) return
        val now = System.nanoTime()
        totals[name] = (totals[name] ?: 0.0) + (now - last) / 1e6
        last = now
    }
}

private val LabelShadow = Shadow(Color.Black.copy(alpha = 0.85f), Offset(0f, 1f), 3f)
private val RegionLand = Color(0xFF33475A)
private val SkylineColor = Color(0xFF4A5A6C)

/** One raised region: its ground footprint, top, lit walls and height in pixels. */
internal class RegionSolid(val code: String, val ground: Path, val top: Path, val walls: Array<Path>, val h: Double, val lift: Double) {
    val bounds: Rect = top.getBounds()
}

/** A building standing on the map: ground offsets from the view centre, floor and top in pixels. */
internal class Building(
    val sector: String, val plants: Int, val gx: Float, val gy: Float, val half: Float, val floor: Double, val top: Double,
    val lon: Double, val lat: Double, val key: String,
)

internal class TopPoint(val x: Float, val y: Float, val h: Double)

/** A city drawn as a voxel block: ground offsets from the view centre, the tile it stands on, and a size stand-in. */
internal class CityBlock(val name: String, val gx: Float, val gy: Float, val floor: Double, val plants: Int)

/**
 * The map projected for one camera and view size. Each path is built the first time it is drawn
 * and kept, so a redraw that moves nothing (a new week, a selection) projects nothing. Raised
 * tiles, buildings and the moving layer's tracks are kept for the last heights and data.
 */
internal class MapGeometry(val map: WorldMap, val cam: MapCamera, val size: Size, val detailLevel: MapDetail, val pitch: Double) {
    val p = MapProjection(cam, size, pitch)
    /** How big 3D things are drawn: 1 at the scenario's framing, larger closer in. */
    val scale: Float = (cam.zoom / MapCamera.scenario(size, map).zoom).coerceIn(0.6, 2.5).toFloat()
    /**
     * Zoomed in past the scenario's framing: only then do divisions and cities stand as voxel
     * buildings. Further out they are flat badges, so the overview stays clean.
     */
    val closeUp: Boolean get() = zoomRatio >= 1.4

    /** The zoom against the scenario's framing, unclamped: 1 at the framing, under 1 further out. */
    val zoomRatio: Double = cam.zoom / MapCamera.scenario(size, map).zoom

    /** The height of the scenario's base in pixels. */
    val plinth: Double get() = 8.0 * scale * p.t
    val graticule: Path by lazy { p.lines(graticuleRings) }
    val land: Path by lazy { Path().apply { fillType = PathFillType.EvenOdd; map.countries.forEach { p.addPolygons(it.rings, this) } } }
    val biomes: List<Path> by lazy { biomePaths(map, p) }
    val provinces: Path by lazy { p.lines(map.provinces) }
    val roads: Path by lazy { p.lines(map.roads) }
    val regionPaths: List<Pair<String, Path>> by lazy { map.regions.map { it.code to p.polygons(it.rings) } }
    val counties: Path by lazy { p.lines(map.counties) }
    val lakes: Path by lazy { p.polygons(map.lakes) }
    val rivers: Path by lazy { p.lines(map.rivers) }
    val interstates: Path by lazy { p.lines(map.interstates) }
    val playStates: List<Path> by lazy { map.playStates.map { p.polygons(it.rings) } }

    /** Pixels high for a region at a layer intensity, flattening towards the base up close. */
    private fun height(intensity: Double): Double {
        val close = if (detailLevel != MapDetail.High) 0.0 else ((cam.zoom - 7) / 5).coerceIn(0.0, 1.0)
        return (8 + 34 * intensity.coerceIn(0.0, 1.0) * (1 - 0.75 * close)) * scale * p.t
    }

    fun regionLift(code: String, heights: Map<String, Double>): Double = if (heights === FlatGround) 0.0 else height(heights[code] ?: 0.0)

    private var nightKey: Sun? = null
    private var nightPaths: List<Path> = emptyList()

    fun night(sun: Sun): List<Path> {
        if (sun != nightKey) {
            nightPaths = nightRings(sun, p).map { p.polygons(listOf(it)) }
            nightKey = sun
        }
        return nightPaths
    }

    private var raisedKey: Map<String, Double>? = null
    private var raisedValue: List<RegionSolid> = emptyList()

    /** The scenario's regions as raised tiles, far ones first. */
    fun raised(heights: Map<String, Double>): List<RegionSolid> {
        if (raisedKey !== heights) {
            val base = p.lift(0.0, 0.0, 0.0).y
            raisedValue = map.regions.map { region ->
                val h = height(heights[region.code] ?: 0.0)
                val walls = Array(3) { Path() }
                p.walls(region.rings, h, walls)
                val anchorY = map.anchor(region.code)?.let { p.project(it)?.y } ?: 0f
                anchorY to RegionSolid(region.code, regionPaths.first { it.first == region.code }.second, p.polygons(region.rings, h), walls, h, (base - p.lift(0.0, 0.0, h).y).toDouble())
            }.sortedBy { it.first }.map { it.second }
            raisedKey = heights
        }
        return raisedValue
    }

    private var shadowKey: Map<String, Double>? = null
    private var shadowPath = Path()

    /** Every tile's shadow in one path, cast down and right of its footprint. */
    fun shadows(heights: Map<String, Double>): Path {
        if (shadowKey !== heights) {
            shadowPath = Path()
            raised(heights).forEach { solid ->
                shadowPath.addPath(solid.ground, Offset((0.25 * solid.lift).toFloat(), (0.18 * solid.lift).toFloat()))
            }
            shadowKey = heights
        }
        return shadowPath
    }

    private var buildingsKey: Pair<List<MapDivisionDto>, Map<String, Double>>? = null
    private var buildingsValue: List<Building> = emptyList()

    /** The player's divisions as buildings round their region's hub, far ones first. */
    fun buildings(divisions: List<MapDivisionDto>, heights: Map<String, Double>): List<Building> {
        val key = divisions to heights
        if (buildingsKey?.first !== divisions || buildingsKey?.second !== heights) {
            val perRegion = HashMap<String, Int>()
            buildingsValue = divisionSlots(divisions).mapNotNull { (region, sector, plants) ->
                val anchor = map.anchor(region) ?: return@mapNotNull null
                val ground = p.ground(anchor.lon[0], anchor.lat[0])
                if (!ground.isSpecified) return@mapNotNull null
                val i = perRegion.merge(region, 1, Int::plus)!! - 1
                val gx = ground.x + (i % 3 - 1) * 34f * scale
                val gy = ground.y + (18f + (i / 3) * 30f) * scale
                val floor = regionLift(region, heights)
                val h = (12 + 7 * sqrt(plants.toDouble())) * scale
                Building(sector, plants, gx, gy, 7.5f * scale, floor, floor + h, anchor.lon[0], anchor.lat[0], "$region/$sector")
            }.sortedBy { it.gy }
            buildingsKey = key
        }
        return buildingsValue
    }

    private val cityRegion: Map<String, String> by lazy { cityRegions(map) }

    /** The height of the tile a city stands on. */
    fun cityFloor(city: String, heights: Map<String, Double>): Double =
        cityRegion[city]?.let { regionLift(it, heights) } ?: 0.0

    private var skylineKey: Map<String, Double>? = null
    private var skylineValue: List<Building> = emptyList()
    private val skylineTops = HashMap<String, TopPoint>()

    /** Skylines for the biggest cities on screen, each a small cluster of towers. */
    fun skylines(heights: Map<String, Double>): List<Building> {
        if (skylineKey !== heights) {
            skylineTops.clear()
            val boxes = ArrayList<Building>()
            map.cities.sortedByDescending { it.first.population }.asSequence()
                .mapNotNull { (city, point) ->
                    val ground = p.ground(point.lon[0], point.lat[0])
                    if (!ground.isSpecified) return@mapNotNull null
                    val screen = p.lift(ground.x.toDouble(), ground.y.toDouble(), 0.0)
                    if (screen.x !in -40f..size.width + 40 || screen.y !in -40f..size.height + 40) return@mapNotNull null
                    Triple(city, point, ground)
                }
                .take(30)
                .forEach { (city, point, ground) ->
                    val hash = stableHash(city.name)
                    val count = 3 + hash % 5
                    val floor = cityFloor(city.name, heights)
                    val tall = (6 + 4 * log10(max(city.population / 1e5, 1.0))) * scale
                    var best = 0.0
                    var bestAt = TopPoint(ground.x, ground.y, floor)
                    for (k in 0 until count) {
                        val h2 = stableHash(city.name + k)
                        val dx = ((h2 % 17) - 8) * 1.3f * scale
                        val dy = (((h2 / 17) % 13) - 6) * 1.1f * scale
                        val h = tall * (0.35 + 0.65 * ((h2 / 221) % 100) / 100.0) * (if (k == 0) 1.25 else 1.0)
                        boxes += Building("", 0, ground.x + dx, ground.y + dy, (1.6f + (h2 % 3)) * scale, floor, floor + h, point.lon[0], point.lat[0], city.name)
                        if (h > best) { best = h; bestAt = TopPoint(ground.x + dx, ground.y + dy, floor + h) }
                    }
                    skylineTops[city.name] = bestAt
                }
            skylineValue = boxes.sortedBy { it.gy }
            skylineKey = heights
        }
        return skylineValue
    }

    private var rivalsKey: Pair<List<uniffi.cw_ffi.MapPlantDto>, Map<String, Double>>? = null
    private var rivalsValue: List<Building> = emptyList()

    /**
     * Rivals' plants as buildings: each rival's plants in an industry and region together, the four
     * biggest per region, set in a row behind the region's hub (north of it, where the player's own
     * stand to the south), far ones first.
     */
    fun rivalBuildings(plants: List<uniffi.cw_ffi.MapPlantDto>, heights: Map<String, Double>): List<Building> {
        if (rivalsKey?.first !== plants || rivalsKey?.second !== heights) {
            rivalsValue = plants.filter { !it.player }
                .groupBy { Triple(it.region, it.industry, it.firm) }
                .map { (key, items) -> key to items.sumOf { it.blocks.toInt() } }
                .groupBy { it.first.first }
                .flatMap { (region, firms) ->
                    val anchor = map.anchor(region) ?: return@flatMap emptyList()
                    val ground = p.ground(anchor.lon[0], anchor.lat[0])
                    if (!ground.isSpecified) return@flatMap emptyList()
                    firms.sortedByDescending { it.second }.take(4).mapIndexed { i, (key, blocks) ->
                        val gx = ground.x + (i - 1.5f) * 26f * scale
                        val gy = ground.y - (14f + (i % 2) * 10f) * scale
                        val floor = regionLift(region, heights)
                        Building(key.second, blocks, gx, gy, 6f * scale, floor, floor + 10 * scale, anchor.lon[0], anchor.lat[0], "$region/${key.second}/${key.third}")
                    }
                }
                .sortedBy { it.gy }
            rivalsKey = plants to heights
        }
        return rivalsValue
    }

    private var blocksKey: Map<String, Double>? = null
    private var blocksValue: List<CityBlock> = emptyList()

    /**
     * The thirty biggest cities on screen as voxel city blocks, far ones first; [CityBlock.plants]
     * carries a stand-in plant count that picks the block size (town, city, metropolis).
     */
    fun cityBlocks(heights: Map<String, Double>): List<CityBlock> {
        if (blocksKey !== heights) {
            blocksValue = map.cities.sortedByDescending { it.first.population }.asSequence()
                .mapNotNull { (city, point) ->
                    val ground = p.ground(point.lon[0], point.lat[0])
                    if (!ground.isSpecified) return@mapNotNull null
                    val screen = p.lift(ground.x.toDouble(), ground.y.toDouble(), 0.0)
                    if (screen.x !in -60f..size.width + 60 || screen.y !in -60f..size.height + 60) return@mapNotNull null
                    // Below a million people a town block, below four million a city, above that a metropolis.
                    val stand = when { city.population < 1_000_000 -> 2; city.population < 4_000_000 -> 5; else -> 12 }
                    CityBlock(city.name, ground.x, ground.y, cityFloor(city.name, heights), stand)
                }
                // At the scenario's framing only the biggest cities, so the map stays readable.
                .take(if (cam.zoom > 10) 30 else 16).sortedBy { it.gy }.toList()
            blocksKey = heights
        }
        return blocksValue
    }

    /** The top of a city's tallest tower, once its skyline has been built. */
    fun skylineTop(city: String): TopPoint? = skylineTops[city]

    private var overlayKey: List<Any?>? = null
    private var overlayValue: OverlayGeometry? = null

    /** What the moving layer needs, rebuilt only when the camera or the data under it changes. */
    fun overlay(
        lanes: List<uniffi.cw_ffi.LaneDto>, divisions: List<MapDivisionDto>, selected: String?, heights: Map<String, Double>,
        graphics: GraphicsConfig, freightLayer: Boolean, density: Float = 1f,
    ): OverlayGeometry {
        val key = listOf(lanes, divisions, selected, heights, graphics, freightLayer, density)
        val held = overlayValue
        if (held != null && overlayKey?.let { k -> k.size == key.size && k.indices.all { k[it] === key[it] || k[it] == key[it] } } == true) return held
        val raised = graphics.extrude && p.t > 0.05
        val ground = if (raised) heights else FlatGround
        val laneCount = if (graphics.traffic >= 2 || freightLayer) 120 else if (graphics.traffic == 1) 60 else 0
        val tracks = if (p.t > 0.5 && laneCount > 0) laneTracks(map, p, lanes, laneCount) else emptyList()
        val roadLines = if (graphics.traffic > 0 && detailLevel == MapDetail.High && cam.zoom > 9) buildPolylines(p, map.interstates, if (raised) plinth else 0.0, size) else emptyList()
        val spots = if (graphics.buildings > 0 && closeUp && p.t > 0.5) buildings(divisions, ground).map { b ->
            val base = p.lift(b.gx.toDouble(), b.gy.toDouble(), b.floor)
            // The rooftop light and smoke sit on the sprite's top when there is one.
            val frame = Voxels.frame(b.sector, Voxels.size(b.plants), Voxels.scale(scale, density))
            val top = frame?.let { Offset(base.x, (base.y - it.ay + 4).toFloat()) } ?: p.lift(b.gx.toDouble(), b.gy.toDouble(), b.top)
            BuildingSpot(base, top, Sectors.color(b.sector), smokes(b.sector), (stableHash(b.key) % 1000) / 1000f)
        } else emptyList()
        val top = selected?.let { code ->
            val region = map.regions.firstOrNull { it.code == code } ?: return@let null
            p.polygons(region.rings, regionLift(code, ground))
        }
        val airports = if (graphics.traffic > 0 && p.t > 0.5) map.cities.sortedByDescending { it.first.population }.take(14)
            .mapNotNull { (_, point) -> p.ground(point.lon[0], point.lat[0]).takeIf { it.isSpecified } } else emptyList()
        return OverlayGeometry(p, tracks, roadLines, spots, top, scale, if (raised) plinth else 0.0, airports).also {
            overlayValue = it
            overlayKey = key
        }
    }

    fun matches(map: WorldMap, cam: MapCamera, size: Size, detailLevel: MapDetail, pitch: Double) =
        this.map === map && this.cam == cam && this.size == size && this.detailLevel == detailLevel && this.pitch == pitch
}

/** Holds the last [MapGeometry] so an unchanged camera is projected once. */
internal class MapGeometryCache {
    private var held: MapGeometry? = null
    /** The small soft-light layers, kept so their bitmaps are reused. */
    val front = LightLayer()
    val back = LightLayer()

    fun of(map: WorldMap, cam: MapCamera, size: Size, detailLevel: MapDetail, pitch: Double = 0.0): MapGeometry =
        held?.takeIf { it.matches(map, cam, size, detailLevel, pitch) } ?: MapGeometry(map, cam, size, detailLevel, pitch).also { held = it }
}

/** Which region each city stands in, by a point-in-polygon test on the sphere's coordinates. */
private val cityRegionCache = java.util.WeakHashMap<WorldMap, Map<String, String>>()

internal fun cityRegions(map: WorldMap): Map<String, String> = synchronized(cityRegionCache) {
    cityRegionCache.getOrPut(map) {
        map.cities.mapNotNull { (city, point) ->
            val lon = point.lon[0]
            val lat = point.lat[0]
            map.regions.firstOrNull { region ->
                var inside = false
                region.rings.forEach { ring ->
                    var j = ring.size - 1
                    for (i in 0 until ring.size) {
                        if ((ring.lat[i] > lat) != (ring.lat[j] > lat) &&
                            lon < (ring.lon[j] - ring.lon[i]) * (lat - ring.lat[i]) / (ring.lat[j] - ring.lat[i]) + ring.lon[i]
                        ) inside = !inside
                        j = i
                    }
                }
                inside
            }?.let { city.name to it.code }
        }.toMap()
    }
}

private val OutsideLand = Color(0xFF25282B)
private val OutsideBorder = Color(0xFF55595D)
private val OutsideRoad = Color(0xFF6A6E72)

/** Meridians and parallels every 15 degrees, for the globe. */
private val graticuleRings: List<GeoRing> =
    (-180 until 180 step 15).map { lon -> GeoRing((-80..80 step 4).flatMap { listOf(lon * 10_000, it * 10_000) }) } +
        (-75..75 step 15).map { lat -> GeoRing((-180..180 step 4).flatMap { listOf(it * 10_000, lat * 10_000) }) }

@Composable
private fun MapButton(symbol: String, onClick: () -> Unit) {
    Box(Modifier.size(40.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        PixelIcon(symbol, size = 22.dp, tint = Palette.primaryText)
    }
}

internal fun layerTint(layer: MapLayer): Color = when (layer) {
    MapLayer.Presence -> Palette.gold
    MapLayer.Room -> Palette.positive
    MapLayer.Wages -> Color(0xFF42A5F5)
    MapLayer.Freight -> Palette.gold
}

/** 0..1 intensity per region for the active layer. */
internal fun shading(store: GameStore, layer: MapLayer): Map<String, Double> = when (layer) {
    MapLayer.Presence -> {
        val plants = store.mapDivisions.groupBy { it.region }.mapValues { (_, items) -> items.sumOf { it.blocks.toInt() }.toDouble() }
        val top = plants.values.maxOrNull() ?: 1.0
        plants.mapValues { it.value / maxOf(top, 1.0) }
    }
    MapLayer.Room -> {
        val values = store.boards.associate { board -> board.code to board.room.sumOf { (it.importLots * it.importPriceCents).toDouble() } }
        val top = values.values.maxOrNull() ?: 1.0
        values.mapValues { if (top > 0) it.value / top else 0.0 }
    }
    MapLayer.Wages -> {
        val low = store.boards.minOfOrNull { it.wageIndexBps } ?: 0
        val high = store.boards.maxOfOrNull { it.wageIndexBps } ?: 1
        store.boards.associate { it.code to (it.wageIndexBps - low).toDouble() / maxOf(high - low, 1) }
    }
    // Lanes are drawn over the land instead of shading it.
    MapLayer.Freight -> emptyMap()
}

@Composable
private fun MapHud(store: GameStore) {
    val overview = store.overview
    Column(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panel.copy(alpha = 0.94f)).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Eyebrow(overview?.weekLabel ?: "")
                Text(overview?.primaryName?.takeIf { it.isNotBlank() } ?: "No company yet", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, maxLines = 1)
            }
            TimeBar(store)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
            Column {
                Eyebrow("Cash")
                Text(Format.compact(overview?.firmCashCents ?: 0, overview?.primaryCurrency.orEmpty()), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
            }
            val profit = overview?.quarterNetIncomeCents ?: 0
            Column {
                Eyebrow(if ((overview?.quarterWeeks ?: 0u) >= 13u) "Profit/qtr" else "Profit so far")
                Text(Format.compact(profit, overview?.primaryCurrency.orEmpty()), color = if (profit >= 0) Palette.positive else Palette.negative, fontWeight = FontWeight.SemiBold)
            }
        }
        // What the desktop shows in its Alerts pane: decisions due, a pause, the engine's warnings.
        val due = store.events.count { it.mandatory }
        val alerts = buildList {
            if (due > 0) add("$due decision${if (due == 1) "" else "s"} waiting in the Inbox" to Palette.warning)
            overview?.pause?.let { add("Paused: $it" to Palette.warning) }
            overview?.alerts?.forEach { add(it to Palette.warning) }
        }
        alerts.take(3).forEach { (text, tone) ->
            Row(verticalAlignment = Alignment.Top) {
                PixelIcon("alert", size = 12.dp, tint = tone, modifier = Modifier.padding(top = 2.dp, end = 6.dp))
                Text(text, color = Palette.primaryText, fontSize = 12.sp)
            }
        }
        if (alerts.size > 3) Text("${alerts.size - 3} more on the Portfolio tab", color = Palette.secondary, fontSize = 11.sp)
    }
}

@Composable
internal fun RegionSheet(store: GameStore, board: RegionBoardDto, onBuild: () -> Unit, onClose: () -> Unit) {
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text(board.name) },
        text = {
            Column(Modifier.verticalScroll(androidx.compose.foundation.rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("${board.stateName} · hub ${board.hubCity}", color = Palette.secondary, fontSize = 13.sp)
                Text(
                    "Wages ${board.wageIndexBps / 100}% · skill ${board.skillBps / 100} · roads ${board.roadBps / 100} · power ${board.powerGridBps / 100}",
                    color = Palette.secondary,
                    fontSize = 12.sp,
                )
                if (board.specialization.isNotEmpty()) {
                    Text("Specialized in " + board.specialization.joinToString(" and ") { Sectors.name(it) }, color = Palette.positive, fontSize = 12.sp)
                }
                val room = board.room.filter { it.importLots > 0 }.sortedByDescending { it.importLots * it.importPriceCents }.take(8)
                if (room.isNotEmpty()) {
                    Eyebrow("Room to build")
                    room.forEach { Text("${it.productName}: ${it.importLots} lots a week imported at ${Format.money(it.importPriceCents)}", color = Palette.primaryText, fontSize = 13.sp) }
                }
                if (board.storage.isNotEmpty()) {
                    Eyebrow("Your storage here")
                    StorageLines(board.storage)
                }
                val ours = store.divisions.filter { it.region == board.code }
                if (ours.isNotEmpty()) {
                    Eyebrow("Your divisions here")
                    ours.forEach {
                        Text(
                            "${Sectors.name(it.industry)} · ${it.strategyName} · ${it.blocks} plants · ${it.shareBps / 100}% share",
                            color = Palette.primaryText, fontSize = 13.sp,
                            modifier = Modifier.clickable { store.showDivision(it) },
                        )
                    }
                }
                val rivalLeaders = board.sectors.filter { it.leaderFirm != 0uL && it.leaderFirm != store.selectedFirm && it.ownBlocks == 0u }
                if (rivalLeaders.isNotEmpty()) {
                    Eyebrow("Rivals here")
                    rivalLeaders.forEach { RivalLeaderRow(store, it.leader, it.leaderFirm, "${Sectors.name(it.industry)} · ${it.leaderShareBps / 100}%", board.code, it.industry) }
                }
            }
        },
        confirmButton = { GoldButton("Build here", enabled = store.selectedFirm != null) { onBuild() } },
        dismissButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
}

/** Time controls folded into one pill; tap to open Next week and the speeds, tap again to fold. */
@Composable
fun TimeBar(store: GameStore) {
    var open by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        if (open) {
            GoldButton("Next", enabled = !store.busy && !store.running) { store.advance() }
            SpeedControl(store)
        }
        Row(
            Modifier.clip(Pixel.small).background(Palette.panelRaised).clickable { open = !open }
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            val speedIcon = if (!store.running) "pause" else when (store.speed) { 1 -> "speed1"; 2 -> "speed2"; else -> "speed3" }
            PixelIcon(speedIcon, size = 16.dp, tint = Palette.primaryText)
            PixelIcon(if (open) "chevron_right" else "chevron_left", size = 12.dp, tint = Palette.primaryText)
        }
    }
}
