package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClarityRadius
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun AssistantMessageBubble(
    content: String,
    isStreaming: Boolean,
    timestamp: String,
    reasoningContent: String? = null,
    onCopy: (() -> Unit)? = null,
    onRegenerate: (() -> Unit)? = null,
    onDelete: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    var showReasoning by remember { mutableStateOf(false) }
    var showMenu by remember { mutableStateOf(false) }

    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.Start
    ) {
        Box {
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = ClarityColors.AssistantBubble
                ),
                shape = RoundedCornerShape(
                    topStart = ClarityRadius.lg,
                    topEnd = ClarityRadius.lg,
                    bottomStart = 4.dp,
                    bottomEnd = ClarityRadius.lg
                ),
                modifier = Modifier
                    .fillMaxWidth(0.85f)
                    .combinedClickable(
                        onClick = {},
                        onLongClick = { showMenu = true }
                    )
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    reasoningContent?.takeIf { it.isNotBlank() }?.let { reasoning ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Text(
                                text = "Thinking",
                                style = MaterialTheme.typography.labelMedium,
                                color = ClarityColors.TextSecondary,
                                modifier = Modifier.weight(1f)
                            )
                            IconButton(onClick = { showReasoning = !showReasoning }) {
                                Icon(
                                    imageVector = if (showReasoning) Icons.Default.KeyboardArrowUp else Icons.Default.KeyboardArrowDown,
                                    contentDescription = if (showReasoning) "Collapse" else "Expand",
                                    tint = ClarityColors.TextSecondary
                                )
                            }
                        }
                        if (showReasoning) {
                            Text(
                                text = reasoning,
                                style = MaterialTheme.typography.bodySmall,
                                color = ClarityColors.TextTertiary,
                                modifier = Modifier.padding(bottom = ClaritySpacing.sm)
                            )
                        }
                        Spacer(modifier = Modifier.height(ClaritySpacing.xs))
                    }

                    MarkdownText(content = content)
                    if (isStreaming) {
                        Text(
                            text = "▌",
                            style = MaterialTheme.typography.bodyLarge,
                            color = ClarityColors.Primary
                        )
                    }

                    if (timestamp.isNotBlank()) {
                        Text(
                            text = timestamp,
                            style = MaterialTheme.typography.labelSmall,
                            color = ClarityColors.TextTertiary,
                            modifier = Modifier
                                .padding(top = 4.dp)
                                .align(Alignment.Start)
                        )
                    }
                }
            }

            DropdownMenu(
                expanded = showMenu,
                onDismissRequest = { showMenu = false },
                containerColor = ClarityColors.SurfaceElevated
            ) {
                onCopy?.let {
                    DropdownMenuItem(
                        text = { Text("Copy", color = ClarityColors.TextPrimary) },
                        onClick = { showMenu = false; it() }
                    )
                }
                onRegenerate?.let {
                    DropdownMenuItem(
                        text = { Text("Regenerate", color = ClarityColors.TextPrimary) },
                        onClick = { showMenu = false; it() }
                    )
                }
                onDelete?.let {
                    DropdownMenuItem(
                        text = { Text("Delete", color = ClarityColors.Error) },
                        onClick = { showMenu = false; it() }
                    )
                }
            }
        }
    }
}
