package com.juice094.clarity.mobile.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Lightbulb
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.SmartToy
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
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
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.model.ConnectionStatus
import com.juice094.clarity.mobile.ui.components.ChatItemRenderer
import com.juice094.clarity.mobile.ui.components.MessageInput
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing
import com.juice094.clarity.mobile.viewmodel.ChatViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(viewModel: ChatViewModel, modifier: Modifier = Modifier) {
    val messages = viewModel.messages
    val isLoading by viewModel.isLoading
    val statusText by viewModel.statusText
    val isClaw by viewModel.isClawMode
    val isAgent by viewModel.isAgentMode
    val isSearch by viewModel.isSearchEnabled
    val isThinking by viewModel.isThinkingEnabled
    val connectionStatus by viewModel.connectionStatus
    val model = viewModel.effectiveModelName()
    val listState = rememberLazyListState()
    var showModelMenu by remember { mutableStateOf(false) }

    // Scroll to the bottom when messages change.
    LaunchedEffect(messages.size, messages.lastOrNull()) {
        if (messages.isNotEmpty()) {
            listState.animateScrollToItem(messages.size - 1)
        }
    }

    Scaffold(
        containerColor = ClarityColors.Background,
        modifier = modifier
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            if (connectionStatus !is ConnectionStatus.Connected) {
                ConnectionBanner(
                    status = connectionStatus,
                    modifier = Modifier.align(Alignment.TopCenter)
                )
            }

            LazyColumn(
                state = listState,
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(
                    top = 56.dp,
                    start = ClaritySpacing.lg,
                    end = ClaritySpacing.lg,
                    bottom = ClaritySpacing.sm
                ),
                verticalArrangement = Arrangement.spacedBy(ClaritySpacing.md)
            ) {
                items(messages, key = { it.id }) { item ->
                    ChatItemRenderer(
                        item = item,
                        onCopy = { viewModel.copyToClipboard(it) },
                        onRegenerate = { viewModel.regenerateMessage(it) },
                        onDelete = { viewModel.deleteMessage(it) }
                    )
                }
            }

            // Floating back button, DeepSeek-style minimal chrome.
            IconButton(
                onClick = { viewModel.backToThreadList() },
                modifier = Modifier
                    .padding(start = ClaritySpacing.sm, top = ClaritySpacing.sm)
                    .align(Alignment.TopStart)
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = "Back",
                    tint = ClarityColors.TextPrimary
                )
            }

            // Bottom toolbar: model switcher + feature toggles + input.
            Column(
                modifier = Modifier.align(Alignment.BottomCenter)
            ) {
                if (statusText.isNotBlank()) {
                    Text(
                        text = statusText,
                        style = MaterialTheme.typography.bodySmall,
                        color = ClarityColors.Primary,
                        modifier = Modifier.padding(horizontal = ClaritySpacing.lg, vertical = ClaritySpacing.xs)
                    )
                }

                if (!isClaw) {
                    FeatureToolbar(
                        model = model,
                        onModelClick = { showModelMenu = true },
                        showModelMenu = showModelMenu,
                        onDismissModelMenu = { showModelMenu = false },
                        viewModel = viewModel,
                        isAgent = isAgent,
                        onAgentChange = {
                            viewModel.isAgentMode.value = it
                            viewModel.syncAgentMode()
                        },
                        isSearch = isSearch,
                        onSearchChange = { viewModel.setSearchEnabled(it) },
                        isThinking = isThinking,
                        onThinkingChange = { viewModel.setThinkingEnabled(it) }
                    )
                } else {
                    val isPairing by viewModel.isPairing
                    val pairingStatus by viewModel.pairingStatus
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = ClaritySpacing.lg, vertical = ClaritySpacing.sm),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = pairingStatus.takeIf { it.isNotBlank() } ?: statusText,
                            style = MaterialTheme.typography.bodySmall,
                            color = ClarityColors.TextSecondary,
                            modifier = Modifier.weight(1f)
                        )
                        TextButton(
                            onClick = { viewModel.requestPairing() },
                            enabled = !isPairing
                        ) {
                            Text(
                                if (isPairing) "Pairing..." else "Pair Device",
                                color = if (isPairing) ClarityColors.TextTertiary else ClarityColors.Success
                            )
                        }
                    }
                }

                MessageInput(
                    onSend = { viewModel.sendMessage(it) },
                    onStop = { viewModel.stopTurn() },
                    isLoading = isLoading
                )
            }
        }
    }
}

