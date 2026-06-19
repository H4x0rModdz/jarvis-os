//! Bootc OS upgrade probe.
//!
//! Two flows:
//!
//! - **Check** — refresh the registry view with `bootc upgrade --check`, then
//!   read `bootc status --format json` and compare the **booted image digest**
//!   against the **staged / cached-update digest**. An update is available
//!   when a different digest is staged (reboot pending) or cached (ready to
//!   pull). We deliberately do NOT trust `bootc upgrade --check`'s exit code:
//!   contrary to an earlier assumption it does NOT exit 77 for "update
//!   available" — it exits 0 even when one exists, which made the UI
//!   permanently report "system up to date" while `bootc status` clearly
//!   showed a newer digest. Digest comparison is the version/locale-stable
//!   source of truth (and version strings can't be compared — dev builds all
//!   read `0.0.0-dev`).
//! - **Apply** — `bootc upgrade` pulls + stages the new image. A reboot is
//!   required to actually boot into it; bootc never reboots without
//!   `--reboot`, which we don't pass — the user decides when.
//!
//! Privilege: `bootc` requires root. The updater is a per-user daemon, so we
//! invoke `bootc` through `pkexec` (gated by the `com.jarvis.updater.bootc`
//! polkit rule — see iso/assets/polkit/). `JARVIS_UPDATER_NO_PKEXEC=1` runs
//! `bootc` directly (tests / a daemon already running as root).
//!
//! Env override:
//!   JARVIS_UPDATER_BOOTC      path to the `bootc` binary (default /usr/bin/bootc)
//!   JARVIS_UPDATER_NO_PKEXEC  skip the pkexec wrapper when set

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use tokio::process::Command;

pub const DEFAULT_BOOTC: &str = "/usr/bin/bootc";
const PKEXEC: &str = "/usr/bin/pkexec";

