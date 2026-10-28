package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.DivisionDto
import uniffi.cw_ffi.LaneDto
import uniffi.cw_ffi.PolicyOptionDto
import uniffi.cw_ffi.ReserveQuoteDto
import uniffi.cw_ffi.ShippingSuggestionDto
import uniffi.cw_ffi.ShippingSummaryDto
import kotlin.math.abs
import kotlin.math.hypot

/** The pixel icon for a freight mode id. */
internal fun modeArt(modeId: String): String = if (modeId == "sea") "ship" else modeId

/** Green while a lane has room, amber as it fills, red when it is full or spilling to outside carriers. */
internal fun laneColor(lane: LaneDto): Color = when {
    lane.outsideLots > 0 || lane.fillBps >= 9_000 -> Palette.negative
    lane.fillBps >= 6_000 -> Palette.warning
    else -> Palette.positive
}

internal fun laneKey(lane: LaneDto): String = "${lane.a}-${lane.b}-${lane.mode}"

/** Whether the player's group owns capacity on the lane, ships on it or holds a contract there. */
internal fun laneIsMine(lane: LaneDto): Boolean = lane.myCapacityLots > 0 || lane.myShippedLots > 0 || lane.myContractLots > 0

private fun busiestLanes(lanes: List<LaneDto>): List<LaneDto> =
    lanes.sortedByDescending { it.capacityLots + it.outsideLots }.take(90)

/**
 * Draws the busiest lanes between region anchors: thicker with more freight, coloured by how full
 * they are, dashed for sea. Lanes the player owns or ships on get a gold edge; the lane picked for
 * the panel gets a white one.
 */
internal fun DrawScope.drawLanes(map: WorldMap, projection: MapProjection, lanes: List<LaneDto>, focus: String?, picked: String? = null) {
    val busiest = busiestLanes(lanes)
    val top = busiest.maxOfOrNull { it.capacityLots + it.outsideLots }?.coerceAtLeast(1) ?: return
    busiest.forEach { lane ->
        val a = map.anchor(lane.a)?.let { projection.project(it) } ?: return@forEach
        val b = map.anchor(lane.b)?.let { projection.project(it) } ?: return@forEach
        val weight = (lane.capacityLots + lane.outsideLots).toFloat() / top
        val dim = if (focus != null && lane.a != focus && lane.b != focus && picked == null) 0.35f else 0.9f
        val width = 1.5f + 5f * weight
        val dash = if (lane.modeId == "sea") PathEffect.dashPathEffect(floatArrayOf(12f, 9f)) else null
        if (laneKey(lane) == picked) {
            drawLine(Color.White.copy(alpha = 0.9f), a, b, strokeWidth = width + 7f, pathEffect = dash)
        } else if (laneIsMine(lane)) {
            drawLine(Palette.gold.copy(alpha = 0.75f), a, b, strokeWidth = width + 4f, pathEffect = dash)
        }
        drawLine(color = laneColor(lane).copy(alpha = dim), start = a, end = b, strokeWidth = width, pathEffect = dash)
    }
}

/** The busiest lanes as faint lines under the moving freight, outside the Freight layer. */
internal fun DrawScope.drawFaintLanes(map: WorldMap, projection: MapProjection, lanes: List<LaneDto>) {
    busiestLanes(lanes).take(60).forEach { lane ->
        val a = map.anchor(lane.a)?.let { projection.project(it) } ?: return@forEach
        val b = map.anchor(lane.b)?.let { projection.project(it) } ?: return@forEach
        drawLine(laneColor(lane).copy(alpha = 0.10f), a, b, strokeWidth = 1f)
    }
}

/** The lane drawn nearest a tap, within a finger's width, or null. */
internal fun laneAt(map: WorldMap, projection: MapProjection, lanes: List<LaneDto>, tap: Offset): LaneDto? {
    var best: LaneDto? = null
    var bestDistance = 22f
    busiestLanes(lanes).forEach { lane ->
        val a = map.anchor(lane.a)?.let { projection.project(it) } ?: return@forEach
        val b = map.anchor(lane.b)?.let { projection.project(it) } ?: return@forEach
        val dx = b.x - a.x
        val dy = b.y - a.y
        val len2 = dx * dx + dy * dy
        val t = if (len2 <= 0f) 0f else (((tap.x - a.x) * dx + (tap.y - a.y) * dy) / len2).coerceIn(0f, 1f)
        val d = hypot(tap.x - (a.x + t * dx), tap.y - (a.y + t * dy))
        if (d < bestDistance) {
            bestDistance = d
            best = lane
        }
    }
    return best
}

