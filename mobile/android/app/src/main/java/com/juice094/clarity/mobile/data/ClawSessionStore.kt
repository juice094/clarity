package com.juice094.clarity.mobile.data

import android.content.Context
import com.juice094.clarity.mobile.model.ChatItem
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID

/**
 * Local persistence for Claw (Gateway remote) chat sessions.
 *
 * The Gateway remote mode is stateless from the mobile runtime's point of view,
 * so we keep a lightweight JSON store in the app's private files directory.
 * Each session is a separate file named `claw-<uuid>.json`.
 */
object ClawSessionStore {

    private const val DIR = "claw_sessions"
    private const val PREFIX = "claw-"

    private fun dir(context: Context): File {
        val d = File(context.filesDir, DIR)
        if (!d.exists()) d.mkdirs()
        return d
    }

    /**
     * Return summaries of all saved Claw sessions, most recent first.
     */
    fun listSessions(context: Context): List<ClawSessionSummary> {
        val files = dir(context).listFiles { f -> f.isFile && f.name.startsWith(PREFIX) && f.name.endsWith(".json") }
            ?: return emptyList()
        return files.mapNotNull { readSummary(it) }
            .sortedByDescending { it.updatedAt }
    }

    /**
     * Load a single saved session, or null if it does not exist / is corrupt.
     */
    fun loadSession(context: Context, sessionId: String): ClawSession? {
        val file = sessionFile(context, sessionId) ?: return null
        return try {
            val json = JSONObject(file.readText())
            ClawSession(
                id = json.getString("id"),
                title = json.optString("title", "Claw"),
                createdAt = json.getLong("createdAt"),
                updatedAt = json.getLong("updatedAt"),
                messages = parseMessages(json.getJSONArray("messages"))
            )
        } catch (_: Exception) {
            null
        }
    }

    /**
     * Save the current messages of a Claw session.
     */
    fun saveSession(context: Context, sessionId: String, messages: List<ChatItem>) {
        val existing = loadSession(context, sessionId)
        val now = System.currentTimeMillis()
        val title = deriveTitle(messages) ?: existing?.title ?: "Claw"
        val createdAt = existing?.createdAt ?: now
        val json = JSONObject().apply {
            put("id", sessionId)
            put("title", title)
            put("createdAt", createdAt)
            put("updatedAt", now)
            put("messages", serializeMessages(messages))
        }
        sessionFile(context, sessionId)?.writeText(json.toString())
    }

    /**
     * Create a new empty Claw session and return its id.
     */
    fun createSession(context: Context): String {
        val id = "$PREFIX${UUID.randomUUID()}"
        val now = System.currentTimeMillis()
        val json = JSONObject().apply {
            put("id", id)
            put("title", "Claw")
            put("createdAt", now)
            put("updatedAt", now)
            put("messages", JSONArray())
        }
        sessionFile(context, id)?.writeText(json.toString())
        return id
    }

    /**
     * Delete a Claw session.
     */
    fun deleteSession(context: Context, sessionId: String) {
        sessionFile(context, sessionId)?.delete()
    }

    private fun sessionFile(context: Context, sessionId: String): File? {
        if (!sessionId.startsWith(PREFIX)) return null
        return File(dir(context), "$sessionId.json")
    }

    private fun readSummary(file: File): ClawSessionSummary? = try {
        val json = JSONObject(file.readText())
        ClawSessionSummary(
            id = json.getString("id"),
            title = json.optString("title", "Claw"),
            updatedAt = json.getLong("updatedAt")
        )
    } catch (_: Exception) {
        null
    }

    private fun deriveTitle(messages: List<ChatItem>): String? {
        val firstUser = messages.firstOrNull { it is ChatItem.UserText } as? ChatItem.UserText
        return firstUser?.content?.trim()?.take(40)?.let { if (it.length < (firstUser.content.length)) "$it..." else it }
    }

    private fun parseMessages(array: JSONArray): List<ChatItem> {
        val result = mutableListOf<ChatItem>()
        for (i in 0 until array.length()) {
            val obj = array.getJSONObject(i)
            val type = obj.getString("type")
            val id = obj.getString("id")
            val timestamp = obj.optString("timestamp", "")
            when (type) {
                "user" -> result.add(
                    ChatItem.UserText(
                        id = id,
                        content = obj.getString("content"),
                        timestamp = timestamp
                    )
                )
                "assistant" -> result.add(
                    ChatItem.AssistantText(
                        id = id,
                        content = obj.getString("content"),
                        isStreaming = false,
                        reasoningContent = obj.optString("reasoningContent", "").takeIf { it.isNotBlank() },
                        timestamp = timestamp
                    )
                )
                "tool_call" -> result.add(
                    ChatItem.ToolCallCard(
                        id = id,
                        turnId = obj.optString("turnId", ""),
                        callId = obj.optString("callId", ""),
                        toolName = obj.getString("toolName"),
                        argumentsJson = obj.getString("argumentsJson"),
                        timestamp = timestamp
                    )
                )
                "tool_result" -> result.add(
                    ChatItem.ToolResultCard(
                        id = id,
                        turnId = obj.optString("turnId", ""),
                        callId = obj.optString("callId", ""),
                        toolName = obj.getString("toolName"),
                        resultJson = obj.getString("resultJson"),
                        timestamp = timestamp
                    )
                )
            }
        }
        return result
    }

    private fun serializeMessages(messages: List<ChatItem>): JSONArray {
        val array = JSONArray()
        messages.forEach { item ->
            val obj = when (item) {
                is ChatItem.UserText -> JSONObject().apply {
                    put("type", "user")
                    put("id", item.id)
                    put("content", item.content)
                    put("timestamp", item.timestamp)
                }
                is ChatItem.AssistantText -> JSONObject().apply {
                    put("type", "assistant")
                    put("id", item.id)
                    put("content", item.content)
                    put("reasoningContent", item.reasoningContent ?: "")
                    put("timestamp", item.timestamp)
                }
                is ChatItem.ToolCallCard -> JSONObject().apply {
                    put("type", "tool_call")
                    put("id", item.id)
                    put("turnId", item.turnId)
                    put("callId", item.callId)
                    put("toolName", item.toolName)
                    put("argumentsJson", item.argumentsJson)
                    put("timestamp", item.timestamp)
                }
                is ChatItem.ToolResultCard -> JSONObject().apply {
                    put("type", "tool_result")
                    put("id", item.id)
                    put("turnId", item.turnId)
                    put("callId", item.callId)
                    put("toolName", item.toolName)
                    put("resultJson", item.resultJson)
                    put("timestamp", item.timestamp)
                }
            }
            array.put(obj)
        }
        return array
    }
}

/**
 * Summary of a persisted Claw session, suitable for the thread list.
 */
data class ClawSessionSummary(
    val id: String,
    val title: String,
    val updatedAt: Long,
) {
    fun formattedTime(): String =
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date(updatedAt))
}

/**
 * Full persisted Claw session including messages.
 */
data class ClawSession(
    val id: String,
    val title: String,
    val createdAt: Long,
    val updatedAt: Long,
    val messages: List<ChatItem>,
)
