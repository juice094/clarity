# Clarity Android Client

Android frontend for Project Clarity. Talks to the Rust engine through `clarity-mobile-core` via UniFFI / JNI.

## Prerequisites

- Android SDK (API 28–35)
- Android Emulator or device
- Rust toolchain with Android targets installed:
  ```bash
  rustup target add aarch64-linux-android x86_64-linux-android
  cargo install cargo-ndk
  ```

## Build

1. Generate UniFFI Kotlin bindings and build Rust shared libraries:
   ```bash
   bash mobile/android/rust/build-android.sh
   ```
   Output lands in `mobile/android/app/src/main/jniLibs/` and
   `mobile/android/app/src/main/java/uniffi/clarity_mobile_core/`.

2. Build the Android APK:
   ```bash
   mobile/android/gradlew.bat -p mobile/android assembleDebug
   ```

## Run

Start a local Gateway (optional, for Claw remote mode):

```bash
CLARITY_ADMIN_TOKEN=claw-test-admin target/release/clarity-gateway.exe
```

Install and launch on the emulator:

```bash
adb install -r mobile/android/app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.juice094.clarity.mobile/.MainActivity
```

## Test

```bash
# JVM unit tests
mobile/android/gradlew.bat -p mobile/android testDebugUnitTest

# Rust FFI crate tests
cargo test -p clarity-mobile-core --lib
```

## Current capabilities

- **Claw-first entry**: app opens to the thread list; tap the top "Claw" item to connect to a Gateway, or tap "+" to set up a local agent.
- Local agent mode with OpenAI / Kimi / DeepSeek / Anthropic providers.
- DeepSeek-style dark theme and design tokens.
- Thread list with relative timestamps and a persistent Claw entry.
- Streaming assistant responses with user/assistant bubble styles.
- Markdown rendering for assistant messages (headings, bold/italic, inline code, code blocks, bullet lists, links).
- Tool-call cards with expandable JSON.
- Approval dialogs for high-risk tools.
- Stop-generation button.
- Model switcher and per-turn feature toggles (Agent / Search / Thinking) in the chat screen.
- Settings screen placeholder (to be expanded).
- Claw remote mode: connect to a `clarity-gateway` WebSocket endpoint.

## Project layout

```text
mobile/android/
├── app/
│   ├── src/main/java/com/juice094/clarity/mobile/
│   │   ├── MainActivity.kt            # App root / navigation shell
│   │   ├── ClarityApplication.kt      # Application class
│   │   ├── model/                     # Screen, ChatItem, PendingApproval
│   │   ├── viewmodel/                 # ChatViewModel, EventHandler
│   │   ├── ui/
│   │   │   ├── components/            # Bubbles, input, thread item, Claw entry, Markdown
│   │   │   ├── screens/               # ProviderSetup, ThreadList, Chat, Settings
│   │   │   └── theme/                 # DeepSeek-inspired design tokens
│   │   └── uniffi/...                 # UniFFI-generated Rust bindings
│   ├── src/test/java/...              # JVM unit tests
│   └── src/main/jniLibs/              # cargo-ndk artifacts
├── rust/build-android.sh              # UniFFI + cargo-ndk build script
└── README.md                          # This file
```