@Composable
private fun Pill(text: String, on: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (on) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.clip(Pixel.small).background(if (on) Palette.gold else Palette.panelRaised)
            .clickable(onClick = onClick).padding(horizontal = 10.dp, vertical = 4.dp),
    )
}

private fun firmMoney(store: GameStore, cents: Long): String = Format.money(cents, store.selectedFirmDto?.currency.orEmpty())

private fun firmCompact(store: GameStore, cents: Long): String = Format.compact(cents, store.selectedFirmDto?.currency.orEmpty())

private fun firmPrice(store: GameStore, cents: Long): String = Format.price(cents, store.selectedFirmDto?.currency.orEmpty())

private fun percent(bps: Int): String = "${(bps + 50) / 100}%"

private fun changeText(bps: Int): String = when {
    abs(bps) < 100 -> "about the same"
    bps < 0 -> "${-bps / 100}% less"
    else -> "${bps / 100}% more"
}

@Composable
private fun LegendDot(color: Color, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(Pixel.small).background(color))
        Spacer(Modifier.width(4.dp))
        Text(text, color = Palette.secondary, fontSize = 11.sp)
    }
}

/** What the lane colours, thickness and gold edge on the Freight map mean. */
@Composable
internal fun FreightLegend() {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            LegendDot(Palette.positive, "Room")
            LegendDot(Palette.warning, "Filling up")
            LegendDot(Palette.negative, "Full")
        }
        Text("Thicker line, more freight. Gold edge, you own or ship on it. Tap a line for details.", color = Palette.secondary, fontSize = 11.sp)
    }
}

/** One division's (or, with a null [facility], the whole company's) freight for the last week, read from the engine. */
@Composable
private fun rememberShipping(store: GameStore, facility: ULong?): ShippingSummaryDto? {
    val firm = store.selectedFirm
    val revision = store.overview?.revision
    var summary by remember(firm, facility) { mutableStateOf<ShippingSummaryDto?>(null) }
    LaunchedEffect(firm, facility, revision) {
        if (firm != null) store.shipping(firm, facility ?: 0uL) { summary = it }
    }
    return summary
}

@Composable
private fun PolicyRow(store: GameStore, option: PolicyOptionDto, onPick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(Pixel.shape)
            .background(if (option.selected) Palette.gold.copy(alpha = 0.14f) else Palette.panelRaised.copy(alpha = 0.5f))
            .clickable(onClick = onPick).padding(horizontal = 8.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(option.label, color = if (option.selected) Palette.gold else Palette.primaryText, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
            Text(option.blurb, color = Palette.secondary, fontSize = 11.sp)
        }
        Spacer(Modifier.width(8.dp))
        Column(horizontalAlignment = Alignment.End) {
            if (option.hasEstimate) {
                Text("${firmCompact(store, option.weeklyCents)} a week", color = Palette.primaryText, fontSize = 11.sp)
                Text(
                    if (option.selected) "your choice" else changeText(option.changeBps),
                    color = when { option.selected -> Palette.gold; option.changeBps <= -100 -> Palette.positive; option.changeBps >= 100 -> Palette.negative; else -> Palette.secondary },
                    fontSize = 11.sp,
                )
            } else if (option.selected) {
                Text("your choice", color = Palette.gold, fontSize = 11.sp)
            }
        }
    }
}

/**
 * Freight in plain words for a division (pass its [division]) or, with none, the whole company:
 * what shipping costs a week, who is paid, the busiest lanes, the route policy and one suggestion.
 * A division also gets the policy picker, with each choice priced before it is picked.
 */
