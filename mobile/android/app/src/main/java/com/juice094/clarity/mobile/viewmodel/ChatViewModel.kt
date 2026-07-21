package com.juice094.clarity.mobile.viewmodel

import android.app.Application
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.juice094.clarity.mobile.data.ClawSessionStore
import com.juice094.clarity.mobile.data.PreferencesStore
import com.juice094.clarity.mobile.model.ChatItem
import com.juice094.clarity.mobile.model.ConnectionStatus
import com.juice094.clarity.mobile.model.PendingApproval
import com.juice094.clarity.mobile.model.Screen
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelChildren
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID
import uniffi.clarity_mobile_core.MobileConfig
import uniffi.clarity_mobile_core.MobileRuntime
import uniffi.clarity_mobile_core.ProviderProfile
import uniffi.clarity_mobile_core.ProviderType
import uniffi.clarity_mobile_core.ThreadSummary

/**
 * ViewModel that owns the [MobileRuntime] and bridges Rust events to Compose state.
 *
 * This class is intentionally kept as a thin bridge: it holds the runtime, runs the
 * event loop, and exposes observable state. UI rendering logic lives in the Compose
 * screens/components.
 */
class ChatViewModel(application: Application) : AndroidViewModel(application) {

    var runtime: MobileRuntime? = null
        private set

    // Navigation / UI state
    val currentScreen = mutableStateOf(Screen.ThreadList)
    val isClawMode = mutableStateOf(false)

    // Active Claw session id, used to persist/restore remote Gateway chats.
    val clawSessionId = mutableStateOf("")

    // Thread list state
    val threads = mutableStateListOf<ThreadSummary>()

    // Chat state
    val messages = mutableStateListOf<ChatItem>()
    val isLoading = mutableStateOf(false)
    val statusText = mutableStateOf("")
    val errorText = mutableStateOf("")
    val pendingApproval = mutableStateOf<PendingApproval?>(null)
    val isAgentMode = mutableStateOf(true)

    // Provider configuration edited by the user.
    // Default to DeepSeek device-login because it is the primary way users
    // authenticate without an API key and matches the DeepSeek App flow.
    val providerType = mutableStateOf(ProviderType.DEEPSEEK_DEVICE)
    val apiKey = mutableStateOf("")
    val model = mutableStateOf("deepseek-chat")
    val mobile = mutableStateOf("")
    val password = mutableStateOf("")

    // DeepSeek-style per-turn feature toggles.
    val isSearchEnabled = mutableStateOf(false)
    val isThinkingEnabled = mutableStateOf(false)

    // Last-used mode, persisted to skip the setup screen on cold start.
    val launchMode = mutableStateOf(PreferencesStore.LaunchMode.Unset)

    // Connection state for Gateway (Claw) mode.
    val connectionStatus = mutableStateOf<ConnectionStatus>(ConnectionStatus.Connected)

    // Claw Gateway configuration.
    // Default to the OpenClaw scheme so the mobile client talks to remote
    // Kimi/Gray OpenClaw Gateways out of the box. The Rust layer strips the
    // scheme and opens a plain WebSocket underneath.
    val gatewayUrl = mutableStateOf("openclaw://10.0.2.2:18790/ws")
    val gatewayToken = mutableStateOf("")

    // Device pairing state for OpenClaw gateways that enforce device scopes.
    val isPairing = mutableStateOf(false)
    val pairingStatus = mutableStateOf("")

    // Cached DeepSeek device-login token. When present it is used instead of
    // mobile+password to avoid the PoW login flow on every cold start.
    val deepseekDeviceToken = mutableStateOf("")

    private var eventLoopJob: Job? = null

    // Watchdog for remote (Claw) turns so the UI does not spin forever if the
    // Gateway stops producing events.
    internal var turnTimeoutJob: Job? = null

    // Set when the user stops a turn; late streaming events for the current turn
    // are ignored until a terminal event arrives.
    internal var stopRequested = false

