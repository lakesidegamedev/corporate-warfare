package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import uniffi.cw_ffi.ClassifiedDto
import uniffi.cw_ffi.PaperDto
import uniffi.cw_ffi.PaperLinkDto
import uniffi.cw_ffi.StoryDto

/** Seconds each headline stays on the ticker while time runs. */
private const val TICKER_MILLIS = 5_000L

/** Width from which the paper lays out as a front page with columns; narrower stacks. */
private val WIDE = 760.dp

private fun change(bps: Long) = "%s%.1f%%".format(if (bps > 0) "+" else if (bps < 0) "-" else "", kotlin.math.abs(bps) / 100.0)
private fun changeColor(bps: Long) = if (bps > 0) Palette.positive else if (bps < 0) Palette.negative else Palette.secondary

/** A small pixel arrow, up in green, down in red, a dash when flat. */
@Composable
private fun MoveArrow(bps: Long) {
    when {
        bps > 0 -> PixelIcon("arrow_up", size = 12.dp, tint = Palette.positive)
        bps < 0 -> PixelIcon("arrow_down", size = 12.dp, tint = Palette.negative)
        else -> PixelIcon("minus", size = 12.dp, tint = Palette.secondary)
    }
}

/**
 * The weekly paper as a front page: masthead, date line, a lead story, briefs, a markets column
 * and classifieds. Wide screens put the briefs in columns beside the markets; phones stack.
 * Tapping a story opens what it is about.
 */
@Composable
fun PaperScreen(store: GameStore) {
    val now = store.overview?.tick?.toInt() ?: 0
    var picked by remember { mutableStateOf<Int?>(null) }
    // A new week brings the new issue unless the reader is deliberately on a back issue.
    LaunchedEffect(now) { if (picked == now) picked = null }
    val week = picked ?: now
    val paper = rememberEngine(store, week) { it.paper(week.toUInt()) }
    val archive = rememberEngine(store, now) { it.paperArchive() }.orEmpty().map { it.toInt() }
    var open by remember { mutableStateOf<PaperLinkDto?>(null) }
    var openStory by remember { mutableStateOf<StoryDto?>(null) }
    fun follow(story: StoryDto) {
        if (story.link.kind == "none") return
        open = story.link
        openStory = story
    }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= WIDE
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(if (wide) 14.dp else 12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (paper == null) {
                Text("Printing", color = Palette.secondary)
                return@Column
            }
            Masthead(paper, wide)
            IssuePicker(week, now, archive) { picked = if (it == now) null else it }
            if (wide) {
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    Column(Modifier.weight(2.2f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        paper.lead?.let { Lead(it, ::follow) }
                            ?: Quiet()
                        if (paper.briefs.isNotEmpty()) {
                            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                                paper.briefs.forEach { Brief(it, Modifier.weight(1f), ::follow) }
                                repeat(3 - paper.briefs.size) { Spacer(Modifier.weight(1f)) }
                            }
                        }
                    }
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Markets(store, paper, ::follow)
                        Classifieds(paper.classifieds) { open = it }
                    }
                }
            } else {
                paper.lead?.let { Lead(it, ::follow) } ?: Quiet()
                paper.briefs.forEach { Brief(it, Modifier.fillMaxWidth(), ::follow) }
                Markets(store, paper, ::follow)
                Classifieds(paper.classifieds) { open = it }
            }
        }
    }
    open?.let { link -> PaperTarget(store, link, openStory) { open = null; openStory = null } }
}

@Composable
private fun Rule(thick: Boolean = false) {
    Box(Modifier.fillMaxWidth().height(if (thick) 3.dp else 1.dp).background(Palette.primaryText.copy(alpha = if (thick) 0.9f else 0.5f)))
}

@Composable
private fun Masthead(paper: PaperDto, wide: Boolean) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(5.dp)) {
        Text(
            paper.masthead.uppercase(), color = Palette.primaryText, fontFamily = Pixel.font, fontWeight = FontWeight.Bold,
            fontSize = if (wide) 40.sp else 26.sp, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth(),
        )
        Rule(thick = true)
        Text(
            paper.dateLine.uppercase(), color = Palette.secondary, fontFamily = Pixel.font, fontSize = 11.sp,
            letterSpacing = 1.sp, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth(),
        )
        Rule()
    }
}

