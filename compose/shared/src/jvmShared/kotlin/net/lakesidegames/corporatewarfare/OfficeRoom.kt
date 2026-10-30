package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlin.math.floor
import kotlin.math.roundToInt

/**
 * The CEO Office as a room (office-room.json, drawn by tools/office_room.py, shared with the Apple
 * apps): a still picture, a front layer the people sit behind, a few swappable sprites, and for
 * every tappable object its rectangle, its spoken label and where live figures are drawn.
 */
object OfficeRoomArt {
    @Serializable class Layer(val palette: Map<String, String>, val rows: List<String>)
    @Serializable class Sprite(val at: List<Int> = emptyList(), val palette: Map<String, String>, val rows: List<String>)
    @Serializable class Object(val id: String, val label: String, val rect: List<Int>, val live: List<Int>? = null)
    @Serializable class File(
        val size: List<Int>, val room: Layer, val front: Layer, val sprites: Map<String, Sprite>,
        val trophy_slots: List<List<Int>>, val objects: List<Object>,
    )

    private val file: File? by lazy {
        val stream = OfficeRoomArt::class.java.getResourceAsStream("/office-room.json") ?: return@lazy null
        Json { ignoreUnknownKeys = true }.decodeFromString(File.serializer(), stream.bufferedReader().readText())
    }

    val art: File? get() = file
    val width: Int get() = file?.size?.getOrNull(0) ?: 288
    val height: Int get() = file?.size?.getOrNull(1) ?: 176
    val objects: List<Object> get() = file?.objects ?: emptyList()
    fun obj(id: String): Object? = objects.firstOrNull { it.id == id }

    fun bitmap(key: String, palette: Map<String, String>, rows: List<String>): ImageBitmap =
        PixelBitmaps.of("office:$key", rows) { ch -> palette[ch.toString()]?.let { SectorIcons.color(it) } }
}

/** Where a tapped object goes. */
internal sealed interface OfficeRoute {
    /** A whole screen: map, stats, inbox, paper, hall, research or actions. */
    data class Screen(val id: String) : OfficeRoute

    /** A part of the CEO Office's own details, scrolled into view. */
    data class Section(val section: OfficeSection) : OfficeRoute
}

internal enum class OfficeSection { Brief, Goal, Executives, Figures }

/** Every object in the room and where it leads. Null for an id that is not an object. */
internal fun officeRoute(id: String): OfficeRoute? = when (id) {
    "window" -> OfficeRoute.Screen("map")
    "chart", "cabinet" -> OfficeRoute.Screen("stats")
    "phone" -> OfficeRoute.Screen("inbox")
    "paper" -> OfficeRoute.Screen("paper")
    "board", "trophies" -> OfficeRoute.Screen("hall")
    "door" -> OfficeRoute.Screen("research")
    "chief", "nameplate" -> OfficeRoute.Screen("actions")
    "mandate" -> OfficeRoute.Section(OfficeSection.Goal)
    "memo" -> OfficeRoute.Section(OfficeSection.Brief)
    "safe" -> OfficeRoute.Section(OfficeSection.Figures)
    "exec0", "exec1", "exec2" -> OfficeRoute.Section(OfficeSection.Executives)
    else -> null
}

/** One of the three executive chairs. */
internal data class OfficeSeat(val title: String, val short: String, val portrait: String?, val name: String?)

