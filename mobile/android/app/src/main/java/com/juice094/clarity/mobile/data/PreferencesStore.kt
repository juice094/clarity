package com.juice094.clarity.mobile.data

import android.content.Context
import android.content.SharedPreferences
import com.juice094.clarity.mobile.viewmodel.ChatViewModel
import uniffi.clarity_mobile_core.ProviderType

/**
 * Local persistence for the last-used provider / gateway configuration.
 *
 * API keys are stored in plain SharedPreferences for now. If this app ever
 * handles production secrets, migrate to EncryptedSharedPreferences or the
 * platform Keystore.
 */
object PreferencesStore {
    private const val PREFS_NAME = "clarity_mobile_prefs"

    private const val KEY_PROVIDER = "provider"
    private const val KEY_MODEL = "model"
    private const val KEY_GATEWAY_URL = "gateway_url"
    private const val KEY_AGENT_MODE = "agent_mode"
    private const val KEY_SEARCH_ENABLED = "search_enabled"
    private const val KEY_THINKING_ENABLED = "thinking_enabled"
    private const val KEY_LAUNCH_MODE = "launch_mode"

    private fun prefs(context: Context): SharedPreferences {
        return context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    /**
     * How the app was last used. Used to auto-restore the primary chat mode
     * on cold start so the user does not have to re-select provider/gateway.
     */
    enum class LaunchMode {
        LocalChat,
        Claw,
        Unset
    }

    /**
     * Load persisted values into the ViewModel. Missing values keep their
     * current defaults.
     */
    fun load(context: Context, viewModel: ChatViewModel) {
        val p = prefs(context)
        p.getString(KEY_PROVIDER, null)?.let { name ->
            runCatching { ProviderType.valueOf(name) }.getOrNull()
        }?.let { viewModel.providerType.value = it }
        viewModel.model.value = p.getString(KEY_MODEL, viewModel.model.value) ?: ""
        viewModel.gatewayUrl.value = p.getString(KEY_GATEWAY_URL, viewModel.gatewayUrl.value) ?: ""
        viewModel.isAgentMode.value = p.getBoolean(KEY_AGENT_MODE, viewModel.isAgentMode.value)
        viewModel.isSearchEnabled.value = p.getBoolean(KEY_SEARCH_ENABLED, viewModel.isSearchEnabled.value)
        viewModel.isThinkingEnabled.value = p.getBoolean(KEY_THINKING_ENABLED, viewModel.isThinkingEnabled.value)
        viewModel.launchMode.value = runCatching {
            LaunchMode.valueOf(p.getString(KEY_LAUNCH_MODE, LaunchMode.Unset.name) ?: LaunchMode.Unset.name)
        }.getOrDefault(LaunchMode.Unset)
        SecurePreferencesStore.load(context, viewModel)
    }

    /**
     * Persist the current configuration from the ViewModel.
     */
    fun save(context: Context, viewModel: ChatViewModel) {
        prefs(context).edit().apply {
            putString(KEY_PROVIDER, viewModel.providerType.value.name)
            putString(KEY_MODEL, viewModel.model.value)
            putString(KEY_GATEWAY_URL, viewModel.gatewayUrl.value)
            putBoolean(KEY_AGENT_MODE, viewModel.isAgentMode.value)
            putBoolean(KEY_SEARCH_ENABLED, viewModel.isSearchEnabled.value)
            putBoolean(KEY_THINKING_ENABLED, viewModel.isThinkingEnabled.value)
            putString(KEY_LAUNCH_MODE, viewModel.launchMode.value.name)
            apply()
        }
        SecurePreferencesStore.save(context, viewModel)
    }

    /**
     * Clear any saved credentials. Useful for sign-out or debug resets.
     */
    fun clear(context: Context) {
        prefs(context).edit().clear().apply()
        SecurePreferencesStore.clear(context)
    }
}
