package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.LaneDto

/**
 * Markets: Freight and lanes. Every lane with its capacity, fill, price and your share; the lane's
 * detail, your reservations on it and the reserve form beside the grid, with the shipping policy
 * and the logistics figures for the company.
 */
@Composable
fun FreightScreen(store: GameStore) {
    val lanes = store.lanes
    var pick by remember { mutableStateOf<String?>(null) }
    val chosen = lanes.firstOrNull { laneKey(it) == pick } ?: lanes.firstOrNull { laneIsMine(it) } ?: lanes.maxByOrNull { it.usedLots }
    BoxWithConstraints(Modifier.fillMaxSize().padding(8.dp)) {
        val wide = maxWidth >= 900.dp || LocalPhonePanes.current != null
        Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            DataGrid(
                "Freight lanes", lanes, laneColumns(store), key = ::laneKey,
                views = GridViews("lanes", LaneViews, mine = ::laneIsMine, scopes = laneScopes()),
                modifier = Modifier.weight(1f).fillMaxHeight(),
                filterText = { "${it.aName} ${it.bName} ${it.modeName}" },
                selected = chosen?.let(::laneKey), onSelect = { pick = laneKey(it) }, onOpen = if (wide) null else { l: LaneDto -> pick = laneKey(l) },
                breakdown = { l ->
                    "${l.aName} to ${l.bName} by ${l.modeName.lowercase()}, ${l.km} km: room for ${Format.count(l.capacityLots.toLong())} loads, ${Format.count(l.usedLots.toLong())} used (${l.fillBps / 100}% full), " +
                        "${Format.money(l.rateCentsPerLot)} a load" + (if (l.outsideLots > 0) ", ${Format.count(l.outsideLots.toLong())} spilled to outside carriers" else "")
                },
                initialSort = "fill",
            )
            if (wide) FreightPane(store, chosen, Modifier.width(430.dp).fillMaxHeight())
        }
    }
}

@Composable
private fun FreightPane(store: GameStore, lane: LaneDto?, modifier: Modifier) {
    DetailPane(lane?.let { "${it.aName} to ${it.bName}" } ?: "Freight", modifier) {
        store.freight?.let { f ->
            FigureLine(
                Triple("Moved", Format.count(f.movedLots), Palette.primaryText),
                Triple("Outside", "${f.outsideShareBps / 100}%", if (f.outsideShareBps > 2_000) Palette.warning else Palette.primaryText),
                Triple("Congested", "${f.congestedLanes}", if (f.congestedLanes > 0u) Palette.warning else Palette.primaryText),
                Triple("Paid, week", Format.compact(f.paidCents), Palette.primaryText),
            )
        }
        if (lane == null) {
            Text("No freight lanes yet. Build trucking, rail, a pipeline or shipping to own one.", color = Palette.secondary, fontSize = 12.sp)
        } else {
            val mine = store.laneContracts.filter { it.mine && it.a == lane.a && it.b == lane.b && it.mode == lane.mode }
            val rivals = store.laneContracts.count { !it.mine && it.a == lane.a && it.b == lane.b && it.mode == lane.mode }
            FigureLine(
                Triple("Full", "${lane.fillBps / 100}%", laneColor(lane)),
                Triple("Price", Format.money(lane.rateCentsPerLot), Palette.primaryText),
                Triple("Room", Format.count(lane.capacityLots.toLong()), Palette.primaryText),
                Triple("Your share", Format.count(lane.myShippedLots.toLong()), Palette.gold),
            )
            Text(
                "${lane.km} km by ${lane.modeName.lowercase()}. ${Format.count(lane.usedLots.toLong())} loads used" +
                    (if (lane.heldLots > 0) ", ${Format.count(lane.heldLots.toLong())} reserved" else "") + ". " +
                    (if (lane.outsideLots > 0) "${Format.count(lane.outsideLots.toLong())} spilled to outside carriers at ${Format.money(lane.outsideRateCentsPerLot)} each. " else "") +
                    (if (lane.owners.isEmpty()) "Nobody owns trucks, rail or ships here, so outside carriers haul it." else "Owned by " + lane.owners.joinToString(", ") { "${it.name}${if (it.mine) " (you)" else ""} ${Format.count(it.lots.toLong())}" } + "."),
                color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
            )
            mine.forEach {
                Text("Your reservation: ${Format.count(it.heldLots.toLong())} loads at ${Format.money(it.feeCentsPerLot)}, ${it.weeksLeft} weeks left.", color = Palette.gold, fontSize = 12.sp)
            }
            if (rivals > 0) Text("$rivals rival reservation${if (rivals == 1) "" else "s"} on this lane.", color = Palette.warning, fontSize = 12.sp)
            if (lane.reservableLots >= 10 && mine.isEmpty() && store.selectedFirm != null) ReserveLane(store, lane)
            else if (mine.isEmpty() && lane.reservableLots < 10) Text("Not enough free room on this lane to reserve.", color = Palette.secondary, fontSize = 11.sp)
        }
        // The shipping policy and what logistics costs the company, from the same engine figures as the old Office panel.
        ShippingSection(store, null)
        LogisticsStatsSection(store)
    }
}

