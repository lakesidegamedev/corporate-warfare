package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.AmbitionGuideDto
import uniffi.cw_ffi.HelmOfferDto

/**
 * The career choices of the new-game screen (2.0): the three companies offered when you start at the helm,
 * the ambition with what each medal takes, and the length. The medal wording and the lengths come from the
 * engine's content table through the FFI, so no threshold is typed into the app.
 */

/** What the player picked. An empty ambition is a sandbox with no goal; length 0 takes the default. */
internal class CareerChoice {
    var ambition by mutableStateOf("")
    var years by mutableStateOf(0)
    var helmFirm by mutableStateOf<ULong?>(null)
}

/** Whether the choices change how the career is made (otherwise the ordinary new game starts, exactly as before). */
internal fun CareerChoice.isDefault(helm: Boolean) = !helm && ambition.isEmpty() && years == 0

internal fun CareerChoice.options(helm: Boolean) = uniffi.cw_ffi.CareerOptionsDto(
    start = if (helm) "helm" else "founder",
    helmFirm = if (helm) helmFirm else null,
    ambition = ambition,
    years = years.toUInt(),
)

@Composable
private fun PickCard(on: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit, content: @Composable () -> Unit) {
    val shape = Pixel.shape
    Column(
        modifier.clip(shape).background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.panel)
            .border(if (on) 2.dp else 1.dp, if (on) Palette.gold else Palette.stroke, shape)
            .clickable { onClick() }.padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) { content() }
}

/** The three companies on offer, as a small grid on a wide screen and as cards on a narrow one. */
@Composable
internal fun HelmOfferGrid(offers: List<HelmOfferDto>?, error: String?, pick: ULong?, wide: Boolean, onPick: (ULong) -> Unit) {
    if (error != null) { Text(error, color = Palette.warning, fontSize = 13.sp); return }
    if (offers == null) { Text("Finding the companies on offer...", color = Palette.gold, fontSize = 13.sp); return }
    if (offers.isEmpty()) { Text("This scenario has no company to take the helm of. Found your own instead.", color = Palette.secondary, fontSize = 13.sp); return }
    val chosen = offers.firstOrNull { it.firm == pick }
    if (wide) {
        // A column with nothing in it at week 0 is left out rather than shown as dashes.
        val hasSales = offers.any { it.revenueYearCents > 0 }
        val cols = listOf("Rank" to 56.dp, "Company" to 220.dp, "Sector" to 120.dp, "Value" to 84.dp, "A year's sales" to 130.dp, "Plants" to 70.dp, "Workers" to 84.dp, "Price of control" to 140.dp, "Your fortune" to 120.dp)
            .filter { hasSales || it.first != "A year's sales" }
        Column(Modifier.clip(Pixel.shape).border(1.dp, Palette.stroke, Pixel.shape)) {
            Row(Modifier.fillMaxWidth().background(Palette.panelRaised).padding(horizontal = 10.dp, vertical = 6.dp)) {
                cols.forEach { (name, w) -> Text(name, color = Palette.secondary, fontSize = 11.sp, fontFamily = Pixel.font, modifier = Modifier.width(w)) }
            }
            offers.forEach { o ->
                val on = o.firm == pick
                Row(
                    Modifier.fillMaxWidth().background(if (on) Palette.gold.copy(alpha = 0.14f) else Palette.panel).clickable { onPick(o.firm) }.padding(horizontal = 10.dp, vertical = 8.dp),
                ) {
                    val cells = listOfNotNull(
                        "#${o.rank}", o.name, Sectors.name(o.industry), Format.compact(o.valueCents), if (hasSales) Format.compact(o.revenueYearCents) else null,
                        "${o.plants}", Format.count(o.workers), Format.compact(o.priceCents), Format.compact(o.fortuneCents),
                    )
                    cells.forEachIndexed { i, text ->
                        Text(
                            text, color = if (i == 1 || on) Palette.primaryText else Palette.secondary, fontSize = 13.sp,
                            fontWeight = if (i == 1) FontWeight.SemiBold else FontWeight.Normal, maxLines = 1, modifier = Modifier.width(cols[i].second),
                        )
                    }
                }
            }
        }
    } else {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            offers.forEach { o ->
                PickCard(o.firm == pick, Modifier.fillMaxWidth(), { onPick(o.firm) }) {
                    Text("#${o.rank}  ${o.name}", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                    Text("${Sectors.name(o.industry)} in ${o.region}", color = Palette.secondary, fontSize = 12.sp)
                    Text(
                        "Value ${Format.compact(o.valueCents)}${if (o.revenueYearCents > 0) ", sales " + Format.compact(o.revenueYearCents) + " a year" else ""}, ${o.plants} plants, ${Format.count(o.workers)} workers",
                        color = Palette.primaryText, fontSize = 12.sp,
                    )
                    Text("Price of control ${Format.compact(o.priceCents)}. Your fortune ${Format.compact(o.fortuneCents)}.", color = Palette.gold, fontSize = 12.sp)
                }
            }
        }
    }
    if (chosen != null) {
        val board = remember { uniffi.cw_ffi.boardExpectation() }
        Column(
            Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panel).border(1.dp, Palette.stroke, Pixel.shape).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(chosen.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 18.sp)
            Text(
                "#${chosen.rank} by value. A ${Sectors.name(chosen.industry).lowercase()} company in ${chosen.region} with ${chosen.plants} plants and ${Format.count(chosen.workers)} workers, worth ${Format.compact(chosen.valueCents)}.",
                color = Palette.primaryText, fontSize = 13.sp,
            )
            Text(
                "Taking the helm buys control from its outside shareholders at today's price: " +
                    "${Format.compact(chosen.priceCents)} for ${Format.count(chosen.sharesNeeded)} shares. The sitting chief executive leaves; the board stays.",
                color = Palette.primaryText, fontSize = 13.sp,
            )
            Text(fortuneNote(chosen), color = if (chosen.fortuneCents != chosen.standardFortuneCents) Palette.warning else Palette.secondary, fontSize = 13.sp)
            Text(
                "What the board expects: after ${board.graceWeeks} weeks it reads the results every four weeks. ${board.lossPeriods} periods of losses in a row draw its warning, " +
                    "and if no four-week period turns a profit within ${board.warningWeeks} weeks it removes you (unless you hold the majority of the votes).",
                color = Palette.gold, fontSize = 13.sp,
            )
        }
    }
}

