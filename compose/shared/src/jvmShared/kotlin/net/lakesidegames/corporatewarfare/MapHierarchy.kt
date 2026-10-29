package net.lakesidegames.corporatewarfare

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import kotlin.math.floor

/**
 * What a city is known for, so close in a port, a steel town and a car town do not all read as the
 * same grey block. The map data has no such field, so this is a short table of the cities the
 * scenarios use. Anything not listed stays a plain city block of towers. The Apple app keeps the
 * same table in MapData.swift; change both together.
 */
internal object CityKinds {
    private val ports = setOf(
        "Long Beach", "San Diego", "Oakland", "Houston", "New Orleans", "Corpus Christi", "Hamburg", "Rotterdam",
        "Antwerpen", "Bremen", "Baltimore", "Norfolk", "Virginia Beach", "Gent", "Szczecin", "Stockton",
    )
    private val steel = setOf(
        "Pittsburgh", "Cleveland", "Duisburg", "Essen", "Dortmund", "Saarbrücken", "Liège", "Hamilton", "Buffalo", "Akron", "Wuppertal",
    )
    private val autos = setOf("Detroit", "Stuttgart", "Munich", "Cologne", "Dayton", "Mannheim", "Düsseldorf", "Wolfsburg")

    /** The model family for a city: city_port, city_steel, city_auto, or plain city. */
    fun of(name: String): String = when (name) {
        in ports -> "city_port"
        in steel -> "city_steel"
        in autos -> "city_auto"
        else -> "city"
    }
}

/**
 * Decides which map labels show, so none overlap each other or a building, and keeps the answer
 * steady from frame to frame. Callers reserve every building and badge first, then place labels in
 * priority order (cities, then regions): a label that does not fit is dropped, so the lower
 * priority one always loses. A label that was showing last frame keeps its place unless it truly
 * collides, and one that was hidden needs a few pixels of clear air to come back, so a label at
 * the edge of fitting does not blink while the map pans or zooms. Not thread safe: use [frame].
 */
internal class LabelPlanner {
    private var shown = HashSet<String>()
    private var next = HashSet<String>()
    private val slotOf = HashMap<String, Int>()
    private val nextSlot = HashMap<String, Int>()
    private var taken = FloatArray(256)
    private var count = 0

    /** One whole frame of planning under the planner's own lock. */
    fun <T> frame(block: LabelPlanner.() -> T): T = synchronized(this) {
        count = 0
        next = HashSet()
        nextSlot.clear()
        try { block() } finally {
            shown = next
            slotOf.clear(); slotOf.putAll(nextSlot)
        }
    }

    /** Marks a screen rectangle as occupied. */
    fun reserve(l: Float, t: Float, r: Float, b: Float) {
        if (count * 4 + 4 > taken.size) taken = taken.copyOf(taken.size * 2)
        taken[count * 4] = l; taken[count * 4 + 1] = t; taken[count * 4 + 2] = r; taken[count * 4 + 3] = b
        count++
    }

    private fun free(l: Float, t: Float, r: Float, b: Float): Boolean {
        for (i in 0 until count) {
            if (l < taken[i * 4 + 2] && r > taken[i * 4] && t < taken[i * 4 + 3] && b > taken[i * 4 + 1]) return false
        }
        return true
    }

    /**
     * Tries the label's candidate top-left corners ([slots] is x, y pairs), the one it had last time first,
     * and reserves the first that is clear. Returns the slot used, or -1 when the label is dropped.
     */
    fun place(key: String, w: Float, h: Float, slots: FloatArray): Int {
        val n = slots.size / 2
        val kept = key in shown
        // A label already showing may squeeze a pixel or two; a hidden one needs clear air.
        val pad = if (kept) -1.5f else 3f
        val first = if (kept) slotOf[key] ?: 0 else 0
        for (j in 0 until n) {
            val i = (first + j) % n
            val x = slots[2 * i]; val y = slots[2 * i + 1]
            if (free(x - pad, y - pad, x + w + pad, y + h + pad)) {
                reserve(x, y, x + w, y + h)
                next += key
                nextSlot[key] = i
                return i
            }
        }
        return -1
    }

    /**
     * A big city's name: the preferred [slots] first, then rings further out round ([cx], [cy]) with a leader line,
     * and when nothing at all is clear the place that covers least. It is never dropped.
     */
    fun placeCity(key: String, w: Float, h: Float, slots: FloatArray, cx: Float, cy: Float, reach: Float, viewW: Float = Float.MAX_VALUE, viewH: Float = Float.MAX_VALUE): CityLabel {
        // Only places that are on screen, when any are: a name off the edge of the view is no name at all.
        val every = slots + ringSlots(cx, cy, w, h, reach)
        val keep = ArrayList<Float>(every.size)
        for (i in 0 until every.size / 2) {
            val x = every[2 * i]; val y = every[2 * i + 1]
            if (x >= 0f && y >= 0f && x + w <= viewW && y + h <= viewH) { keep += x; keep += y }
        }
        val all = if (keep.isEmpty()) every else keep.toFloatArray()
        var i = place(key, w, h, all)
        if (i < 0) {
            // Crowded to the edge: take the place that overlaps the least and let the label sit over it.
            var best = Float.MAX_VALUE
            for (j in 0 until all.size / 2) {
                val o = overlap(all[2 * j], all[2 * j + 1], all[2 * j] + w, all[2 * j + 1] + h)
                if (o < best) { best = o; i = j }
            }
            reserve(all[2 * i], all[2 * i + 1], all[2 * i] + w, all[2 * i + 1] + h)
            next += key
            nextSlot[key] = i
        }
        return CityLabel(all[2 * i], all[2 * i + 1], (0 until slots.size / 2).none { j -> slots[2 * j] == all[2 * i] && slots[2 * j + 1] == all[2 * i + 1] })
    }

