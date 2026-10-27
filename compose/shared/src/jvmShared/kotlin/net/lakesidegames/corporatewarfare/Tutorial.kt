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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** One tutorial step: what to do, why it matters, where it happens, and how it is recognised. */
data class TutorialStep(
    val id: String,
    val title: String,
    val what: String,
    val why: String,
    /** The screen "Show me" opens. */
    val page: String,
    /** Done when the player opens this page, or sends one of these commands, or plays a week. */
    val visit: String? = null,
    val commands: Set<String> = emptySet(),
    val playWeek: Boolean = false,
)

/**
 * The guided tutorial: eight steps that tick off when the player actually does them. Screens
 * report visits with [visited]; the store reports every command it submits with [sent]; shells
 * open the page a step asks for through [requestedPage].
 */
object Tutorial {
    val steps = listOf(
        TutorialStep(
            "office", "Meet your company",
            "Open the CEO Office.",
            "It is your company's dashboard: cash, profit, what you make, what it costs and where you sell.",
            page = "office", visit = "office",
        ),
        TutorialStep(
            "week", "Play a week",
            "Press Next week, or set the speed to 1x and let time run.",
            "Everything happens a week at a time: plants produce, buyers buy, wages and loans get paid.",
            page = "office", playWeek = true,
        ),
        TutorialStep(
            "price", "Set a price",
            "In the pricing table, pick a product and slide its price against the market.",
            "Buyers take the cheapest offer first. Undercut to sell out, or price higher for margin and risk unsold stock.",
            page = "office", commands = setOf("SetPricePosture", "SetPrice"),
        ),
        TutorialStep(
            "actions", "Find any action",
            "Open the Command Center and search for something, like IPO or loan.",
            "Every action in the game is listed there with a plain explanation and a preview of what it costs.",
            page = "actions", visit = "actions",
        ),
        TutorialStep(
            "research", "Fund research",
            "On the Research screen, set a weekly research budget.",
            "Each technology level makes your plants need fewer inputs and workers and lifts quality.",
            page = "research", commands = setOf("SetResearchBudget"),
        ),
        TutorialStep(
            "economy", "Read the economy",
            "Open Markets and look at the economy card.",
            "Booms lift demand and busts cut it. The central bank's rate sets what your loans cost.",
            page = "markets", visit = "markets",
        ),
        TutorialStep(
            "build", "Grow",
            "Pick a region on the map with room to build and build plants there.",
            "Room to build means the region imports what it could buy from you. New plants take a few weeks to open.",
            page = "map", commands = setOf("BuildFacility", "ExpandFacility"),
        ),
        TutorialStep(
            "stats", "Check the results",
            "Open Stats to see revenue, profit and the balance sheet over time.",
            "Watch whether each move paid off. Cash running low is the first thing that sinks a company.",
            page = "stats", visit = "stats",
        ),
    )

    var settings: AppSettings? = null
    var done by mutableStateOf(setOf<String>())
        private set
    var hidden by mutableStateOf(false)
    var requestedPage by mutableStateOf<String?>(null)
    private var startTick: UInt? = null

    fun load(settings: AppSettings) {
        this.settings = settings
        done = settings.tutorialDone
        hidden = settings.tutorialHidden
    }

    val current: TutorialStep? get() = steps.firstOrNull { it.id !in done }
    val finished: Boolean get() = current == null

    private fun complete(step: TutorialStep) {
        if (step.id in done) return
        done = done + step.id
        settings?.chooseTutorial(done, hidden)
    }

    fun visited(page: String) {
        steps.filter { it.visit == page }.forEach(::complete)
    }

    fun sent(command: String) {
        steps.filter { command in it.commands }.forEach(::complete)
    }

    fun week(tick: UInt) {
        val start = startTick
        if (start == null) {
            startTick = tick
        } else if (tick > start) {
            steps.filter { it.playWeek }.forEach(::complete)
        }
    }

    fun hide() {
        hidden = true
        settings?.chooseTutorial(done, true)
    }

    fun restart() {
        done = emptySet()
        hidden = false
        startTick = null
        settings?.chooseTutorial(done, false)
    }
}

/** The current step: docked in a corner on desktop, a single slim row on phones. */
@Composable
fun TutorialPanel(compact: Boolean, modifier: Modifier = Modifier) {
    val step = Tutorial.current ?: return
    if (Tutorial.hidden) return
    val number = Tutorial.steps.indexOf(step) + 1
    if (compact) {
        Row(
            modifier.background(Palette.panelRaised).padding(start = 12.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(Modifier.size(20.dp).clip(CircleShape).background(Palette.gold), contentAlignment = Alignment.Center) {
                Text("$number", color = Palette.ink, fontWeight = FontWeight.Bold, fontSize = 11.sp)
            }
            Spacer(Modifier.width(8.dp))
            Column(Modifier.weight(1f)) {
                Text(step.title, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 13.sp, maxLines = 1)
                Text(step.what, color = Palette.secondary, fontSize = 11.sp, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
            }
            Text(
                "Show me", color = Palette.ink, fontWeight = FontWeight.Bold, fontSize = 12.sp,
                modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Palette.gold)
                    .clickable { Tutorial.requestedPage = step.page }.padding(horizontal = 10.dp, vertical = 5.dp),
            )
            TextButton(onClick = { Tutorial.hide() }) { Text("✕", color = Palette.secondary, fontSize = 13.sp) }
        }
        return
    }
    val shape = RoundedCornerShape(14.dp)
    Column(
        modifier.clip(shape).background(Palette.panelRaised).border(1.dp, Palette.gold.copy(alpha = 0.6f), shape)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(22.dp).clip(CircleShape).background(Palette.gold), contentAlignment = Alignment.Center) {
                Text("$number", color = Palette.ink, fontWeight = FontWeight.Bold, fontSize = 12.sp)
            }
            Spacer(Modifier.width(8.dp))
            Text(step.title, color = Palette.primaryText, fontWeight = FontWeight.Bold, fontSize = 14.sp, modifier = Modifier.weight(1f))
            Text("$number of ${Tutorial.steps.size}", color = Palette.secondary, fontSize = 11.sp)
        }
        Text(step.what, color = Palette.primaryText, fontSize = 13.sp)
        Text(step.why, color = Palette.secondary, fontSize = 12.sp)
        Row(verticalAlignment = Alignment.CenterVertically) {
            Tutorial.steps.forEach { s ->
                Box(
                    Modifier.padding(end = 4.dp).size(7.dp).clip(CircleShape)
                        .background(if (s.id in Tutorial.done) Palette.positive else if (s == step) Palette.gold else Palette.stroke),
                )
            }
            Spacer(Modifier.weight(1f))
            TextButton(onClick = { Tutorial.hide() }) { Text("Hide", color = Palette.secondary, fontSize = 12.sp) }
            Text(
                "Show me", color = Palette.ink, fontWeight = FontWeight.Bold, fontSize = 13.sp,
                modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(Palette.gold)
                    .clickable { Tutorial.requestedPage = step.page }.padding(horizontal = 12.dp, vertical = 6.dp),
            )
        }
    }
}
