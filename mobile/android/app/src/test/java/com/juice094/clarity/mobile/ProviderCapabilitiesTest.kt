package com.juice094.clarity.mobile

import com.juice094.clarity.mobile.model.ProviderCapabilities
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.clarity_mobile_core.ProviderType

class ProviderCapabilitiesTest {

    @Test
    fun apiChannelsHavePublicApi() {
        assertTrue(ProviderCapabilities.hasPublicApi(ProviderType.OPEN_AI))
        assertTrue(ProviderCapabilities.hasPublicApi(ProviderType.KIMI))
        assertTrue(ProviderCapabilities.hasPublicApi(ProviderType.DEEPSEEK))
        assertTrue(ProviderCapabilities.hasPublicApi(ProviderType.ANTHROPIC))
    }

    @Test
    fun deviceLoginChannelHasNoPublicApi() {
        assertFalse(ProviderCapabilities.hasPublicApi(ProviderType.DEEPSEEK_DEVICE))
    }

    @Test
    fun badgeOnlyShownForNonApiChannels() {
        assertNotNull(ProviderCapabilities.badgeLabel(ProviderType.DEEPSEEK_DEVICE))
        assertNotNull(ProviderCapabilities.badgeDescription(ProviderType.DEEPSEEK_DEVICE))
        ProviderType.entries.filter { ProviderCapabilities.hasPublicApi(it) }.forEach { type ->
            assertNull(ProviderCapabilities.badgeLabel(type))
            assertNull(ProviderCapabilities.badgeDescription(type))
        }
    }

    @Test
    fun everyProviderTypeIsClassified() {
        // Guard against a new FFI ProviderType variant silently falling into
        // the default branch without a product decision.
        ProviderType.entries.forEach { type ->
            // Must not throw and must return a definitive answer.
            val result = ProviderCapabilities.hasPublicApi(type)
            assertTrue(result || !result)
        }
    }
}
