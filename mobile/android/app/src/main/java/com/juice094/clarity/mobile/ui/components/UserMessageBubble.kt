package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun UserMessageBubble(
    content: String,
    timestamp: String,
    onCopy: (() -> Unit)? = null,
    onDelete: (() -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    var showMenu by remember { mutableStateOf(false) }

    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End
    ) {
        Box {
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = ClarityColors.UserBubble
                ),
                shape = RoundedCornerShape(
                    topStart = ClarityRadius.lg,
                    topEnd = ClarityRadius.lg,
                    bottomStart = ClarityRadius.lg,
                    bottomEnd = 4.dp
                ),
                modifier = Modifier
                    .fillMaxWidth(0.85f)
                    .combinedClickable(
                        onClick = {},
                        onLongClick = { showMenu = true }
                    )
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        text = content,
                        color = ClarityColors.TextPrimary,
                        style = MaterialTheme.typography.bodyLarge
                    )
                    if (timestamp.isNotBlank()) {
                        Text(
                            text = timestamp,
                            style = MaterialTheme.typography.labelSmall,
                            color = ClarityColors.TextTertiary,
                            modifier = Modifier
                                .padding(top = 4.dp)
                                .align(Alignment.End)
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