/** The figures the room shows, read from what the store already has. */
internal data class OfficeFacts(
    val firmName: String,
    val chief: String?,
    val chiefName: String? = null,
    val chiefMood: String,
    val profitText: String,
    val profitLabel: String,
    val profitGood: Boolean,
    val profitSeries: List<Long>,
    val unread: Int,
    val headline: String,
    val mandateTitle: String?,
    val mandateProgressBps: Int,
    val mandateLost: Boolean,
    val offerWaiting: Boolean,
    val boardBusts: List<String>,
    val boardMood: String,
    val seats: List<OfficeSeat>,
    val cash: String,
    val credit: String,
    val trophiesEarned: Int,
    val trophiesTotal: Int,
    val briefMoves: Int?,
    val weekLabel: String,
    val region: String,
) {
    /** What a screen reader says for an object, with its figure. */
    fun describe(id: String, label: String): String = when (id) {
        "chief" -> "${chiefName?.let { "$it, chief executive" } ?: "Chief executive"} at the desk of $firmName, looking ${when (chiefMood) { "pleased" -> "pleased with the results"; "displeased" -> "unhappy with the results"; else -> "steady" }}. Opens Actions."
        "nameplate" -> "Nameplate, $firmName. Opens Actions."
        "window" -> "Window onto the map at your headquarters in $region. Opens the map."
        "chart" -> "Wall chart: $profitLabel $profitText. Opens Financials."
        "phone" -> if (unread > 0) "Phone, $unread unread ${if (unread == 1) "letter" else "letters"}. Opens the Inbox." else "Phone, no unread letters. Opens the Inbox."
        "paper" -> "Newspaper, today's headline: $headline. Opens the Paper."
        "board" -> "Board table. Opens the Hall of Careers."
        "mandate" -> when {
            mandateTitle != null -> "Mandate card: $mandateTitle, ${mandateProgressBps / 100} percent${if (mandateLost) ", lost" else ""}. Shows the chapter goal."
            offerWaiting -> "Mandate card: the board has an offer waiting. Shows the chapter goal."
            else -> "Mandate card: no goal set. Shows the chapter goal."
        }
        "exec0", "exec1", "exec2" -> seats.getOrNull(id.last() - '0')?.let {
            if (it.portrait != null) "${it.title}${it.name?.let { n -> ", $n" } ?: ""}. Opens Executives." else "Empty chair, ${it.title}. Hire. Opens Executives."
        } ?: label
        "cabinet" -> "Filing cabinet, financial statements. Opens Financials."
        "safe" -> "Safe, company cash $cash, credit $credit. Shows the company figures."
        "trophies" -> "Trophy shelf, $trophiesEarned of $trophiesTotal achievements earned. Opens the Hall of Careers."
        "door" -> "Door to Research."
        "memo" -> "Memo, this week's brief${briefMoves?.let { ", $it ${if (it == 1) "move" else "moves"}" } ?: ""}. Shows the brief."
        else -> label
    }

    /** The short words that show on hover and long press. */
    fun short(id: String): String = when (id) {
        "chief" -> "Chief executive"
        "nameplate" -> "Nameplate"
        "window" -> "Map"
        "chart" -> "Financials"
        "phone" -> if (unread > 0) "Inbox, $unread new" else "Inbox"
        "paper" -> "The paper"
        "board" -> "The Hall"
        "mandate" -> "Chapter goal"
        "exec0", "exec1", "exec2" -> seats.getOrNull(id.last() - '0')?.let { if (it.portrait != null) it.title else "Hire ${it.title.lowercase()}" } ?: "Executive"
        "cabinet" -> "Statements"
        "safe" -> "Cash and credit"
        "trophies" -> "Achievements"
        "door" -> "Research"
        "memo" -> "This week's brief"
        else -> id
    }
}

/** The mood the chief wears: what the quarter's profit says. */
internal fun chiefMood(quarterProfitCents: Long?): String = when {
    quarterProfitCents == null -> "neutral"
    quarterProfitCents > 0 -> "pleased"
    quarterProfitCents < 0 -> "displeased"
    else -> "neutral"
}

/**
 * Reads the room's figures from the store: only what the Office already loads (the overview, the
 * four-weekly history, letters, the hall, executives, the brief and the ticker). No new engine calls.
 */
