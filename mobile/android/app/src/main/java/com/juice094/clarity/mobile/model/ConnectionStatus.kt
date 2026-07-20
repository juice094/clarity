package com.juice094.clarity.mobile.model

/**
 * High-level connection state used to render the Claw/Gateway status banner.
 */
sealed class ConnectionStatus {
    data object Connected : ConnectionStatus()
    data class Reconnecting(val reason: String) : ConnectionStatus()
    data class Error(val message: String) : ConnectionStatus()
    data class Disconnected(val reason: String) : ConnectionStatus()
}
