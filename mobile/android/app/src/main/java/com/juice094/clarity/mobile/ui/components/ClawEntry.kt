package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Build
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing

@Composable
fun ClawEntry(onClick: () -> Unit, modifier: Modifier = Modifier) {
    ListItem(
        headlineContent = {
            Text(
                text = "Claw",
                style = MaterialTheme.typography.bodyLarge,
                color = ClarityColors.TextPrimary
            )
        },
        supportingContent = {
            Text(
                text = "Remote Gateway session",
                style = MaterialTheme.typography.bodySmall,
                color = ClarityColors.TextSecondary
            )
        },
        leadingContent = {
            Icon(
                imageVector = Icons.Default.Build,
                contentDescription = null,
                tint = ClarityColors.Primary
            )
        },
        colors = ListItemDefaults.colors(
            containerColor = ClarityColors.Background
        ),
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = ClaritySpacing.xs)
    )
    HorizontalDivider(color = ClarityColors.Divider)
}