/// Build a `Command` that runs `bootc <args…>` with privilege. Wraps in
/// `pkexec` unless `JARVIS_UPDATER_NO_PKEXEC` is set (tests / root daemon).
fn bootc_command(binary: &PathBuf, args: &[&str]) -> Command {
    if std::env::var_os("JARVIS_UPDATER_NO_PKEXEC").is_some() {
        let mut cmd = Command::new(binary);
        cmd.args(args);
        cmd
    } else {
        let mut cmd = Command::new(PKEXEC);
        cmd.arg(binary);
        cmd.args(args);
        cmd
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsUpdateInfo {
    pub available: bool,
    /// Best-effort short description of the available image — the candidate's
    /// version, or a short digest when versions are uninformative. Surfaced
    /// verbatim to the user.
    pub version: Option<String>,
}

/// Probe whether a bootc OS update is available. Errors when the bootc binary
/// is unreachable or `bootc status` fails — callers degrade gracefully (the
/// model-pull half of the updater keeps working).
pub async fn check_update() -> Result<OsUpdateInfo> {
    let binary = env_path("JARVIS_UPDATER_BOOTC", DEFAULT_BOOTC);
    if !binary.exists() {
        return Err(anyhow!(
            "bootc binary missing at {} (set JARVIS_UPDATER_BOOTC to override)",
            binary.display()
        ));
    }

    // Best-effort: refresh the registry view so `booted.cachedUpdate` reflects
    // the latest remote. Its exit code is unreliable across bootc versions, so
    // we ignore it and read the truth from `bootc status` below. A failure
    // here (offline, etc.) just means we compare against the last cached check.
    let _ = bootc_command(&binary, &["upgrade", "--check"])
        .output()
        .await;

    let output = bootc_command(&binary, &["status", "--format", "json"])
        .output()
        .await
        .with_context(|| format!("spawn {} status", binary.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "bootc status --format json exited with {}: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    parse_status(&String::from_utf8_lossy(&output.stdout))
}

/// Decide "is an update available?" from `bootc status --format json` by
/// comparing image digests — independent of any exit-code convention.
///
/// A staged image (already pulled, reboot pending) wins over a cached update
/// (seen at the registry, not yet pulled); either counts as available when its
/// digest differs from the booted one. Missing/null fields resolve to JSON
/// null, whose `.as_str()` is `None`, so this is robust to absent sections.
fn parse_status(json: &str) -> Result<OsUpdateInfo> {
    let v: serde_json::Value =
        serde_json::from_str(json).context("parsing `bootc status --format json`")?;
    let status = &v["status"];

    let booted_digest = status["booted"]["image"]["imageDigest"]
        .as_str()
        .unwrap_or_default();

    // Best update candidate: staged (reboot pending) first, else the cached
    // registry check on the booted deployment.
    let staged = &status["staged"]["image"];
    let cached = &status["booted"]["cachedUpdate"];
    let (cand_digest, cand_version) = if let Some(d) = staged["imageDigest"].as_str() {
        (Some(d), staged["version"].as_str())
    } else if let Some(d) = cached["imageDigest"].as_str() {
        (Some(d), cached["version"].as_str())
    } else {
        (None, None)
    };

    let available = matches!(cand_digest, Some(d) if !d.is_empty() && d != booted_digest);
    let version = if available {
        cand_version
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| cand_digest.map(short_digest))
    } else {
        None
    };

    Ok(OsUpdateInfo { available, version })
}

/// `"sha256:0123456789abcdef…"` -> `"0123456789ab"` for a compact UI label.
fn short_digest(d: &str) -> String {
    d.strip_prefix("sha256:")
        .unwrap_or(d)
        .chars()
        .take(12)
        .collect()
}

/// Outcome classification for one upgrade attempt, so the caller can tell a
/// "bootc didn't like the progress argument" failure (worth retrying plainly)
/// apart from a genuine upgrade error (worth surfacing).
enum UpgradeError {
    /// bootc rejected `--progress-fd` — either it doesn't know the flag, or it
    /// refuses the fd we can hand it through pkexec. Retry without progress.
    UnsupportedProgressArg,
    /// A real upgrade failure — surface it verbatim.
    Other(anyhow::Error),
}

/// Run `bootc upgrade`, streaming real download progress when bootc supports it.
/// Pulls + stages the new image; the user reboots to apply. We deliberately do
/// NOT pass `--reboot`.
///
/// Progress: bootc writes JSON-lines progress events to a file descriptor given
/// by `--progress-fd`. pkexec only forwards fds 0/1/2 to the privileged process,
/// and bootc **refuses fd 1** ("Cannot use fd 1 for progress JSON"), so we use
/// **fd 2 (stderr)** — the one usable stream. We parse each line into
/// `(percent, description)` so the UI shows a real bar instead of a frozen
/// "Pulling…". `percent` is -1 for indeterminate phases; duplicates are coalesced.
///
/// Resilience: a stricter/older bootc may reject `--progress-fd` entirely. We
/// detect that and fall back to a plain `bootc upgrade` (indeterminate progress)
/// so the upgrade always applies — losing only the live percentage.
pub async fn apply_upgrade(
    progress: tokio::sync::mpsc::UnboundedSender<(i32, String)>,
) -> Result<()> {
    let binary = env_path("JARVIS_UPDATER_BOOTC", DEFAULT_BOOTC);
    if !binary.exists() {
        return Err(anyhow!(
            "bootc binary missing at {} (set JARVIS_UPDATER_BOOTC to override)",
            binary.display()
        ));
    }

    match run_upgrade(&binary, true, &progress).await {
        Ok(()) => Ok(()),
        Err(UpgradeError::UnsupportedProgressArg) => {
            tracing::warn!("bootc rejected --progress-fd; falling back to a plain upgrade");
            // Indeterminate so the UI still reads as "working".
            let _ = progress.send((-1, "Baixando atualização…".to_string()));
            match run_upgrade(&binary, false, &progress).await {
                Ok(()) => Ok(()),
                // The plain attempt can't hit the progress-arg path.
                Err(UpgradeError::UnsupportedProgressArg) => {
                    Err(anyhow!("bootc upgrade failed unexpectedly"))
                }
                Err(UpgradeError::Other(e)) => Err(e),
            }
        }
        Err(UpgradeError::Other(e)) => Err(e),
    }
}

/// One `bootc upgrade` attempt. With `with_progress`, asks bootc to stream JSON
/// progress on fd 2 and relays it over `progress`; otherwise runs plain. bootc's
/// output (progress feed and/or errors) comes on stderr either way, so we always
/// capture it — both to parse progress and to give a real error message.
async fn run_upgrade(
    binary: &PathBuf,
    with_progress: bool,
    progress: &tokio::sync::mpsc::UnboundedSender<(i32, String)>,
) -> Result<(), UpgradeError> {
    use tokio::io::AsyncBufReadExt;

    let mut cmd = if with_progress {
        bootc_command(binary, &["upgrade", "--quiet", "--progress-fd", "2"])
    } else {
        bootc_command(binary, &["upgrade"])
    };
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))
        .map_err(UpgradeError::Other)?;

    // Keep the tail of non-progress stderr lines for error context.
    let mut stderr_tail: Vec<String> = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        let mut last: Option<(i32, String)> = None;
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|e| UpgradeError::Other(e.into()))?
        {
            if with_progress {
                if let Some(update) = parse_progress_line(&line) {
                    if last.as_ref() != Some(&update) {
                        let _ = progress.send(update.clone());
                        last = Some(update);
                    }
                    continue;
                }
            }
            stderr_tail.push(line);
            if stderr_tail.len() > 12 {
                stderr_tail.remove(0);
            }
        }
    }

    let status = child
        .wait()
        .await
        .context("wait for bootc upgrade")
        .map_err(UpgradeError::Other)?;
    if status.success() {
        return Ok(());
    }

    let stderr_text = stderr_tail.join("\n");
    if with_progress && is_progress_arg_error(status.code(), &stderr_text) {
        return Err(UpgradeError::UnsupportedProgressArg);
    }
    Err(UpgradeError::Other(anyhow!(
        "bootc upgrade exited with {status}{}",
        if stderr_text.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", stderr_text.trim())
        }
    )))
}

