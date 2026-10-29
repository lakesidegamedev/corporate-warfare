package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.drawscope.Stroke

import androidx.compose.ui.graphics.StrokeCap

import androidx.compose.ui.graphics.Path

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
import kotlin.math.acos
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.roundToInt

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

/** Lanes drawn: the 30 busiest, and every lane the player uses wherever it ranks. */
private const val SHOWN_LANES = 30

/** How strongly rivals' and outside carriers' stripes are drawn, so the player's gold reads first. */
private const val OTHERS_ALPHA = 0.5f

/** The lanes drawn, busiest first. */
internal fun busiestLanes(lanes: List<LaneDto>): List<LaneDto> =
    lanes.sortedByDescending { it.capacityLots + it.outsideLots }.filterIndexed { i, lane -> i < SHOWN_LANES || laneIsMine(lane) }

/** Company colours for the Freight mode: the player always gold, rivals from a fixed set by firm. */
private val RivalColors = listOf(
    Color(0xFF42A5F5), Color(0xFFEF5350), Color(0xFF66BB6A), Color(0xFFAB47BC),
    Color(0xFF26C6DA), Color(0xFFFF8A65), Color(0xFFEC407A), Color(0xFF9CCC65),
)
private val OutsideCarriers = Color(0xFF8A939C)

internal fun companyColor(firm: ULong, mine: Boolean): Color =
    if (mine) Palette.gold else RivalColors[((firm % RivalColors.size.toULong()).toInt())]

/** One company's stripe on a lane: its colour and how many lots it carries there. */
internal class Stripe(val color: Color, val lots: Int, val name: String, val mine: Boolean = false)

/** Who carries a lane: each owner's stripe, biggest first, then outside carriers. */
internal fun stripes(lane: LaneDto): List<Stripe> {
    val owners = lane.owners.filter { it.lots > 0 }.sortedByDescending { it.lots }
        .map { Stripe(companyColor(it.firm, it.mine), it.lots, if (it.mine) "You" else it.name, it.mine) }
    val outside = if (lane.outsideLots > 0) listOf(Stripe(OutsideCarriers, lane.outsideLots, "Outside carriers")) else emptyList()
    return (owners + outside).ifEmpty { listOf(Stripe(OutsideCarriers, maxOf(lane.capacityLots, 1), "Idle")) }
}

/**
 * A lane's arc: from hub to hub, bowed to one side so arcs between nearby hubs stand apart, more
 * for rail and most for sea so the modes between the same two regions never overlap.
 */
private class Arc(val a: Offset, val c: Offset, val b: Offset) {
    fun at(t: Float): Offset {
        val u = 1 - t
        return Offset(u * u * a.x + 2 * u * t * c.x + t * t * b.x, u * u * a.y + 2 * u * t * c.y + t * t * b.y)
    }

    /** The arc moved [d] pixels to its side, for stacking stripes. */
    fun shifted(d: Float): Arc {
        val dir = b - a
        val len = dir.getDistance().coerceAtLeast(1e-3f)
        val n = Offset(-dir.y / len, dir.x / len) * d
        return Arc(a + n, c + n, b + n)
    }

    fun path(): Path = Path().apply { moveTo(a.x, a.y); quadraticTo(c.x, c.y, b.x, b.y) }

    /** Pixels along the arc. */
    fun length(): Float {
        var total = 0f
        var last = a
        for (k in 1..24) {
            val point = at(k / 24f)
            total += (point - last).getDistance()
            last = point
        }
        return total
    }
}

/**
 * A sea lane's dash pattern, a whole number of dashes along the arc from hub to hub, so the dashes
 * stay put on the lane as the camera moves instead of crawling from one end.
 */
private fun seaDash(arc: Arc, km: Double): PathEffect {
    val count = (km / 60).roundToInt().coerceAtLeast(3)
    val period = arc.length() / count
    return PathEffect.dashPathEffect(floatArrayOf(period * 0.62f, period * 0.38f))
}

/** Kilometres on the ground between a lane's two hubs. */
private fun hubKm(map: WorldMap, lane: LaneDto): Double {
    val a = map.anchor(lane.a) ?: return 0.0
    val b = map.anchor(lane.b) ?: return 0.0
    val dot = a.cosLat[0] * b.cosLat[0] * cos(a.lon[0] - b.lon[0]) + a.sinLat[0] * b.sinLat[0]
    return acos(dot.coerceIn(-1.0, 1.0)) * GroundDashes.EARTH_KM
}

private fun arcOf(map: WorldMap, projection: MapProjection, lane: LaneDto, plinth: Double): Arc? {
    // The same two hubs always bow the same way, whichever end is listed first.
    val (from, to) = if (lane.a < lane.b) lane.a to lane.b else lane.b to lane.a
    val a = map.anchor(from)?.let { projection.project(it, plinth) } ?: return null
    val b = map.anchor(to)?.let { projection.project(it, plinth) } ?: return null
    val d = b - a
    val len = d.getDistance()
    if (len < 2f) return null
    val bow = when (lane.modeId) { "rail" -> 0.26f; "sea" -> 0.38f; else -> 0.14f }
    val c = (a + b) / 2f + Offset(-d.y, d.x) * bow
    return Arc(a, c, b)
}

