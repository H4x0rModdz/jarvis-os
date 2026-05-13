//! Jarvis Compat — the Windows compatibility daemon.
//!
//! V2: per-app prefix support. The default prefix at
//! `~/.jarvis/wine/default/` still works (RunExe with no prefix
//! specified). New methods let callers create / list / target named
//! prefixes so games and heavyweight productivity apps can have an
//! isolated WINEPREFIX without polluting the shared one.

use anyhow::Context;
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};

const WINE_BINARY: &str = "wine";
const WINE_ARCH: &str = "win64";
const DEFAULT_PREFIX: &str = "default";

/// Override for the Proton-GE install dir. Resolved via env first
/// (dev / advanced users), then falls back to `~/.jarvis/proton-ge/`.
const PROTON_DIR_ENV: &str = "JARVIS_PROTON_DIR";

struct CompatService {
    /// Held during `wineboot --init` so concurrent first-time calls
    /// queue up behind the prefix bring-up instead of racing it.
    prefix_init: Arc<AsyncMutex<()>>,
}

#[derive(Debug, Clone, Serialize)]
struct PrefixInfo {
    name: String,
    path: String,
    initialised: bool,
    created_at: Option<String>,
    last_used_at: Option<String>,
    /// Last engine recorded for this prefix — `"wine"` (default) or
    /// `"proton"` when the prefix was last spawned through
    /// `run_proton`. Future UI uses this to badge prefixes correctly.
    engine: String,
}

#[interface(name = "com.jarvis.Compat")]
impl CompatService {
    /// Run a Windows binary under Wine in the default prefix.
    /// V1 entry point; kept for backwards compatibility. New callers
    /// should prefer `RunExeIn` so the prefix is explicit.
    async fn run_exe(
        &self,
        path: &str,
        args: Vec<String>,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        self.run_in_prefix(DEFAULT_PREFIX, path, args, ctx).await
    }

    /// Run a Windows binary under Wine in a named prefix. The prefix
    /// is created on first use (same `wineboot --init` path as the
    /// default prefix's first call).
    async fn run_exe_in(
        &self,
        prefix_name: &str,
        path: &str,
        args: Vec<String>,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        if !is_valid_prefix_name(prefix_name) {
            return json!({
                "started": false,
                "reason": format!("invalid prefix name '{prefix_name}' \
                                   (must match ^[a-z0-9][a-z0-9_-]*$)"),
            })
            .to_string();
        }
        self.run_in_prefix(prefix_name, path, args, ctx).await
    }

    /// Pre-create a named prefix without running anything in it. Lets
    /// the caller force the `wineboot --init` cost up front before
    /// the first actual app launch.
    async fn create_prefix(&self, prefix_name: &str) -> String {
        if !is_valid_prefix_name(prefix_name) {
            return json!({
                "ok": false,
                "reason": format!("invalid prefix name '{prefix_name}' \
                                   (must match ^[a-z0-9][a-z0-9_-]*$)"),
            })
            .to_string();
        }
        let prefix = match prefix_dir(prefix_name) {
            Ok(p) => p,
            Err(e) => return json!({ "ok": false, "reason": e.to_string() }).to_string(),
        };

        let _gate = self.prefix_init.lock().await;
        if prefix.join("system.reg").exists() {
            return json!({ "ok": true, "already": true }).to_string();
        }

        if let Err(e) = wineboot_init(&prefix).await {
            return json!({ "ok": false, "reason": e.to_string() }).to_string();
        }
        write_meta(&prefix, /*touch_used=*/ false);
        json!({ "ok": true, "already": false, "path": prefix.to_string_lossy() }).to_string()
    }

