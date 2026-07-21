package com.juice094.clarity.mobile.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import com.juice094.clarity.mobile.model.Screen
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing
import com.juice094.clarity.mobile.viewmodel.ChatViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    viewModel: ChatViewModel,
    onBack: () -> Unit,
    modifier: Modifier = Modifier
) {
    val providerType by viewModel.providerType
    val model by viewModel.model
    val isAgent by viewModel.isAgentMode

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                },
                colors = androidx.compose.material3.TopAppBarDefaults.topAppBarColors(
                    containerColor = ClarityColors.Background,
                    titleContentColor = ClarityColors.TextPrimary
                )
            )
        },
        containerColor = ClarityColors.Background,
        modifier = modifier
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(ClaritySpacing.lg)
        ) {
            SettingsSection(title = "Provider") {
                SettingsRow(label = "Provider", value = providerType.name)
                SettingsRow(label = "Model", value = model)

                var showChangeConfirm by remember { mutableStateOf(false) }
                TextButton(
                    onClick = { showChangeConfirm = true },
                    modifier = Modifier.align(Alignment.End)
                ) {
                    Text(
                        text = "更改 Provider / Change Provider",
                        color = ClarityColors.Primary
                    )
                }
                if (showChangeConfirm) {
                    AlertDialog(
                        onDismissRequest = { showChangeConfirm = false },
                        containerColor = ClarityColors.SurfaceElevated,
                        title = {
                            Text(
                                text = "更改 Provider？",
                                color = ClarityColors.TextPrimary
                            )
                        },
                        text = {
                            Text(
                                text = "将断开当前连接并清除已保存的凭据，本地聊天历史会保留。\n" +
                                    "This disconnects the current session and clears saved credentials. " +
                                    "Local chat history is kept.",
                                color = ClarityColors.TextSecondary
                            )
                        },
                        confirmButton = {
                            TextButton(
                                onClick = {
                                    showChangeConfirm = false
                                    viewModel.signOutToProviderSetup()
                                }
                            ) {
                                Text("确认 / Confirm", color = ClarityColors.Primary)
                            }
                        },
                        dismissButton = {
                            TextButton(onClick = { showChangeConfirm = false }) {
                                Text("取消 / Cancel", color = ClarityColors.TextSecondary)
                            }
                        }
                    )
                }
            }

            HorizontalDivider(color = ClarityColors.Divider)

            SettingsSection(title = "Preferences") {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = "Default agent mode",
                        style = MaterialTheme.typography.bodyLarge,
                        color = ClarityColors.TextPrimary
                    )
                    Switch(
                        checked = isAgent,
                        onCheckedChange = {
                            viewModel.isAgentMode.value = it
                            viewModel.syncAgentMode()
                        }
                    )
                }
            }

            HorizontalDivider(color = ClarityColors.Divider)

            Text(
                text = "Changing the provider returns to the connect screen; local chat history is preserved.",
                style = MaterialTheme.typography.bodySmall,
                color = ClarityColors.TextSecondary,
                modifier = Modifier.padding(horizontal = ClaritySpacing.lg)
            )
        }
    }
}

@Composable
private fun SettingsSection(
    title: String,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = ClaritySpacing.lg),
        verticalArrangement = Arrangement.spacedBy(ClaritySpacing.md)
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = ClarityColors.Primary
        )
        content()
    }
}

@Composable
private fun SettingsRow(label: String, value: String, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
            color = ClarityColors.TextSecondary
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodyLarge,
            color = ClarityColors.TextPrimary,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis
        )
    }
}