@Composable
internal fun ShippingSection(store: GameStore, division: DivisionDto?) {
    val plant = division?.plants?.firstOrNull()
    if (division != null && plant == null) return
    val carrier = division != null && division.carrierMode.isNotEmpty()
    val summary = if (carrier) null else rememberShipping(store, plant)
    var building by remember { mutableStateOf<ShippingSuggestionDto?>(null) }
    building?.let { s ->
        val board = store.boards.firstOrNull { it.code == s.a }
        if (board != null) BuildDialog(store, board, initialRecipe = s.recipe.toInt(), initialBlocks = s.lots.toInt()) { building = null }
    }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(5.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("lane", size = 12.dp, tint = Palette.secondary, modifier = Modifier.padding(end = 6.dp))
            Eyebrow(if (division != null) "Shipping into ${division.regionName}" else "Shipping")
        }
        if (division != null && carrier) {
            val where = if (division.serves.isEmpty()) "the busiest lanes in reach" else division.serves.joinToString(", ")
            Text(
                "Carries freight by ${division.carrierMode.lowercase()} on $where. Every shipper can use its capacity; your group goes first and pays cost.",
                color = Palette.secondary, fontSize = 11.sp,
            )
            return@Column
        }
        if (summary == null) {
            Text("Reading last week's freight.", color = Palette.secondary, fontSize = 11.sp)
            return@Column
        }
        Text(summary.policyText, color = Palette.primaryText, fontSize = 12.sp)
        if (!summary.hasData) {
            Text(
                if (division != null) "Nothing was shipped in last week, so there is no bill yet. A load is one lot of goods."
                else "No freight bill yet. It fills in after the first week with deliveries. A load is one lot of goods.",
                color = Palette.secondary, fontSize = 11.sp,
            )
        } else {
            Text(
                "Freight bill ${firmMoney(store, summary.weeklyCents)} a week, ${firmPrice(store, summary.centsPerLot)} a load across ${Format.count(summary.lots)} loads." +
                    (if (summary.feeCents > 0) " Includes ${firmMoney(store, summary.feeCents)} in lane reservations." else ""),
                color = Palette.primaryText, fontSize = 12.sp,
            )
            Text(
                "Paid to your carriers ${percent(summary.ownShareBps)}, other firms' carriers ${percent(summary.otherShareBps)}, outside carriers ${percent(summary.outsideShareBps)}.",
                color = Palette.secondary, fontSize = 11.sp,
            )
            if (summary.outsideExtraCents > 0) {
                Text(
                    "Outside carriers charge a premium: ${firmMoney(store, summary.outsideExtraCents)} a week more than your own carriers would.",
                    color = Palette.warning, fontSize = 11.sp,
                )
            }
            summary.lanes.forEach { lane ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    PixelIcon(modeArt(lane.modeId), size = 13.dp, tint = if (lane.congested) Palette.negative else Palette.positive, modifier = Modifier.padding(end = 6.dp))
                    Text("${lane.aName} to ${lane.bName}", color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 1)
                    Text(
                        "${Format.count(lane.lots)} loads, ${firmCompact(store, lane.cents)}" + (if (lane.congested) ", congested" else ""),
                        color = if (lane.congested) Palette.negative else Palette.secondary, fontSize = 11.sp,
                    )
                }
            }
        }
        summary.suggestion?.let { s ->
            val line = if (s.kind == "build") {
                "${s.headline}. It costs ${firmMoney(store, s.upfrontCents)} and saves about ${firmMoney(store, s.weeklySavingCents)} a week, so it pays back in ${s.paybackWeeks} weeks."
            } else {
                "${s.headline}. At last week's volume it saves about ${firmMoney(store, s.weeklySavingCents)} a week with no upfront cost."
            }
            Callout(line, Palette.gold)
            if (s.kind == "build" && store.selectedFirm != null) {
                Row(Modifier.fillMaxWidth()) {
                    Spacer(Modifier.weight(1f))
                    GoldButton("Review this build", Modifier.padding(top = 2.dp)) { building = s }
                }
            }
            if (s.kind == "reserve" && store.selectedFirm != null) {
                Row(Modifier.fillMaxWidth()) {
                    Spacer(Modifier.weight(1f))
                    GoldButton("Reserve ${s.lots} loads for 13 weeks", Modifier.padding(top = 2.dp)) {
                        store.reserveLane(s.a, s.b, s.mode, s.lots.toInt(), 13)
                    }
                }
            }
        }
        if (division != null && plant != null && summary.kinds.isNotEmpty()) {
            Text("How should goods get here? Pick once; the engine routes every week.", color = Palette.secondary, fontSize = 11.sp)
            summary.kinds.forEach { option ->
                PolicyRow(store, option) { store.setShippingPolicy(plant, option.id, division.shipMode.takeIf { it >= 0 }) }
            }
            Text("Preferred vehicle", color = Palette.secondary, fontSize = 11.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                summary.modes.forEach { option ->
                    val tail = if (!option.selected && option.hasEstimate && abs(option.changeBps) >= 100) " ${if (option.changeBps < 0) "-" else "+"}${abs(option.changeBps) / 100}%" else ""
                    Pill(option.label + tail, option.selected) {
                        store.setShippingPolicy(plant, division.shipKind.toInt(), option.id.takeIf { it >= 0 })
                    }
                }
            }
            summary.modes.firstOrNull { it.selected }?.let {
                Text(it.blurb + " Percentages show the change in this division's freight bill at today's prices.", color = Palette.secondary, fontSize = 11.sp)
            }
        }
    }
}