    private fun overlap(l: Float, t: Float, r: Float, b: Float): Float {
        var sum = 0f
        for (i in 0 until count) {
            val w = minOf(r, taken[i * 4 + 2]) - maxOf(l, taken[i * 4])
            val h = minOf(b, taken[i * 4 + 3]) - maxOf(t, taken[i * 4 + 1])
            if (w > 0 && h > 0) sum += w * h
        }
        return sum
    }
}

/** Where [LabelPlanner.placeCity] put a name: its top-left corner, and whether a leader line must tie it to the city. */
internal class CityLabel(val x: Float, val y: Float, val leader: Boolean)

/** Population from which a city is a city and not a town: its name always shows while its block is on screen. */
internal const val TOWN_POPULATION = 1_000_000

/**
 * Candidate top-left corners for a name of size ([w], [h]) on rings round ([cx], [cy]), nearest first, [reach] pixels
 * from the centre to the first ring. Twelve places a ring, rings 14 px apart, six rings.
 */
internal fun ringSlots(cx: Float, cy: Float, w: Float, h: Float, reach: Float): FloatArray {
    val out = FloatArray(6 * 12 * 2)
    var n = 0
    for (ring in 0 until 6) {
        val r = reach + ring * 14f
        for (a in 0 until 12) {
            val ang = a * (Math.PI.toFloat() * 2f / 12f)
            val dx = kotlin.math.cos(ang); val dy = kotlin.math.sin(ang)
            // Put the label's nearest edge, not its corner, on the ring.
            out[n++] = cx + dx * r - w / 2f + dx * w / 2f
            out[n++] = cy + dy * r - h / 2f + dy * h / 2f
        }
    }
    return out
}

/** Label layouts kept between frames, so panning and zooming never lay the same text out twice. */
internal object LabelText {
    private val lock = Any()
    private val cache = HashMap<String, TextLayoutResult>()

    /** Text layout and drawing both go through this lock: the structures bake on a background thread. */
    val guard: Any get() = lock

    fun measure(measurer: TextMeasurer, text: String, style: TextStyle, tag: String): TextLayoutResult = synchronized(lock) {
        if (cache.size > 600) cache.clear()
        cache.getOrPut("${System.identityHashCode(measurer)}|$tag|$text") { measurer.measure(text, style) }
    }
}

/** How freight on a lane is coloured: the player gold, a rival in its sector's colour, outside carriers grey. */
internal object FreightLook {
    val outside = Color(0xFF8A939C)

    /** Livery colours of the baked neutral vehicles, the ones recoloured by owner. */
    private val neutralLivery = setOf(0xFF4F7FAF, 0xFFC24A40, 0xFFE5D04A, 0xFF2E4A66).map { Color(it) }.toSet()
    private val tones = HashMap<Color, (Color) -> Color>()

    /** A recolouring that swaps the neutral livery for [owner]. */
    fun tone(owner: Color): (Color) -> Color = synchronized(tones) {
        tones.getOrPut(owner) { { c -> if (c in neutralLivery) owner else c } }
    }

    /**
     * The owner colour of a lane: gold when the player moves goods on it, else the sector of the firm that
     * holds the most capacity, else grey when outside carriers carry more than any firm.
     */
    fun ownerColor(lane: uniffi.cw_ffi.LaneDto, firmSector: Map<ULong, String>): Color {
        if (laneIsMine(lane)) return Palette.gold
        val top = lane.owners.filter { !it.mine }.maxByOrNull { it.lots }
        if (top == null || top.lots <= 0 || top.lots < lane.outsideLots) return outside
        return firmSector[top.firm]?.let { Sectors.color(it) } ?: outside
    }
}

/** Each firm's biggest industry by plants, to colour its freight and buildings. */
internal fun firmSectors(plants: List<uniffi.cw_ffi.MapPlantDto>): Map<ULong, String> =
    plants.groupBy { it.firm }.mapValues { (_, items) ->
        items.groupBy { it.industry }.maxByOrNull { (_, v) -> v.sumOf { it.blocks.toInt() } }!!.key
    }

/**
 * Vehicles that may stand on a lane at once in one direction. Neighbours keep at least two vehicle
 * lengths apart, so a busy lane reads as busy without becoming one unbroken line of specks.
 * [laneLength] and [vehicleLength] are on the ground, in the same units.
 */
internal fun maxPerLane(laneLength: Float, vehicleLength: Float): Int =
    floor(laneLength / (2f * vehicleLength.coerceAtLeast(1e-3f))).toInt().coerceAtLeast(1)
