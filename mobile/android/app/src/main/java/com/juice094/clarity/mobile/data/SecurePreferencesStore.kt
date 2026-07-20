package com.juice094.clarity.mobile.data

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import com.juice094.clarity.mobile.viewmodel.ChatViewModel

/**
 * Encrypted storage for sensitive Clarity Mobile data.
 *
 * API keys, account passwords, and device tokens are kept here instead of the
 * plain-text [PreferencesStore]. Non-sensitive UI toggles and URLs remain in the
 * default SharedPreferences.
 */
object SecurePreferencesStore {
    private const val FILE_NAME = "clarity_secure_prefs"

    private const val KEY_API_KEY = "api_key"
    private const val KEY_MOBILE = "mobile"
    private const val KEY_PASSWORD = "password"
    private const val KEY_GATEWAY_TOKEN = "gateway_token"
    private const val KEY_DEEPSEEK_DEVICE_TOKEN = "deepseek_device_token"

    private fun prefs(context: Context): SharedPreferences {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        return EncryptedSharedPreferences.create(
            context,
            FILE_NAME,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
    }

    fun load(context: Context, viewModel: ChatViewModel) {
        val p = prefs(context)
        viewModel.apiKey.value = p.getString(KEY_API_KEY, viewModel.apiKey.value) ?: ""
        viewModel.mobile.value = p.getString(KEY_MOBILE, viewModel.mobile.value) ?: ""
        viewModel.password.value = p.getString(KEY_PASSWORD, viewModel.password.value) ?: ""
        viewModel.gatewayToken.value = p.getString(KEY_GATEWAY_TOKEN, viewModel.gatewayToken.value) ?: ""
        viewModel.deepseekDeviceToken.value =
            p.getString(KEY_DEEPSEEK_DEVICE_TOKEN, viewModel.deepseekDeviceToken.value) ?: ""
        android.util.Log.d(
            "ClarityMobile",
            "SecurePreferencesStore.load mobileBlank=${viewModel.mobile.value.isBlank()} passwordBlank=${viewModel.password.value.isBlank()}"
        )
    }

    fun save(context: Context, viewModel: ChatViewModel) {
        prefs(context).edit().apply {
            putString(KEY_API_KEY, viewModel.apiKey.value)
            putString(KEY_MOBILE, viewModel.mobile.value)
            putString(KEY_PASSWORD, viewModel.password.value)
            putString(KEY_GATEWAY_TOKEN, viewModel.gatewayToken.value)
            putString(KEY_DEEPSEEK_DEVICE_TOKEN, viewModel.deepseekDeviceToken.value)
            apply()
        }
    }

    fun clear(context: Context) {
        prefs(context).edit().clear().apply()
    }
}
