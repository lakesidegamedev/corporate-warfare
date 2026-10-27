package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.LaneDto

/** The pixel icon for a freight mode id. */
internal fun modeArt(modeId: String): String = if (modeId == "sea") "ship" else modeId

/** Green while a lane has room, amber as it fills, red when it is full or spilling to outside carriers. */
internal fun laneColor(lane: LaneDto): Color = when {
    lane.outsideLots > 0 || lane.fillBps >= 9_000 -> Palette.negative
    lane.fillBps >= 6_000 -> Palette.warning
    else -> Palette.positive
}

/** Draws the busiest lanes between region anchors, wider with more traffic and coloured by congestion. */
internal fun DrawScope.drawLanes(map: WorldMap, projection: MapProjection, lanes: List<LaneDto>, focus: String?) {
    val busiest = lanes.sortedByDescending { it.capacityLots + it.outsideLots }.take(90)
    val top = busiest.maxOfOrNull { it.capacityLots + it.outsideLots }?.coerceAtLeast(1) ?: return
    busiest.forEach { lane ->
        val a = map.anchor(lane.a)?.let { projection.project(it) } ?: return@forEach
        val b = map.anchor(lane.b)?.let { projection.project(it) } ?: return@forEach
        val weight = (lane.capacityLots + lane.outsideLots).toFloat() / top
        val dim = if (focus != null && lane.a != focus && lane.b != focus) 0.35f else 0.9f
        drawLine(
            color = laneColor(lane).copy(alpha = dim),
            start = a, end = b,
            strokeWidth = 1.5f + 5f * weight,
            pathEffect = if (lane.modeId == "sea") PathEffect.dashPathEffect(floatArrayOf(12f, 9f)) else null,
        )
    }
}

@Composable
private fun Pill(text: String, on: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (on) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(CircleShape).background(if (on) Palette.gold else Palette.panelRaised)
            .clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

private val policyNames = listOf("Cheapest", "Fastest", "Own first", "Reliable")
private val modeNames = listOf("Any", "Truck", "Rail", "Pipeline", "Ship")

/**
 * A division's standing shipping policy for the freight it pays for, and what a carrier division
 * carries. Set once; the engine routes every week.
 */
@Composable
internal fun DivisionShipping(store: GameStore, division: DivisionDto) {
    val plant = division.plants.firstOrNull() ?: return
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("lane", size = 12.dp, tint = Palette.secondary, modifier = Modifier.padding(end = 6.dp))
            Eyebrow("Shipping · ${division.regionName}")
        }
        if (division.carrierMode.isNotEmpty()) {
            val where = if (division.serves.isEmpty()) "the busiest lanes in reach" else division.serves.joinToString(", ")
            Text(
                "Carries freight by ${division.carrierMode}, on $where. Its capacity is sold to every shipper; your group ships on it first, at cost.",
                color = Palette.secondary, fontSize = 11.sp,
            )
            return@Column
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            policyNames.forEachIndexed { kind, name ->
                Pill(name, division.shipKind.toInt() == kind) {
                    store.setShippingPolicy(plant, kind, division.shipMode.takeIf { it >= 0 })
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            modeNames.forEachIndexed { index, name ->
                val pick = index - 1
                Pill(name, division.shipMode == pick) { store.setShippingPolicy(plant, division.shipKind.toInt(), pick.takeIf { it >= 0 }) }
            }
        }
        Text(
            "Applies to goods arriving here. A preferred mode is used when a route allows it.",
            color = Palette.secondary, fontSize = 11.sp,
        )
    }
}

/** Corridor freight at a glance, then the lanes: capacity, owners, rate, and a contract action. */
@Composable
internal fun FreightPanel(store: GameStore, focus: String?, modifier: Modifier = Modifier) {
    val summary = store.freight
    var open by remember { mutableStateOf<String?>(null) }
    val lanes = store.lanes
        .filter { focus == null || it.a == focus || it.b == focus }
        .sortedByDescending { it.capacityLots + it.outsideLots }
        .take(14)
    Column(modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (summary != null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                PixelIcon("lane", size = 14.dp, tint = Palette.gold, modifier = Modifier.padding(end = 6.dp))
                Text(
                    "${Format.count(summary.movedLots)} lots moved · ${summary.outsideShareBps / 100}% by outside carriers · ${summary.congestedLanes} congested",
                    color = Palette.primaryText, fontSize = 12.sp,
                )
            }
        }
        if (lanes.isEmpty()) {
            Text("No freight capacity here yet. Build trucking, rail, a pipeline or shipping to own a lane.", color = Palette.secondary, fontSize = 12.sp)
        }
        lanes.forEach { lane ->
            val key = "${lane.a}-${lane.b}-${lane.mode}"
            Column(Modifier.fillMaxWidth().clickable { open = if (open == key) null else key }) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    PixelIcon(modeArt(lane.modeId), size = 14.dp, tint = laneColor(lane), modifier = Modifier.padding(end = 6.dp))
                    Text("${lane.aName} to ${lane.bName}", color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 1)
                    Text("${lane.fillBps / 100}% · ${Format.price(lane.rateCentsPerLot)} a lot", color = laneColor(lane), fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                }
                if (open == key) LaneDetail(store, lane)
            }
        }
    }
}