    // Id of the assistant message currently receiving streaming content.
    // Explicit tracking is more robust than checking the last item's flag when
    // TurnEnd/Error events may interleave with late ContentParts.
    // Internal so the EventHandler extension can read/write it.
    internal var streamingAssistantId: String? = null

    // Latency tracking: timestamp when the latest user message was sent.
    // Used to compute first-token latency on the first ContentPart/ReasoningPart.
    internal var sendTimestampMs: Long = 0L

    // Observed first-token latency for the most recent turn (ms). -1 means not yet measured.
    val firstTokenLatencyMs = mutableStateOf(-1L)

    init {
        PreferencesStore.load(getApplication<Application>(), this)
        tryAutoLogin()
    }

    /**
     * On cold start, if we already have saved credentials for a previous mode,
     * initialize the runtime and jump straight into chat. This mirrors the
     * DeepSeek App behaviour where authenticated users land directly on a new
     * conversation.
     */
    private fun tryAutoLogin() {
        Log.d(
            "ClarityMobile",
            "tryAutoLogin launchMode=${launchMode.value} mobileBlank=${mobile.value.isBlank()} passwordBlank=${password.value.isBlank()} tokenBlank=${deepseekDeviceToken.value.isBlank()}"
        )
        when (launchMode.value) {
            PreferencesStore.LaunchMode.LocalChat -> {
                val isDeviceLogin = providerType.value == ProviderType.DEEPSEEK_DEVICE
                val hasDeviceCreds = isDeviceLogin &&
                    (deepseekDeviceToken.value.isNotBlank() ||
                        (mobile.value.isNotBlank() && password.value.isNotBlank()))
                val hasApiKey = !isDeviceLogin && apiKey.value.isNotBlank()
                if (hasDeviceCreds || hasApiKey) {
                    Log.d("ClarityMobile", "auto-login local mode")
                    initialize()
                }
            }
            PreferencesStore.LaunchMode.Claw -> {
                if (gatewayUrl.value.isNotBlank()) {
                    Log.d("ClarityMobile", "auto-login claw mode")
                    initializeClaw()
                }
            }
            PreferencesStore.LaunchMode.Unset -> {
                // First launch or after sign-out: show the setup screen so the
                // user can authenticate before any chat starts.
                currentScreen.value = Screen.ProviderSetup
            }
        }
    }

    /**
     * Generate a unique message id for LazyColumn keys.
     *
     * Uses UUID so that ids never collide with messages loaded from persistent
     * history (whose ids were also generated by this function in earlier runs).
     */
    fun generateMessageId(): String = UUID.randomUUID().toString()

    /**
     * Clear the active runtime and related in-memory UI state. Used by
     * instrumentation tests to ensure each test starts from a clean slate
     * without recreating the Activity (and therefore the ViewModel), and by
     * [signOutToProviderSetup] to drop the current connection.
     *
     * Note this only clears in-memory state; on-disk chat history (Rust
     * session store and [ClawSessionStore] files) is intentionally kept.
     */
    fun resetRuntime() {
        eventLoopJob?.cancel()
        // Cancel any in-flight initialization from a previous test so it cannot
        // overwrite the cleared runtime / screen after we have reset state.
        viewModelScope.coroutineContext.cancelChildren()
        runtime = null
        messages.clear()
        threads.clear()
        isClawMode.value = false
        clawSessionId.value = ""
        isLoading.value = false
        errorText.value = ""
        statusText.value = ""
        pendingApproval.value = null
        currentScreen.value = Screen.ThreadList

        // Reset provider form fields so instrumentation tests always start from
        // the same defaults regardless of what a previous test selected.
        providerType.value = ProviderType.DEEPSEEK_DEVICE
        model.value = "deepseek-chat"
        apiKey.value = ""
        mobile.value = ""
        password.value = ""
        deepseekDeviceToken.value = ""
        isAgentMode.value = true
        isSearchEnabled.value = false
        isThinkingEnabled.value = false
        firstTokenLatencyMs.value = -1L
        streamingAssistantId = null
        stopRequested = false
        sendTimestampMs = 0L
    }

