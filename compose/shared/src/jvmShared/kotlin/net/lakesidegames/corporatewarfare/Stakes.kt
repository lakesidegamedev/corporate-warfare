package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.StakeDto

/** A share of the votes in basis points as a percent with one decimal. */
internal fun stakePercent(bps: Int): String = "%.1f%%".format(bps / 100.0)

/**
 * Holdings of 5 percent or more in a company, as filed. A rival building toward a bid shows its
 * stake over time as a small line, from the week the holding became public.
 */
@Composable
fun DisclosedStakes(stakes: List<StakeDto>, onOpenFirm: (ULong) -> Unit = {}) {
    if (stakes.isEmpty()) return
    Eyebrow("Disclosed stakes")
    stakes.forEach { s ->
        Column(Modifier.fillMaxWidth().clickable { onOpenFirm(s.holder) }.padding(vertical = 2.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FirmLogo(s.holderName, size = 20.dp)
                Spacer(Modifier.width(8.dp))
                Text(s.holderName, color = Palette.primaryText, fontSize = 12.sp, modifier = Modifier.weight(1f), maxLines = 1)
                Text(stakePercent(s.bps), color = if (s.bps >= 2_500) Palette.warning else Palette.gold, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
            }
            val since = s.since
            if (since != null) {
                val first = s.history.firstOrNull()
                Text(
                    "Public since ${ResultBuckets.weekLabel(since.toInt())}" + (first?.let { ", from ${stakePercent(it.bps)}" } ?: ""),
                    color = Palette.secondary, fontSize = 11.sp,
                )
            }
            if (s.history.size >= 2) {
                Sparkline(s.history.map { it.bps.toLong() }, if (s.campaign) Palette.warning else Palette.secondary, Modifier.fillMaxWidth().height(34.dp))
            }
        }
    }
}

/** One line on what a rival is likely to do next, from its filings and the public record only. */
@Composable
fun LikelyNextMove(text: String?) {
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Top) {
        PixelIcon("target", size = 12.dp, tint = Palette.gold)
        Text(
            buildString {
                append("Likely next move: ")
                append(text ?: "nothing signalled. It has filed no stake and made no open bid.")
            },
            color = if (text != null) Palette.primaryText else Palette.secondary, fontSize = 12.sp,
        )
    }
}