internal fun officeFacts(store: GameStore): OfficeFacts? {
    val firm = store.selectedFirmDto ?: return null
    val overview = store.overview
    val quarter = if (overview?.primaryFirm == firm.handle) overview.quarterNetIncomeCents else null
    val weeks = overview?.quarterWeeks?.toInt() ?: 0
    val hall = store.hall
    val mandate = store.activeMandate
    val rivalChiefs = store.rivals.map { it.tag.ceoId }.toSet()
    // The investor's own firm has no chief of its own in the cast, so the room seats one of the named
    // office portraits no rival is using, the same one every time for this company.
    val chiefs = CastPortraits.office.keys.sorted().filter { it !in rivalChiefs }
    val own = store.playerChief
    val chief = own?.portrait ?: chiefs.takeIf { it.isNotEmpty() }?.get((firm.handle % chiefs.size.toULong()).toInt())
    val extras = CastPortraits.all.keys.filter { it.startsWith("x_") && it !in rivalChiefs }.sorted()
    val boardStart = if (extras.isEmpty()) 0 else (firm.handle * 5uL % extras.size.toULong()).toInt()
    val board = (0 until 5).mapNotNull { extras.getOrNull((boardStart + it) % extras.size.coerceAtLeast(1)) }
    val boardMood = when {
        mandate == null -> "neutral"
        mandate.second.lost -> "displeased"
        mandate.second.progressBps >= 5_000 -> "pleased"
        else -> "neutral"
    }
    val posts = store.executives?.posts.orEmpty()
    val titles = listOf("Chief financial officer" to "CFO", "Chief operating officer" to "COO", "Logistics chief" to "LOG")
    val seats = (0 until 3).map { i ->
        val post = posts.getOrNull(i)
        val holder = post?.holder
        OfficeSeat(post?.title ?: titles[i].first, titles[i].second, holder?.portraitId, holder?.name)
    }
    val series = store.history.takeLast(13).map { it.netIncomeCents }
    return OfficeFacts(
        firmName = firm.name,
        chief = chief,
        chiefName = own?.name,
        chiefMood = chiefMood(quarter),
        profitText = quarter?.let { Format.compact(it, firm.currency) } ?: "n/a",
        profitLabel = if (weeks in 1..12) "profit, $weeks weeks" else "profit, quarter",
        profitGood = (quarter ?: 0L) >= 0,
        profitSeries = series,
        unread = store.unreadLetters,
        headline = store.headline ?: "A quiet week",
        mandateTitle = mandate?.second?.title,
        mandateProgressBps = mandate?.second?.progressBps ?: 0,
        mandateLost = mandate?.second?.lost ?: false,
        offerWaiting = store.boardOffer != null,
        boardBusts = board,
        boardMood = boardMood,
        seats = seats,
        cash = Format.compact(firm.cashCents, firm.currency),
        credit = firm.grade,
        trophiesEarned = hall?.achievements?.count { it.earned } ?: 0,
        trophiesTotal = hall?.achievements?.size ?: 0,
        briefMoves = store.brief?.moves?.size,
        weekLabel = overview?.weekLabel.orEmpty(),
        region = firm.region,
    )
}

private val Cream = Color(0xFFE8E0CC)
private val Ink = Color(0xFF0B1622)

/**
 * The room. [window] draws the live map crop inside the window frame; [onRoute] gets whatever the
 * player taps. Scaled by whole pixels so the art stays crisp; the room is centred when there is spare width.
 * Every object has a screen reader label; the short label shows on hover and on long press. With
 * [blink] false (Reduce Motion) the phone's light stays steady.
 */