    /**
     * Sign out of the current provider and return to the setup screen so the
     * user can pick a different provider or gateway.
     *
     * Clears the runtime, saved credentials and launch mode, but keeps local
     * chat history on disk (Rust session store and Claw session files) so old
     * conversations are still listed after re-connecting.
     */
    fun signOutToProviderSetup() {
        resetRuntime()
        val app = getApplication<Application>()
        PreferencesStore.clear(app)
        // Persist the cleared state so the next cold start also lands on the
        // setup screen instead of auto-logging in with stale defaults.
        launchMode.value = PreferencesStore.LaunchMode.Unset
        PreferencesStore.save(app, this)
        currentScreen.value = Screen.ProviderSetup
    }

    /**
     * Initialize the Rust runtime in local agent mode with the user-supplied provider config.
     */
    fun initialize() {
        val isDeviceLogin = providerType.value == ProviderType.DEEPSEEK_DEVICE
        val useDevicePassword = isDeviceLogin && mobile.value.isNotBlank() && password.value.isNotBlank()
        val hasDeviceToken = isDeviceLogin && deepseekDeviceToken.value.isNotBlank()
        Log.d(
            "ClarityMobile",
            "initialize check usePassword=$useDevicePassword hasToken=$hasDeviceToken mobileBlank=${mobile.value.isBlank()} passwordBlank=${password.value.isBlank()}"
        )
        if (isDeviceLogin) {
            if (!useDevicePassword && !hasDeviceToken) {
                errorText.value = "Mobile number and password are required for DeepSeek device login"
                return
            }
        } else if (apiKey.value.isBlank()) {
            errorText.value = "API key is required"
            return
        }
        errorText.value = ""
        // Capture credential values on the main thread before moving to the IO
        // coroutine. SnapshotState reads from a background thread can return stale
        // values if the state was just mutated on the main thread, which caused
        // cold-start auto-login to pass empty credentials to Rust even though the
        // UI state appeared correct.
        val currentMobile = mobile.value
        val currentPassword = password.value
        val currentApiKey = apiKey.value
        val currentDeviceToken = deepseekDeviceToken.value
        val currentProvider = providerType.value
        val currentModel = effectiveModelName()
        val currentSearch = isSearchEnabled.value
        val currentReasoning = isThinkingEnabled.value
        PreferencesStore.save(getApplication<Application>(), this)

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val dataDir = getApplication<Application>().filesDir.absolutePath
                // Prefer explicitly-entered mobile+password over a cached device token:
                // this keeps stale tokens from shadowing fresh credentials and lets the
                // user re-authenticate without clearing app storage.
                val effectiveApiKey = when {
                    useDevicePassword -> ""
                    hasDeviceToken -> currentDeviceToken
                    else -> currentApiKey
                }
                val profile = ProviderProfile(
                    provider = currentProvider,
                    model = currentModel,
                    apiKey = effectiveApiKey,
                    baseUrl = null,
                    mobile = if (useDevicePassword) currentMobile.takeIf { it.isNotBlank() } else null,
                    password = if (useDevicePassword) currentPassword.takeIf { it.isNotBlank() } else null,
                    searchEnabled = currentSearch,
                    reasoningEnabled = currentReasoning,
                )
                val config = MobileConfig(
                    dataDir = dataDir,
                    defaultProvider = profile,
                    gatewayUrl = null,
                    gatewayToken = null,
                )
                val rt = MobileRuntime(config)
                setRuntime(rt, isClaw = false)
                Log.d(
                    "ClarityMobile",
                    "initialize local runtime ok provider=${providerType.value} " +
                        "usePassword=$useDevicePassword hasToken=$hasDeviceToken"
                )
                withContext(Dispatchers.Main) {
                    // DeepSeek device-login endpoint does not support tool calling;
                    // fall back to direct chat mode for this provider.
                    if (providerType.value == ProviderType.DEEPSEEK_DEVICE) {
                        isAgentMode.value = false
                        syncAgentMode()
                    }
                    launchMode.value = PreferencesStore.LaunchMode.LocalChat
                    PreferencesStore.save(getApplication<Application>(), this@ChatViewModel)
                    // Skip the thread list and land directly on a new chat, matching
                    // the DeepSeek App cold-start behaviour.
                    createNewChat()
                }
            } catch (e: Exception) {
                Log.e("ClarityMobile", "initialize failed", e)
                withContext(Dispatchers.Main) {
                    errorText.value = "Init failed: ${e.message}"
                }
            }
        }
    }

    /**
     * Initialize the Rust runtime in Gateway remote (Claw) mode.
     *
     * Reuses the most recent persisted Claw session if one exists, otherwise
     * creates a new session file. Pass [resumeSessionId] to force resuming a
     * specific session (used when the user selects a Claw thread from the list).
     */
    fun initializeClaw(resumeSessionId: String? = null) {
        if (gatewayUrl.value.isBlank()) {
            errorText.value = "Gateway URL is required"
            return
        }
        errorText.value = ""
        // Capture gateway configuration on the main thread to avoid stale
        // SnapshotState reads in the IO coroutine (same issue as local mode).
        val currentGatewayUrl = gatewayUrl.value
        val currentGatewayToken = gatewayToken.value
        PreferencesStore.save(getApplication<Application>(), this)

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val app = getApplication<Application>()
                val dataDir = app.filesDir.absolutePath
                // Dummy profile is ignored in remote mode but required by the config.
                val profile = ProviderProfile(
                    provider = ProviderType.DEEPSEEK,
                    model = "remote",
                    apiKey = "remote",
                    baseUrl = null,
                    mobile = null,
                    password = null,
                    searchEnabled = false,
                    reasoningEnabled = false,
                )
                val config = MobileConfig(
                    dataDir = dataDir,
                    defaultProvider = profile,
                    gatewayUrl = currentGatewayUrl,
                    gatewayToken = currentGatewayToken.takeIf { it.isNotBlank() },
                )
                val rt = MobileRuntime(config)
                setRuntime(rt, isClaw = true)

                // Restore the requested or most recent persisted Claw session.
                val session = resumeSessionId?.let { ClawSessionStore.loadSession(app, it) }
                    ?: ClawSessionStore.listSessions(app)
                        .firstOrNull()
                        ?.let { ClawSessionStore.loadSession(app, it.id) }
                val sessionId = session?.id ?: ClawSessionStore.createSession(app)
                withContext(Dispatchers.Main) {
                    clawSessionId.value = sessionId
                    messages.clear()
                    session?.messages?.let { messages.addAll(it) }
                    launchMode.value = PreferencesStore.LaunchMode.Claw
                    PreferencesStore.save(getApplication<Application>(), this@ChatViewModel)
                    currentScreen.value = Screen.Chat
                }
                rt.createThread("Claw")
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    errorText.value = "Claw init failed: ${e.message}"
                }
            }
        }
    }

    /**
     * Request device pairing with the current OpenClaw gateway.
     *
     * Blocks up to the gateway-side approval timeout (Rust uses 120s). On success
     * the device token is persisted by Rust; this method then reinitializes the
     * runtime so the next connection uses device-paired auth and gains the
     * [operator.write] scope required for chat.send.
     */
    fun requestPairing() {
        Log.i("ClarityMobile", "requestPairing clicked; runtime=${runtime != null}, isClawMode=${isClawMode.value}")
        val rt = runtime ?: run {
            errorText.value = "Connect to a Claw gateway first"
            Log.w("ClarityMobile", "requestPairing aborted: runtime is null")
            return
        }
        if (!isClawMode.value) {
            errorText.value = "Pairing only applies to Claw gateway mode"
            Log.w("ClarityMobile", "requestPairing aborted: not in Claw mode")
            return
        }
        errorText.value = ""
        isPairing.value = true
        pairingStatus.value = "Requesting pairing..."

        viewModelScope.launch(Dispatchers.IO) {
            try {
                val deviceToken = rt.requestPairing()
                withContext(Dispatchers.Main) {
                    isPairing.value = false
                    if (deviceToken != null) {
                        pairingStatus.value = "Paired. Reconnecting with device token..."
                        // Reinitialize so the new device token is used.
                        initializeClaw(resumeSessionId = clawSessionId.value.takeIf { it.isNotBlank() })
                    } else {
                        pairingStatus.value = "Pairing failed or timed out"
                        errorText.value = "Pairing failed or timed out"
                    }
                }
            } catch (e: Exception) {
                Log.e("ClarityMobile", "requestPairing failed", e)
                withContext(Dispatchers.Main) {
                    isPairing.value = false
                    pairingStatus.value = "Pairing error: ${e.message}"
                    errorText.value = "Pairing error: ${e.message}"
                }
            }
        }
    }

    /**
     * Replace the active runtime and restart the event loop.
     */
    private fun setRuntime(rt: MobileRuntime, isClaw: Boolean) {
        runtime = rt
        viewModelScope.launch(Dispatchers.Main) {
            isClawMode.value = isClaw
            messages.clear()
            statusText.value = ""
            errorText.value = ""
            pendingApproval.value = null
        }
        syncAgentMode()
        applyProviderOptions()

        eventLoopJob?.cancel()
        eventLoopJob = viewModelScope.launch(Dispatchers.IO) {
            eventLoop(rt)
        }
    }

    /**
     * Capture and persist the provider's cached auth token (e.g. DeepSeek
     * device-login token) after a successful turn.
     */
    fun captureAuthToken() {
        val rt = runtime ?: return
        val token = try {
            rt.lastAuthToken()
        } catch (_: Exception) {
            null
        } ?: return
        if (token.isNotBlank() && token != deepseekDeviceToken.value) {
            deepseekDeviceToken.value = token
            apiKey.value = token
            PreferencesStore.save(getApplication<Application>(), this)
        }
    }

    /**
     * Switch the active model for subsequent turns.
     *
     * Has no effect in Gateway remote mode.
     */
    fun setModel(newModel: String) {
        model.value = newModel
        PreferencesStore.save(getApplication<Application>(), this)
        applyProviderOptions()
    }

    /**
     * Toggle web-search hint for subsequent messages.
     */
    fun setSearchEnabled(enabled: Boolean) {
        isSearchEnabled.value = enabled
        PreferencesStore.save(getApplication<Application>(), this)
        applyProviderOptions()
    }

    /**
     * Toggle DeepSeek-style reasoning (R1) model.
     *
     * For the DeepSeek provider this switches between `deepseek-chat` and
     * `deepseek-reasoner`. Other providers keep the user-selected model.
     */
    fun setThinkingEnabled(enabled: Boolean) {
        isThinkingEnabled.value = enabled
        PreferencesStore.save(getApplication<Application>(), this)
        applyProviderOptions()
    }

    /**
     * Compute the model name that should be active given the current toggles.
     */
    fun effectiveModelName(): String = when {
        providerType.value == ProviderType.DEEPSEEK && isThinkingEnabled.value -> "deepseek-reasoner"
        else -> model.value
    }

    /**
     * Return a short list of models commonly used with the selected provider.
     */
    fun availableModels(): List<String> = when (providerType.value) {
        ProviderType.DEEPSEEK -> listOf("deepseek-chat", "deepseek-reasoner", "deepseek-coder")
        ProviderType.OPEN_AI -> listOf("gpt-4o", "gpt-4o-mini", "o3-mini")
        ProviderType.KIMI -> listOf("kimi-k2.6", "kimi-k1.5", "moonshot-v1-8k")
        ProviderType.ANTHROPIC -> listOf("claude-3-5-sonnet-20241022", "claude-3-opus-20240229")
        ProviderType.DEEPSEEK_DEVICE -> listOf("deepseek-chat")
    }

    /**
     * Propagate the current provider profile (model + toggles) to the Rust runtime.
     */
    private fun applyProviderOptions() {
        val rt = runtime ?: return
        if (isClawMode.value) return
        val effectiveModel = effectiveModelName()
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val profile = ProviderProfile(
                    provider = providerType.value,
                    model = effectiveModel,
                    apiKey = apiKey.value,
                    baseUrl = null,
                    mobile = mobile.value.takeIf { it.isNotBlank() },
                    password = password.value.takeIf { it.isNotBlank() },
                    searchEnabled = isSearchEnabled.value,
                    reasoningEnabled = isThinkingEnabled.value,
                )
                Log.d(
                    "ClarityMobile",
                    "applyProviderOptions provider=${profile.provider} model=${profile.model} " +
                        "hasMobile=${profile.mobile != null} hasPassword=${profile.password != null}"
                )
                rt.setProvider(profile)
            } catch (e: Exception) {
                Log.e("ClarityMobile", "applyProviderOptions failed", e)
                withContext(Dispatchers.Main) {
                    errorText.value = "Model switch failed: ${e.message}"
                }
            }
        }
    }

    /**
     * Load the thread list from the local runtime and merge persisted Claw sessions.
     */
    fun loadThreads() {
        val rt = runtime
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val localThreads = if (rt != null && !isClawMode.value) {
                    rt.listThreads()
                } else {
                    emptyList()
                }
                val clawSessions = ClawSessionStore.listSessions(getApplication())
                    .map { summary ->
                        ThreadSummary(
                            threadId = summary.id,
                            title = "[Claw] ${summary.title}",
                            updatedAt = summary.formattedTime()
                        )
                    }
                viewModelScope.launch(Dispatchers.Main) {
                    threads.clear()
                    threads.addAll(clawSessions)
                    threads.addAll(localThreads)
                }
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    errorText.value = "Load threads failed: ${e.message}"
                }
            }
        }
    }

    /**
     * Switch to an existing thread and open the chat screen.
     * Loads persisted messages from the Rust session store or from the local
     * Claw session store depending on the thread id.
     */
    fun switchToThread(threadId: String) {
        messages.clear()
        if (threadId.startsWith("claw-")) {
            // Switching to a persisted Claw session requires reconnecting to the
            // Gateway; we load the cached messages immediately and mark Claw mode.
            clawSessionId.value = threadId
            val session = ClawSessionStore.loadSession(getApplication(), threadId)
            session?.messages?.let { messages.addAll(it) }
            currentScreen.value = Screen.Chat
            isClawMode.value = true
            // If no Claw runtime is active, initialize it silently using the saved config.
            if (runtime == null) {
                initializeClaw(resumeSessionId = threadId)
            }
            return
        }

        isClawMode.value = false
        clawSessionId.value = ""
        val rt = runtime
        if (rt == null) {
            currentScreen.value = Screen.ProviderSetup
            return
        }
        rt.switchThread(threadId)
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val history = rt.getMessages(threadId).map { msg ->
                    val ts = formatIsoTimestamp(msg.timestamp)
                    when (msg.role) {
                        "user" -> ChatItem.UserText(
                            id = generateMessageId(),
                            content = msg.content,
                            timestamp = ts,
                        )
                        else -> ChatItem.AssistantText(
                            id = generateMessageId(),
                            content = msg.content,
                            timestamp = ts,
                        )
                    }
                }
                viewModelScope.launch(Dispatchers.Main) {
                    messages.addAll(history)
                }
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    errorText.value = "Load history failed: ${e.message}"
                }
            }
        }
        currentScreen.value = Screen.Chat
    }

    /**
     * Format an ISO-8601 timestamp into the same HH:mm display used for live messages.
     */
    private fun formatIsoTimestamp(iso: String): String = try {
        val parser = java.time.format.DateTimeFormatter.ISO_OFFSET_DATE_TIME
        val formatter = java.time.format.DateTimeFormatter.ofPattern("HH:mm")
        java.time.OffsetDateTime.parse(iso, parser).format(formatter)
    } catch (_: Exception) {
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date())
    }

    /**
     * Create a new normal chat thread and open it.
     */
    fun createNewChat() {
        val rt = runtime
        if (rt == null) {
            currentScreen.value = Screen.ProviderSetup
            return
        }
        isClawMode.value = false
        clawSessionId.value = ""
        messages.clear()
        rt.createThread("New chat")
        currentScreen.value = Screen.Chat
    }

    /**
     * Navigate back to the thread list and refresh it.
     */
    fun backToThreadList() {
        currentScreen.value = Screen.ThreadList
        loadThreads()
    }

    /**
     * Send a user message and append it to the local UI immediately.
     */
    fun sendMessage(text: String) {
        val rt = runtime ?: return
        if (text.isBlank()) return

        Log.d("ClarityMobile", "sendMessage text='$text' isClaw=${isClawMode.value} runtime=$rt")
        streamingAssistantId = null
        firstTokenLatencyMs.value = -1L
        messages.add(
            ChatItem.UserText(
                id = generateMessageId(),
                content = text,
                timestamp = SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date()),
            )
        )
        isLoading.value = true
        statusText.value = "Thinking..."
        sendTimestampMs = System.currentTimeMillis()

        // Persist the Claw session immediately so the user's message survives
        // unexpected Gateway disconnects or slow first-token latency.
        if (isClawMode.value && clawSessionId.value.isNotBlank()) {
            ClawSessionStore.saveSession(
                getApplication(),
                clawSessionId.value,
                messages.toList()
            )
        }

        // Cap remote turns at 90s to match the Rust local-agent timeout.
        turnTimeoutJob?.cancel()
        turnTimeoutJob = viewModelScope.launch(Dispatchers.IO) {
            delay(90_000)
            viewModelScope.launch(Dispatchers.Main) {
                if (isLoading.value) {
                    isLoading.value = false
                    statusText.value = ""
                    errorText.value = "Remote turn timed out"
                }
            }
        }

        val messageToSend = if (isSearchEnabled.value) {
            "Please search the web if it helps answer this: $text"
        } else {
            text
        }

        viewModelScope.launch(Dispatchers.IO) {
            try {
                rt.sendMessage(messageToSend)
            } catch (e: Exception) {
                Log.e("ClarityMobile", "sendMessage failed", e)
                withContext(Dispatchers.Main) {
                    errorText.value = "Send failed: ${e.message}"
                    isLoading.value = false
                }
                turnTimeoutJob?.cancel()
            }
        }
    }

    /**
     * Stop an in-flight turn.
     */
    fun stopTurn() {
        val rt = runtime ?: return
        stopRequested = true
        isLoading.value = false
        statusText.value = "Stopped"
        viewModelScope.launch(Dispatchers.IO) {
            try {
                rt.stopTurn()
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    errorText.value = "Stop failed: ${e.message}"
                }
            }
        }
    }

    /**
     * Copy the given text to the system clipboard.
     */
    fun copyToClipboard(text: String) {
        val clipboard = getApplication<Application>()
            .getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText("Clarity", text))
    }

    /**
     * Delete a message from the current chat UI.
     */
    fun deleteMessage(id: String) {
        messages.removeAll { it.id == id }
    }

    /**
     * Regenerate the assistant response for the message at [id].
     *
     * Finds the preceding user message, removes the old assistant bubble, and
     * re-sends the user prompt. This is intentionally simple: it does not yet
     * rewrite history in the Rust session store.
     */
    fun regenerateMessage(id: String) {
        val index = messages.indexOfLast { it.id == id }
        if (index <= 0) return
        val userIndex = (index - 1 downTo 0).firstOrNull { messages[it] is ChatItem.UserText }
            ?: return
        val userItem = messages[userIndex] as ChatItem.UserText
        messages.removeAt(index)
        sendMessage(userItem.content)
    }

    /**
     * Resolve the pending approval request.
     */
    fun resolveApproval(allow: Boolean, remember: Boolean = false) {
        val approval = pendingApproval.value ?: return
        val rt = runtime ?: return
        pendingApproval.value = null
        viewModelScope.launch(Dispatchers.IO) {
            try {
                rt.approve(approval.requestId, allow, remember)
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    errorText.value = "Approval failed: ${e.message}"
                }
            }
        }
    }

    /**
     * Propagate the Agent/Chat mode toggle to the Rust runtime.
     */
    fun syncAgentMode() {
        val rt = runtime ?: return
        if (!isClawMode.value) {
            rt.setAgentMode(isAgentMode.value)
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
}
