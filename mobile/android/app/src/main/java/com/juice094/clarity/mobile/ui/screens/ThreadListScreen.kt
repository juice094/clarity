package com.juice094.clarity.mobile.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.model.Screen
import com.juice094.clarity.mobile.ui.components.ClarityTextField
import com.juice094.clarity.mobile.ui.components.ClawEntry
import com.juice094.clarity.mobile.ui.components.ThreadItem
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing
import com.juice094.clarity.mobile.viewmodel.ChatViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ThreadListScreen(viewModel: ChatViewModel, modifier: Modifier = Modifier) {
    val threads = viewModel.threads
    var showClawDialog by remember { mutableStateOf(false) }

    // Refresh thread list when this screen becomes visible.
    LaunchedEffect(Unit) {
        viewModel.loadThreads()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = "Clarity",
                        style = MaterialTheme.typography.titleLarge,
                        color = ClarityColors.TextPrimary
                    )
                },
                actions = {
                    IconButton(onClick = { /* TODO: search threads */ }) {
                        Icon(
                            imageVector = Icons.Default.Search,
                            contentDescription = "Search",
                            tint = ClarityColors.TextPrimary
                        )
                    }
                    IconButton(onClick = { viewModel.currentScreen.value = Screen.Settings }) {
                        Icon(
                            imageVector = Icons.Default.Settings,
                            contentDescription = "Settings",
                            tint = ClarityColors.TextPrimary
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = ClarityColors.Background,
                    titleContentColor = ClarityColors.TextPrimary,
                    actionIconContentColor = ClarityColors.TextPrimary
                )
            )
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = {
                    if (viewModel.runtime != null) {
                        viewModel.createNewChat()
                    } else {
                        viewModel.currentScreen.value = Screen.ProviderSetup
                    }
                },
                containerColor = ClarityColors.Primary,
                contentColor = ClarityColors.OnPrimary
            ) {
                Icon(Icons.Default.Add, contentDescription = "New chat")
            }
        },
        containerColor = ClarityColors.Background,
        modifier = modifier
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            contentPadding = PaddingValues(vertical = ClaritySpacing.sm)
        ) {
            // ── Fixed Claw entry at the top ──
            item(key = "claw_entry") {
                ClawEntry(onClick = { showClawDialog = true })
            }

            // ── Normal chat threads ──
            items(threads, key = { it.threadId }) { thread ->
                ThreadItem(
                    thread = thread,
                    onClick = { viewModel.switchToThread(thread.threadId) }
                )
            }
        }
    }

    if (showClawDialog) {
        ClawSetupDialog(
            viewModel = viewModel,
            onDismiss = { showClawDialog = false }
        )
    }
}

@Composable
private fun ClawSetupDialog(viewModel: ChatViewModel, onDismiss: () -> Unit) {
    val gatewayUrl by viewModel.gatewayUrl
    val gatewayToken by viewModel.gatewayToken
    var showToken by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = ClarityColors.Surface,
        titleContentColor = ClarityColors.TextPrimary,
        textContentColor = ClarityColors.TextSecondary,
        title = { Text("Connect to Claw Gateway") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                ClarityTextField(
                    value = gatewayUrl,
                    onValueChange = { viewModel.gatewayUrl.value = it },
                    label = "Gateway WebSocket URL",
                    modifier = Modifier.fillMaxWidth()
                )
                ClarityTextField(
                    value = gatewayToken,
                    onValueChange = { viewModel.gatewayToken.value = it },
                    label = "Token (optional)",
                    visualTransformation = if (showToken) VisualTransformation.None else PasswordVisualTransformation(),
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Password),
                    trailingIcon = {
                        TextButton(onClick = { showToken = !showToken }) {
                            Text(if (showToken) "Hide" else "Show", color = ClarityColors.Primary)
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    onDismiss()
                    viewModel.initializeClaw()
                }
            ) {
                Text("Connect", color = ClarityColors.Primary)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = ClarityColors.TextSecondary)
            }
        }
    )
}