@Composable
private fun IssuePicker(week: Int, now: Int, archive: List<Int>, onPick: (Int) -> Unit) {
    val index = archive.indexOf(week)
    val older = archive.getOrNull(index + 1)
    val newer = if (index > 0) archive.getOrNull(index - 1) else null
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("BACK ISSUES", color = Palette.secondary, fontFamily = Pixel.font, fontSize = 11.sp, modifier = Modifier.weight(1f))
        Chip("Older", older != null) { older?.let(onPick) }
        Text(if (week == now) "Today" else "Issue $week", color = Palette.primaryText, fontSize = 12.sp, fontFamily = Pixel.font)
        Chip("Newer", newer != null) { newer?.let(onPick) }
        Chip("Latest", week != now) { onPick(now) }
    }
}

@Composable
private fun Chip(text: String, enabled: Boolean, onClick: () -> Unit) {
    Text(
        text, color = if (enabled) Palette.primaryText else Palette.secondary.copy(alpha = 0.5f), fontSize = 11.sp, fontFamily = Pixel.font,
        modifier = Modifier.clip(Pixel.small).background(Palette.panelRaised).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 10.dp, vertical = 5.dp),
    )
}

@Composable
private fun Byline(story: StoryDto) {
    Text(
        (story.byline + if (story.yours) " · about you" else "").uppercase(), color = if (story.yours) Palette.gold else Palette.secondary,
        fontSize = 10.sp, fontFamily = Pixel.font, letterSpacing = 0.6.sp,
    )
}

@Composable
private fun Lead(story: StoryDto, onOpen: (StoryDto) -> Unit) {
    Column(
        Modifier.fillMaxWidth().clickable(enabled = story.link.kind != "none") { onOpen(story) },
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(story.headline, color = Palette.primaryText, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, fontSize = 26.sp, lineHeight = 30.sp)
        Byline(story)
        Text(story.body, color = Palette.primaryText.copy(alpha = 0.9f), fontSize = 15.sp, lineHeight = 21.sp)
        if (story.link.kind != "none") Text("READ MORE", color = Palette.gold, fontFamily = Pixel.font, fontSize = 11.sp)
    }
}

@Composable
private fun Brief(story: StoryDto, modifier: Modifier, onOpen: (StoryDto) -> Unit) {
    Column(
        modifier.clip(Pixel.shape).background(Palette.panel).clickable(enabled = story.link.kind != "none") { onOpen(story) }.padding(10.dp),
        verticalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Text(story.headline, color = Palette.primaryText, fontFamily = Pixel.font, fontWeight = FontWeight.Bold, fontSize = 15.sp, lineHeight = 18.sp)
        Byline(story)
        Text(story.body, color = Palette.secondary, fontSize = 12.sp, lineHeight = 17.sp)
    }
}

