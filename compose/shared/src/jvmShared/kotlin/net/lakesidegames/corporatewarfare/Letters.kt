package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.cw_ffi.LetterDto
import uniffi.cw_ffi.LetterSenderDto
import uniffi.cw_ffi.PaperLinkDto

// The stationery: warm paper, dark ink, one red seal. Fixed colours so a letter reads the same
// under any theme.
private val Sheet = Color(0xFFEADFC0)
private val SheetEdge = Color(0xFFB9A57A)
private val SheetRule = Color(0xFFC9B88F)
private val Ink = Color(0xFF2A2116)
private val Faded = Color(0xFF6B5B3E)
private val Seal = Color(0xFF9B3B2E)

/** Weeks the Inbox keeps before a letter moves to the archive. */
const val INBOX_WEEKS = 8

/**
 * A sender's face: the cast portrait for a person (the lookup is by id, so a larger atlas
 * needs nothing here), a plain role tile for an office.
 */
@Composable
fun SenderPortrait(sender: LetterSenderDto, size: androidx.compose.ui.unit.Dp = 40.dp) {
    if (sender.person.isNotEmpty() && CastPortraits.all.containsKey(sender.person)) {
        PixelPortrait(sender.person, size = size)
    } else {
        Box(
            Modifier.size(size).clip(Pixel.small).background(Palette.panelRaised).border(1.dp, Palette.stroke, Pixel.small),
            contentAlignment = Alignment.Center,
        ) {
            PixelIcon("people", size = size / 2, tint = Palette.secondary)
        }
    }
}

private fun openLabel(link: PaperLinkDto) = when (link.kind) {
    "company" -> "Open company"
    "deal" -> "Review the bid"
    "event" -> "Read the notice"
    "good" -> "Open market"
    "region" -> "Open region"
    else -> ""
}

