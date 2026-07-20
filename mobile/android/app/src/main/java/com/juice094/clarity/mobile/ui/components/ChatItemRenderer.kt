package com.juice094.clarity.mobile.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.juice094.clarity.mobile.model.ChatItem

@Composable
fun ChatItemRenderer(
    item: ChatItem,
    onCopy: ((String) -> Unit)? = null,
    onRegenerate: ((String) -> Unit)? = null,
    onDelete: ((String) -> Unit)? = null,
    modifier: Modifier = Modifier
) {
    when (item) {
        is ChatItem.UserText -> UserMessageBubble(
            content = item.content,
            timestamp = item.timestamp,
            onCopy = onCopy?.let { { it(item.content) } },
            onDelete = onDelete?.let { { it(item.id) } },
            modifier = modifier
        )
        is ChatItem.AssistantText -> AssistantMessageBubble(
            content = item.content,
            isStreaming = item.isStreaming,
            timestamp = item.timestamp,
            reasoningContent = item.reasoningContent,
            onCopy = onCopy?.let { { it(item.content) } },
            onRegenerate = onRegenerate?.let { { it(item.id) } },
            onDelete = onDelete?.let { { it(item.id) } },
            modifier = modifier
        )
        is ChatItem.ToolCallCard -> ToolCallCard(
            toolName = item.toolName,
            argumentsJson = item.argumentsJson,
            modifier = modifier
        )
        is ChatItem.ToolResultCard -> ToolResultCard(
            toolName = item.toolName,
            resultJson = item.resultJson,
            modifier = modifier
        )
    }
}
