/// Outcome of evaluating a scope against the built-in policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyVerdict {
    /// Scope is in the safe-by-default list — auto-allow.
    AutoAllow,
    /// Scope is dangerous — require an explicit grant.
    RequireGrant,
    /// Scope is unrecognized — deny.
    Unknown,
}

/// Classify a scope per the policy declared in module.md.
///
/// Matching is by exact name or by a dotted prefix:
/// `filesystem.write` matches `filesystem.write` itself **and** any extension
/// like `filesystem.write.downloads` — the more specific scope is still
/// considered dangerous because its parent is.
pub fn classify(scope: &str) -> PolicyVerdict {
    if matches_any(scope, SAFE_SCOPES) {
        return PolicyVerdict::AutoAllow;
    }
    if matches_any(scope, DANGEROUS_SCOPES) {
        return PolicyVerdict::RequireGrant;
    }
    PolicyVerdict::Unknown
}

fn matches_any(scope: &str, bucket: &[&str]) -> bool {
    bucket
        .iter()
        .any(|prefix| scope == *prefix || scope.starts_with(&format!("{prefix}.")))
}

const SAFE_SCOPES: &[&str] = &[
    "app.launch",
    "window.control",
    "system.notify",
    "settings.read",
    "filesystem.read",
    // Audio output changes are reversible by the user (volume knob,
    // headphone unplug). Same family as `system.notify` — useful, not
    // sensitive.
    "audio.control",
    // Writing to the clipboard cannot leak information back to the caller;
    // the user can always paste over our value. Reading the clipboard is a
    // different story (see `clipboard.read` below).
    "clipboard.write",
];

const DANGEROUS_SCOPES: &[&str] = &[
    "app.install",
    "app.uninstall",
    "filesystem.write",
    "filesystem.delete",
    "settings.modify",
    "terminal.execute",
    "network.request.external",
    "microphone.listen",
    "camera.access",
    // Reading the clipboard or the screen can hand the AI passwords,
    // private messages, banking info — same privacy class as listening
    // through the mic.
    "clipboard.read",
    "screen.read",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_scopes_auto_allow() {
        assert_eq!(classify("app.launch"), PolicyVerdict::AutoAllow);
        assert_eq!(classify("system.notify"), PolicyVerdict::AutoAllow);
        assert_eq!(classify("window.control"), PolicyVerdict::AutoAllow);
        assert_eq!(classify("settings.read"), PolicyVerdict::AutoAllow);
        assert_eq!(classify("audio.control"), PolicyVerdict::AutoAllow);
        assert_eq!(classify("clipboard.write"), PolicyVerdict::AutoAllow);
    }

    #[test]
    fn dangerous_scopes_require_grant() {
        assert_eq!(classify("filesystem.delete"), PolicyVerdict::RequireGrant);
        assert_eq!(classify("app.install"), PolicyVerdict::RequireGrant);
        assert_eq!(classify("terminal.execute"), PolicyVerdict::RequireGrant);
        assert_eq!(
            classify("network.request.external"),
            PolicyVerdict::RequireGrant
        );
        assert_eq!(classify("clipboard.read"), PolicyVerdict::RequireGrant);
        assert_eq!(classify("screen.read"), PolicyVerdict::RequireGrant);
    }

    #[test]
    fn dotted_subscopes_inherit_parent() {
        // .skills/ai-safety.md treats filesystem.delete.<path> the same as
        // filesystem.delete — the more specific shouldn't accidentally become safe.
        assert_eq!(
            classify("filesystem.delete.downloads"),
            PolicyVerdict::RequireGrant
        );
        assert_eq!(
            classify("filesystem.write./home/user/docs"),
            PolicyVerdict::RequireGrant
        );
        assert_eq!(classify("filesystem.read./etc"), PolicyVerdict::AutoAllow);
    }

    #[test]
    fn unknown_scope_denied() {
        assert_eq!(classify("totally.made.up"), PolicyVerdict::Unknown);
        assert_eq!(classify(""), PolicyVerdict::Unknown);
    }

    #[test]
    fn no_substring_false_positive() {
        // "appinstall" or "appdotinstall" must not match "app.install".
        assert_eq!(classify("appinstall"), PolicyVerdict::Unknown);
        assert_eq!(classify("app.install_helper"), PolicyVerdict::Unknown);
    }
}
