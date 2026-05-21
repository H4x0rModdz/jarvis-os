//! Jarvis PAM module — `pam_jarvis.so`.
//!
//! V2: reads `PAM_USER` from the libpam handle, execs
//! `/usr/libexec/jarvis-pam-helper verify <user>`, translates the
//! helper's exit code into the matching `PAM_*` return.
//!
//! The PAM module itself stays minimal. All the DBus / tokio / zbus
//! plumbing lives in the helper binary, so `sudo` doesn't load a
//! 5 MB stack into its address space just to find out the user
//! hasn't enrolled a voiceprint yet.
//!
//! ## Return-code policy
//!
//! - helper exits 0 (match)         → `PAM_SUCCESS`
//! - helper exits 1 (mismatch)      → `PAM_AUTH_ERR`
//! - helper exits 2 (unavailable)   → `PAM_IGNORE` (defer to next module)
//! - helper missing / fails to spawn → `PAM_IGNORE`
//!
//! Failure-open on transient errors is deliberate: a crashed daemon,
//! a stripped chroot, or a pre-login PAM stack (no session bus yet)
//! should never lock the user out. The PAM service-config file
//! decides whether to make the voiceprint check `sufficient` (skip
//! password on match) or `required` (still need password too).
//!
//! See ADR 0019 for the full design rationale.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

/// Opaque libpam handle; we never dereference the pointer ourselves.
#[repr(C)]
pub struct PamHandle {
    _opaque: [u8; 0],
}

// Subset of the PAM return constants we care about.
const PAM_SUCCESS: c_int = 0;
const PAM_IGNORE: c_int = 25;
const PAM_AUTH_ERR: c_int = 7;

const HELPER_PATH: &str = "/usr/libexec/jarvis-pam-helper";

extern "C" {
    /// libpam supplies this; we don't link it ourselves — libpam
    /// resolves the symbol when it `dlopen`s us.
    fn pam_get_user(
        pamh: *mut PamHandle,
        user_ptr: *mut *const c_char,
        prompt: *const c_char,
    ) -> c_int;
}

/// PAM authentication entry point.
///
/// # Safety
/// libpam guarantees `pamh` is a valid PamHandle pointer; we treat it
/// strictly as an opaque token (only pass it back to libpam).
#[no_mangle]
pub unsafe extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    if pamh.is_null() {
        return PAM_IGNORE;
    }

    // ── PAM_USER ─────────────────────────────────────────────────────
    let mut user_ptr: *const c_char = std::ptr::null();
    let rc = pam_get_user(pamh, &mut user_ptr, std::ptr::null());
    if rc != PAM_SUCCESS || user_ptr.is_null() {
        return PAM_IGNORE;
    }
    let user = match CStr::from_ptr(user_ptr).to_str() {
        Ok(s) if !s.is_empty() => s,
        _ => return PAM_IGNORE,
    };

    // ── Helper subprocess ────────────────────────────────────────────
    let user_c = match CString::new(user) {
        Ok(c) => c,
        Err(_) => return PAM_IGNORE,
    };
    let helper_path = CString::new(HELPER_PATH).expect("static literal cstring");
    let arg_verify = CString::new("verify").expect("static literal cstring");
    let helper_basename = CString::new("jarvis-pam-helper").expect("static literal cstring");

    let argv: [*const c_char; 4] = [
        helper_basename.as_ptr(),
        arg_verify.as_ptr(),
        user_c.as_ptr(),
        std::ptr::null(),
    ];

    // fork + execvp + waitpid. Manual rather than using std::process
    // so we don't pull a tokio/std-process runtime into the PAM
    // address space — keeps the dynamic-library footprint small for
    // services that load us frequently (sudo, login).
    let pid = libc::fork();
    if pid < 0 {
        return PAM_IGNORE;
    }
    if pid == 0 {
        // Child: silence stdio (PAM doesn't expect chatter on our fd),
        // then exec. /dev/null swap is best-effort.
        let devnull = libc::open(b"/dev/null\0".as_ptr() as *const c_char, libc::O_RDWR);
        if devnull >= 0 {
            libc::dup2(devnull, 0);
            libc::dup2(devnull, 1);
            libc::dup2(devnull, 2);
            if devnull > 2 {
                libc::close(devnull);
            }
        }
        libc::execv(helper_path.as_ptr(), argv.as_ptr() as *const *const c_char);
        // exec failed — exit "unavailable" so the parent picks PAM_IGNORE.
        libc::_exit(2);
    }

    // Parent: wait, examine exit code.
    let mut status: c_int = 0;
    let waited = libc::waitpid(pid, &mut status, 0);
    if waited < 0 {
        return PAM_IGNORE;
    }
    if !libc::WIFEXITED(status) {
        return PAM_IGNORE;
    }
    match libc::WEXITSTATUS(status) {
        0 => PAM_SUCCESS,
        1 => PAM_AUTH_ERR,
        _ => PAM_IGNORE,
    }
}

/// PAM credential management. We have no credentials to set; required
/// by libpam to be exported.
///
/// # Safety
/// Pointers are libpam-owned; we don't touch them.
#[no_mangle]
pub unsafe extern "C" fn pam_sm_setcred(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V2 unit-tests can only exercise the safety-net paths since the
    /// real auth call requires a live PAM handle. The null-pamh case
    /// must always return PAM_IGNORE so accidental misuse is safe.
    #[test]
    fn authenticate_null_pamh_returns_ignore() {
        let r = unsafe { pam_sm_authenticate(std::ptr::null_mut(), 0, 0, std::ptr::null()) };
        assert_eq!(r, PAM_IGNORE);
    }

    #[test]
    fn setcred_returns_success() {
        let r = unsafe { pam_sm_setcred(std::ptr::null_mut(), 0, 0, std::ptr::null()) };
        assert_eq!(r, PAM_SUCCESS);
    }
}