/** The fortune this start gives, saying plainly when it is not the scenario's standard one. */
internal fun fortuneNote(o: HelmOfferDto): String =
    if (o.fortuneCents == o.standardFortuneCents) "Your fortune after buying control: ${Format.compact(o.fortuneCents)}, the scenario's own."
    else "This start gives you ${Format.compact(o.fortuneCents)} instead of the scenario's usual ${Format.compact(o.standardFortuneCents)}, " +
        "because the usual fortune cannot pay for control of this company. It is scaled up to cover the price."

/** The ambition (or none) with what each medal takes, and the career length. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun AmbitionPicker(helm: Boolean, choice: CareerChoice, wide: Boolean) {
    val start = if (helm) "helm" else "founder"
    val lengths = remember { uniffi.cw_ffi.careerLengths() }
    val shownYears = if (choice.years == 0) lengths.defaultYears.toInt() else choice.years
    val guide: List<AmbitionGuideDto> = remember(start, shownYears) { uniffi.cw_ffi.ambitionGuide(start, shownYears.toUInt()) }
    // Fixed width beside each other on a wide screen, the full width one under the other on a phone.
    val cardModifier = if (wide) Modifier.width(270.dp) else Modifier.fillMaxWidth()
    FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        PickCard(choice.ambition.isEmpty(), cardModifier, { choice.ambition = "" }) {
            Text("No ambition", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
            Text("A sandbox: no goal, no medals and no ending. Play as long as you like.", color = Palette.secondary, fontSize = 12.sp)
        }
        guide.forEach { a ->
            PickCard(choice.ambition == a.key, cardModifier, { choice.ambition = a.key }) {
                Text(a.name, color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                Text(a.sentence, color = Palette.secondary, fontSize = 12.sp)
                Text("Bronze: ${a.bronze}.", color = Palette.primaryText, fontSize = 12.sp)
                Text("Silver: ${a.silver}.", color = Palette.primaryText, fontSize = 12.sp)
                Text("Gold: ${a.gold}.", color = Palette.gold, fontSize = 12.sp)
            }
        }
    }
    if (choice.ambition.isNotEmpty()) {
        Eyebrow("How long")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            lengths.years.forEach { y ->
                val on = shownYears == y.toInt()
                PickCard(on, Modifier.width(150.dp), { choice.years = y.toInt() }) {
                    Text("$y years", color = Palette.primaryText, fontWeight = FontWeight.SemiBold, fontSize = 15.sp)
                    Text(if (y == lengths.defaultYears) "The usual length" else if (y < lengths.defaultYears) "A shorter game" else "A long game", color = Palette.secondary, fontSize = 12.sp)
                }
            }
        }
        Text("The medals above are for $shownYears years. A career ends after that, or sooner if the board removes you, a rival takes your company or you are left with none.", color = Palette.secondary, fontSize = 12.sp, modifier = Modifier.padding(top = 2.dp))
    }
}
