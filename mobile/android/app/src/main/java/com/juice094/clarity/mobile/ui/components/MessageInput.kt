package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClarityRadius
import com.juice094.clarity.mobile.ui.theme.ClaritySpacing

@Composable
fun MessageInput(
    onSend: (String) -> Unit,
    onStop: () -> Unit,
    isLoading: Boolean,
    modifier: Modifier = Modifier
) {
    var text by remember { mutableStateOf("") }
    val focusManager = LocalFocusManager.current

    Row(
        modifier = modifier
            .fillMaxWidth()
            .imePadding()
            .padding(ClaritySpacing.lg),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            placeholder = { Text("Message Clarity...", color = ClarityColors.TextTertiary) },
            modifier = Modifier.weight(1f),
            enabled = !isLoading,
            shape = RoundedCornerShape(ClarityRadius.xl),
            colors = OutlinedTextFieldDefaults.colors(
                focusedContainerColor = ClarityColors.SurfaceElevated,
                unfocusedContainerColor = ClarityColors.SurfaceElevated,
                disabledContainerColor = ClarityColors.SurfaceElevated,
                focusedBorderColor = ClarityColors.Divider,
                unfocusedBorderColor = ClarityColors.Divider,
                focusedTextColor = ClarityColors.TextPrimary,
                unfocusedTextColor = ClarityColors.TextPrimary,
                disabledTextColor = ClarityColors.TextSecondary,
                focusedPlaceholderColor = ClarityColors.TextTertiary,
                unfocusedPlaceholderColor = ClarityColors.TextTertiary
            ),
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
            keyboardActions = KeyboardActions(onSend = {
                if (text.isNotBlank()) {
                    onSend(text)
                    text = ""
                    focusManager.clearFocus()
                }
            })
        )
        Spacer(modifier = Modifier.width(8.dp))
        if (isLoading) {
            IconButton(onClick = onStop) {
                Icon(
                    imageVector = Icons.Default.Stop,
                    contentDescription = "Stop",
                    tint = ClarityColors.Error
                )
            }
        } else {
            IconButton(
                onClick = {
                    if (text.isNotBlank()) {
                        onSend(text)
                        text = ""
                        focusManager.clearFocus()
                    }
                },
                enabled = text.isNotBlank()
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.Send,
                    contentDescription = "Send",
                    tint = if (text.isNotBlank()) ClarityColors.Primary else ClarityColors.TextTertiary
                )
            }
        }
    }
}
