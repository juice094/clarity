package com.juice094.clarity.mobile.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.model.ProviderCapabilities
import com.juice094.clarity.mobile.ui.components.ClarityTextField
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing
import com.juice094.clarity.mobile.viewmodel.ChatViewModel
import uniffi.clarity_mobile_core.ProviderType

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProviderSetupScreen(viewModel: ChatViewModel, modifier: Modifier = Modifier) {
    val providerType by viewModel.providerType
    val apiKey by viewModel.apiKey
    val model by viewModel.model
    val mobile by viewModel.mobile
    val password by viewModel.password
    val gatewayUrl by viewModel.gatewayUrl
    val gatewayToken by viewModel.gatewayToken
    var expanded by remember { mutableStateOf(false) }
    var showKey by remember { mutableStateOf(false) }
    var showPassword by remember { mutableStateOf(false) }
    var showToken by remember { mutableStateOf(false) }
    val isDeviceLogin = providerType == ProviderType.DEEPSEEK_DEVICE

    Card(
        modifier = modifier
            .fillMaxWidth()
            .padding(ClaritySpacing.lg),
        elevation = CardDefaults.cardElevation(defaultElevation = 4.dp),
        colors = CardDefaults.cardColors(containerColor = ClarityColors.Surface)
    ) {
        Column(
            modifier = Modifier
                .padding(ClaritySpacing.lg)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(ClaritySpacing.md)
        ) {
            Text(
                text = "Connect to Clarity",
                style = MaterialTheme.typography.headlineSmall,
                color = ClarityColors.TextPrimary
            )

            Text(
                text = "Local agent (requires API key)",
                style = MaterialTheme.typography.titleSmall,
                color = ClarityColors.Primary
            )

            Box(modifier = Modifier.fillMaxWidth()) {
                ClarityTextField(
                    value = providerType.name,
                    onValueChange = {},
                    label = "Provider",
                    readOnly = true,
                    trailingIcon = {
                        IconButton(onClick = { expanded = !expanded }) {
                            Icon(
                                imageVector = Icons.Default.KeyboardArrowDown,
                                contentDescription = "Select provider",
                                tint = ClarityColors.TextSecondary
                            )
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
                DropdownMenu(
                    expanded = expanded,
                    onDismissRequest = { expanded = false },
                    containerColor = ClarityColors.SurfaceElevated,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    // Show the most commonly-used provider first so the default
                    // option is reachable without scrolling on small screens.
                    val orderedTypes = listOf(ProviderType.DEEPSEEK_DEVICE) +
                        ProviderType.entries.filter { it != ProviderType.DEEPSEEK_DEVICE }
                    orderedTypes.forEach { type ->
                        DropdownMenuItem(
                            text = {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Text(
                                        text = type.name,
                                        color = ClarityColors.TextPrimary
                                    )
                                    ProviderCapabilities.badgeLabel(type)?.let { badge ->
                                        Text(
                                            text = badge,
                                            style = MaterialTheme.typography.labelSmall,
                                            color = ClarityColors.Warning
                                        )
                                    }
                                }
                            },
                            onClick = {
                                viewModel.providerType.value = type
                                viewModel.model.value = defaultModelFor(type)
                                expanded = false
                            }
                        )
                    }
                }
            }

            // Non-API channels (App login flow) have no public API and no
            // enumerable model list; say so instead of implying an API flow.
            ProviderCapabilities.badgeDescription(providerType)?.let { description ->
                Text(
                    text = description,
                    style = MaterialTheme.typography.bodySmall,
                    color = ClarityColors.Warning
                )
            }

            ClarityTextField(
                value = apiKey,
                onValueChange = { viewModel.apiKey.value = it },
                label = if (isDeviceLogin) "Device token (optional)" else "API Key",
                visualTransformation = if (showKey) VisualTransformation.None else PasswordVisualTransformation(),
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Password),
                trailingIcon = {
                    TextButton(onClick = { showKey = !showKey }) {
                        Text(if (showKey) "Hide" else "Show", color = ClarityColors.Primary)
                    }
                },
                modifier = Modifier.fillMaxWidth()
            )

            if (isDeviceLogin) {
                ClarityTextField(
                    value = mobile,
                    onValueChange = { viewModel.mobile.value = it },
                    label = "Mobile number",
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Phone),
                    modifier = Modifier.fillMaxWidth()
                )

                ClarityTextField(
                    value = password,
                    onValueChange = { viewModel.password.value = it },
                    label = "Password",
                    visualTransformation = if (showPassword) VisualTransformation.None else PasswordVisualTransformation(),
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(keyboardType = KeyboardType.Password),
                    trailingIcon = {
                        TextButton(onClick = { showPassword = !showPassword }) {
                            Text(if (showPassword) "Hide" else "Show", color = ClarityColors.Primary)
                        }
                    },
                    modifier = Modifier.fillMaxWidth()
                )
            }

            ClarityTextField(
                value = model,
                onValueChange = { viewModel.model.value = it },
                // TODO(clarity-mobile-core): expose a `listModels()` FFI so API
                // channels can refresh the model list from `/v1/models`; the
                // Rust core has no such endpoint binding yet. Until then the
                // model name stays free-form input with static suggestions
                // (ChatViewModel.availableModels) in the chat screen.
                label = if (ProviderCapabilities.hasPublicApi(providerType)) {
                    "Model"
                } else {
                    "Model (由服务端决定 / set by server)"
                },
                modifier = Modifier.fillMaxWidth()
            )

            Button(
                onClick = { viewModel.initialize() },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(containerColor = ClarityColors.Primary)
            ) {
                Text("Connect Local Agent", color = ClarityColors.OnPrimary)
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp), color = ClarityColors.Divider)

            Text(
                text = "Remote Claw Gateway",
                style = MaterialTheme.typography.titleSmall,
                color = ClarityColors.Primary
            )

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

            Button(
                onClick = { viewModel.initializeClaw() },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.buttonColors(containerColor = ClarityColors.Primary)
            ) {
                Text("Connect via Claw", color = ClarityColors.OnPrimary)
            }

            val isPairing by viewModel.isPairing
            val pairingStatus by viewModel.pairingStatus
            Button(
                onClick = { viewModel.requestPairing() },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isPairing,
                colors = ButtonDefaults.buttonColors(containerColor = ClarityColors.Success)
            ) {
                Text(
                    if (isPairing) "Pairing..." else "Pair Device",
                    color = ClarityColors.OnPrimary
                )
            }

            if (pairingStatus.isNotBlank()) {
                Text(
                    text = pairingStatus,
                    style = MaterialTheme.typography.bodySmall,
                    color = ClarityColors.TextSecondary
                )
            }
        }
    }
}

private fun defaultModelFor(type: ProviderType): String = when (type) {
    ProviderType.OPEN_AI -> "gpt-4o"
    ProviderType.KIMI -> "kimi-k2.6"
    ProviderType.DEEPSEEK -> "deepseek-chat"
    ProviderType.ANTHROPIC -> "claude-3-sonnet-20240229"
    ProviderType.DEEPSEEK_DEVICE -> "deepseek-chat"
}
