package com.juice094.clarity.mobile.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * DeepSeek-inspired design tokens for the Clarity mobile client.
 *
 * The palette is intentionally static (no dynamic color) so the app looks
 * identical across Android versions and matches the desktop/web brand.
 */
object ClarityColors {
    // Backgrounds
    val Background = Color(0xFF0D0D12)
    val Surface = Color(0xFF17171F)
    val SurfaceElevated = Color(0xFF20202B)
    val Divider = Color(0xFF2A2A37)

    // Accents
    val Primary = Color(0xFF5B8DEF)
    val PrimaryHover = Color(0xFF7AA4F7)
    val OnPrimary = Color(0xFFFFFFFF)

    // Text
    val TextPrimary = Color(0xFFF0F0F5)
    val TextSecondary = Color(0xFF9CA3AF)
    val TextTertiary = Color(0xFF6B7280)

    // Bubbles
    val UserBubble = Color(0xFF2B3A55)
    val AssistantBubble = Color(0xFF17171F)
    val ToolBubble = Color(0xFF252336)
    val ToolResultBubble = Color(0xFF1E1E2A)

    // States
    val Error = Color(0xFFEF4444)
    val Success = Color(0xFF22C55E)
    val Warning = Color(0xFFF59E0B)
}

object ClaritySpacing {
    val xs = 4.dp
    val sm = 8.dp
    val md = 12.dp
    val lg = 16.dp
    val xl = 20.dp
    val xxl = 24.dp
}

object ClarityRadius {
    val sm = 8.dp
    val md = 12.dp
    val lg = 16.dp
    val xl = 24.dp
    val full = 1000.dp
}
