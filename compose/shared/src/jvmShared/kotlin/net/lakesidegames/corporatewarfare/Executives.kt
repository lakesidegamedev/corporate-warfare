package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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
import uniffi.cw_ffi.CandidateDto
import uniffi.cw_ffi.ExecutiveDto
import uniffi.cw_ffi.ExecutivePostDto
import uniffi.cw_ffi.ExecutivesDto

/**
 * The Executives section of the CEO Office: the three posts (chief financial officer, chief
 * operating officer, logistics chief), the person in each with their portrait, level, what they
 * change in plain words with the number, pay and contract left. Hire opens the candidate list with
 * the effect each would have at this company and the cost; Dismiss quotes the severance first.
 */
@Composable
internal fun ExecutivesPanel(store: GameStore, currency: String) {
    DisposableEffect(store) {
        store.executivesOpen = true
        store.refreshExecutives()
        onDispose { store.executivesOpen = false }
    }
    var hirePost by remember { mutableStateOf<ExecutivePostDto?>(null) }
    var dismissPost by remember { mutableStateOf<ExecutivePostDto?>(null) }
    val data = store.executives
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            PixelIcon("people", size = 14.dp, tint = Palette.gold)
            androidx.compose.foundation.layout.Spacer(Modifier.padding(horizontal = 4.dp))
            Column(Modifier.weight(1f)) { Eyebrow("Executives") }
            if (data != null && data.weeklyPayCents > 0) {
                Text("${Format.compact(data.weeklyPayCents, currency)} a week", color = Palette.secondary, fontSize = 12.sp)
            }
        }
        if (data == null) Text("Reading the books...", color = Palette.secondary, fontSize = 12.sp)
        else ExecutivesBody(data, currency, store.busy, onHire = { hirePost = it }, onDismiss = { dismissPost = it })
    }
    val data2 = store.executives
    hirePost?.let { post ->
        val list = data2?.candidates?.firstOrNull { it.postCode == post.postCode }
        if (list == null) hirePost = null
        else ExecutiveHireDialog(
            post, list.people, currency, store.busy,
            onHire = { store.hireExecutive(data2.firm, post.postCode.toInt(), it.id.toInt()); hirePost = null },
            onClose = { hirePost = null },
        )
    }
    dismissPost?.let { post ->
        val holder = post.holder
        if (holder == null) dismissPost = null
        else AlertDialog(
            onDismissRequest = { dismissPost = null },
            containerColor = Palette.panel,
            title = { Text("Dismiss ${holder.name}?") },
            text = {
                Column(Modifier.dialogScroll()) { Text(
                    "Severance is ${Format.compact(holder.severanceCents, currency)}, paid now from company cash. The ${post.title.lowercase()} post is open at once and you lose: ${holder.effect.replaceFirstChar { it.lowercase() }}.",
                    color = Palette.secondary, fontSize = 13.sp,
                ) }
            },
            confirmButton = {
                GoldButton("Dismiss") {
                    data2?.let { store.dismissExecutive(it.firm, post.postCode.toInt()) }
                    dismissPost = null
                }
            },
            dismissButton = { TextButton(onClick = { dismissPost = null }) { Text("Keep") } },
        )
    }
}

/** The three posts, drawn from the engine's numbers with nothing behind it, so it can be rendered from a saved game. */
@Composable
internal fun ExecutivesBody(
    data: ExecutivesDto,
    currency: String,
    busy: Boolean,
    onHire: (ExecutivePostDto) -> Unit,
    onDismiss: (ExecutivePostDto) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        data.posts.forEach { post ->
            val holder = post.holder
            Column(
                Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.shape).padding(10.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(post.title.uppercase(), color = Palette.gold, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                if (holder == null) {
                    Text("Empty. ${post.blurb}", color = Palette.secondary, fontSize = 12.sp)
                    GoldButton("Hire", Modifier.fillMaxWidth(), enabled = !busy) { onHire(post) }
                } else {
                    HolderRow(holder, currency)
                    OutlinedButton(onClick = { onDismiss(post) }, enabled = !busy) { Text("Dismiss", color = Palette.primaryText) }
                }
            }
        }
    }
}

