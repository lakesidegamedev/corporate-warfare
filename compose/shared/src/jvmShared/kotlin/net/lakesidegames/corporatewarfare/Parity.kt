package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Panels the desktop shows in side columns and tables, laid out for a phone. They sit in the
 * Inbox, Portfolio and CEO Office tabs so every desktop figure has a place on a small screen.
 */

/** Recent raids by and against anyone, newest first; your own side is coloured. */
@Composable
internal fun RivalMovesPanel(store: GameStore, limit: Int = 14) {
    val raids = store.recentRaids.take(limit)
    Panel {
        Eyebrow("Rival moves")
        if (raids.isEmpty()) Text("No raids yet.", color = Palette.secondary, fontSize = 12.sp)
        raids.forEachIndexed { index, r ->
            if (index > 0) HorizontalDivider(color = Palette.stroke)
            val tone = when {
                r.playerDefender && r.success -> Palette.negative
                r.playerDefender -> Palette.warning
                r.playerAttacker -> Palette.positive
                else -> Palette.secondary
            }
            val region = store.boards.firstOrNull { it.code == r.region }?.name ?: r.region
            Column(Modifier.fillMaxWidth().padding(vertical = 2.dp)) {
                Text("${r.attacker} ${if (r.success) "took" else "failed to take"} ${r.blocks} plants from ${r.defender}", color = tone, fontSize = 13.sp)
                Text("${ResultBuckets.weekLabel(r.week.toInt())} · ${Sectors.name(r.industry)} · $region", color = Palette.secondary, fontSize = 11.sp)
            }
        }
    }
}

/** Shares you hold outside the companies you control, with stake, value and the quarter's move. */
@Composable
internal fun ShareholdingsPanel(store: GameStore, onOpen: (uniffi.cw_ffi.StockDto) -> Unit = {}) {
    val held = store.stocks.filter { it.yourShares > 0 }.sortedByDescending { it.yourShares * it.priceCents }
    Panel {
        Eyebrow("Your shareholdings · ${held.size}")
        if (held.isEmpty()) Text("You hold no shares outside your companies. Buy on the Markets screen.", color = Palette.secondary, fontSize = 12.sp)
        held.forEachIndexed { index, s ->
            if (index > 0) HorizontalDivider(color = Palette.stroke)
            Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                SectorBadge(s.industry, 24)
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(s.name + if (s.controlledByYou) " ★" else "", color = Palette.primaryText, fontWeight = FontWeight.Medium, maxLines = 1)
                    val stake = if (s.sharesIssued > 0) "%.1f%% stake".format(s.yourShares * 100.0 / s.sharesIssued) else ""
                    Text("${Format.count(s.yourShares)} shares · $stake · ${Format.price(s.priceCents)}", color = Palette.secondary, fontSize = 11.sp, maxLines = 1)
                }
                Column(horizontalAlignment = Alignment.End) {
                    Text(Format.compact(s.yourShares * s.priceCents), color = Palette.primaryText, fontWeight = FontWeight.SemiBold)
                    Text(changeText(s.changeQuarterBps), color = changeTint(s.changeQuarterBps), fontSize = 11.sp)
                }
            }
        }
    }
}

/** The company's four-weekly revenue and profit bars, as on the desktop CEO Office. */
@Composable
internal fun FourWeeklyResults(store: GameStore) {
    val periods = ResultBuckets.make(StatsPeriod.Month, emptyList(), store.history)
    if (periods.size > 1) {
        Panel {
            Eyebrow("Four-weekly results")
            ResultsChart(periods)
        }
    }
}