@Composable
private fun FeatureToolbar(
    model: String,
    onModelClick: () -> Unit,
    showModelMenu: Boolean,
    onDismissModelMenu: () -> Unit,
    viewModel: ChatViewModel,
    isAgent: Boolean,
    onAgentChange: (Boolean) -> Unit,
    isSearch: Boolean,
    onSearchChange: (Boolean) -> Unit,
    isThinking: Boolean,
    onThinkingChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = ClaritySpacing.lg, vertical = ClaritySpacing.sm),
        horizontalArrangement = Arrangement.spacedBy(ClaritySpacing.sm),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box {
            FeatureChip(
                label = model,
                selected = false,
                onClick = onModelClick,
                icon = {
                    Icon(
                        imageVector = Icons.Default.KeyboardArrowDown,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp)
                    )
                }
            )
            ModelDropdownMenu(
                viewModel = viewModel,
                expanded = showModelMenu,
                onDismiss = onDismissModelMenu
            )
        }

        FeatureChip(
            label = "Agent",
            selected = isAgent,
            onClick = { onAgentChange(!isAgent) },
            icon = { Icon(Icons.Default.SmartToy, contentDescription = null, modifier = Modifier.size(16.dp)) }
        )
        FeatureChip(
            label = "Search",
            selected = isSearch,
            onClick = { onSearchChange(!isSearch) },
            icon = { Icon(Icons.Default.Search, contentDescription = null, modifier = Modifier.size(16.dp)) }
        )
        FeatureChip(
            label = "Thinking",
            selected = isThinking,
            onClick = { onThinkingChange(!isThinking) },
            icon = { Icon(Icons.Default.Lightbulb, contentDescription = null, modifier = Modifier.size(16.dp)) }
        )
    }
}

@Composable
private fun ModelDropdownMenu(
    viewModel: ChatViewModel,
    expanded: Boolean,
    onDismiss: () -> Unit
) {
    val models = viewModel.availableModels()
    val current = viewModel.effectiveModelName()

    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismiss,
        containerColor = ClarityColors.Surface
    ) {
        models.forEach { model ->
            DropdownMenuItem(
                text = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = model,
                            color = ClarityColors.TextPrimary,
                            modifier = Modifier.weight(1f)
                        )
                        if (model == current) {
                            Icon(
                                imageVector = Icons.Default.Check,
                                contentDescription = "Selected",
                                tint = ClarityColors.Primary,
                                modifier = Modifier.size(18.dp)
                            )
                        }
                    }
                },
                onClick = {
                    viewModel.setModel(model)
                    onDismiss()
                }
            )
        }
    }
}

@Composable
private fun ConnectionBanner(
    status: ConnectionStatus,
    modifier: Modifier = Modifier
) {
    val (text, color) = when (status) {
        is ConnectionStatus.Connected -> "" to ClarityColors.Primary
        is ConnectionStatus.Reconnecting -> "Reconnecting: ${status.reason}" to ClarityColors.Warning
        is ConnectionStatus.Error -> "Connection error: ${status.message}" to ClarityColors.Error
        is ConnectionStatus.Disconnected -> "Disconnected. ${status.reason}" to ClarityColors.Error
    }
    if (text.isNotEmpty()) {
        Surface(
            color = color.copy(alpha = 0.15f),
            contentColor = color,
            modifier = modifier
                .fillMaxWidth()
                .padding(horizontal = ClaritySpacing.lg, vertical = ClaritySpacing.sm)
        ) {
            Text(
                text = text,
                style = MaterialTheme.typography.labelMedium,
                color = color,
                modifier = Modifier.padding(ClaritySpacing.md)
            )
        }
    }
}

@Composable
private fun FeatureChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    icon: @Composable () -> Unit,
    modifier: Modifier = Modifier
) {
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = if (selected) ClarityColors.OnPrimary else ClarityColors.TextSecondary
            )
        },
        leadingIcon = icon,
        colors = FilterChipDefaults.filterChipColors(
            selectedContainerColor = ClarityColors.Primary,
            selectedLabelColor = ClarityColors.OnPrimary,
            containerColor = ClarityColors.SurfaceElevated,
            labelColor = ClarityColors.TextSecondary
        ),
        modifier = modifier
    )
}
