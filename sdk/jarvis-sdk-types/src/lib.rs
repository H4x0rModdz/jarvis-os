//! Manifest types and the scanner for Jarvis SDK apps.
//!
//! Pure data + parsing — no DBus, no async. The Action Bus calls
//! `load_manifests()` on startup; every other piece of the system that
//! cares about the SDK shape (a future `jarvis-app` CLI, the docs site
//! generator) parses through the same types.
//!
//! See `module.md` for the manifest format and ADR 0011 for the design
//! rationale.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("{path}: app.id '{id}' does not match the required pattern (^[a-z][a-z0-9_]*$)")]
    InvalidAppId { path: PathBuf, id: String },
    #[error("{path}: directory name '{dir}' must equal app.id '{id}'")]
    DirIdMismatch {
        path: PathBuf,
        dir: String,
        id: String,
    },
    #[error("{path}: action '{name}' must start with '{prefix}.'")]
    BadActionPrefix {
        path: PathBuf,
        name: String,
        prefix: String,
    },
    #[error("{path}: action '{name}' does not match the required pattern (^[a-z][a-z0-9_.]*$)")]
    InvalidActionName { path: PathBuf, name: String },
    #[error("{path}: action '{name}' schema.type must be 'object' (got '{got}')")]
    SchemaNotObject {
        path: PathBuf,
        name: String,
        got: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub app: AppMeta,
    #[serde(default, rename = "actions")]
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppMeta {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Optional path to a launcher binary. Action Bus uses this only as
    /// a hint — DBus activation via `.service` files is the actual
    /// summon mechanism.
    #[serde(default)]
    pub exec: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Verbatim JSON Schema. Forwarded to Lilith / Ollama as the tool
    /// description; the Action Bus does not enforce it on dispatch — apps
    /// are responsible for their own param validation, same as built-in
    /// handlers.
    #[serde(default)]
    pub schema: serde_json::Value,
}

impl Manifest {
    /// DBus name the Action Bus expects this app to host.
    pub fn dbus_service(&self) -> String {
        format!("com.jarvis.app.{}", self.app.id)
    }

    /// DBus object path that hosts `Dispatch`.
    pub fn dbus_path(&self) -> String {
        format!("/com/jarvis/app/{}", self.app.id)
    }
}

static APP_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z][a-z0-9_]*$").unwrap());
static ACTION_NAME_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z][a-z0-9_.]*$").unwrap());

/// True when `id` matches the SDK app id pattern. Exposed so callers
/// (e.g. the `jarvis-app new` scaffold) can validate input without
/// touching the filesystem.
pub fn is_valid_app_id(id: &str) -> bool {
    APP_ID_RE.is_match(id)
}

