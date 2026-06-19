package com.juice094.clarity.mobile

import android.app.Application
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.clarity_mobile_core.MobileConfig
import uniffi.clarity_mobile_core.MobileRuntime
import uniffi.clarity_mobile_core.ProviderProfile
import uniffi.clarity_mobile_core.ProviderType
import uniffi.clarity_mobile_core.UiEvent

/**
 * A single chat message displayed in the UI.
 */
data class ChatMessage(
    val id: String,
    val role: String,
    val content: String,
    val isStreaming: Boolean = false,
)

/**
 * ViewModel that owns the [MobileRuntime] and bridges Rust events to Compose state.
 */
class ChatViewModel(application: Application) : AndroidViewModel(application) {

    var runtime: MobileRuntime? = null
        private set

    val messages = mutableStateListOf<ChatMessage>()
    val isLoading = mutableStateOf(false)
    val statusText = mutableStateOf("")
    val errorText = mutableStateOf("")

    // Provider configuration edited by the user.
    val providerType = mutableStateOf(ProviderType.DEEPSEEK)
    val apiKey = mutableStateOf("")
    val model = mutableStateOf("deepseek-chat")
    val isInitialized = mutableStateOf(false)

    /**
     * Initialize the Rust runtime with the user-supplied provider config.
     */
    fun initialize() {
        if (apiKey.value.isBlank()) {
            errorText.value = "API key is required"
            return
        }
        errorText.value = ""

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val dataDir = getApplication<Application>().filesDir.absolutePath
                val profile = ProviderProfile(
                    provider = providerType.value,
                    model = model.value,
                    apiKey = apiKey.value,
                    baseUrl = null,
                )
                val config = MobileConfig(
                    dataDir = dataDir,
                    defaultProvider = profile,
                )
                val rt = MobileRuntime(config)
                runtime = rt

                // Create the first thread.
                rt.createThread("New chat")

                // Start the Rust → UI event loop.
                launch(Dispatchers.IO) { eventLoop(rt) }

                isInitialized.value = true
            } catch (e: Exception) {
                errorText.value = "Init failed: ${e.message}"
            }
        }
    }

    /**
     * Send a user message and append it to the local UI immediately.
     */
    fun sendMessage(text: String) {
        val rt = runtime ?: return
        if (text.isBlank()) return

        messages.add(
            ChatMessage(
                id = "msg_${System.currentTimeMillis()}",
                role = "user",
                content = text,
            )
        )
        isLoading.value = true
        statusText.value = "Thinking..."

        viewModelScope.launch(Dispatchers.IO) {
            try {
                rt.sendMessage(text)
            } catch (e: Exception) {
                errorText.value = "Send failed: ${e.message}"
                isLoading.value = false
            }
        }
    }

    /**
     * Blocking poll loop that forwards Rust events to Compose state on the main dispatcher.
     */
    private suspend fun eventLoop(rt: MobileRuntime) {
        while (true) {
            try {
                val event = rt.pollEvent(5000u)
                if (event != null) {
                    viewModelScope.launch(Dispatchers.Main) {
                        handleEvent(event)
                    }
                }
            } catch (e: Exception) {
                viewModelScope.launch(Dispatchers.Main) {
                    errorText.value = "Event error: ${e.message}"
                }
            }
        }
    }

    /**
     * Map a Rust [UiEvent] to local UI state changes.
     */
    private fun handleEvent(event: UiEvent) {
        when (event) {
            is UiEvent.TurnBegin -> {
                // User message is already added by [sendMessage].
                statusText.value = ""
            }
            is UiEvent.ContentPart -> {
                val last = messages.lastOrNull()
                if (last != null && last.role == "assistant" && last.isStreaming) {
                    val updated = last.copy(content = last.content + event.text)
                    messages[messages.lastIndex] = updated
                } else {
                    messages.add(
                        ChatMessage(
                            id = "msg_${System.currentTimeMillis()}",
                            role = "assistant",
                            content = event.text,
                            isStreaming = true,
                        )
                    )
                }
            }
            is UiEvent.TurnEnd -> {
                val last = messages.lastOrNull()
                if (last != null && last.role == "assistant") {
                    messages[messages.lastIndex] = last.copy(isStreaming = false)
                }
                isLoading.value = false
                statusText.value = ""
            }
            is UiEvent.StatusUpdate -> {
                statusText.value = event.message
            }
            is UiEvent.Usage -> {
                statusText.value = "Tokens: ${event.promptTokens} → ${event.completionTokens}"
            }
            is UiEvent.Error -> {
                errorText.value = "${event.code}: ${event.message}"
                isLoading.value = false
            }
            is UiEvent.ToolCall -> {
                statusText.value = "Tool: ${event.name}"
            }
            is UiEvent.ToolResult -> {
                statusText.value = "Tool result received"
            }
            is UiEvent.ThreadActive -> {
                // Thread switch acknowledged.
            }
        }
    }
}
