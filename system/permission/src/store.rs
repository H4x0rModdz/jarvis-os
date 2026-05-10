use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

/// In-memory store of granted (caller, scope) pairs.
///
/// Phase 1 — lives in process. Phase 2 will back this with SQLite at
/// `~/.jarvis/permissions.db` per `.jarvis/architecture/permissions.md`.
pub struct GrantStore {
    grants: Mutex<HashMap<GrantKey, Grant>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GrantKey {
    caller: String,
    scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Grant {
    pub caller: String,
    pub scope: String,
    pub granted_at: DateTime<Utc>,
    pub persistent: bool,
}

impl GrantStore {
    pub fn new() -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
        }
    }

    pub fn grant(&self, caller: &str, scope: &str, persistent: bool) -> Grant {
        let grant = Grant {
            caller: caller.to_owned(),
            scope: scope.to_owned(),
            granted_at: Utc::now(),
            persistent,
        };
        let key = GrantKey {
            caller: caller.to_owned(),
            scope: scope.to_owned(),
        };
        self.grants.lock().unwrap().insert(key, grant.clone());
        grant
    }

    pub fn has(&self, caller: &str, scope: &str) -> bool {
        let key = GrantKey {
            caller: caller.to_owned(),
            scope: scope.to_owned(),
        };
        self.grants.lock().unwrap().contains_key(&key)
    }

    /// Returns true if a grant was removed.
    pub fn revoke(&self, caller: &str, scope: &str) -> bool {
        let key = GrantKey {
            caller: caller.to_owned(),
            scope: scope.to_owned(),
        };
        self.grants.lock().unwrap().remove(&key).is_some()
    }

    pub fn list(&self) -> Vec<Grant> {
        let mut out: Vec<Grant> = self.grants.lock().unwrap().values().cloned().collect();
        out.sort_by(|a, b| a.caller.cmp(&b.caller).then_with(|| a.scope.cmp(&b.scope)));
        out
    }
}

impl Default for GrantStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_and_check() {
        let s = GrantStore::new();
        assert!(!s.has("lilith", "filesystem.delete"));
        s.grant("lilith", "filesystem.delete", false);
        assert!(s.has("lilith", "filesystem.delete"));
    }

    #[test]
    fn revoke_removes_grant() {
        let s = GrantStore::new();
        s.grant("lilith", "app.install", true);
        assert!(s.has("lilith", "app.install"));
        assert!(s.revoke("lilith", "app.install"));
        assert!(!s.has("lilith", "app.install"));
        // second revoke is a no-op
        assert!(!s.revoke("lilith", "app.install"));
    }

    #[test]
    fn list_is_sorted() {
        let s = GrantStore::new();
        s.grant("lilith", "z.last", false);
        s.grant("lilith", "a.first", false);
        s.grant("app:firefox", "network.request.external", true);
        let listed = s.list();
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].caller, "app:firefox");
        assert_eq!(listed[1].scope, "a.first");
        assert_eq!(listed[2].scope, "z.last");
    }

    #[test]
    fn grant_idempotent_on_same_key() {
        let s = GrantStore::new();
        s.grant("lilith", "settings.modify", false);
        s.grant("lilith", "settings.modify", true);
        assert_eq!(s.list().len(), 1);
        assert!(s.list()[0].persistent);
    }
}
