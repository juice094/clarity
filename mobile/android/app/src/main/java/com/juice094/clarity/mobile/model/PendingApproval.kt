package com.juice094.clarity.mobile.model

/**
 * A pending approval request surfaced by the Agent.
 */
data class PendingApproval(
    val requestId: String,
    val turnId: String,
    val toolName: String,
    val argumentsJson: String,
    val description: String?,
)