@Composable
internal fun OfficeScene(
    facts: OfficeFacts,
    modifier: Modifier = Modifier,
    blink: Boolean = true,
    window: @Composable (Modifier) -> Unit = {},
    onRoute: (OfficeRoute) -> Unit,
) {
    val art = OfficeRoomArt.art ?: return
    val density = LocalDensity.current
    BoxWithConstraints(modifier) {
        val widthPx = with(density) { maxWidth.toPx() }
        val unit = floor(widthPx / art.size[0]).toInt().coerceAtLeast(1)
        // A room pixel in dp, and the room's size in dp.
        val px = with(density) { unit.toDp() }
        val roomW = px * art.size[0]
        val roomH = px * art.size[1]
        val left = (maxWidth - roomW) / 2
        fun Int.d(): Dp = px * this
        fun sp(roomPx: Float) = with(density) { (roomPx * unit).toSp() }

        Box(Modifier.width(maxWidth).height(roomH).background(Ink).clipToBounds()) {
            Box(Modifier.offset(x = left).size(roomW, roomH)) {
                val roomBitmap = remember(art) { OfficeRoomArt.bitmap("room", art.room.palette, art.room.rows) }
                val frontBitmap = remember(art) { OfficeRoomArt.bitmap("front", art.front.palette, art.front.rows) }
                Canvas(Modifier.fillMaxSize()) {
                    drawPixelBitmap(roomBitmap, width = size.width.roundToInt(), height = size.height.roundToInt())
                }

                fun rect(id: String) = OfficeRoomArt.obj(id)
                fun Modifier.at(r: List<Int>) = offset(r[0].d(), r[1].d()).size(r[2].d(), r[3].d())

                // The window onto the map.
                rect("window")?.live?.let { window(Modifier.at(it).clipToBounds()) }

                // The chief behind the desk.
                val chiefRect = rect("chief")?.live
                if (chiefRect != null && facts.chief != null) {
                    PixelPortrait(facts.chief, Modifier.at(chiefRect), size = 64.d(), mood = facts.chiefMood, office = true)
                }
                // The board's busts behind their table.
                rect("board")?.live?.let { r ->
                    facts.boardBusts.forEachIndexed { k, id ->
                        Box(Modifier.offset((r[0] + 4 + k * 20).d(), (r[1] + 4).d())) {
                            PixelPortrait(id, size = 16.d(), mood = facts.boardMood, forceSmall = true)
                        }
                    }
                }
                // The executives in their chairs.
                for (k in 0 until 3) {
                    val r = rect("exec$k")?.live ?: continue
                    val seat = facts.seats.getOrNull(k) ?: continue
                    seat.portrait?.let { PixelPortrait(it, Modifier.at(r), size = 32.d()) }
                }

                // Everything that stands in front of the people: the desk, the table, the sprites.
                val phoneOn = facts.unread > 0
                var lit by remember { mutableStateOf(true) }
                LaunchedEffect(blink, phoneOn) {
                    lit = true
                    while (blink && phoneOn) {
                        delay(650)
                        lit = !lit
                    }
                }
                Canvas(Modifier.fillMaxSize()) {
                    val s = size.width / art.size[0]
                    drawPixelBitmap(frontBitmap, width = size.width.roundToInt(), height = size.height.roundToInt())
                    fun sprite(key: String, x: Int, y: Int) {
                        val sp = art.sprites[key] ?: return
                        val b = OfficeRoomArt.bitmap(key, sp.palette, sp.rows)
                        drawPixelBitmap(b, IntOffset((x * s).roundToInt(), (y * s).roundToInt()), (b.width * s).roundToInt(), (b.height * s).roundToInt())
                    }
                    art.sprites["window_bars"]?.let { sprite("window_bars", it.at[0], it.at[1]) }
                    art.sprites[if (phoneOn && lit) "phone_on" else "phone_off"]?.let { sprite(if (phoneOn && lit) "phone_on" else "phone_off", it.at[0], it.at[1]) }
                    art.sprites["memo"]?.let { sprite("memo", it.at[0], it.at[1]) }
                    art.trophy_slots.forEachIndexed { k, slot ->
                        val key = if (k < facts.trophiesEarned) "trophy_on" else "trophy_off"
                        sprite(key, slot[0], slot[1])
                    }
                }

                // Live figures drawn on the objects.
                rect("chart")?.live?.let { r ->
                    Box(Modifier.at(r)) {
                        RoomText(facts.profitText, if (facts.profitGood) Palette.positive else Palette.negative, sp(7f), Modifier.offset(1.d(), 0.d()), bold = true)
                        Canvas(Modifier.offset(0.d(), 9.d()).size(r[2].d(), (r[3] - 9).d())) {
                            drawSpark(facts.profitSeries, if (facts.profitGood) Palette.positive else Palette.negative, size.width / r[2])
                        }
                    }
                }
                rect("safe")?.live?.let { r ->
                    Box(Modifier.at(r), contentAlignment = Alignment.Center) {
                        RoomText(facts.cash, Palette.gold, sp(fitRoomPx(facts.cash, r[2], 6.5f)))
                    }
                }
                rect("nameplate")?.live?.let { r ->
                    Box(Modifier.at(r), contentAlignment = Alignment.Center) {
                        RoomText(facts.firmName.uppercase(), Palette.gold, sp(fitRoomPx(facts.firmName, r[2], 5.5f)))
                    }
                }
                rect("paper")?.live?.let { r ->
                    Box(Modifier.offset((r[0] + 2).d(), (r[1] + 7).d()).size((r[2] - 4).d(), (r[3] - 8).d()).clipToBounds()) {
                        RoomText(facts.headline, Ink, sp(4.2f), lines = 3, lineHeight = sp(4.8f), bold = true)
                    }
                }
                rect("trophies")?.live?.let { r ->
                    Box(Modifier.at(r)) {
                        RoomText("AWARDS ${facts.trophiesEarned}/${facts.trophiesTotal}", Palette.gold, sp(5.2f), Modifier.offset(1.d(), 0.d()))
                    }
                }
                rect("memo")?.live?.let { r ->
                    Box(Modifier.offset((r[0] + 2).d(), (r[1] + 7).d())) {
                        RoomText(facts.briefMoves?.let { "BRIEF $it" } ?: "BRIEF", Ink, sp(4.6f), bold = true)
                    }
                }
                if (phoneOn) rect("phone")?.live?.let { r ->
                    val label = if (facts.unread > 9) "9+" else "${facts.unread}"
                    Box(
                        Modifier.offset((r[0] + 14).d(), (r[1] - 8).d()).size(9.d(), 8.d()).background(Palette.negative).border(px, Ink),
                        contentAlignment = Alignment.Center,
                    ) { RoomText(label, Color.White, sp(5.5f), bold = true) }
                }
                rect("mandate")?.live?.let { r -> MandateCard(facts, Modifier.at(r), px, ::sp) }
                for (k in 0 until 3) {
                    val r = rect("exec$k")?.live ?: continue
                    val seat = facts.seats.getOrNull(k) ?: continue
                    if (seat.portrait == null) {
                        Box(Modifier.offset((r[0] + 2).d(), (r[1] + 14).d()).size(28.d(), 9.d()), contentAlignment = Alignment.Center) {
                            RoomText("HIRE", Palette.gold, sp(6.5f), bold = true)
                        }
                    }
                    Box(Modifier.offset((r[0] + 2).d(), (r[1] + 33).d()).size(28.d(), 7.d()), contentAlignment = Alignment.Center) {
                        RoomText(seat.short, Cream, sp(5f), bold = true)
                    }
                }

                // Tap, hover and long press targets, biggest first so small objects stay on top.
                for (o in OfficeRoomArt.objects.sortedByDescending { it.rect[2] * it.rect[3] }) {
                    if (o.id == "nameplate") continue
                    val route = officeRoute(o.id) ?: continue
                    RoomHotspot(o, facts, px, ::sp, onTap = { onRoute(route) })
                }
            }
        }
    }
}

