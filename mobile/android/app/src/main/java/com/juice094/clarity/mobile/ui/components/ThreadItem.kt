package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import uniffi.clarity_mobile_core.ThreadSummary
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.temporal.ChronoUnit

@Composable
fun ThreadItem(thread: ThreadSummary, onClick: () -> Unit, modifier: Modifier = Modifier) {
    ListItem(
        headlineContent = {
            Text(
                text = thread.title ?: "Chat",
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = ClarityColors.TextPrimary
            )
        },
        supportingContent = {
            Text(
                text = formatRelativeTime(thread.updatedAt),
                style = MaterialTheme.typography.bodySmall,
                color = ClarityColors.TextSecondary
            )
        },
        colors = ListItemDefaults.colors(
            containerColor = ClarityColors.Background
        ),
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
    )
    HorizontalDivider(color = ClarityColors.Divider)
}

private fun formatRelativeTime(rfc3339: String): String {
    return try {
        val instant = Instant.parse(rfc3339)
        val now = Instant.now()
        val minutes = ChronoUnit.MINUTES.between(instant, now)
        val hours = ChronoUnit.HOURS.between(instant, now)
        val days = ChronoUnit.DAYS.between(instant, now)
        when {
            minutes < 1 -> "just now"
            minutes < 60 -> "$minutes min ago"
            hours < 24 -> "$hours h ago"
            days < 7 -> "$days d ago"
            else -> instant.atZone(ZoneId.systemDefault())
                .format(DateTimeFormatter.ofPattern("MMM d"))
        }
    } catch (_: Exception) {
        rfc3339
    }
}