    /// Run a Windows binary under Proton-GE in a named prefix.
    /// Proton uses its own Steam-style compat-data layout (a `pfx/`
    /// subdir for the actual WINEPREFIX, plus `tracked_files`, etc.),
    /// so its prefixes live under `~/.jarvis/proton-data/<name>/`
    /// rather than the Wine root. Engine is recorded in the prefix
    /// meta — `list_prefixes` exposes it.
    ///
    /// V1 of Proton support: Proton-GE must already be present at
    /// `JARVIS_PROTON_DIR` (or `~/.jarvis/proton-ge/`). We don't
    /// auto-download — Proton-GE is ~300 MB and baking it into the
    /// ISO would blow the size budget. See ADR 0017.
    async fn run_proton(
        &self,
        prefix_name: &str,
        path: &str,
        args: Vec<String>,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        if !is_valid_prefix_name(prefix_name) {
            return json!({
                "started": false,
                "reason": format!("invalid prefix name '{prefix_name}'"),
            })
            .to_string();
        }

        let exe = Path::new(path);
        if !exe.exists() {
            return json!({
                "started": false,
                "reason": format!("binary not found: {path}"),
            })
            .to_string();
        }

        let proton = match proton_binary() {
            Some(p) => p,
            None => {
                return json!({
                    "started": false,
                    "reason": format!(
                        "proton not installed — expected at {} (set {} to override). \
                         Drop a Proton-GE release at that path; see ADR 0017.",
                        proton_root()
                            .map(|r| r.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "~/.jarvis/proton-ge".into()),
                        PROTON_DIR_ENV,
                    ),
                })
                .to_string();
            }
        };

        let compat_data = match proton_data_dir(prefix_name) {
            Ok(p) => p,
            Err(e) => return json!({ "started": false, "reason": e.to_string() }).to_string(),
        };
        if let Err(e) = tokio::fs::create_dir_all(&compat_data).await {
            return json!({
                "started": false,
                "reason": format!("create compat-data dir: {e}"),
            })
            .to_string();
        }

        // Proton expects STEAM_COMPAT_CLIENT_INSTALL_PATH to point at
        // *something* that exists, even though without real Steam
        // it's only used for logging. A throwaway dir under the same
        // root keeps Proton happy without faking a Steam install.
        let stub_steam = compat_data.join(".steam-stub");
        let _ = tokio::fs::create_dir_all(&stub_steam).await;

        let mut cmd = tokio::process::Command::new(&proton);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data)
            .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &stub_steam)
            .arg("run")
            .arg(path)
            .args(&args);

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return json!({
                    "started": false,
                    "reason": format!("spawn proton: {e}"),
                })
                .to_string();
            }
        };

        // Record engine=proton in the meta so `list_prefixes` surfaces
        // which engine the prefix was last run with.
        write_proton_meta(&compat_data);

        let pid = child.id().unwrap_or(0);
        tracing::info!(pid, prefix = prefix_name, %path, "Spawned proton process");

        let ctx_owned = ctx.to_owned();
        tokio::spawn(async move {
            let mut child = child;
            let status = child.wait().await;
            let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
            tracing::info!(pid, exit = code, "Proton process exited");
            if let Err(e) = Self::process_exited(&ctx_owned, pid, code).await {
                tracing::warn!("ProcessExited emit failed: {e}");
            }
        });

        json!({
            "started": true,
            "pid": pid,
            "prefix": prefix_name,
            "engine": "proton",
        })
        .to_string()
    }

    /// Returns every prefix under `~/.jarvis/wine/` with its
    /// metadata. Used by Lilith for "which prefixes do I have?"
    /// queries and by future settings UI.
    async fn list_prefixes(&self) -> String {
        match enumerate_prefixes() {
            Ok(items) => json!({ "prefixes": items }).to_string(),
            Err(e) => json!({ "prefixes": [], "error": e.to_string() }).to_string(),
        }
    }

    #[zbus(signal)]
    async fn process_exited(ctx: &SignalContext<'_>, pid: u32, status: i32) -> zbus::Result<()>;
}

