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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.RegionBoardDto

private enum class MapLayer(val title: String) { Presence("Your divisions"), Room("Room to build"), Wages("Wages") }

/** The corridor map: your divisions, where demand is unmet, and every region's board on tap. */
@Composable
fun MapScreen(store: GameStore) {
    val map = CorridorMap.shared
    var layer by remember { mutableStateOf(MapLayer.Presence) }
    var selected by remember { mutableStateOf<String?>(null) }
    var building by remember { mutableStateOf<RegionBoardDto?>(null) }
    var zoom by remember { mutableFloatStateOf(1f) }
    var pan by remember { mutableStateOf(Offset.Zero) }
    val measurer = rememberTextMeasurer()

    Box(Modifier.fillMaxSize().background(Palette.ink)) {
        if (map == null) {
            Text("The map file is missing from this build.", color = Palette.secondary, modifier = Modifier.padding(24.dp))
        } else {
            val shading = shading(store, layer)
            Canvas(
                Modifier.fillMaxSize()
                    .pointerInput(Unit) {
                        detectTransformGestures { _, panChange, zoomChange, _ ->
                            zoom = (zoom * zoomChange).coerceIn(1f, 6f)
                            pan += panChange
                        }
                    }
                    .pointerInput(zoom, pan) {
                        detectTapGestures { tap ->
                            val base = minOf(size.width / map.width, size.height / map.height)
                            val scale = base * zoom
                            val origin = Offset((size.width - map.width * scale) / 2f, (size.height - map.height * scale) / 2f) + pan
                            selected = map.regionAt((tap - origin) / scale)
                        }
                    },
            ) {
                val base = minOf(size.width / map.width, size.height / map.height)
                val scale = base * zoom
                val origin = Offset((size.width - map.width * scale) / 2f, (size.height - map.height * scale) / 2f) + pan
                withTransform({
                    translate(origin.x, origin.y)
                    scale(scale, scale, pivot = Offset.Zero)
                }) {
                    val hair = 1.2f / scale
                    map.otherStates.forEach { drawPath(it, Color(0xFF1E2A36)) }
                    map.playStates.forEach { drawPath(it, Color(0xFF2B3B4A)) }
                    map.regions.forEach { (code, path) ->
                        val intensity = shading[code] ?: 0.0
                        if (intensity > 0) drawPath(path, layerTint(layer).copy(alpha = (0.15 + 0.6 * intensity).toFloat()))
                        if (code == selected) drawPath(path, Palette.gold.copy(alpha = 0.35f))
                        drawPath(path, Palette.stroke, style = Stroke(width = hair))
                    }
                    map.lakes.forEach { drawPath(it, Color(0xFF16354F)) }
                    map.interstates.forEach { drawPath(it, Color(0xFFC68A3D).copy(alpha = 0.35f), style = Stroke(width = hair)) }
                    map.playStates.forEach { drawPath(it, Color(0xFFB8C7D6).copy(alpha = 0.6f), style = Stroke(width = 2.4f / scale)) }
                }
                // Division markers stay a fixed size on screen: one circle per owned sector in a
                // region, with its plant count.
                val grouped = store.mapDivisions.groupBy { it.region to it.industry }
                val perRegion = mutableMapOf<String, Int>()
                grouped.forEach { (key, items) ->
                    val anchor = map.anchor(key.first) ?: return@forEach
                    val index = perRegion.merge(key.first, 1, Int::plus)!! - 1
                    val center = origin + anchor * scale + Offset(index * 26f, 0f)
                    drawCircle(Sectors.color(key.second), radius = 11f, center = center)
                    drawCircle(Palette.ink, radius = 11f, center = center, style = Stroke(2f))
                    val plants = items.sumOf { it.blocks.toInt() }
                    val label = measurer.measure("$plants", TextStyle(color = Palette.ink, fontSize = 10.sp, fontWeight = FontWeight.Bold))
                    drawText(label, topLeft = center - Offset(label.size.width / 2f, label.size.height / 2f))
                }
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
            GoldButton("Next", enabled = !store.busy && !store.running) { store.advance() }
            Spacer(Modifier.width(6.dp))
            SpeedControl(store)
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
