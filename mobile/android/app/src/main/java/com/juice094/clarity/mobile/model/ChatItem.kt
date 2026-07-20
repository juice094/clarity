package com.juice094.clarity.mobile.model

/**
 * A single item displayed in the chat stream.
 */
sealed class ChatItem(
    open val id: String,
    open val timestamp: String = "",
) {
    data class UserText(
        override val id: String,
        val content: String,
        override val timestamp: String = "",
    ) : ChatItem(id, timestamp)

    data class AssistantText(
        override val id: String,
        val content: String,
        val isStreaming: Boolean = false,
        val reasoningContent: String? = null,
        override val timestamp: String = "",
    ) : ChatItem(id, timestamp)

    data class ToolCallCard(
        override val id: String,
        val turnId: String,
        val callId: String,
        val toolName: String,
        val argumentsJson: String,
        override val timestamp: String = "",
    ) : ChatItem(id, timestamp)

    data class ToolResultCard(
        override val id: String,
        val turnId: String,
        val callId: String,
        val toolName: String,
        val resultJson: String,
        override val timestamp: String = "",
    ) : ChatItem(id, timestamp)
}
