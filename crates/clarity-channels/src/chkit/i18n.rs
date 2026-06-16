//! Minimal i18n stub for channel user-facing CLI strings.
//!
//! Uses a hardcoded key→message map so the channel can operate without
//! pulling in a full localization framework. A future iteration can
//! re-introduce Fluent or another i18n system.

/// Return a required CLI string by key.
pub fn get_required_cli_string(key: &str) -> String {
    get_required_cli_string_with_args(key, &[])
}

/// Return a required CLI string by key, substituting named placeholders.
pub fn get_required_cli_string_with_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut message = match key {
        "cli-wechat-pairing-required" => "WeChat iLink pairing required. Pairing code: {code}",
        "cli-wechat-send-bind-command" => {
            "Send '{command} <code>' from WeChat to bind this device."
        }
        "cli-wechat-bound-success" => "Bound successfully. You can now send messages.",
        "cli-wechat-invalid-bind-code" => {
            "Invalid or expired pairing code. Please check and try again."
        }
        "cli-wechat-login-success" => "WeChat login successful.",
        "cli-wechat-login-timeout" => "WeChat QR code login timed out. Please restart.",
        "cli-wechat-send-qr-in-terminal" => "Scan the QR code below with WeChat:",
        "cli-wechat-pairing-error" => "Pairing error. Please try again later.",
        "cli-wechat-unauthorized" => "You are not authorized. Send '{command} <code>' to bind.",
        "cli-wechat-qr-login" => "WeChat QR login:",
        "cli-wechat-scan-to-connect" => "Scan the QR code with WeChat to connect.",
        "cli-wechat-qr-url" => "QR image URL: {url}",
        "cli-wechat-qr-fetch-failed" => "Failed to fetch WeChat QR code.",
        "cli-wechat-qr-fetch-status-failed" => "Failed to check QR status: {status}",
        "cli-wechat-qr-expired-giving-up" => {
            "QR code expired after {attempts} attempts. Giving up."
        }
        "cli-wechat-qr-expired-refreshing" => "QR code expired, refreshing...",
        "cli-wechat-scanned-confirm" => "Scanned, confirming on phone...",
        "cli-wechat-connected" => "Connected to WeChat.",
        "cli-wechat-login-confirmed-missing-field" => "Login confirmed but missing field: {field}",
        _ => key,
    }
    .to_string();

    for (name, value) in args {
        message = message.replace(&format!("{{{}}}", name), value);
    }
    message
}
