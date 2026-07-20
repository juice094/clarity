package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextFieldColors
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.VisualTransformation
import com.juice094.clarity.mobile.ui.theme.ClarityColors
import com.juice094.clarity.mobile.ui.theme.ClarityRadius

@Composable
fun clarityTextFieldColors(): TextFieldColors = OutlinedTextFieldDefaults.colors(
    focusedContainerColor = ClarityColors.SurfaceElevated,
    unfocusedContainerColor = ClarityColors.SurfaceElevated,
    disabledContainerColor = ClarityColors.SurfaceElevated,
    focusedBorderColor = ClarityColors.Divider,
    unfocusedBorderColor = ClarityColors.Divider,
    disabledBorderColor = ClarityColors.Divider,
    focusedTextColor = ClarityColors.TextPrimary,
    unfocusedTextColor = ClarityColors.TextPrimary,
    disabledTextColor = ClarityColors.TextSecondary,
    focusedLabelColor = ClarityColors.TextSecondary,
    unfocusedLabelColor = ClarityColors.TextTertiary,
    disabledLabelColor = ClarityColors.TextTertiary,
    focusedPlaceholderColor = ClarityColors.TextTertiary,
    unfocusedPlaceholderColor = ClarityColors.TextTertiary,
    errorPlaceholderColor = ClarityColors.Error,
    errorLabelColor = ClarityColors.Error,
    errorBorderColor = ClarityColors.Error,
    errorTextColor = ClarityColors.Error,
    cursorColor = ClarityColors.Primary,
    focusedTrailingIconColor = ClarityColors.Primary,
    unfocusedTrailingIconColor = ClarityColors.TextSecondary
)

@Composable
fun ClarityTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String? = null,
    placeholder: String? = null,
    singleLine: Boolean = true,
    readOnly: Boolean = false,
    enabled: Boolean = true,
    isError: Boolean = false,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    trailingIcon: @Composable (() -> Unit)? = null,
    leadingIcon: @Composable (() -> Unit)? = null
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = modifier,
        label = label?.let { { Text(it) } },
        placeholder = placeholder?.let { { Text(it, color = ClarityColors.TextTertiary) } },
        singleLine = singleLine,
        readOnly = readOnly,
        enabled = enabled,
        isError = isError,
        visualTransformation = visualTransformation,
        keyboardOptions = keyboardOptions,
        trailingIcon = trailingIcon,
        leadingIcon = leadingIcon,
        shape = RoundedCornerShape(ClarityRadius.md),
        colors = clarityTextFieldColors()
    )
}
