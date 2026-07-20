package com.juice094.clarity.mobile

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.juice094.clarity.mobile.model.ChatItem
import com.juice094.clarity.mobile.model.Screen
import com.juice094.clarity.mobile.viewmodel.ChatViewModel

import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * End-to-end smoke test for the critical user flows.
 *
 * Requires a running Clarity Gateway at ws://10.0.2.2:18790/ws for the Claw
 * portion, and valid DeepSeek credentials for the device-login portion.
 */
@RunWith(AndroidJUnit4::class)
class EndToEndFlowTest {

    @get:Rule
    val composeTestRule = createAndroidComposeRule<MainActivity>()

    @OptIn(ExperimentalTestApi::class)
    @Before
    fun resetState() {
        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        composeTestRule.runOnUiThread {
            viewModel.resetRuntime()
        }
        composeTestRule.waitForIdle()
        // Wait for the thread list to be visible, not just any screen with "Clarity".
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("New chat"),
            timeoutMillis = 5000
        )
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun deepseekDeviceLoginFlow() {
        // 1. Open provider setup from thread list.
        openProviderSetup()

        // 2. Switch provider to DeepSeek device-login.
        composeTestRule.onNodeWithText("DEEPSEEK").performClick()
        composeTestRule.onNodeWithText("DEEPSEEK_DEVICE").performClick()

        // 3. Enter credentials.
        composeTestRule.onNodeWithText("Mobile number").performTextInput("13626566112")
        composeTestRule.onNodeWithText("Password").performTextInput("zjx040507")

        // 4. Connect.
        composeTestRule.onNodeWithText("Connect Local Agent").performClick()

        // 5. Should land on thread list after successful init.
        composeTestRule.waitUntilAtLeastOneExists(hasText("Clarity"), timeoutMillis = 30000)
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawGatewayConnectionFlow() {
        // 1. Open provider setup.
        openProviderSetup()

        // 2. Connect to Claw Gateway.
        composeTestRule.onNodeWithText("Connect via Claw").performClick()

        // 3. Should land on chat screen (message input becomes visible).
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawMessageExchangeFlow() {
        // 1. Connect to Claw Gateway.
        connectViaClaw()

        // 2. Send a message and wait for any non-empty assistant response.
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("Hi")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 20000)
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawShortStressFlow() {
        // 1. Connect to Claw Gateway.
        openProviderSetup()
        composeTestRule.onNodeWithText("Connect via Claw").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )

        // 2. Send a short burst of messages and ensure the app stays alive.
        val prompts = listOf("Hi", "What is 2+2?", "Tell me a joke")
        for (prompt in prompts) {
            composeTestRule.onNodeWithText("Message Clarity...").performTextInput(prompt)
            composeTestRule.onNodeWithContentDescription("Send").performClick()
            // Wait for the turn to finish (Send button reappears and input is re-enabled).
            composeTestRule.waitUntilAtLeastOneExists(
                hasContentDescription("Send"),
                timeoutMillis = 25000
            )
        }
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawFirstTokenLatencyFlow() {
        // 1. Connect to Claw Gateway.
        connectViaClaw()

        // 2. Send a message and wait for the first assistant response.
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("Hi")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 20000)

        // 3. Read the measured first-token latency from the ViewModel.
        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        val latency = viewModel.firstTokenLatencyMs.value
        assert(latency in 0..3000) {
            "Expected first token latency <= 3000 ms, got $latency ms"
        }
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawFiveMinuteStressFlow() {
        // 1. Connect to Claw Gateway.
        openProviderSetup()
        composeTestRule.onNodeWithText("Connect via Claw").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )

        // 2. Loop for ~5 minutes, sending varied short prompts.
        //    Wait for each turn to finish (Send button reappears) and sleep a
        //    few seconds between turns so we do not flood the Gateway's single-
        //    turn Agent semaphore or trigger provider rate limits.
        val prompts = listOf(
            "Hi",
            "What is 2+2?",
            "Tell me a joke",
            "Summarize Android",
            "What day is it?",
            "Explain Kotlin coroutines in one sentence",
        )
        val start = System.currentTimeMillis()
        val fiveMinutes = 5 * 60 * 1000L
        var turnCount = 0
        while (System.currentTimeMillis() - start < fiveMinutes) {
            val prompt = prompts[turnCount % prompts.size]
            composeTestRule.onNodeWithText("Message Clarity...").performTextInput(prompt)
            composeTestRule.onNodeWithContentDescription("Send").performClick()
            // Wait for the turn to finish (Send reappears after Stop).
            composeTestRule.waitUntilAtLeastOneExists(
                hasContentDescription("Send"),
                timeoutMillis = 30000
            )
            // Brief pause between turns to keep the load reasonable.
            Thread.sleep(3000)
            turnCount++
        }

        // 3. Assert we completed at least a handful of turns and the app never crashed.
        assert(turnCount >= 5) { "Expected at least 5 turns in 5 minutes, got $turnCount" }
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun deepseekLocalChatFlow() {
        // 1. Open provider setup.
        openProviderSetup()

        // 2. Use DeepSeek device-login locally (no Gateway).
        composeTestRule.onNodeWithText("DEEPSEEK").performClick()
        composeTestRule.onNodeWithText("DEEPSEEK_DEVICE").performClick()
        composeTestRule.onNodeWithText("Mobile number").performTextInput("13626566112")
        composeTestRule.onNodeWithText("Password").performTextInput("zjx040507")
        composeTestRule.onNodeWithText("Connect Local Agent").performClick()

        // 3. Should reach the thread list; open a new chat.
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("New chat"),
            timeoutMillis = 30000
        )
        composeTestRule.onNodeWithContentDescription("New chat").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )

        // 4. Send a multi-turn exchange and assert on the ViewModel state so we
        //    are independent of whether the model answers in English or Chinese.
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("Hi")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = 0)