/** How big a line of [text] can be, in room pixels, to fit [width] room pixels. */
private fun fitRoomPx(text: String, width: Int, max: Float): Float =
    minOf(max, width / (text.length.coerceAtLeast(1) * 0.78f))

@Composable
private fun RoomText(
    text: String, color: Color, size: androidx.compose.ui.unit.TextUnit, modifier: Modifier = Modifier,
    lines: Int = 1, lineHeight: androidx.compose.ui.unit.TextUnit = androidx.compose.ui.unit.TextUnit.Unspecified, bold: Boolean = false,
) {
    Text(
        text, modifier, color = color, maxLines = lines, softWrap = lines > 1, overflow = TextOverflow.Clip,
        style = TextStyle(fontFamily = Pixel.font, fontSize = size, lineHeight = lineHeight, fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal, textAlign = TextAlign.Center),
    )
}

/** A small line chart, one pixel thick in room pixels, hard corners, no smoothing. */
private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawSpark(series: List<Long>, color: Color, scale: Float) {
    if (series.size < 2) {
        drawLine(color, Offset(0f, size.height - scale), Offset(size.width, size.height - scale), strokeWidth = scale)
        return
    }
    val lo = minOf(series.min(), 0L)
    val hi = maxOf(series.max(), 0L).let { if (it == lo) lo + 1 else it }
    fun y(v: Long) = size.height - 2 * scale - ((v - lo).toFloat() / (hi - lo)) * (size.height - 3 * scale)
    // The zero line, then the line itself, stepped like the rest of the room.
    drawLine(Color(0xFF3A5068), Offset(0f, y(0)), Offset(size.width, y(0)), strokeWidth = scale)
    val step = size.width / (series.size - 1)
    for (i in 1 until series.size) {
        val x0 = (i - 1) * step
        val x1 = i * step
        drawLine(color, Offset(x0, y(series[i - 1])), Offset(x1, y(series[i])), strokeWidth = scale, cap = androidx.compose.ui.graphics.StrokeCap.Square)
    }
}