@Composable
private fun LaneDetail(store: GameStore, lane: LaneDto) {
    Column(Modifier.fillMaxWidth().padding(start = 20.dp, bottom = 6.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(
            "${lane.modeName} · ${lane.km} km · ${Format.count(lane.usedLots.toLong())} of ${Format.count(lane.capacityLots.toLong())} lots used" +
                (if (lane.heldLots > 0) ", ${Format.count(lane.heldLots.toLong())} reserved" else "") +
                (if (lane.outsideLots > 0) ", ${Format.count(lane.outsideLots.toLong())} overflow to outside carriers at ${Format.price(lane.outsideRateCentsPerLot)}" else ""),
            color = Palette.secondary, fontSize = 11.sp,
        )
        if (lane.owners.isEmpty()) {
            Text("No firm owns capacity here; outside carriers haul it.", color = Palette.secondary, fontSize = 11.sp)
        }
        lane.owners.forEach { owner ->
            Text(
                "${owner.name}: ${Format.count(owner.lots.toLong())} lots",
                color = if (owner.mine) Palette.gold else Palette.primaryText, fontSize = 11.sp,
            )
        }
        val mine = store.laneContracts.filter { it.mine && it.a == lane.a && it.b == lane.b && it.mode == lane.mode }
        mine.forEach {
            Text("Your contract: ${Format.count(it.heldLots.toLong())} lots at ${Format.price(it.feeCentsPerLot)}, ${it.weeksLeft} weeks left", color = Palette.gold, fontSize = 11.sp)
        }
        val others = store.laneContracts.count { !it.mine && it.a == lane.a && it.b == lane.b && it.mode == lane.mode }
        if (others > 0) Text("$others rival contract${if (others == 1) "" else "s"} on this lane", color = Palette.warning, fontSize = 11.sp)
        if (lane.reservableLots >= 10 && mine.isEmpty() && store.selectedFirm != null) ReserveLane(store, lane)
    }
}

/** Reserve lots on the lane for a term at a locked rate: the way to keep rivals off a lane you depend on. */
@Composable
private fun ReserveLane(store: GameStore, lane: LaneDto) {
    val most = minOf(lane.reservableLots, (lane.capacityLots * 8L) / 10).coerceAtLeast(10).toDouble()
    var lots by remember(lane.a, lane.b, lane.mode) { mutableStateOf(minOf(100.0, most)) }
    var weeks by remember(lane.a, lane.b, lane.mode) { mutableStateOf(13.0) }
    HorizontalDivider(color = Palette.stroke)
    ValueSlider(
        lots, { lots = it }, 10.0..most, 10.0,
        label = { "Reserve ${Format.count(it.toLong())} lots a week" },
        detail = { "Locked at ${Format.price(lane.rateCentsPerLot * 9 / 10)} a lot, paid every week whether you ship or not. Rivals cannot use these lots." },
    )
    ValueSlider(weeks, { weeks = it }, 4.0..52.0, 1.0, label = { "${it.toInt()} weeks" })
    Row(verticalAlignment = Alignment.CenterVertically) {
        Spacer(Modifier.weight(1f))
        GoldButton("Reserve lane", Modifier.width(150.dp)) {
            store.reserveLane(lane.a, lane.b, lane.mode, lots.toInt(), weeks.toInt())
        }
    }
    Spacer(Modifier.height(2.dp))
}
