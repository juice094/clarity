package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.model.PendingApproval
import com.juice094.clarity.mobile.ui.theme.ClarityColors

@Composable
fun ApprovalDialog(
    approval: PendingApproval,
    onAllow: (Boolean) -> Unit,
    onDeny: () -> Unit
) {
    var remember by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = { /* require explicit choice */ },
        containerColor = ClarityColors.Surface,
        titleContentColor = ClarityColors.TextPrimary,
        textContentColor = ClarityColors.TextSecondary,
        title = { Text("Approve tool execution?") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = approval.description
                        ?: "The agent wants to run ${approval.toolName}.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = ClarityColors.TextPrimary
                )
                Text(
                    text = "Tool: ${approval.toolName}",
                    style = MaterialTheme.typography.labelLarge,
                    color = ClarityColors.TextSecondary
                )
                FormattedJson(json = approval.argumentsJson)
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.padding(top = 8.dp)
                ) {
                    Switch(
                        checked = remember,
                        onCheckedChange = { remember = it }
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Remember for this session", color = ClarityColors.TextSecondary)
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onAllow(remember) }) {
                Text("Allow", color = ClarityColors.Primary)
            }
        },
        dismissButton = {
            TextButton(onClick = onDeny) {
                Text("Deny", color = ClarityColors.TextSecondary)
            }
        }
    )
}
