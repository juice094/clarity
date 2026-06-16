//! Shared `allowed_users` matching used by chat channels.

/// Case-sensitivity selector for the allowlist comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// Exact `==` match.
    Sensitive,
    /// `eq_ignore_ascii_case` — IRC nicks, Matrix MXIDs.
    CaseInsensitive,
}

/// Return `true` when `user` is allowed under `allowed`.
///
/// - `["*"]` (or any list containing `"*"`) means "anyone".
/// - Empty list means "deny everyone".
/// - Otherwise, exact match against the user's identifier wins.
#[must_use]
pub fn is_user_allowed(allowed: &[String], user: &str, mode: Match) -> bool {
    if allowed.iter().any(|u| u == "*") {
        return true;
    }
    match mode {
        Match::Sensitive => allowed.iter().any(|u| u == user),
        Match::CaseInsensitive => allowed.iter().any(|u| u.eq_ignore_ascii_case(user)),
    }
}

/// Return `true` when `user` is allowed under `allowed`, using a
/// caller-provided comparison for the per-entry check.
#[must_use]
pub fn is_user_allowed_by(
    allowed: &[String],
    user: &str,
    match_fn: impl Fn(&str, &str) -> bool,
) -> bool {
    if allowed.iter().any(|u| u == "*") {
        return true;
    }
    allowed.iter().any(|entry| match_fn(entry, user))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_allows_anyone() {
        let list = vec!["*".to_string()];
        assert!(is_user_allowed(&list, "alice", Match::Sensitive));
        assert!(is_user_allowed(&list, "ALICE", Match::Sensitive));
    }

    #[test]
    fn empty_list_denies_everyone() {
        assert!(!is_user_allowed(&[], "alice", Match::Sensitive));
        assert!(!is_user_allowed(&[], "alice", Match::CaseInsensitive));
    }

    #[test]
    fn exact_match_case_sensitive() {
        let list = vec!["alice".to_string()];
        assert!(is_user_allowed(&list, "alice", Match::Sensitive));
        assert!(!is_user_allowed(&list, "Alice", Match::Sensitive));
    }

    #[test]
    fn exact_match_case_insensitive() {
        let list = vec!["Alice".to_string()];
        assert!(is_user_allowed(&list, "alice", Match::CaseInsensitive));
        assert!(is_user_allowed(&list, "ALICE", Match::CaseInsensitive));
    }
}