private fun laneColumns(store: GameStore): List<GridColumn<LaneDto>> = listOf(
    GridColumn("name", "Lane", null, minWidth = 190.dp, numeric = false, text = { "${it.aName} to ${it.bName}" }, tone = { if (laneIsMine(it)) Palette.gold else Palette.primaryText }, bold = true,
        lead = { l -> PixelIcon(modeArt(l.modeId), size = 14.dp, tint = laneColor(l), modifier = Modifier.padding(end = 6.dp)) }),
    GridColumn("mode", "Mode", 76.dp, numeric = false, text = { it.modeName }, tone = { Palette.secondary }, topic = "About"),
    GridColumn("km", "Km", 56.dp, text = { Format.count(it.km.toLong()) }, sort = { it.km.toLong() }, topic = "About"),
    GridColumn("cap", "Room", 70.dp, text = { Format.count(it.capacityLots.toLong()) }, sort = { it.capacityLots.toLong() }, total = true, topic = "Capacity", tip = "Loads a week the lane can carry."),
    GridColumn("used", "Used", 70.dp, text = { Format.count(it.usedLots.toLong()) }, sort = { it.usedLots.toLong() }, total = true, topic = "Capacity"),
    GridColumn("fill", "Full", 90.dp, text = { "${it.fillBps / 100}%" }, sort = { it.fillBps.toLong() }, bar = { it.fillBps / 10_000f }, tone = { laneColor(it) }, scale = 100.0, topic = "Capacity",
        tip = "Green while the lane has room, amber as it fills, red when it is full or spilling to outside carriers."),
    GridColumn("price", "Price", 76.dp, text = { Format.money(it.rateCentsPerLot) }, sort = { it.rateCentsPerLot }, scale = 100.0, topic = "Money", tip = "What a load costs on the lane today."),
    GridColumn("outside", "Spilled", 70.dp, text = { if (it.outsideLots > 0) Format.count(it.outsideLots.toLong()) else "" }, sort = { it.outsideLots.toLong() }, tone = { Palette.negative }, total = true, topic = "Capacity",
        tip = "Loads that did not fit and went to outside carriers at a premium."),
    GridColumn("outprice", "Outside price", 92.dp, text = { if (it.outsideLots > 0) Format.money(it.outsideRateCentsPerLot) else "" }, sort = { it.outsideRateCentsPerLot }, scale = 100.0, topic = "Money"),
    GridColumn("hot", "Hot weeks", 76.dp, text = { if (it.hotWeeks > 0u) "${it.hotWeeks}" else "" }, sort = { it.hotWeeks.toLong() }, topic = "Capacity", tip = "Weeks in a row the lane has run close to full."),
    GridColumn("owners", "Carriers", 66.dp, text = { "${it.owners.size}" }, sort = { it.owners.size.toLong() }, topic = "About"),
    GridColumn("myroom", "Your room", 80.dp, text = { if (it.myCapacityLots > 0) Format.count(it.myCapacityLots.toLong()) else "" }, sort = { it.myCapacityLots.toLong() }, tone = { Palette.gold }, total = true, topic = "You"),
    GridColumn("myship", "You shipped", 90.dp, text = { if (it.myShippedLots > 0) Format.count(it.myShippedLots.toLong()) else "" }, sort = { it.myShippedLots.toLong() }, tone = { Palette.gold }, total = true, topic = "You"),
    GridColumn("myres", "Your reserved", 94.dp, text = { if (it.myContractLots > 0) Format.count(it.myContractLots.toLong()) else "" }, sort = { it.myContractLots.toLong() }, tone = { Palette.gold }, total = true, topic = "You"),
    GridColumn("held", "Reserved", 76.dp, text = { if (it.heldLots > 0) Format.count(it.heldLots.toLong()) else "" }, sort = { it.heldLots.toLong() }, total = true, topic = "Capacity", tip = "Loads held back by reservations, yours and rivals'."),
    GridColumn("free", "To reserve", 84.dp, text = { Format.count(it.reservableLots) }, sort = { it.reservableLots }, topic = "Capacity", tip = "Loads a week you could still reserve here."),
)

private fun laneScopes(): List<ScopeOption<LaneDto>> =
    listOf("Truck" to "truck", "Rail" to "rail", "Pipeline" to "pipeline", "Ship" to "sea").map { (label, id) -> ScopeOption<LaneDto>("mode:$id", label, "Mode") { it.modeId == id } }

internal val LaneViews = listOf(
    ViewState("Busiest", cols("name", "mode", "cap", "used", "fill", "price", "outside", "myship"), sort = "fill"),
    ViewState("Prices", cols("name", "mode", "km", "price", "outprice", "outside", "fill"), sort = "price"),
    ViewState("Congested", cols("name", "mode", "fill", "hot", "outside", "outprice", "free"), sort = "outside", filters = listOf(Filter(FilterOp.Above, "fill", number = 85.0))),
    ViewState("My lanes", cols("name", "mode", "myroom", "myship", "myres", "price", "fill"), sort = "myship", filters = listOf(Filter(FilterOp.Mine))),
    ViewState("Reservations", cols("name", "mode", "myres", "held", "free", "price", "fill"), sort = "myres", filters = listOf(Filter(FilterOp.Above, "myres", number = 0.0))),
)