/** A division's shipping panel: the Shipping section with its policy picker. */
@Composable
internal fun DivisionShipping(store: GameStore, division: DivisionDto) = ShippingSection(store, division)

/** The whole company's freight, for the CEO Office. */
@Composable
internal fun CompanyShipping(store: GameStore) = ShippingSection(store, null)

/** Corridor freight at a glance, then the lanes, then a simple panel for the lane tapped. */
@Composable
internal fun FreightPanel(store: GameStore, focus: String?, picked: String?, onPick: (String?) -> Unit, modifier: Modifier = Modifier) {
    val summary = store.freight
    val pickedLane = picked?.let { key -> store.lanes.firstOrNull { laneKey(it) == key } }
    val lanes = store.lanes
        .filter { focus == null || it.a == focus || it.b == focus }
        .sortedWith(compareByDescending<LaneDto> { laneIsMine(it) }.thenByDescending { it.capacityLots + it.outsideLots })
        .take(14)
    Column(modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (pickedLane != null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Back to all lanes", color = Palette.gold, fontSize = 12.sp, modifier = Modifier.clickable { onPick(null) }.padding(vertical = 2.dp))
            }
            LaneDetail(store, pickedLane)
            return@Column
        }
        FreightLegend()
        if (summary != null) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                PixelIcon("lane", size = 14.dp, tint = Palette.gold, modifier = Modifier.padding(end = 6.dp))
                Text(
                    "Last week ${Format.count(summary.movedLots)} loads moved on ${summary.lanesWithCapacity} lanes. " +
                        "${summary.outsideShareBps / 100}% went to outside carriers at a premium. ${summary.congestedLanes} lanes are congested.",
                    color = Palette.primaryText, fontSize = 12.sp,
                )
            }
        }
        if (lanes.isEmpty()) {
            Text("No freight lanes here yet. Build trucking, rail, a pipeline or shipping to own one.", color = Palette.secondary, fontSize = 12.sp)
        }
        lanes.forEach { lane ->
            Row(Modifier.fillMaxWidth().clickable { onPick(laneKey(lane)) }.padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                PixelIcon(modeArt(lane.modeId), size = 14.dp, tint = laneColor(lane), modifier = Modifier.padding(end = 6.dp))
                Column(Modifier.weight(1f)) {
                    Text("${lane.aName} to ${lane.bName}", color = Palette.primaryText, fontSize = 12.sp, maxLines = 1)
                    if (laneIsMine(lane)) Text("You ship ${Format.count(lane.myShippedLots.toLong())} loads here", color = Palette.gold, fontSize = 11.sp)
                }
                Text("${lane.fillBps / 100}% full · ${firmPrice(store, lane.rateCentsPerLot)} a load", color = laneColor(lane), fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
            }
        }
    }
}