impl CompatService {
    /// Shared body between `run_exe` and `run_exe_in`. Splits prefix
    /// bring-up, command construction, and the background watcher
    /// that emits ProcessExited.
    async fn run_in_prefix(
        &self,
        prefix_name: &str,
        path: &str,
        args: Vec<String>,
        ctx: SignalContext<'_>,
    ) -> String {
        let exe = Path::new(path);
        if !exe.exists() {
            return json!({
                "started": false,
                "reason": format!("binary not found: {path}"),
            })
            .to_string();
        }

        let prefix = match prefix_dir(prefix_name) {
            Ok(p) => p,
            Err(e) => {
                return json!({ "started": false, "reason": e.to_string() }).to_string();
            }
        };

        let initialised = prefix.join("system.reg").exists();
        if !initialised {
            let _gate = self.prefix_init.lock().await;
            if !prefix.join("system.reg").exists() {
                tracing::info!(prefix = %prefix.display(), "Initialising Wine prefix");
                if let Err(e) = wineboot_init(&prefix).await {
                    return json!({
                        "started": false,
                        "reason": format!("prefix init failed: {e}"),
                    })
                    .to_string();
                }
                write_meta(&prefix, /*touch_used=*/ false);
            }
        }

        let mut cmd = tokio::process::Command::new(WINE_BINARY);
        cmd.env("WINEPREFIX", &prefix)
            .env("WINEARCH", WINE_ARCH)
            .arg(path)
            .args(&args);

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let reason = match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        "wine not installed (no `wine` in PATH)".to_string()
                    }
                    _ => format!("spawn wine: {e}"),
                };
                return json!({ "started": false, "reason": reason }).to_string();
            }
        };

        // Stamp the last-used timestamp on every successful spawn.
        write_meta(&prefix, /*touch_used=*/ true);

        let pid = child.id().unwrap_or(0);
        tracing::info!(pid, prefix = prefix_name, %path, "Spawned wine process");

        let ctx_owned = ctx.to_owned();
        tokio::spawn(async move {
            let mut child = child;
            let status = child.wait().await;
            let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
            tracing::info!(pid, exit = code, "Wine process exited");
            if let Err(e) = Self::process_exited(&ctx_owned, pid, code).await {
                tracing::warn!("ProcessExited emit failed: {e}");
            }
        });

        json!({ "started": true, "pid": pid, "prefix": prefix_name }).to_string()
    }
}

fn wine_root() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no HOME directory"))?;
    Ok(home.join(".jarvis/wine"))
}

fn prefix_dir(name: &str) -> anyhow::Result<PathBuf> {
    Ok(wine_root()?.join(name))
}

/// Proton-GE install root. Env override first, then the canonical
/// `~/.jarvis/proton-ge/` path. The directory must contain the
/// `proton` script — that's what we exec.
fn proton_root() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(PROTON_DIR_ENV) {
        return Some(PathBuf::from(p));
    }
    let home = dirs::home_dir()?;
    Some(home.join(".jarvis/proton-ge"))
}

fn proton_binary() -> Option<PathBuf> {
    let root = proton_root()?;
    let candidate = root.join("proton");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

/// Per-prefix compat-data root for Proton. Proton's `proton` script
/// expects `$STEAM_COMPAT_DATA_PATH/pfx/` to be its WINEPREFIX, plus
/// a few siblings (`version`, `tracked_files`) it manages itself.
fn proton_data_dir(prefix_name: &str) -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no HOME directory"))?;
    Ok(home.join(".jarvis/proton-data").join(prefix_name))
}