/// Load a single manifest from `manifest_path`. The parent directory's
/// name is checked against `app.id` so reverse lookups stay trivial.
pub fn load_one(manifest_path: &Path) -> Result<Manifest, ManifestError> {
    let bytes = std::fs::read(manifest_path).map_err(|e| ManifestError::Io {
        path: manifest_path.to_path_buf(),
        source: e,
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let manifest: Manifest = toml::from_str(&text).map_err(|e| ManifestError::Parse {
        path: manifest_path.to_path_buf(),
        source: e,
    })?;

    if !APP_ID_RE.is_match(&manifest.app.id) {
        return Err(ManifestError::InvalidAppId {
            path: manifest_path.to_path_buf(),
            id: manifest.app.id,
        });
    }

    if let Some(parent) = manifest_path.parent() {
        if let Some(dir) = parent.file_name().and_then(|s| s.to_str()) {
            if dir != manifest.app.id {
                return Err(ManifestError::DirIdMismatch {
                    path: manifest_path.to_path_buf(),
                    dir: dir.to_string(),
                    id: manifest.app.id,
                });
            }
        }
    }

    let prefix = format!("{}.", manifest.app.id);
    for action in &manifest.actions {
        if !ACTION_NAME_RE.is_match(&action.name) {
            return Err(ManifestError::InvalidActionName {
                path: manifest_path.to_path_buf(),
                name: action.name.clone(),
            });
        }
        if !action.name.starts_with(&prefix) {
            return Err(ManifestError::BadActionPrefix {
                path: manifest_path.to_path_buf(),
                name: action.name.clone(),
                prefix: manifest.app.id.clone(),
            });
        }
        // Schemas are optional but if present must describe an object.
        if let Some(ty) = action.schema.get("type").and_then(|v| v.as_str()) {
            if ty != "object" {
                return Err(ManifestError::SchemaNotObject {
                    path: manifest_path.to_path_buf(),
                    name: action.name.clone(),
                    got: ty.to_string(),
                });
            }
        }
    }

    Ok(manifest)
}

/// Scan every well-known directory and return validated manifests.
///
/// Order: system paths first, then the per-user override path. When two
/// manifests share an `app.id` the *first* one wins (system shadows
/// user only because the user one is loaded last and rejected — see
/// the de-dup comment below). Failures on individual manifests are
/// logged via `tracing` and otherwise ignored, so one broken app can't
/// take the rest down.
pub fn load_manifests(scan_paths: &[PathBuf]) -> Vec<Manifest> {
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for root in scan_paths {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(path = %root.display(), error = %e, "SDK scan dir absent");
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("manifest.toml");
            if !manifest_path.exists() {
                continue;
            }
            match load_one(&manifest_path) {
                Ok(m) => {
                    if !seen_ids.insert(m.app.id.clone()) {
                        tracing::warn!(
                            id = %m.app.id,
                            path = %manifest_path.display(),
                            "Duplicate SDK app id — skipping"
                        );
                        continue;
                    }
                    tracing::info!(
                        id = %m.app.id,
                        actions = m.actions.len(),
                        path = %manifest_path.display(),
                        "Loaded SDK manifest"
                    );
                    out.push(m);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Skipping malformed SDK manifest");
                }
            }
        }
    }

    out
}

/// The two production scan paths. Tests use their own.
pub fn default_scan_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/usr/share/jarvis/apps")];
    if let Some(home) = dirs_home() {
        paths.push(home.join(".local/share/jarvis/apps"));
    }
    paths
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "jarvis-sdk-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_app(root: &Path, id: &str, manifest_body: &str) -> PathBuf {
        let dir = root.join(id);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.toml");
        fs::write(&path, manifest_body).unwrap();
        path
    }

    #[test]
    fn loads_a_well_formed_manifest() {
        let root = tempdir();
        let path = write_app(
            &root,
            "demo",
            r#"
            [app]
            id = "demo"
            name = "Demo"
            version = "1.0"

            [[actions]]
            name = "demo.echo"
            description = "Echo back"
            schema = { type = "object", properties = { msg = { type = "string" } }, required = ["msg"] }
            "#,
        );
        let m = load_one(&path).expect("loads");
        assert_eq!(m.app.id, "demo");
        assert_eq!(m.actions.len(), 1);
        assert_eq!(m.actions[0].name, "demo.echo");
        assert_eq!(m.dbus_service(), "com.jarvis.app.demo");
        assert_eq!(m.dbus_path(), "/com/jarvis/app/demo");
    }

    #[test]
    fn rejects_actions_outside_app_namespace() {
        let root = tempdir();
        let path = write_app(
            &root,
            "notes",
            r#"
            [app]
            id = "notes"
            [[actions]]
            name = "mail.send"
            "#,
        );
        let err = load_one(&path).unwrap_err();
        assert!(matches!(err, ManifestError::BadActionPrefix { .. }));
    }

    #[test]
    fn rejects_dir_mismatch() {
        let root = tempdir();
        // App id is "notes" but the dir is "wrong-name"
        let dir = root.join("wrong-name");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.toml");
        fs::write(
            &path,
            r#"
            [app]
            id = "notes"
            "#,
        )
        .unwrap();
        let err = load_one(&path).unwrap_err();
        assert!(matches!(err, ManifestError::DirIdMismatch { .. }));
    }

    #[test]
    fn rejects_invalid_app_id() {
        let root = tempdir();
        let dir = root.join("X");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.toml");
        fs::write(
            &path,
            r#"
            [app]
            id = "X"
            "#,
        )
        .unwrap();
        let err = load_one(&path).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidAppId { .. }));
    }

    #[test]
    fn rejects_non_object_schema() {
        let root = tempdir();
        let path = write_app(
            &root,
            "demo",
            r#"
            [app]
            id = "demo"
            [[actions]]
            name = "demo.x"
            schema = { type = "array" }
            "#,
        );
        assert!(matches!(
            load_one(&path).unwrap_err(),
            ManifestError::SchemaNotObject { .. }
        ));
    }

    #[test]
    fn scan_returns_all_valid_manifests_skips_broken() {
        let root = tempdir();
        write_app(
            &root,
            "good",
            r#"
            [app]
            id = "good"
            [[actions]]
            name = "good.do"
            "#,
        );
        // Broken: bad app id
        let bad_dir = root.join("BAD");
        fs::create_dir_all(&bad_dir).unwrap();
        fs::write(
            bad_dir.join("manifest.toml"),
            r#"
            [app]
            id = "BAD"
            "#,
        )
        .unwrap();

        let manifests = load_manifests(&[root]);
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].app.id, "good");
    }

    #[test]
    fn duplicate_ids_only_first_wins() {
        let a = tempdir();
        let b = tempdir();
        write_app(
            &a,
            "shared",
            r#"
            [app]
            id = "shared"
            name = "from-a"
            "#,
        );
        write_app(
            &b,
            "shared",
            r#"
            [app]
            id = "shared"
            name = "from-b"
            "#,
        );
        let manifests = load_manifests(&[a, b]);
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].app.name, "from-a");
    }
}
