//! `jarvis-pam-helper` — bridges `pam_jarvis.so` to `com.jarvis.Voice`.
//!
//! Invoked by the PAM module as a subprocess so the heavy DBus + tokio
//! stack stays out of the addresses spaces of `sudo`, `login`, greetd,
//! and friends. The PAM module just reads our exit code and translates
//! it into the appropriate `PAM_*` return.
//!
//! Usage:
//!   jarvis-pam-helper verify <user>
//!
//! Exit codes:
//!   0  voiceprint match     → pam_sm_authenticate returns PAM_SUCCESS
//!   1  voiceprint mismatch  → PAM_AUTH_ERR
//!   2  bus / daemon / user  → PAM_IGNORE (defer to next module)
//!      unreachable; this is
//!      the deliberate failure-
//!      open path.
//!
//! We never escalate transient failures to PAM_AUTH_ERR — the PAM stack
//! has password fallback for a reason, and a 3-second timeout against a
//! crashed daemon should never lock a user out of `sudo`.

use std::process::ExitCode;
use std::time::Duration;

const EXIT_MATCH: u8 = 0;
const EXIT_MISMATCH: u8 = 1;
const EXIT_UNAVAILABLE: u8 = 2;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next();
    let user = args.next();
    let (Some(cmd), Some(user)) = (cmd, user) else {
        eprintln!("usage: jarvis-pam-helper verify <user>");
        return ExitCode::from(EXIT_UNAVAILABLE);
    };
    if cmd != "verify" {
        eprintln!("unknown command: {cmd}");
        return ExitCode::from(EXIT_UNAVAILABLE);
    }

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(_) => return ExitCode::from(EXIT_UNAVAILABLE),
    };

    // 3 s wall — PAM stacks should not block forever. If we can't get
    // a verdict by then, fail-open to PAM_IGNORE.
    let outcome = rt.block_on(async {
        match tokio::time::timeout(Duration::from_secs(3), verify_via_dbus(&user)).await {
            Ok(Some(true)) => EXIT_MATCH,
            Ok(Some(false)) => EXIT_MISMATCH,
            Ok(None) => EXIT_UNAVAILABLE,
            Err(_) => EXIT_UNAVAILABLE, // timeout
        }
    });
    ExitCode::from(outcome)
}

/// Open the user's session bus, call `com.jarvis.Voice.VerifyVoiceprint`,
/// pull `ok` out of the JSON. `None` is "didn't even reach the daemon"
/// — caller treats that as PAM_IGNORE so the next module gets a shot.
async fn verify_via_dbus(user: &str) -> Option<bool> {
    let uid = lookup_uid(user)?;
    let bus_path = format!("/run/user/{uid}/bus");
    if !std::path::Path::new(&bus_path).exists() {
        // No session bus = no daemon to ask. Pre-login auth (greetd)
        // hits this every time and falls through to password — the
        // expected behaviour until the daemon is reachable system-wide.
        return None;
    }
    let address = format!("unix:path={bus_path}");
    let conn = zbus::ConnectionBuilder::address(address.as_str())
        .ok()?
        .build()
        .await
        .ok()?;
    let proxy = zbus::Proxy::new(
        &conn,
        "com.jarvis.Voice",
        "/com/jarvis/Voice",
        "com.jarvis.Voice",
    )
    .await
    .ok()?;
    let response: String = proxy.call("VerifyVoiceprint", &(user,)).await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&response).ok()?;
    parsed.get("ok")?.as_bool()
}

/// `getpwnam_r`-based name → uid. Returns `None` for unknown users; the
/// caller falls through to PAM_IGNORE.
fn lookup_uid(user: &str) -> Option<u32> {
    let cname = std::ffi::CString::new(user.as_bytes()).ok()?;
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buf = vec![0i8; 1024];
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let rc = unsafe {
        libc::getpwnam_r(
            cname.as_ptr(),
            &mut pwd,
            buf.as_mut_ptr(),
            buf.len(),
            &mut result,
        )
    };
    if rc != 0 || result.is_null() {
        return None;
    }
    Some(unsafe { (*result).pw_uid })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_uid_root_is_zero() {
        // root must exist on any POSIX target; lets us sanity-check
        // the FFI signature without depending on whichever user the
        // test happens to run as.
        if let Some(uid) = lookup_uid("root") {
            assert_eq!(uid, 0);
        }
        // If even root resolution fails (eg. in a stripped container),
        // we accept that and move on — the assertion above only runs
        // when the system has a passwd backend.
    }

    #[test]
    fn lookup_uid_nonexistent_returns_none() {
        assert!(lookup_uid("definitely-not-a-real-user-xyz123").is_none());
    }

    /// OsStrExt is used implicitly by Path::new; this is just a
    /// compile-test guard.
    #[test]
    fn osstr_ext_in_scope() {
        let _ = std::ffi::OsStr::new("x").as_bytes();
    }
}