/** One letter in the stack: portrait, sender, first line, date and an unread dot. */
@Composable
fun LetterRow(letter: LetterDto, selected: Boolean, onClick: () -> Unit) {
    val tint = if (selected) Palette.panelRaised else Palette.panel
    Row(
        Modifier.fillMaxWidth().clip(Pixel.shape).background(tint)
            .border(1.dp, if (selected) Palette.gold.copy(alpha = 0.6f) else Palette.stroke, Pixel.shape)
            .clickable(onClick = onClick).padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.Top,
    ) {
        SenderPortrait(letter.sender, 44.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    letter.sender.name, color = Palette.primaryText, fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    fontWeight = if (letter.read) FontWeight.Normal else FontWeight.Bold, modifier = Modifier.weight(1f, fill = false),
                )
                if (!letter.read) Box(Modifier.size(8.dp).clip(Pixel.small).background(Palette.gold))
            }
            Text(
                listOf(letter.sender.role, letter.sender.organisation).filter { it.isNotEmpty() && it != letter.sender.name }.joinToString(" · "),
                color = Palette.secondary, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
            Text(letter.summary, color = Palette.primaryText.copy(alpha = if (letter.read) 0.7f else 1f), fontSize = 12.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Column(horizontalAlignment = Alignment.End, verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(ResultBuckets.weekLabel(letter.week.toInt()), color = Palette.secondary, fontSize = 10.sp, fontFamily = Pixel.font)
            if (letter.event != null) {
                SignalPill(if (letter.mandatory) "Due ${ResultBuckets.weekLabel(letter.deadline.toInt())}" else "Reply", if (letter.mandatory) Palette.warning else Palette.gold)
            }
        }
    }
}

/**
 * An open letter on a paper-like panel: letterhead with the organisation, the sender's portrait,
 * the salutation, the body, the sign-off and signature, then the same buttons the inbox always
 * offered: the card's choices, and Open for what the letter is about.
 */
@Composable
fun LetterPaper(store: GameStore, letter: LetterDto, modifier: Modifier = Modifier) {
    var open by remember(letter.id) { mutableStateOf<PaperLinkDto?>(null) }
    LaunchedEffect(letter.id) { if (!letter.read) store.markRead(listOf(letter.id)) }
    Column(
        modifier.widthIn(max = 620.dp).fillMaxWidth().padding(end = 3.dp, bottom = 3.dp)
            .background(SheetEdge).padding(end = 3.dp, bottom = 3.dp)
            .clip(Pixel.shape).background(Sheet).padding(horizontal = 18.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        // Letterhead.
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            SenderPortrait(letter.sender, 56.dp)
            Column(Modifier.weight(1f)) {
                Text(
                    (letter.sender.organisation.ifEmpty { letter.sender.role }).uppercase(), color = Ink, fontFamily = Pixel.font,
                    fontWeight = FontWeight.Bold, fontSize = 17.sp, letterSpacing = 0.6.sp, maxLines = 2,
                )
                Text(
                    listOf(letter.sender.role, ResultBuckets.weekLabel(letter.week.toInt())).filter { it.isNotEmpty() }.joinToString(" · "),
                    color = Faded, fontSize = 11.sp,
                )
            }
            if (letter.event != null && letter.mandatory) {
                Text("DUE ${ResultBuckets.weekLabel(letter.deadline.toInt())}", color = Seal, fontFamily = Pixel.font, fontSize = 11.sp, fontWeight = FontWeight.Bold)
            }
        }
        Box(Modifier.fillMaxWidth().height(2.dp).background(Ink))
        Box(Modifier.fillMaxWidth().height(1.dp).background(SheetRule))
        // Body.
        Text(letter.salutation, color = Ink, fontSize = 14.sp, fontWeight = FontWeight.Medium)
        letter.paragraphs.forEach { Text(it, color = Ink, fontSize = 14.sp, lineHeight = 20.sp) }
        Spacer(Modifier.height(2.dp))
        Text(letter.signOff + ",", color = Ink, fontSize = 14.sp)
        Text(letter.sender.name, color = Ink, fontSize = 16.sp, fontStyle = FontStyle.Italic, fontWeight = FontWeight.SemiBold)
        // Actions.
        if (letter.choices.isNotEmpty() || openLabel(letter.link).isNotEmpty()) {
            Box(Modifier.fillMaxWidth().height(1.dp).background(SheetRule))
            letter.event?.let { event ->
                letter.choices.forEach { choice ->
                    LetterButton(choice.label, filled = true, enabled = !store.busy) { store.respond(event, choice.id) }
                }
            }
            val label = openLabel(letter.link)
            if (label.isNotEmpty()) LetterButton(label, filled = false, enabled = true) { open = letter.link }
        }
    }
    open?.let { link -> PaperTarget(store, link, null) { open = null } }
}

@Composable
private fun LetterButton(text: String, filled: Boolean, enabled: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (filled) Sheet else Ink, fontSize = 13.sp, fontWeight = FontWeight.SemiBold,
        modifier = Modifier.fillMaxWidth().clip(Pixel.shape)
            .background(if (filled) Ink.copy(alpha = if (enabled) 1f else 0.4f) else Color.Transparent)
            .border(1.dp, Ink.copy(alpha = if (enabled) 1f else 0.4f), Pixel.shape)
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 12.dp, vertical = 9.dp),
    )
}

/** The letters of the last eight weeks, then an archive of older ones on request. */
@Composable
fun LetterStack(store: GameStore, selected: String?, onSelect: (LetterDto) -> Unit) {
    var showArchive by remember { mutableStateOf(false) }
    var archive by remember { mutableStateOf<List<LetterDto>>(emptyList()) }
    val now = store.overview?.tick?.toInt() ?: 0
    // The archive is read when the player asks for it and time is not running, never at speed.
    LaunchedEffect(showArchive, store.running) {
        if (showArchive && !store.running) {
            archive = store.read { it.letters(1u) }.orEmpty().filter { it.week.toInt() < now - INBOX_WEEKS + 1 && it.event == null }
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (store.letters.isEmpty()) Callout("Quiet week. Letters arrive when something happens.", Palette.secondary)
        store.letters.forEach { letter -> LetterRow(letter, letter.id == selected) { onSelect(letter) } }
        Text(
            if (showArchive) "HIDE OLDER LETTERS" else "OLDER LETTERS", color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp,
            modifier = Modifier.clip(Pixel.small).clickable { showArchive = !showArchive }.padding(vertical = 6.dp, horizontal = 4.dp),
        )
        if (showArchive) {
            if (archive.isEmpty()) Text("Nothing older yet.", color = Palette.secondary, fontSize = 12.sp)
            archive.forEach { letter -> LetterRow(letter, letter.id == selected) { onSelect(letter) } }
        }
    }
}
