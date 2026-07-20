package com.juice094.clarity.mobile.viewmodel

import android.util.Log
import com.juice094.clarity.mobile.data.ClawSessionStore
import com.juice094.clarity.mobile.model.ChatItem
import com.juice094.clarity.mobile.model.ConnectionStatus
import uniffi.clarity_mobile_core.UiEvent
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

private fun nowTime(): String {
    return SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date())
}

/**
 * Record first-token latency for the current turn if this is the first content
 * chunk and the send timestamp is available.
 */
private fun ChatViewModel.recordFirstTokenLatency() {
    val sent = sendTimestampMs
    if (sent > 0L && firstTokenLatencyMs.value < 0L) {
        val latency = System.currentTimeMillis() - sent
        firstTokenLatencyMs.value = latency
        Log.d("ClarityLatency", "first_token_latency_ms=$latency")
    }
}

/**
 * Maps Rust [UiEvent]s into the observable UI state owned by [ChatViewModel].
 *
 * Kept separate from the ViewModel so that event interpretation can be tested
 * independently of the Android runtime lifecycle.
 */
internal fun ChatViewModel.handleEvent(event: UiEvent) {
    Log.d("ClarityEvent", "handleEvent $event messages=${messages.size}")
    when (event) {
        is UiEvent.TurnBegin -> {
            stopRequested = false
            statusText.value = ""
        }
        is UiEvent.ContentPart -> {
            if (stopRequested) return
            recordFirstTokenLatency()
            val streamingId = streamingAssistantId
            if (streamingId != null) {
                val index = messages.indexOfLast { it.id == streamingId }
                if (index >= 0) {
                    val item = messages[index]
                    if (item is ChatItem.AssistantText) {
                        messages[index] = item.copy(content = item.content + event.text)
                        persistClawSession()
                        return
                    }
                }
            }
            val newId = generateMessageId()
            streamingAssistantId = newId
            messages.add(
                ChatItem.AssistantText(
                    id = newId,
                    content = event.text,
                    isStreaming = true,
                    timestamp = nowTime(),
                )
            )
            persistClawSession()
        }
        is UiEvent.ReasoningPart -> {
            if (stopRequested) return
            recordFirstTokenLatency()
            val streamingId = streamingAssistantId
            if (streamingId != null) {
                val index = messages.indexOfLast { it.id == streamingId }
                if (index >= 0) {
                    val item = messages[index]
                    if (item is ChatItem.AssistantText) {
                        val current = item.reasoningContent ?: ""
                        messages[index] = item.copy(reasoningContent = current + event.text)
                        persistClawSession()
                        return
                    }
                }
            }
            val newId = generateMessageId()
            streamingAssistantId = newId
            messages.add(
                ChatItem.AssistantText(
                    id = newId,
                    content = "",
                    reasoningContent = event.text,
                    isStreaming = true,
                    timestamp = nowTime(),
                )
            )
            persistClawSession()
        }
        is UiEvent.TurnEnd -> {
            stopRequested = false
            captureAuthToken()
            streamingAssistantId?.let { id ->
                val index = messages.indexOfLast { it.id == id }
                if (index >= 0) {
                    val item = messages[index]
                    if (item is ChatItem.AssistantText) {
                        messages[index] = item.copy(isStreaming = false)
                    }
                }
                streamingAssistantId = null
            }
            isLoading.value = false
            statusText.value = ""
            turnTimeoutJob?.cancel()
            persistClawSession()
        }
        is UiEvent.StatusUpdate -> {
            statusText.value = event.message
            connectionStatus.value = when {
                event.message == "Connected to Gateway" -> ConnectionStatus.Connected
                event.message.startsWith("Reconnecting:") -> {
                    val reason = event.message.removePrefix("Reconnecting:").trim()
                    ConnectionStatus.Reconnecting(reason)
                }
                event.message.startsWith("Connection closed:") -> ConnectionStatus.Disconnected(event.message)
                else -> connectionStatus.value
            }
        }
        is UiEvent.Usage -> {
            statusText.value = "Tokens: ${event.promptTokens} -> ${event.completionTokens}"
        }
        is UiEvent.Error -> {
            stopRequested = false
            streamingAssistantId = null
            errorText.value = "${event.code}: ${event.message}"
            isLoading.value = false
            turnTimeoutJob?.cancel()
            if (event.code == "transport_error") {
                connectionStatus.value = ConnectionStatus.Error(event.message)
            }
        }
        is UiEvent.ToolCall -> {
            messages.add(
                ChatItem.ToolCallCard(
                    id = "tc_${event.id}",
                    turnId = event.turnId,
                    callId = event.id,
                    toolName = event.name,
                    argumentsJson = event.argumentsJson,
                    timestamp = nowTime(),
                )
            )
            statusText.value = "Tool: ${event.name}"
        }
        is UiEvent.ApprovalRequest -> {
            pendingApproval.value = com.juice094.clarity.mobile.model.PendingApproval(
                requestId = event.requestId,
                turnId = event.turnId,
                toolName = event.toolName,
                argumentsJson = event.argumentsJson,
                description = event.description,
            )
            statusText.value = "Approval needed: ${event.toolName}"
        }
        is UiEvent.ToolResult -> {
            messages.add(
                ChatItem.ToolResultCard(
                    id = "tr_${event.id}",
                    turnId = event.turnId,
                    callId = event.id,
                    toolName = findToolName(event.id),
                    resultJson = event.resultJson,
                    timestamp = nowTime(),
                )
            )
            statusText.value = "Tool result received"
        }
        is UiEvent.ThreadActive -> {
            // Thread switch acknowledged.
        }
        is UiEvent.DevicePaired -> {
            val status = if (event.approved) {
                "Device paired: ${event.token?.take(8)?.plus("...") ?: "no token"}"
            } else {
                "Device pairing rejected"
            }
            statusText.value = status
            Log.d("ClarityEvent", "DevicePaired approved=${event.approved} deviceId=${event.deviceId}")
        }
    }
}

private fun ChatViewModel.findToolName(callId: String): String {
    for (item in messages.asReversed()) {
        if (item is ChatItem.ToolCallCard && item.callId == callId) {
            return item.toolName
        }
    }
    return "tool"
}

/**
 * Persist the current Claw session to local storage if we are in Claw mode.
 *
 * Called on every streaming update and at turn end so that the conversation
 * survives unexpected Gateway disconnects, app backgrounding, or process death.
 */
private fun ChatViewModel.persistClawSession() {
    if (isClawMode.value && clawSessionId.value.isNotBlank()) {
        ClawSessionStore.saveSession(
            getApplication(),
            clawSessionId.value,
            messages.toList()
        )
    }
}
