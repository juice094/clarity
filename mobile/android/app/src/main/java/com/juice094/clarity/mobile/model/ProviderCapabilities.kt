package com.juice094.clarity.mobile.model

import uniffi.clarity_mobile_core.ProviderType

/**
 * Static capability matrix for provider channels.
 *
 * Not every provider exposes a public API: some channels authenticate through
 * an App login flow (DeepSeek device PoW login) and have no `/v1/models`
 * endpoint to enumerate. The setup UI uses this matrix to show an explanatory
 * badge instead of letting the user assume an API-key flow exists.
 *
 * ponytail: KIMI_CODE (OAuth), OPENCLAW and local GGUF models are also non-API
 * channels per product decision, but the UniFFI `ProviderType` enum in
 * `clarity-mobile-core` does not expose them yet. When those variants are
 * added to the FFI, extend [hasPublicApi] here and the setup badge follows.
 */
object ProviderCapabilities {

    /**
     * Whether the channel has a public HTTP API (API key auth, enumerable
     * model list). Channels without one authenticate via an App login flow.
     */
    fun hasPublicApi(type: ProviderType): Boolean = when (type) {
        ProviderType.DEEPSEEK_DEVICE -> false
        ProviderType.OPEN_AI,
        ProviderType.KIMI,
        ProviderType.DEEPSEEK,
        ProviderType.ANTHROPIC -> true
    }

    /**
     * Short badge label for non-API channels, shown next to the provider name.
     */
    fun badgeLabel(type: ProviderType): String? =
        if (hasPublicApi(type)) null else "无公开 API · App login"

    /**
     * Longer explanation shown under the provider selector for non-API channels.
     */
    fun badgeDescription(type: ProviderType): String? =
        if (hasPublicApi(type)) {
            null
        } else {
            "该渠道使用 App 登录通道，无公开 API；模型列表不可拉取，模型由服务端决定。\n" +
                "This channel uses App login with no public API; the model list cannot be fetched."
        }
}