@Composable
private fun HolderRow(e: ExecutiveDto, currency: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.Top) {
        PixelPortrait(e.portraitId, size = 56.dp)
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(e.name, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 14.sp)
            Text("Level ${e.level} of 5 · ${temperamentLabel(e.temperament)}", color = Palette.gold, fontSize = 12.sp)
            Text(e.effect + ".", color = Palette.primaryText, fontSize = 12.sp)
            Text(
                "Pay ${Format.compact(e.payCents, currency)} a week · contract ${contractLeft(e.weeksLeft.toInt())}",
                color = Palette.secondary, fontSize = 11.sp,
            )
        }
    }
}

internal fun contractLeft(weeks: Int): String = when {
    weeks <= 0 -> "ends this week"
    weeks < 13 -> "$weeks weeks left"
    else -> "${weeks / 52}.${weeks % 52 * 10 / 52} years left"
}

/** The candidates for a post, best first, each with the effect they would have at this company and the cost. */
@Composable
internal fun ExecutiveHireDialog(
    post: ExecutivePostDto,
    people: List<CandidateDto>,
    currency: String,
    busy: Boolean,
    onHire: (CandidateDto) -> Unit,
    onClose: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onClose,
        containerColor = Palette.panel,
        title = { Text("Hire a ${post.title.lowercase()}") },
        text = { HireListBody(people, currency, busy, onHire) },
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
    )
}

@Composable
internal fun HireListBody(people: List<CandidateDto>, currency: String, busy: Boolean, onHire: (CandidateDto) -> Unit) {
    Column(
        Modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (people.isEmpty()) Text("Nobody is looking for work right now. New names turn up every quarter.", color = Palette.secondary, fontSize = 12.sp)
        people.forEach { c ->
            Column(
                Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.shape).padding(10.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.Top) {
                    PixelPortrait(c.portraitId, size = 48.dp)
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(c.name, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 14.sp)
                        Text("Level ${c.level} of 5 · ${temperamentLabel(c.temperament)} · ${c.age}", color = Palette.gold, fontSize = 12.sp)
                        Text(c.effect + ".", color = Palette.primaryText, fontSize = 12.sp)
                        if (c.hireNote.isNotEmpty()) Text(c.hireNote, color = Palette.secondary, fontSize = 11.sp)
                    }
                }
                Text(
                    "Pay ${Format.compact(c.askCents, currency)} a week for two years, ${Format.compact(c.signingFeeCents, currency)} to sign. " +
                        if (c.paybackWeeks > 0u) "At your figures it is worth about ${Format.compact(c.yearlyValueCents, currency)} a year and the fee is back in ${paybackText(c.paybackWeeks.toLong())}."
                        else "At your figures it does not cover the pay today.",
                    color = Palette.secondary, fontSize = 11.sp,
                )
                if (!c.affordable) Text("Not enough cash: the fee and 26 weeks of pay must be in hand.", color = Palette.warning, fontSize = 11.sp)
                GoldButton("Hire ${c.name.substringBefore(' ')}", Modifier.fillMaxWidth(), enabled = !busy && c.affordable) { onHire(c) }
            }
        }
    }
}

/** A rival's executives in its dossier. */
@Composable
internal fun RivalExecutives(posts: List<ExecutivePostDto>, currency: String) {
    val held = posts.filter { it.holder != null }
    if (held.isEmpty()) return
    Eyebrow("Executives")
    held.forEach { post ->
        val e = post.holder ?: return@forEach
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            PixelPortrait(e.portraitId, size = 32.dp)
            Column {
                Text("${e.name}, ${post.title.lowercase()}", color = Palette.primaryText, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                Text(
                    "Level ${e.level} · pay ${Format.compact(e.payCents, currency)} a week · to move them, offer at least ${Format.compact(e.poachPriceCents, currency)}",
                    color = Palette.secondary, fontSize = 11.sp,
                )
            }
        }
    }
}