/**
 * The Freight mode: each of the busiest lanes as a clean arc between its hubs, one coloured stripe
 * per company that carries goods on it, side by side, each as wide as its share of the lane's
 * freight; your own in gold, everyone else's fainter. Only the 30 busiest lanes and your own are
 * drawn. Sea lanes are dashed. A dark casing keeps the stack readable on any
 * ground; the picked lane is outlined in white and lanes away from a selected region fade.
 */
internal fun DrawScope.drawLanes(map: WorldMap, projection: MapProjection, lanes: List<LaneDto>, focus: String?, picked: String? = null, plinth: Double = 0.0) {
    val busiest = busiestLanes(lanes)
    val top = busiest.maxOfOrNull { it.capacityLots + it.outsideLots }?.coerceAtLeast(1) ?: return
    // Thin, quiet lanes first so the busy ones lie on top, and the player's own over all.
    val (mine, others) = busiest.asReversed().partition(::laneIsMine)
    (others + mine).forEach { lane ->
        val arc = arcOf(map, projection, lane, plinth) ?: return@forEach
        val parts = stripes(lane)
        val total = parts.sumOf { it.lots }.coerceAtLeast(1)
        val width = 5f + 13f * (lane.capacityLots + lane.outsideLots).toFloat() / top
        val dim = if (focus != null && lane.a != focus && lane.b != focus && picked == null) 0.3f else 1f
        val dash = if (lane.modeId == "sea") seaDash(arc, hubKm(map, lane)) else null
        if (laneKey(lane) == picked) drawPath(arc.path(), Color.White, style = Stroke(width + 6f, cap = StrokeCap.Round))
        drawPath(arc.path(), Palette.ink.copy(alpha = 0.85f * dim), style = Stroke(width + 2.5f, cap = StrokeCap.Round))
        var offset = -width / 2
        parts.forEach { part ->
            val w = (width * part.lots / total).coerceAtLeast(1.2f)
            drawPath(arc.shifted(offset + w / 2).path(), part.color.copy(alpha = dim * if (part.mine) 1f else OTHERS_ALPHA), style = Stroke(w, cap = StrokeCap.Butt, pathEffect = dash))
            offset += w
        }
    }
    // The hubs the lanes meet at.
    busiest.flatMap { listOf(it.a, it.b) }.distinct().forEach { code ->
        val at = map.anchor(code)?.let { projection.project(it, plinth) } ?: return@forEach
        drawCircle(Palette.ink, 5f, at)
        drawCircle(Color.White, 3f, at)
    }
}

/** The lane drawn nearest a tap, within a finger's width, or null. */
internal fun laneAt(map: WorldMap, projection: MapProjection, lanes: List<LaneDto>, tap: Offset): LaneDto? {
    var best: LaneDto? = null
    var bestDistance = 22f
    busiestLanes(lanes).forEach { lane ->
        val arc = arcOf(map, projection, lane, 0.0) ?: return@forEach
        for (k in 0..24) {
            val d = (arc.at(k / 24f) - tap).getDistance()
            if (d < bestDistance) {
                bestDistance = d
                best = lane
            }
        }
    }
    return best
}

/** What the Freight mode's colours mean: you, the biggest rivals on the busiest lanes, outside carriers. */
@Composable
internal fun FreightLegend(lanes: List<LaneDto>, modifier: Modifier) {
    val carriers = busiestLanes(lanes).flatMap { it.owners }.filter { it.lots > 0 }
        .groupBy { it.firm }.map { (_, owners) -> owners.first() to owners.sumOf { it.lots } }
        .sortedWith(compareByDescending<Pair<uniffi.cw_ffi.LaneOwnerDto, Int>> { it.first.mine }.thenByDescending { it.second })
        .take(5)
    Column(
        modifier.width(190.dp).padding(end = 3.dp, bottom = 3.dp).pixelFrame(Palette.panel.copy(alpha = 0.94f)).padding(horizontal = 10.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text("FREIGHT", color = Palette.primaryText, fontSize = 10.sp, fontFamily = Pixel.font)
        carriers.forEach { (owner, _) ->
            LegendRow(companyColor(owner.firm, owner.mine).copy(alpha = if (owner.mine) 1f else OTHERS_ALPHA), if (owner.mine) "You" else owner.name)
        }
        LegendRow(OutsideCarriers.copy(alpha = OTHERS_ALPHA), "Outside carriers")
        Text("Wider: more freight. Dashed: by water.", color = Palette.secondary, fontSize = 10.sp)
    }
}

@Composable
private fun LegendRow(color: Color, label: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(width = 18.dp, height = 6.dp).background(color))
        Spacer(Modifier.width(6.dp))
        Text(label, color = Palette.primaryText, fontSize = 11.sp, maxLines = 1)
    }
}

@Composable
private fun Pill(text: String, on: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (on) Palette.ink else Palette.primaryText, fontSize = 11.sp, fontFamily = Pixel.font,
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
internal fun CompanyShipping(store: GameStore) {
    ShippingSection(store, null)
    LogisticsStatsSection(store)
}

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
internal fun ReserveLane(store: GameStore, lane: LaneDto, initialLots: Int? = null, initialWeeks: Int? = null) {
    val most = minOf(lane.reservableLots, (lane.capacityLots * 8L) / 10).coerceAtLeast(10).toDouble()
    var lots by remember(laneKey(lane)) { mutableStateOf(initialLots?.toDouble()?.coerceIn(10.0, most) ?: minOf(100.0, most)) }
    var weeks by remember(laneKey(lane)) { mutableStateOf(initialWeeks?.toDouble()?.coerceIn(4.0, 52.0) ?: 13.0) }
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
