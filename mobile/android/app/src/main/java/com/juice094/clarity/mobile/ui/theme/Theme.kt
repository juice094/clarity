package com.juice094.clarity.mobile.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val DarkColorScheme = darkColorScheme(
    primary = ClarityColors.Primary,
    onPrimary = ClarityColors.OnPrimary,
    primaryContainer = ClarityColors.UserBubble,
    onPrimaryContainer = ClarityColors.TextPrimary,
    secondary = ClarityColors.PrimaryHover,
    secondaryContainer = ClarityColors.AssistantBubble,
    onSecondaryContainer = ClarityColors.TextPrimary,
    tertiary = ClarityColors.Warning,
    tertiaryContainer = ClarityColors.ToolBubble,
    onTertiaryContainer = ClarityColors.TextPrimary,
    surface = ClarityColors.Surface,
    onSurface = ClarityColors.TextPrimary,
    surfaceVariant = ClarityColors.ToolResultBubble,
    onSurfaceVariant = ClarityColors.TextSecondary,
    background = ClarityColors.Background,
    onBackground = ClarityColors.TextPrimary,
    error = ClarityColors.Error,
    outline = ClarityColors.Divider,
)

private val LightColorScheme = lightColorScheme(
    primary = ClarityColors.Primary,
    onPrimary = ClarityColors.OnPrimary,
    primaryContainer = ClarityColors.UserBubble,
    onPrimaryContainer = ClarityColors.TextPrimary,
    secondary = ClarityColors.PrimaryHover,
    secondaryContainer = ClarityColors.AssistantBubble,
    onSecondaryContainer = ClarityColors.TextPrimary,
    tertiary = ClarityColors.Warning,
    tertiaryContainer = ClarityColors.ToolBubble,
    onTertiaryContainer = ClarityColors.TextPrimary,
    surface = ClarityColors.Surface,
    onSurface = ClarityColors.TextPrimary,
    surfaceVariant = ClarityColors.ToolResultBubble,
    onSurfaceVariant = ClarityColors.TextSecondary,
    background = ClarityColors.Background,
    onBackground = ClarityColors.TextPrimary,
    error = ClarityColors.Error,
    outline = ClarityColors.Divider,
)

@Composable
fun ClarityMobileTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    // Dynamic color is disabled by design so the Clarity brand palette is
    // consistent across devices and Android versions.
    dynamicColor: Boolean = false,
    content: @Composable () -> Unit
) {
    val colorScheme = when {
        dynamicColor && darkTheme -> DarkColorScheme
        dynamicColor && !darkTheme -> LightColorScheme
        darkTheme -> DarkColorScheme
        else -> LightColorScheme
    }
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = colorScheme.background.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = false
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )
}
