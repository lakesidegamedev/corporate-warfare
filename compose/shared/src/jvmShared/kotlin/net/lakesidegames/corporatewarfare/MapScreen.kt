package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.PointerEventType
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

private enum class MapLayer(val title: String) { Presence("Your divisions"), Room("Room to build"), Wages("Wages") }

/** The world map: a globe when zoomed out that flattens onto the scenario as you zoom in. */
@Composable
fun MapScreen(store: GameStore) {
    val map = WorldMap.shared
    var layer by remember { mutableStateOf(MapLayer.Presence) }
    var selected by remember { mutableStateOf<String?>(null) }
    var building by remember { mutableStateOf<RegionBoardDto?>(null) }
    var camera by remember { mutableStateOf<MapCamera?>(null) }
    var viewSize by remember { mutableStateOf(Size.Zero) }
    val scope = rememberCoroutineScope()
    var flight by remember { mutableStateOf<Job?>(null) }
    val measurer = rememberTextMeasurer()

    fun current(): MapCamera? = camera ?: map?.let { if (viewSize.width > 0) MapCamera.scenario(viewSize, it) else null }
    fun fly(target: MapCamera) {
        val start = current() ?: return
        flight?.cancel()
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

    Box(Modifier.fillMaxSize().background(Palette.ink)) {
        if (map == null) {
            Text("The map file is missing from this build.", color = Palette.secondary, modifier = Modifier.padding(24.dp))
        } else {
            val shading = shading(store, layer)
            Canvas(
                Modifier.fillMaxSize()
                    .onSizeChanged { viewSize = Size(it.width.toFloat(), it.height.toFloat()) }
                    .pointerInput(Unit) {
                        detectTransformGestures { _, panChange, zoomChange, _ ->
                            flight?.cancel()
                            val now = current() ?: return@detectTransformGestures
                            camera = now.zoomed(zoomChange.toDouble()).panned(panChange, viewSize)
                        }
                    }
                    .pointerInput(Unit) {
                        awaitPointerEventScope {
                            while (true) {
                                val event = awaitPointerEvent()
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
                    .pointerInput(Unit) {
                        detectTapGestures(
                            onDoubleTap = { toggleGlobe() },
                            onTap = { tap ->
                                val now = current() ?: return@detectTapGestures
                                val projection = MapProjection(now, viewSize)
                                val hit = map.regions.firstOrNull { projection.contains(it.rings, tap) }?.code
                                selected = if (hit == selected) null else hit
                            },
                        )
                    },
            ) {
                drawWorldMap(map, camera ?: MapCamera.scenario(size, map), layerTint(layer), shading, selected, store.mapDivisions, measurer)
            }
            Column(
                Modifier.align(Alignment.BottomEnd).padding(end = 12.dp, bottom = 24.dp)
                    .clip(RoundedCornerShape(12.dp)).background(Palette.panel.copy(alpha = 0.94f)),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val zoomedIn = (current()?.zoom ?: 1.0) > MapCamera.FLAT_START
                MapButton("+") { current()?.let { fly(it.zoomed(1.8)) } }
                MapButton("−") { current()?.let { fly(it.zoomed(1 / 1.8)) } }
                MapButton(if (zoomedIn) "◍" else "◎") { toggleGlobe() }
            }
        }

        Column(Modifier.fillMaxWidth().padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            MapHud(store)
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                MapLayer.entries.forEach { option ->
                    Text(
                        option.title,
                        color = if (layer == option) Palette.ink else Palette.primaryText,
                        fontSize = 12.sp,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.clip(CircleShape)
                            .background(if (layer == option) Palette.gold else Palette.panel)
                            .clickable { layer = option }
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }
            }
        }
    }

    selected?.let { code ->
        val board = store.boards.firstOrNull { it.code == code }
        if (board != null) RegionSheet(store, board, onBuild = { building = board; selected = null }, onClose = { selected = null })
    }
    building?.let { board -> BuildDialog(store, board) { building = null } }
}

/** Draws the world through the blended projection: ocean, countries, scenario detail, markers. */
internal fun DrawScope.drawWorldMap(
    map: WorldMap,
    cam: MapCamera,
    tint: Color,
    shading: Map<String, Double>,
    selected: String?,
    divisions: List<MapDivisionDto>,
    measurer: TextMeasurer,
) {
    val p = MapProjection(cam, size)
    val t = p.t
    val center = Offset(p.cx.toFloat(), p.cy.toFloat())
    drawRect(Color(0xFF05090E))
    // Ocean: the globe's disc, growing to fill the view as the map flattens.
    drawCircle(Color(0xFF0E2840), radius = (p.r / max(1 - t, 0.03)).toFloat(), center = center)
    if (t < 1) drawPath(p.lines(graticule), Palette.primaryText.copy(alpha = (0.06 * (1 - t)).toFloat()), style = Stroke(0.6f))
    val land = Path().apply { fillType = PathFillType.EvenOdd }
    map.countries.forEach { p.addPolygons(it.rings, land) }
    drawPath(land, Color(0xFF1E2A36))
    drawPath(land, Palette.stroke, style = Stroke(0.6f))
    if (cam.zoom > 3) {
        val others = Path()
        map.contextStates.forEach { p.addPolygons(it.rings, others) }
        drawPath(others, Palette.stroke.copy(alpha = min((cam.zoom - 3) / 3, 1.0).toFloat()), style = Stroke(0.6f))
    }
    val detail = ((cam.zoom - 7) / 5).coerceIn(0.0, 1.0).toFloat()
    val regionPaths = map.regions.map { it.code to p.polygons(it.rings) }
    regionPaths.forEach { (code, path) ->
        drawPath(path, Color(0xFF2B3B4A))
        val intensity = shading[code] ?: 0.0
        if (intensity > 0) drawPath(path, tint.copy(alpha = (0.15 + 0.6 * intensity).toFloat()))
    }
    if (detail > 0) drawPath(p.lines(map.counties), Palette.stroke.copy(alpha = 0.45f * detail), style = Stroke(0.5f))
    drawPath(p.polygons(map.lakes), Color(0xFF16354F))
    if (detail > 0) {
        drawPath(p.lines(map.rivers), Color(0xFF16354F).copy(alpha = 0.9f * detail), style = Stroke(1.4f))
        drawPath(p.lines(map.interstates), Color(0xFFC68A3D).copy(alpha = 0.35f * detail), style = Stroke(1f))
    }
    regionPaths.forEach { (code, path) ->
        if (code == selected) drawPath(path, Palette.gold.copy(alpha = 0.35f))
        drawPath(path, Palette.stroke, style = Stroke(0.9f))
    }
    map.playStates.forEach {
        drawPath(p.polygons(it.rings), Color(0xFFB8C7D6).copy(alpha = 0.6f), style = Stroke(if (cam.zoom > 3) 2.4f else 1f))
    }
    if (t < 1) {
        // The globe's rim, fading as it unrolls.
        drawCircle(Color(0xFF16354F).copy(alpha = (0.9 * (1 - t)).toFloat()), radius = p.r.toFloat(), center = center, style = Stroke(1.5f))
        drawCircle(Color(0xFF16354F).copy(alpha = (0.3 * (1 - t)).toFloat()), radius = p.r.toFloat() + 4f, center = center, style = Stroke(6f))
    }
    if (cam.zoom > 3) {
        // Cities, then division markers, a fixed size on screen.
        val close = cam.zoom > 14
        map.cities.take(if (close) 40 else 14).forEach { (city, point) ->
            if (!close && city.country != "USA") return@forEach
            val at = p.project(point) ?: return@forEach
            drawCircle(Palette.primaryText.copy(alpha = 0.85f), radius = 2.5f, center = at)
            val label = measurer.measure(city.name, TextStyle(color = Palette.primaryText.copy(alpha = 0.9f), fontSize = 10.sp, fontWeight = FontWeight.SemiBold))
            drawText(label, topLeft = at + Offset(6f, -label.size.height / 2f))
        }
        val grouped = divisions.groupBy { it.region to it.industry }
        val perRegion = mutableMapOf<String, Int>()
        grouped.forEach { (key, items) ->
            val anchor = map.anchor(key.first)?.let { p.project(it) } ?: return@forEach
            val index = perRegion.merge(key.first, 1, Int::plus)!! - 1
            val dot = anchor + Offset(index * 26f, 19f)
            drawCircle(Sectors.color(key.second), radius = 11f, center = dot)
            drawCircle(Palette.ink, radius = 11f, center = dot, style = Stroke(2f))
            val plants = items.sumOf { it.blocks.toInt() }
            val label = measurer.measure("$plants", TextStyle(color = Palette.ink, fontSize = 10.sp, fontWeight = FontWeight.Bold))
            drawText(label, topLeft = dot - Offset(label.size.width / 2f, label.size.height / 2f))
        }
    }
    if (cam.zoom > 14) {
        map.regions.forEach { region ->
            val at = map.anchor(region.code)?.let { p.project(it) } ?: return@forEach
            val label = measurer.measure(region.name.uppercase(), TextStyle(color = Palette.secondary.copy(alpha = 0.8f), fontSize = 9.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.8.sp))
            drawText(label, topLeft = at + Offset(-label.size.width / 2f, 30f))
        }
    }
}

/** Meridians and parallels every 15 degrees, for the globe. */
private val graticule: List<GeoRing> =
    (-180 until 180 step 15).map { lon -> GeoRing((-80..80 step 4).flatMap { listOf(lon * 10_000, it * 10_000) }) } +
        (-75..75 step 15).map { lat -> GeoRing((-180..180 step 4).flatMap { listOf(it * 10_000, lat * 10_000) }) }

@Composable
private fun MapButton(symbol: String, onClick: () -> Unit) {
    Box(Modifier.size(40.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Text(symbol, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 18.sp)
    }
}

private fun layerTint(layer: MapLayer): Color = when (layer) {
    MapLayer.Presence -> Palette.gold
    MapLayer.Room -> Palette.positive
    MapLayer.Wages -> Color(0xFF42A5F5)
}

/** 0..1 intensity per region for the active layer. */
private fun shading(store: GameStore, layer: MapLayer): Map<String, Double> = when (layer) {
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
}

@Composable
private fun MapHud(store: GameStore) {
    val overview = store.overview
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(Palette.panel.copy(alpha = 0.94f)).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Eyebrow(overview?.weekLabel ?: "")
                Text(overview?.primaryName ?: "No company yet", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, maxLines = 1)
            }
            TimeBar(store)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
            Column {
                Eyebrow("Cash")
                Text(Format.compact(overview?.firmCashCents ?: 0), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
            }
            val profit = overview?.quarterNetIncomeCents ?: 0
            Column {
                Eyebrow(if ((overview?.quarterWeeks ?: 0u) >= 13u) "Profit/qtr" else "Profit so far")
                Text(Format.compact(profit), color = if (profit >= 0) Palette.positive else Palette.negative, fontWeight = FontWeight.SemiBold)
            }
        }
    }
}

@Composable
private fun RegionSheet(store: GameStore, board: RegionBoardDto, onBuild: () -> Unit, onClose: () -> Unit) {
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text(board.name) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("${board.stateName} · hub ${board.hubCity}", color = Palette.secondary, fontSize = 13.sp)
                Text(
                    "Wages ${board.wageIndexBps / 100}% · skill ${board.skillBps / 100} · roads ${board.roadBps / 100} · power ${board.powerGridBps / 100}",
                    color = Palette.secondary,
                    fontSize = 12.sp,
                )
                if (board.specialization.isNotEmpty()) {
                    Text("Specialized in " + board.specialization.joinToString(" and ") { Sectors.name(it) }, color = Palette.positive, fontSize = 12.sp)
                }
                val room = board.room.filter { it.importLots > 0 }.sortedByDescending { it.importLots * it.importPriceCents }.take(5)
                if (room.isNotEmpty()) {
                    Eyebrow("Room to build")
                    room.forEach { Text("${it.productName}: ${it.importLots} lots a week imported at ${Format.money(it.importPriceCents)}", color = Palette.primaryText, fontSize = 13.sp) }
                }
                val ours = store.divisions.filter { it.region == board.code }
                if (ours.isNotEmpty()) {
                    Eyebrow("Your divisions here")
                    ours.forEach { Text("${Sectors.name(it.industry)} · ${it.strategyName} · ${it.blocks} plants · ${it.shareBps / 100}% share", color = Palette.primaryText, fontSize = 13.sp) }
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
        Text(
            (if (store.running) "▶ ${store.speed}x" else "II") + if (open) "  ›" else "  ‹",
            color = Palette.primaryText,
            fontWeight = FontWeight.Bold,
            fontSize = 13.sp,
            modifier = Modifier.clip(CircleShape).background(Palette.panelRaised).clickable { open = !open }
                .padding(horizontal = 12.dp, vertical = 8.dp),
        )
    }
}
