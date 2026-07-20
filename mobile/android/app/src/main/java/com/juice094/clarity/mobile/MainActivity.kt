package com.juice094.clarity.mobile

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.juice094.clarity.mobile.model.Screen
import com.juice094.clarity.mobile.ui.components.ApprovalDialog
import com.juice094.clarity.mobile.ui.screens.ChatScreen
import com.juice094.clarity.mobile.ui.screens.ProviderSetupScreen
import com.juice094.clarity.mobile.ui.screens.SettingsScreen
import com.juice094.clarity.mobile.ui.screens.ThreadListScreen
import com.juice094.clarity.mobile.ui.theme.ClarityMobileTheme
import com.juice094.clarity.mobile.viewmodel.ChatViewModel

class MainActivity : ComponentActivity() {

    private val viewModel: ChatViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            ClarityMobileTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    ClarityApp(viewModel = viewModel)
                }
            }
        }
    }
}

@Composable
fun ClarityApp(viewModel: ChatViewModel) {
    val screen by viewModel.currentScreen
    val errorText by viewModel.errorText
    val pendingApproval by viewModel.pendingApproval

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            Box(modifier = Modifier.weight(1f)) {
                when (screen) {
                    Screen.ProviderSetup -> ProviderSetupScreen(viewModel = viewModel)
                    Screen.ThreadList -> ThreadListScreen(viewModel = viewModel)
                    Screen.Chat -> ChatScreen(viewModel = viewModel)
                    Screen.Settings -> SettingsScreen(
                        viewModel = viewModel,
                        onBack = { viewModel.currentScreen.value = Screen.ThreadList }
                    )
                }
            }

            if (errorText.isNotBlank()) {
                Text(
                    text = errorText,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(16.dp)
                )
            }
        }
    }

    pendingApproval?.let { approval ->
        ApprovalDialog(
            approval = approval,
            onAllow = { remember -> viewModel.resolveApproval(true, remember) },
            onDeny = { viewModel.resolveApproval(false, false) }
        )
    }
}