        val countAfterFirst = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("What is 2+2?")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = countAfterFirst)

        val lastResponse = assistantMessages().last().lowercase()
        assert(lastResponse.contains("4") || lastResponse.contains("四")) {
            "Expected response to contain '4' or '四', got: $lastResponse"
        }
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun searchAndThinkingToggleFlow() {
        // 1. Connect local DeepSeek device agent.
        openProviderSetup()
        composeTestRule.onNodeWithText("DEEPSEEK").performClick()
        composeTestRule.onNodeWithText("DEEPSEEK_DEVICE").performClick()
        composeTestRule.onNodeWithText("Mobile number").performTextInput("13626566112")
        composeTestRule.onNodeWithText("Password").performTextInput("zjx040507")
        composeTestRule.onNodeWithText("Connect Local Agent").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("New chat"),
            timeoutMillis = 30000
        )
        composeTestRule.onNodeWithContentDescription("New chat").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )

        // 2. Toggle Search and Thinking chips.
        composeTestRule.onNodeWithText("Search").performClick()
        composeTestRule.onNodeWithText("Thinking").performClick()

        // 3. Send a message that benefits from both toggles.
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("Explain quantum computing")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 30000)
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun markdownCodeBlockRenderingFlow() {
        // 1. Connect local DeepSeek device agent.
        openProviderSetup()
        composeTestRule.onNodeWithText("DEEPSEEK").performClick()
        composeTestRule.onNodeWithText("DEEPSEEK_DEVICE").performClick()
        composeTestRule.onNodeWithText("Mobile number").performTextInput("13626566112")
        composeTestRule.onNodeWithText("Password").performTextInput("zjx040507")
        composeTestRule.onNodeWithText("Connect Local Agent").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("New chat"),
            timeoutMillis = 30000
        )
        composeTestRule.onNodeWithContentDescription("New chat").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )

        // 2. Ask for a code snippet.
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("Write a Python hello world function")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 30000)

        // 3. Assert the response contains a code-like snippet.
        val response = assistantMessages().last().lowercase()
        assert(response.contains("def") || response.contains("print(") || response.contains("```")) {
            "Expected a Python snippet in the response, got: $response"
        }
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun messageLongPressActionsFlow() {
        // 1. Connect to Claw Gateway and produce an assistant message.
        connectViaClaw()

        val prompt = "Long press test"
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput(prompt)
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 20000)

        // 2. Long-press the user message to reveal the dropdown menu.
        composeTestRule.onNodeWithText(prompt).performTouchInput {
            down(center)
            advanceEventTime(700)
            up()
        }
        composeTestRule.waitUntilAtLeastOneExists(hasText("Copy"), timeoutMillis = 5000)

        // 3. Dismiss the menu (tap Copy to exercise the action).
        composeTestRule.onNodeWithText("Copy").performClick()
    }


    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawSessionPersistenceFlow() {
        // 1. Connect to Claw Gateway and send a distinctive message.
        connectViaClaw()

        val marker = "Persist_${System.currentTimeMillis() % 10000}"
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput(marker)
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 20000)
        // Ensure the turn fully finished and the session was persisted.
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("Send"),
            timeoutMillis = 10000
        )

        // 2. Verify the session was persisted locally before navigating back.
        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        composeTestRule.waitUntil(
            timeoutMillis = 5000,
            condition = {
                com.juice094.clarity.mobile.data.ClawSessionStore.listSessions(
                    composeTestRule.activity
                ).isNotEmpty()
            }
        )
        val session = com.juice094.clarity.mobile.data.ClawSessionStore.listSessions(
            composeTestRule.activity
        ).first()

        // 3. Go back to the thread list and switch to the persisted session.
        composeTestRule.onNodeWithContentDescription("Back").performClick()
        composeTestRule.waitUntilAtLeastOneExists(hasText("Clarity"), timeoutMillis = 5000)
        viewModel.switchToThread(session.id)
        composeTestRule.waitUntilAtLeastOneExists(hasText(marker), timeoutMillis = 10000)
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawToolCardRenderingFlow() {
        // Connect to Claw Gateway so the UI is in Claw mode, then directly
        // inject ToolCall / ToolResult events into the ViewModel.  This
        // exercises the Claw-specific UI affordances without depending on the
        // LLM provider choosing to emit a tool call.
        connectViaClaw()

        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        composeTestRule.runOnUiThread {
            viewModel.messages.add(
                com.juice094.clarity.mobile.model.ChatItem.ToolCallCard(
                    id = "tc_test",
                    turnId = "turn_test",
                    callId = "call_test",
                    toolName = "todo",
                    argumentsJson = "{\"task\":\"buy milk\"}"
                )
            )
            viewModel.messages.add(
                com.juice094.clarity.mobile.model.ChatItem.ToolResultCard(
                    id = "tr_test",
                    turnId = "turn_test",
                    callId = "call_test",
                    toolName = "todo",
                    resultJson = "{\"ok\":true}"
                )
            )
        }

        composeTestRule.waitUntilAtLeastOneExists(hasText("Tool: todo"), timeoutMillis = 5000)
        composeTestRule.waitUntilAtLeastOneExists(hasText("Result: todo"), timeoutMillis = 5000)
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawApprovalDialogFlow() {
        // Connect to Claw Gateway, then inject a pending approval request and
        // resolve it.  This verifies the Claw approval UI path end-to-end from
        // the ViewModel to the dialog and back.
        connectViaClaw()

        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        composeTestRule.runOnUiThread {
            viewModel.pendingApproval.value = com.juice094.clarity.mobile.model.PendingApproval(
                requestId = "req_test",
                turnId = "turn_test",
                toolName = "file_write",
                argumentsJson = "{\"path\":\"claw_test.txt\"}",
                description = "The agent wants to write a test file."
            )
        }

        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Approve tool execution?"),
            timeoutMillis = 5000
        )
        composeTestRule.onNodeWithText("Allow").performClick()
        composeTestRule.waitUntil(
            timeoutMillis = 5000,
            condition = { viewModel.pendingApproval.value == null }
        )
    }

    @OptIn(ExperimentalTestApi::class)
    @Test
    fun clawReconnectSurvivesActivityRestartFlow() {
        // 1. Connect to Claw Gateway and establish a session.
        connectViaClaw()

        val marker = "Reconnect_${System.currentTimeMillis() % 10000}"
        val before = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput(marker)
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = before, timeoutMillis = 20000)
        // Wait for the turn to fully finish (Send button returns).
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("Send"),
            timeoutMillis = 10000
        )

        // 2. Simulate a configuration change / activity restart.  The ViewModel
        //    survives, but the UI must re-bind and the persisted Claw session
        //    must still be present.
        composeTestRule.activityRule.scenario.recreate()

        // 3. After recreation we land back on the chat screen and the marker
        //    message is still visible.
        composeTestRule.waitUntilAtLeastOneExists(hasText(marker), timeoutMillis = 10000)
        composeTestRule.waitUntilAtLeastOneExists(
            hasContentDescription("Send"),
            timeoutMillis = 10000
        )

        // 4. A follow-up message still works, proving the Gateway connection
        //    recovered without losing the conversation context.
        val countBeforeFollowUp = assistantMessages().size
        composeTestRule.onNodeWithText("Message Clarity...").performTextInput("Repeat my last message")
        composeTestRule.onNodeWithContentDescription("Send").performClick()
        waitForAssistantResponse(previousCount = countBeforeFollowUp, timeoutMillis = 20000)
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /**
     * Open the provider setup screen directly, bypassing the thread-list FAB.
     *
     * This keeps tests independent of whatever runtime state a previous test may
     * have left behind.
     */
    @OptIn(ExperimentalTestApi::class)
    private fun openProviderSetup() {
        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        composeTestRule.runOnUiThread {
            viewModel.currentScreen.value = Screen.ProviderSetup
        }
        composeTestRule.waitForIdle()
        composeTestRule.waitUntilAtLeastOneExists(hasText("DEEPSEEK"), timeoutMillis = 15000)
    }

    @OptIn(ExperimentalTestApi::class)
    private fun connectViaClaw() {
        openProviderSetup()
        composeTestRule.onNodeWithText("Connect via Claw").performClick()
        composeTestRule.waitUntilAtLeastOneExists(
            hasText("Message Clarity..."),
            timeoutMillis = 15000
        )
    }

    /**
     * Return the text content of every assistant message currently in the UI.
     *
     * Reads the observable list on the UI thread and returns a plain snapshot so
     * we do not observe Compose state from the instrumentation thread.
     */
    private fun assistantMessages(): List<String> {
        val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
        val snapshot = mutableListOf<ChatItem>()
        composeTestRule.runOnUiThread {
            snapshot.addAll(viewModel.messages)
        }
        return snapshot
            .filterIsInstance<ChatItem.AssistantText>()
            .map { it.content }
    }

    /**
     * Wait until a new assistant response (relative to [previousCount]) has
     * fully arrived (i.e. it is no longer streaming).
     *
     * This is more robust than matching expected English text because the model
     * may answer in Chinese depending on the provider and locale, and it also
     * prevents asserting on a stale response from an earlier turn.
     */
    private fun waitForAssistantResponse(
        previousCount: Int = 0,
        timeoutMillis: Long = 30000,
    ) {
        composeTestRule.waitUntil(timeoutMillis = timeoutMillis) {
            val viewModel = ViewModelProvider(composeTestRule.activity)[ChatViewModel::class.java]
            var finishedCount = 0
            composeTestRule.runOnUiThread {
                finishedCount = viewModel.messages
                    .filterIsInstance<ChatItem.AssistantText>()
                    .count { !it.isStreaming }
            }
            finishedCount > previousCount
        }
    }
}
