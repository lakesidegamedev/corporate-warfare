package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Research: the company's technology level on a ladder of twenty, progress to the next level,
 * what the next level buys, and the research and marketing budgets.
 */
@Composable
fun ResearchScreen(store: GameStore) {
    val research = store.research
    val firm = store.selectedFirmDto
    Box(Modifier.fillMaxSize().background(Palette.ink), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier.widthIn(max = 1100.dp).fillMaxWidth().verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text("Research", color = Palette.primaryText, fontSize = 22.sp, fontFamily = Pixel.font, fontWeight = FontWeight.Bold)
            if (firm == null || research == null) {
                Callout("Found or take over a company to research.", Palette.secondary)
                return@Column
            }
            Panel {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Eyebrow("Technology level")
                        Text("${research.level} of ${research.maxLevel}", color = Palette.primaryText, fontSize = 30.sp, fontWeight = FontWeight.Bold)
                    }
                    Column(horizontalAlignment = Alignment.End) {
                        Eyebrow("Industry frontier")
                        Text("Level ${research.frontier}", color = Palette.gold, fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
                        Text("You can research one level past it", color = Palette.secondary, fontSize = 11.sp)
                    }
                }
                // The ladder: one step per level, filled to the company's level.
                Row(Modifier.fillMaxWidth().height(42.dp), horizontalArrangement = Arrangement.spacedBy(3.dp), verticalAlignment = Alignment.Bottom) {
                    for (step in 1..research.maxLevel.toInt()) {
                        val reached = step <= research.level.toInt()
                        val reachable = step <= research.frontier.toInt() + 1
                        Box(
                            Modifier.weight(1f).height((10 + step * 1.6f).dp).clip(Pixel.small)
                                .background(
                                    when {
                                        reached -> Palette.gold
                                        reachable -> Palette.gold.copy(alpha = 0.25f)
                                        else -> Palette.panelRaised
                                    },
                                ),
                        )
                    }
                }
                val toNext = research.nextThresholdCents
                val fraction = if (toNext > 0) (research.progressCents.toFloat() / toNext).coerceIn(0f, 1f) else 1f
                Text(
                    if (research.level >= research.maxLevel) "Top level reached."
                    else "Next level: ${Format.compact(research.progressCents)} of ${Format.compact(toNext)} spent (${(fraction * 100).toInt()}%). " +
                        "At the threshold a breakthrough lands ${research.successBps / 100} times in 100; a miss keeps half the progress.",
                    color = Palette.secondary, fontSize = 12.sp,
                )
                androidx.compose.material3.LinearProgressIndicator(
                    progress = { fraction }, modifier = Modifier.fillMaxWidth().height(8.dp), color = Palette.gold, trackColor = Palette.panelRaised,
                )
            }
            Eyebrow("What the next level buys")
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                StatGroup(
                    null,
                    listOf(
                        Stat("Inputs and labour per unit", "%.0f%% → %.0f%%".format(research.intensityBps / 100.0, research.nextIntensityBps / 100.0), Palette.positive, "of the baseline"),
                        Stat("Quality", "%.0f → %.0f".format(research.qualityBps / 100.0, research.nextQualityBps / 100.0), Palette.positive, "out of 200"),
                    ),
                )
            }
            Panel {
                Eyebrow("Research budget")
                var budget by remember(research.firm) { mutableStateOf(research.researchBudgetCents / 100.0) }
                val cap = maxOf(1_000_000.0, (firm.cashCents / 100.0 / 26).coerceAtMost(5_000_000.0))
                ValueSlider(
                    budget, { budget = it }, 0.0..cap, 5_000.0,
                    label = { if (it == 0.0) "None" else Format.money((it * 100).toLong()) + " a week" },
                    detail = {
                        if (it <= 0.0) "Research is paused." else {
                            val weeks = ((research.nextThresholdCents - research.progressCents).coerceAtLeast(0) / (it * 100)).toInt()
                            "About $weeks weeks to the next threshold at this rate."
                        }
                    },
                    onCommit = { v -> store.act(CommandJson.command("SetResearchBudget", "firm" to firm.handle, "amount_cents" to (v * 100).toLong())) { _, _ -> } },
                )
            }
            Panel {
                Eyebrow("Marketing budget")
                var budget by remember(research.firm) { mutableStateOf(research.marketingBudgetCents / 100.0) }
                ValueSlider(
                    budget, { budget = it }, 0.0..2_000_000.0, 5_000.0,
                    label = { if (it == 0.0) "None" else Format.money((it * 100).toLong()) + " a week" },
                    detail = { "Brand spend lifts demand for your goods in the regions you sell in." },
                    onCommit = { v -> store.act(CommandJson.command("SetMarketingBudget", "firm" to firm.handle, "amount_cents" to (v * 100).toLong())) { _, _ -> } },
                )
            }
        }
    }
}