@Composable
private fun LaneDetail(store: GameStore, lane: LaneDto) {
    val mine = store.laneContracts.filter { it.mine && it.a == lane.a && it.b == lane.b && it.mode == lane.mode }
    val rivals = store.laneContracts.count { !it.mine && it.a == lane.a && it.b == lane.b && it.mode == lane.mode }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon(modeArt(lane.modeId), size = 16.dp, tint = laneColor(lane), modifier = Modifier.padding(end = 6.dp))
            Text("${lane.aName} to ${lane.bName} by ${lane.modeName.lowercase()}", color = Palette.primaryText, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        }
        Text(
            "${lane.fillBps / 100}% full. Room for ${Format.count(lane.capacityLots.toLong())} loads a week over ${lane.km} km; " +
                "${Format.count(lane.usedLots.toLong())} used" + (if (lane.heldLots > 0) ", ${Format.count(lane.heldLots.toLong())} reserved" else "") + ".",
            color = Palette.secondary, fontSize = 12.sp,
        )
        Text(
            "Price ${firmPrice(store, lane.rateCentsPerLot)} a load today.",
            color = Palette.primaryText, fontSize = 12.sp,
        )
        if (lane.outsideLots > 0) {
            Text(
                "${Format.count(lane.outsideLots.toLong())} loads spilled to outside carriers at ${firmPrice(store, lane.outsideRateCentsPerLot)} each.",
                color = Palette.negative, fontSize = 12.sp,
            )
        }
        if (lane.owners.isEmpty()) {
            Text("No firm owns trucks, rail or ships here, so outside carriers haul it.", color = Palette.secondary, fontSize = 12.sp)
        } else {
            Text("Owned by", color = Palette.secondary, fontSize = 11.sp)
            lane.owners.forEach { owner ->
                Text(
                    "${owner.name}${if (owner.mine) " (you)" else ""}: room for ${Format.count(owner.lots.toLong())} loads",
                    color = if (owner.mine) Palette.gold else Palette.primaryText, fontSize = 12.sp,
                )
            }
        }
        if (laneIsMine(lane)) {
            Text(
                "Your share: you own room for ${Format.count(lane.myCapacityLots.toLong())} loads and shipped ${Format.count(lane.myShippedLots.toLong())} here last week.",
                color = Palette.gold, fontSize = 12.sp,
            )
        } else {
            Text("You do not ship on this lane.", color = Palette.secondary, fontSize = 12.sp)
        }
        mine.forEach {
            Text("Your reservation: ${Format.count(it.heldLots.toLong())} loads at ${firmPrice(store, it.feeCentsPerLot)}, ${it.weeksLeft} weeks left", color = Palette.gold, fontSize = 12.sp)
        }
        if (rivals > 0) Text("$rivals rival reservation${if (rivals == 1) "" else "s"} on this lane", color = Palette.warning, fontSize = 12.sp)
        if (lane.reservableLots >= 10 && mine.isEmpty() && store.selectedFirm != null) ReserveLane(store, lane)
    }
}

/** Reserve loads on the lane for a term at a locked price, quoted by the engine before you commit. */
@Composable
private fun ReserveLane(store: GameStore, lane: LaneDto) {
    val most = minOf(lane.reservableLots, (lane.capacityLots * 8L) / 10).coerceAtLeast(10).toDouble()
    var lots by remember(laneKey(lane)) { mutableStateOf(minOf(100.0, most)) }
    var weeks by remember(laneKey(lane)) { mutableStateOf(13.0) }
    var quote by remember(laneKey(lane)) { mutableStateOf<ReserveQuoteDto?>(null) }
    val revision = store.overview?.revision
    LaunchedEffect(laneKey(lane), lots.toInt(), weeks.toInt(), revision) {
        store.reserveQuote(lane, lots.toInt(), weeks.toInt()) { quote = it }
    }
    HorizontalDivider(color = Palette.stroke)
    Text("Reserve loads: a locked price that rivals cannot take from you", color = Palette.secondary, fontSize = 11.sp)
    ValueSlider(
        lots, { lots = it }, 10.0..most, 10.0,
        label = { "${Format.count(it.toLong())} loads a week" },
        detail = { "Rivals cannot use these loads. You pay for them every week, shipped or not." },
    )
    ValueSlider(weeks, { weeks = it }, 4.0..52.0, 1.0, label = { "${it.toInt()} weeks" })
    quote?.takeIf { it.ok }?.let { q ->
        Text(
            "${firmPrice(store, q.feeCentsPerLot)} a load against ${firmPrice(store, q.spotCentsPerLot)} today. " +
                "${firmMoney(store, q.weeklyFeeCents)} a week, ${firmMoney(store, q.totalCents)} over ${weeks.toInt()} weeks.",
            color = Palette.primaryText, fontSize = 12.sp,
        )
        Text(
            if (q.weeklyNetCents >= 0) "At last week's volume (${Format.count(q.expectedUseLots.toLong())} loads) you save about ${firmMoney(store, q.weeklyNetCents)} a week."
            else "At last week's volume (${Format.count(q.expectedUseLots.toLong())} loads) it costs about ${firmMoney(store, -q.weeklyNetCents)} a week more than shipping at today's price. It pays if prices rise or you ship more.",
            color = if (q.weeklyNetCents >= 0) Palette.positive else Palette.warning, fontSize = 12.sp,
        )
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Spacer(Modifier.weight(1f))
        GoldButton("Reserve ${lots.toInt()} loads", Modifier.width(190.dp)) {
            store.reserveLane(lane.a, lane.b, lane.mode, lots.toInt(), weeks.toInt())
        }
    }
    Spacer(Modifier.height(2.dp))
}