@Composable
private fun Quiet() {
    Column(Modifier.fillMaxWidth().padding(vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text("A QUIET WEEK", color = Palette.primaryText, fontFamily = Pixel.font, fontSize = 22.sp)
        Text("Nothing big to report. The markets and notices are below.", color = Palette.secondary, fontSize = 13.sp)
    }
}

@Composable
private fun Markets(store: GameStore, paper: PaperDto, onOpen: (StoryDto) -> Unit) {
    var link by remember { mutableStateOf<PaperLinkDto?>(null) }
    Column(Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panel).padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("MARKETS", color = Palette.secondary, fontFamily = Pixel.font, fontSize = 12.sp, letterSpacing = 0.8.sp)
        Rule()
        val m = paper.markets
        if (!m.open) {
            Text("The exchange has not closed a week yet.", color = Palette.secondary, fontSize = 12.sp)
        } else {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                MoveArrow(m.indexChangeBps)
                Text("Index", color = Palette.primaryText, fontSize = 13.sp, modifier = Modifier.weight(1f))
                Text(change(m.indexChangeBps), color = changeColor(m.indexChangeBps), fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
            }
            m.shares.forEach { s ->
                Row(
                    Modifier.fillMaxWidth().clickable { link = PaperLinkDto("company", s.firm, "") },
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    MoveArrow(s.changeBps)
                    Text(s.name, color = Palette.primaryText, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    Text(Format.price(s.priceCents, Format.numeraire), color = Palette.secondary, fontSize = 12.sp)
                    Text(change(s.changeBps), color = changeColor(s.changeBps), fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                }
            }
            if (m.goods.isNotEmpty()) Rule()
            m.goods.forEach { g ->
                Column(Modifier.fillMaxWidth().clickable { link = PaperLinkDto("good", g.product.toULong(), "") }) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        MoveArrow(g.changeBps)
                        Text(g.name, color = Palette.primaryText, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                        Text(Format.money(g.priceCents), color = Palette.secondary, fontSize = 12.sp)
                        Text(change(g.changeBps), color = changeColor(g.changeBps), fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                    }
                    if (g.reason.isNotEmpty()) Text(g.reason, color = Palette.secondary, fontSize = 11.sp, modifier = Modifier.padding(start = 18.dp))
                }
            }
            if (m.shares.isEmpty() && m.goods.isEmpty()) Text("Prices were flat.", color = Palette.secondary, fontSize = 12.sp)
        }
    }
    link?.let { PaperTarget(store, it, null) { link = null } }
}

@Composable
private fun Classifieds(items: List<ClassifiedDto>, onOpen: (PaperLinkDto) -> Unit) {
    Column(Modifier.fillMaxWidth().clip(Pixel.shape).background(Palette.panel).padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("CLASSIFIEDS", color = Palette.secondary, fontFamily = Pixel.font, fontSize = 12.sp, letterSpacing = 0.8.sp)
        Rule()
        if (items.isEmpty()) Text("No notices this week.", color = Palette.secondary, fontSize = 12.sp)
        items.forEach { c ->
            Text(
                c.text, color = Palette.primaryText, fontSize = 12.sp, lineHeight = 16.sp,
                modifier = Modifier.fillMaxWidth().clickable(enabled = c.link.kind != "none") { onOpen(c.link) },
            )
        }
    }
}

/** What a tapped story or price opens: a company page, a market, a region board, a deal's company or the event. */
@Composable
internal fun PaperTarget(store: GameStore, link: PaperLinkDto, story: StoryDto?, onClose: () -> Unit) {
    when (link.kind) {
        "company" -> CompanyPageDialog(store, link.id, onClose)
        "deal" -> {
            // A deal opens the company it is aimed at.
            var firm by remember(link.id) { mutableStateOf<ULong?>(null) }
            LaunchedEffect(link.id) {
                firm = store.read { h -> h.deals().firstOrNull { it.handle == link.id }?.targetHandle }
                if (firm == null) onClose()
            }
            firm?.let { CompanyPageDialog(store, it, onClose) }
        }
        "good" -> {
            val name = store.commodities.firstOrNull { it.product.toULong() == link.id }?.name ?: "Market"
            AlertDialog(
                onDismissRequest = onClose, containerColor = Palette.panel,
                title = { Text(name) },
                text = { Column(Modifier.verticalScroll(rememberScrollState())) { GoodDepthSection(store, link.id.toUInt()) } },
                confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
            )
        }
        "region" -> {
            val name = store.boards.firstOrNull { it.index.toULong() == link.id }?.name ?: "Region"
            AlertDialog(
                onDismissRequest = onClose, containerColor = Palette.panel,
                title = { Text(name) },
                text = { Column(Modifier.verticalScroll(rememberScrollState())) { RegionDossierSection(store, link.id.toUInt()) } },
                confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
            )
        }
        "event" -> AlertDialog(
            onDismissRequest = onClose, containerColor = Palette.panel,
            title = { Text(story?.headline ?: "News") },
            text = { Text(story?.body.orEmpty(), color = Palette.primaryText) },
            confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
        )
        else -> onClose()
    }
}

/**
 * One line with the top headline, on every screen. Time running cycles the top three every few
 * seconds; paused it holds the first; with animations off it never cycles. Tapping opens the paper.
 * It reads the engine only when paused or at 1x, never at speed.
 */
@Composable
fun TickerBar(store: GameStore, modifier: Modifier = Modifier, onOpen: () -> Unit) {
    // Read the engine only when time is paused or at 1x; at 2x and 4x the last headlines stay up
    // and one read catches up when time slows or stops.
    val live = !store.running || store.speed <= 1
    val revision = store.overview?.revision
    var headlines by remember { mutableStateOf<List<String>>(emptyList()) }
    LaunchedEffect(revision, live) {
        if (live) headlines = store.read { it.ticker() } ?: headlines
    }
    val animate = LocalAppSettings.current?.animations != false
    var shown by remember { mutableStateOf(0) }
    val cycling = store.running && animate && headlines.size > 1
    LaunchedEffect(headlines, cycling) {
        shown = 0
        while (cycling) {
            delay(TICKER_MILLIS)
            shown = (shown + 1) % headlines.size
        }
    }
    Row(
        modifier.fillMaxWidth().background(Palette.ink).clickable(onClick = onOpen).padding(horizontal = 12.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("NEWS", color = Palette.ink, fontFamily = Pixel.font, fontSize = 10.sp, modifier = Modifier.clip(Pixel.small).background(Palette.gold).padding(horizontal = 6.dp, vertical = 1.dp))
        Text(
            headlines.getOrNull(shown) ?: "A quiet week. Open the paper.", color = Palette.primaryText, fontSize = 12.sp,
            maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f),
        )
        PixelIcon("chevron_right", size = 12.dp, tint = Palette.secondary)
    }
}
