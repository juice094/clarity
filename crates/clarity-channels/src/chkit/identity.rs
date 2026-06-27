//! Channel identity resolution.
//!
//! Maps external channel sender information into internal identity types
//! used by the agent for attribution, policy, and persistence.

/// An end-user identity resolved from a channel message.
#[derive(Debug, Clone)]
pub struct User {
    /// Channel / provider name (e.g. `"discord"`, `"slack"`, `"wechat"`).
    pub provider: String,
    /// Platform-specific user identifier.
    pub provider_user_id: String,
    /// Human-readable display name, if available.
    pub display_name: Option<String>,
}

/// Contextual identity information for a channel turn.
#[derive(Debug, Clone)]
pub struct IdentityContext {
    /// The resolved user identity.
    pub user: User,
    /// The channel name this identity was resolved from.
    pub channel_name: String,
}

/// Resolve a channel message sender into an identity context.
///
/// Auto-generates a [`User`] with `provider` set to the channel name and
/// `provider_user_id` set to the sender identifier. The returned tuple
/// provides both the full [`IdentityContext`] and the standalone [`User`]
/// for callers that only need the user record.
pub fn resolve_channel_identity(
    channel_name: impl Into<String>,
    sender_id: impl Into<String>,
    sender_name: Option<String>,
) -> (IdentityContext, User) {
    let channel_name = channel_name.into();
    let sender_id = sender_id.into();
    let user = User {
        provider: channel_name.clone(),
        provider_user_id: sender_id,
        display_name: sender_name,
    };
    let ctx = IdentityContext {
        user: user.clone(),
        channel_name,
    };
    (ctx, user)
}