/// Did bootc fail because it didn't accept the `--progress-fd` argument (unknown
/// flag, or a refused fd like the "Cannot use fd 1 for progress JSON" guard)
/// rather than a real upgrade error? clap exits 2 on argument errors.
fn is_progress_arg_error(code: Option<i32>, stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    code == Some(2)
        || s.contains("progress-fd")
        || s.contains("unexpected argument")
        || s.contains("invalid value")
}

/// Parse one JSON line from `bootc upgrade --progress-fd` into
/// `(percent, description)`. `percent` is -1 for indeterminate events. Returns
/// `None` for lines we don't surface (e.g. the `Start` banner) or non-JSON.
fn parse_progress_line(line: &str) -> Option<(i32, String)> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let pct = |done: u64, total: u64| -> i32 {
        if total == 0 {
            -1
        } else {
            ((done.min(total) as f64 / total as f64) * 100.0).round() as i32
        }
    };
    match v.get("type")?.as_str()? {
        "ProgressBytes" => {
            let done = v.get("bytes")?.as_u64()?;
            let total = v.get("bytes_total")?.as_u64()?;
            let desc = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("Baixando atualização")
                .to_string();
            Some((pct(done, total), desc))
        }
        "ProgressSteps" => {
            let done = v.get("steps")?.as_u64()?;
            let total = v.get("steps_total")?.as_u64()?;
            let desc = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("Preparando")
                .to_string();
            Some((pct(done, total), desc))
        }
        _ => None,
    }
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

    #[test]
    fn parse_progress_bytes_computes_percent() {
        let line = r#"{"type":"ProgressBytes","task":"pulling","description":"Fetching layers","bytes":50,"bytes_total":200,"steps":0,"steps_total":0,"subtasks":[]}"#;
        assert_eq!(
            parse_progress_line(line),
            Some((25, "Fetching layers".to_string()))
        );
    }

    #[test]
    fn parse_progress_steps_and_indeterminate() {
        let steps =
            r#"{"type":"ProgressSteps","description":"Deploying","steps":1,"steps_total":4}"#;
        assert_eq!(
            parse_progress_line(steps),
            Some((25, "Deploying".to_string()))
        );
        // total 0 → indeterminate (-1)
        let zero = r#"{"type":"ProgressBytes","description":"x","bytes":0,"bytes_total":0}"#;
        assert_eq!(parse_progress_line(zero), Some((-1, "x".to_string())));
    }

    #[test]
    fn progress_arg_error_detected() {
        // The exact message bootc prints when it refuses the fd we pass.
        assert!(is_progress_arg_error(
            Some(2),
            "error: invalid value '1' for '--progress-fd <PROGRESS_FD>': Cannot use fd 1 for progress JSON"
        ));
        // Unknown-flag shape (older bootc without the option at all).
        assert!(is_progress_arg_error(
            Some(2),
            "error: unexpected argument '--progress-fd' found"
        ));
    }

    #[test]
    fn real_upgrade_failure_is_not_an_arg_error() {
        // A genuine pull/deploy failure (exit 1, no arg-parsing text) must NOT
        // be mistaken for an unsupported-argument case, or we'd retry pointlessly.
        assert!(!is_progress_arg_error(
            Some(1),
            "error: failed to pull image: connection refused"
        ));
        assert!(!is_progress_arg_error(None, ""));
    }

    #[test]
    fn parse_progress_ignores_start_and_junk() {
        assert_eq!(
            parse_progress_line(r#"{"type":"Start","version":"0.1.0"}"#),
            None
        );
        assert_eq!(parse_progress_line("not json"), None);
    }

    #[test]
    fn parse_status_detects_staged_update() {
        // Real shape: a staged image whose digest differs from booted means an
        // update is ready (reboot pending) — even though both report the same
        // "0.0.0-dev" version. This is exactly the case the old exit-code path
        // mis-reported as "up to date".
        let json = r#"{ "status": {
            "booted": {
                "image": { "imageDigest": "sha256:AAAAAAAAAAAA", "version": "0.0.0-dev" },
                "cachedUpdate": { "imageDigest": "sha256:BBBBBBBBBBBB", "version": "0.0.0-dev" }
            },
            "staged": { "image": { "imageDigest": "sha256:BBBBBBBBBBBB", "version": "0.0.0-dev" } }
        }}"#;
        let info = parse_status(json).unwrap();
        assert!(info.available);
        assert_eq!(info.version.as_deref(), Some("0.0.0-dev"));
    }

    #[test]
    fn parse_status_cached_only_update() {
        // No staged deployment yet, but the registry check cached a newer
        // digest -> available (ready to pull).
        let json = r#"{ "status": {
            "booted": {
                "image": { "imageDigest": "sha256:AAAAAAAAAAAA", "version": "0.0.0-dev" },
                "cachedUpdate": { "imageDigest": "sha256:CCCCCCCCCCCC", "version": "0.0.0-dev" }
            },
            "staged": null
        }}"#;
        let info = parse_status(json).unwrap();
        assert!(info.available);
    }

    #[test]
    fn parse_status_up_to_date_when_no_candidate() {
        let json = r#"{ "status": {
            "booted": { "image": { "imageDigest": "sha256:AAAAAAAAAAAA" }, "cachedUpdate": null },
            "staged": null
        }}"#;
        let info = parse_status(json).unwrap();
        assert!(!info.available);
        assert!(info.version.is_none());
    }

    #[test]
    fn parse_status_same_digest_is_not_an_update() {
        // The bug guard: a cached entry with the SAME digest as booted (and the
        // same version string) must NOT count as an update.
        let json = r#"{ "status": {
            "booted": {
                "image": { "imageDigest": "sha256:AAAAAAAAAAAA", "version": "0.0.0-dev" },
                "cachedUpdate": { "imageDigest": "sha256:AAAAAAAAAAAA", "version": "0.0.0-dev" }
            },
            "staged": null
        }}"#;
        let info = parse_status(json).unwrap();
        assert!(!info.available);
    }
}
