package net.lakesidegames.corporatewarfare

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * Two panes side by side on a wide window, one above the other (scrolling) on a phone. Each side
 * gets the modifier that sizes it, so screens write their panes once.
 */
@Composable
fun SplitOrStack(
    modifier: Modifier,
    leftWeight: Float,
    rightWeight: Float,
    leftPhone: Dp,
    rightPhone: Dp,
    left: @Composable (Modifier) -> Unit,
    right: @Composable (Modifier) -> Unit,
    wideAt: Dp = 800.dp,
) {
    BoxWithConstraints(modifier) {
        if (maxWidth >= wideAt) {
            Row(Modifier.fillMaxWidth().fillMaxHeight(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                left(Modifier.weight(leftWeight).fillMaxHeight())
                right(Modifier.weight(rightWeight).fillMaxHeight())
            }
        } else {
            Column(Modifier.fillMaxWidth().fillMaxHeight().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                left(Modifier.fillMaxWidth().height(leftPhone))
                right(Modifier.fillMaxWidth().height(rightPhone))
            }
        }
    }
}