@Composable
private fun MandateCard(facts: OfficeFacts, modifier: Modifier, px: Dp, sp: (Float) -> androidx.compose.ui.unit.TextUnit) {
    Box(modifier.background(Cream).border(px, Ink).padding(px * 2)) {
        val title = when {
            facts.mandateTitle != null -> facts.mandateTitle
            facts.offerWaiting -> "Board offer waiting"
            else -> "No goal set"
        }
        RoomText(title.uppercase(), Ink, sp(4f), lines = 4, lineHeight = sp(4.6f), bold = true)
        if (facts.mandateTitle != null) {
            val frac = (facts.mandateProgressBps / 10_000f).coerceIn(0.03f, 1f)
            Box(Modifier.align(Alignment.BottomStart).fillMaxWidth().height(px * 3).background(Color(0xFFBDB49C))) {
                Box(Modifier.fillMaxWidth(frac).height(px * 3).background(if (facts.mandateLost) Palette.negative else Color(0xFF2F8A5E)))
            }
        }
    }
}

@Composable
private fun RoomHotspot(
    o: OfficeRoomArt.Object, facts: OfficeFacts, px: Dp, sp: (Float) -> androidx.compose.ui.unit.TextUnit, onTap: () -> Unit,
) {
    val source = remember { MutableInteractionSource() }
    val hovered by source.collectIsHoveredAsState()
    var pressed by remember { mutableStateOf(false) }
    // A long press shows the label for a moment; hover shows it while the pointer is over the object.
    LaunchedEffect(pressed) {
        if (pressed) {
            delay(1_800)
            pressed = false
        }
    }
    val description = facts.describe(o.id, o.label)
    Box(
        Modifier.offset(px * o.rect[0], px * o.rect[1]).size(px * o.rect[2], px * o.rect[3])
            .hoverable(source)
            .semantics(mergeDescendants = true) {
                contentDescription = description
                role = Role.Button
                onClick(label = facts.short(o.id)) { onTap(); true }
            }
            .pointerInput(o.id) {
                detectTapGestures(onTap = { onTap() }, onLongPress = { pressed = true })
            },
    ) {
        if (hovered || pressed) {
            Box(Modifier.border(px, Palette.gold).fillMaxSize())
            Text(
                facts.short(o.id).uppercase(),
                Modifier.align(Alignment.TopCenter).background(Palette.ink).border(px, Palette.gold).padding(horizontal = px * 2, vertical = px),
                color = Palette.gold, maxLines = 1, softWrap = false,
                style = TextStyle(fontFamily = Pixel.font, fontSize = sp(5.6f), fontWeight = FontWeight.Bold),
            )
        }
    }
}

/**
 * The room for the selected company, wired to the store: its figures, the live map in the window and
 * Reduce Motion. Nothing to show before a company exists.
 */
@Composable
internal fun OfficeRoomHost(store: GameStore, onRoute: (OfficeRoute) -> Unit) {
    // The hall is read the way the Hall page reads it, so the shelf and the goal card are current.
    LaunchedEffect(store.overview?.tick) { store.refreshHall() }
    val facts = officeFacts(store) ?: return
    OfficeScene(
        facts, Modifier.fillMaxWidth(),
        blink = LocalAppSettings.current?.animations != false,
        window = { modifier -> OfficeWindowMap(store, modifier) },
        onRoute = onRoute,
    )
}

/** A small view of the map centred on the company's headquarters region, drawn flat by the map's own painter. */
@Composable
internal fun OfficeWindowMap(store: GameStore, modifier: Modifier) {
    val map = WorldMap.forRegions(store.boards.map { it.code })
    val region = store.selectedFirmDto?.region
    val code = store.boards.firstOrNull { it.name == region }?.code
    val measurer = androidx.compose.ui.text.rememberTextMeasurer(cacheSize = 32)
    val geometry = remember(map) { MapGeometryCache() }
    val graphics = remember { GraphicsConfig.Default.copy(earth = false, clouds = 0, traffic = 0, animations = false, dayNight = false, showFps = false, labels = 0, lighting = 1, countryside = false) }
    Canvas(modifier) {
        if (map == null) return@Canvas
        val anchor = code?.let { map.anchor(it) }
        val base = MapCamera.scenario(size, map)
        val camera = if (anchor != null) base.copy(lon = Math.toDegrees(anchor.lon[0]), lat = Math.toDegrees(anchor.lat[0]), zoom = base.zoom * 2.4) else base
        drawWorldMap(
            map, camera, scenarioTint(map.scenarioId.orEmpty()), emptyMap(), code, store.mapDivisions, measurer,
            space = false, geometry = geometry, graphics = graphics, pitch = 0.0, plants = store.mapPlants,
            week = store.overview?.tick?.toInt() ?: 28,
        )
    }
}