/// Record `engine: "proton"` in the prefix meta so `list_prefixes`
/// surfaces which engine was last used. Co-located with the prefix
/// rather than the Wine-style `.jarvis-meta.json` so the two engines
/// never write to the same file.
fn write_proton_meta(compat_data: &Path) {
    let path = compat_data.join(".jarvis-meta.json");
    let now = Utc::now().to_rfc3339();
    let existing: Option<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let mut obj = match existing {
        Some(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    obj.entry("created_at")
        .or_insert_with(|| serde_json::Value::String(now.clone()));
    obj.insert("last_used_at".into(), serde_json::Value::String(now));
    obj.insert("engine".into(), serde_json::Value::String("proton".into()));
    if let Ok(bytes) = serde_json::to_vec_pretty(&serde_json::Value::Object(obj)) {
        let _ = std::fs::write(path, bytes);
    }
}

/// Prefix names are restricted to lowercase + digits + `_` / `-`, with
/// a non-`-`-leading character. Same shape as Flatpak / Docker image
/// tag basics — avoids odd shell quoting in WINEPREFIX paths.
fn is_valid_prefix_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    let mut chars = name.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

async fn wineboot_init(prefix: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(prefix).await?;
    let status = tokio::process::Command::new(WINE_BINARY)
        .env("WINEPREFIX", prefix)
        .env("WINEARCH", WINE_ARCH)
        .arg("wineboot")
        .arg("--init")
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("wineboot --init exited with {status}");
    }
    Ok(())
}

/// Persist a tiny `.jarvis-meta.json` next to the prefix so we can
/// surface created/last-used timestamps without scanning every
/// system.reg mtime. Best-effort — failure here doesn't fail the
/// caller's actual operation.
fn write_meta(prefix: &Path, touch_used: bool) {
    let path = prefix.join(".jarvis-meta.json");
    let now = Utc::now().to_rfc3339();
    let existing: Option<serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let mut obj = match existing {
        Some(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    obj.entry("created_at")
        .or_insert_with(|| serde_json::Value::String(now.clone()));
    if touch_used {
        obj.insert("last_used_at".into(), serde_json::Value::String(now));
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&serde_json::Value::Object(obj)) {
        let _ = std::fs::write(path, bytes);
    }
}

fn enumerate_prefixes() -> anyhow::Result<Vec<PrefixInfo>> {
    let mut out = Vec::new();

    // Wine prefixes — `~/.jarvis/wine/<name>/system.reg` is the marker.
    if let Ok(root) = wine_root() {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = match path.file_name().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                if !is_valid_prefix_name(&name) {
                    continue;
                }
                let initialised = path.join("system.reg").exists();
                let (created_at, last_used_at, engine) = read_prefix_meta(&path, "wine");
                out.push(PrefixInfo {
                    name,
                    path: path.to_string_lossy().into_owned(),
                    initialised,
                    created_at,
                    last_used_at,
                    engine,
                });
            }
        }
    }

    // Proton prefixes — `~/.jarvis/proton-data/<name>/pfx/system.reg`.
    // Same prefix name can exist in both engines; the listing surfaces
    // them as separate rows so the user sees both lineages.
    if let Some(home) = dirs::home_dir() {
        let root = home.join(".jarvis/proton-data");
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = match path.file_name().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                if !is_valid_prefix_name(&name) {
                    continue;
                }
                let initialised = path.join("pfx/system.reg").exists();
                let (created_at, last_used_at, engine) = read_prefix_meta(&path, "proton");
                out.push(PrefixInfo {
                    name,
                    path: path.to_string_lossy().into_owned(),
                    initialised,
                    created_at,
                    last_used_at,
                    engine,
                });
            }
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.engine.cmp(&b.engine)));
    Ok(out)
}

/// Shared meta reader for Wine and Proton prefix dirs. Falls back to
/// the caller's default-engine string when the meta is missing or
/// silent about the engine.
fn read_prefix_meta(prefix_path: &Path, default_engine: &str) -> (Option<String>, Option<String>, String) {
    let meta_path = prefix_path.join(".jarvis-meta.json");
    let parsed: Option<serde_json::Value> = std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let parsed = match parsed {
        Some(v) => v,
        None => return (None, None, default_engine.to_string()),
    };
    let created_at = parsed
        .get("created_at")
        .and_then(|x| x.as_str())
        .map(String::from);
    let last_used_at = parsed
        .get("last_used_at")
        .and_then(|x| x.as_str())
        .map(String::from);
    let engine = parsed
        .get("engine")
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_else(|| default_engine.to_string());
    (created_at, last_used_at, engine)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_compat=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Compat");

    let service = CompatService {
        prefix_init: Arc::new(AsyncMutex::new(())),
    };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.Compat")?
        .serve_at("/com/jarvis/Compat", service)?
        .build()
        .await
        .context("publish DBus service")?;

    tracing::info!("Compat ready on com.jarvis.Compat");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wine_root_under_home() {
        if let Ok(p) = wine_root() {
            let s = p.to_string_lossy();
            assert!(s.contains(".jarvis"));
            assert!(s.ends_with("wine") || s.ends_with("wine\\") || s.contains("wine"));
        }
    }

    #[test]
    fn prefix_name_validation() {
        assert!(is_valid_prefix_name("default"));
        assert!(is_valid_prefix_name("steam-games"));
        assert!(is_valid_prefix_name("ms-office_2019"));
        assert!(is_valid_prefix_name("a"));
        assert!(is_valid_prefix_name("1abc"));

        assert!(!is_valid_prefix_name(""));
        assert!(!is_valid_prefix_name("Default")); // uppercase
        assert!(!is_valid_prefix_name("-leading")); // starts with -
        assert!(!is_valid_prefix_name("with space"));
        assert!(!is_valid_prefix_name("../escape"));
        assert!(!is_valid_prefix_name(&"a".repeat(65)));
    }
}
