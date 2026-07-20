package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import org.json.JSONObject

@Composable
fun FormattedJson(json: String, modifier: Modifier = Modifier) {
    val pretty = remember(json) {
        try {
            JSONObject(json).toString(2)
        } catch (_: Exception) {
            json
        }
    }
    Text(
        text = pretty,
        style = MaterialTheme.typography.bodySmall,
        fontFamily = FontFamily.Monospace,
        modifier = modifier.fillMaxWidth()
    )
}
