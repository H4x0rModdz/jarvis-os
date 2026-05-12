//! Bootc OS upgrade probe.
//!
//! Two flows:
//!
//! - **Check** — `bootc upgrade --check` exits 77 when a newer image
//!   has been staged in the registry, 0 when up-to-date. We translate
//!   that into a structured `OsUpdateInfo`.
//! - **Apply** — `bootc upgrade` pulls + stages the new image. A reboot
//!   is required to actually boot into it; bootc itself never reboots
//!   without an explicit `--reboot` flag, which we don't pass — the
//!   user (or a future post-completion confirm dialog) is the one to
//!   decide when to reboot.
//!
//! Env override:
//!   JARVIS_UPDATER_BOOTC   path to the `bootc` binary (default
//!                          `/usr/bin/bootc`)

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use tokio::process::Command;

pub const DEFAULT_BOOTC: &str = "/usr/bin/bootc";

/// bootc's documented "an update is available" exit code from
/// `upgrade --check`. Anything else we treat as "no update".
const BOOTC_UPDATE_AVAILABLE: i32 = 77;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsUpdateInfo {
    pub available: bool,
    /// Best-effort short description of what bootc reported. Often
    /// includes the image digest or version; opaque enough that we
    /// surface it verbatim to the user.
    pub version: Option<String>,
}

/// Probe whether a bootc OS update is staged. Errors when the bootc
/// binary itself is unreachable (which is a Phase 3 concern — we
/// degrade gracefully, the model-pull half of the updater keeps
/// working).
pub async fn check_update() -> Result<OsUpdateInfo> {
    let binary = env_path("JARVIS_UPDATER_BOOTC", DEFAULT_BOOTC);
    if !binary.exists() {
        return Err(anyhow!(
            "bootc binary missing at {} (set JARVIS_UPDATER_BOOTC to override)",
            binary.display()
        ));
    }

    let output = Command::new(&binary)
        .args(["upgrade", "--check"])
        .output()
        .await
        .with_context(|| format!("spawn {}", binary.display()))?;

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    match code {
        0 => Ok(OsUpdateInfo {
            available: false,
            version: None,
        }),
        BOOTC_UPDATE_AVAILABLE => {
            // bootc prints something like "Update available: ..." — pull
            // out the most informative non-empty line as version, fallback
            // to the whole stdout.
            let version = stdout
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(String::from);
            Ok(OsUpdateInfo {
                available: true,
                version,
            })
        }
        other => Err(anyhow!(
            "bootc upgrade --check exited with {other}: {}",
            stderr.trim()
        )),
    }
}

/// Run `bootc upgrade`. Pulls + stages the new image; the user reboots
/// to apply. We deliberately do NOT pass `--reboot` — that's the user's
/// decision, surfaced after `Completed`.
pub async fn apply_upgrade() -> Result<()> {
    let binary = env_path("JARVIS_UPDATER_BOOTC", DEFAULT_BOOTC);
    if !binary.exists() {
        return Err(anyhow!(
            "bootc binary missing at {} (set JARVIS_UPDATER_BOOTC to override)",
            binary.display()
        ));
    }

    let status = Command::new(&binary)
        .arg("upgrade")
        .status()
        .await
        .with_context(|| format!("spawn {}", binary.display()))?;

    if !status.success() {
        return Err(anyhow!("bootc upgrade exited with {status}"));
    }
    Ok(())
}

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn errors_when_bootc_missing() {
        std::env::set_var("JARVIS_UPDATER_BOOTC", "/does/not/exist/bootc");
        let r = check_update().await;
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("bootc binary missing"));
        std::env::remove_var("JARVIS_UPDATER_BOOTC");
    }

    #[test]
    fn update_info_shape() {
        let info = OsUpdateInfo {
            available: true,
            version: Some("digest:abc".into()),
        };
        assert!(info.available);
        assert_eq!(info.version.as_deref(), Some("digest:abc"));
    }
}
