//! Jarvis PAM module — `pam_jarvis.so`.
//!
//! V1 is a scaffold: it ships, builds, and gets installed as a valid
//! PAM auth module, but it always returns `PAM_IGNORE` — telling the
//! stack "I have nothing to say about this auth attempt, defer to the
//! next module." That way it's safe to wire into a service's PAM
//! config (with `auth optional pam_jarvis.so`) before V2 lands the
//! actual biometric checks. No service hangs on a half-built module.
//!
//! V2 will fill in:
//!   - `voiceprint`: read `PAM_USER` and call
//!     `com.jarvis.Voice.VerifyVoiceprint(user)` over DBus; success
//!     promotes the return to `PAM_SUCCESS`, failure to `PAM_AUTH_ERR`.
//!   - `faceprint`: same shape against `com.jarvis.Voice` (or a
//!     sibling face-id daemon) once the camera capture path exists.
//!
//! See ADR 0016 for the rationale (why a PAM module instead of a
//! greeter-side check, why a single shared module instead of per-
//! biometric ones, why PAM_IGNORE in V1).

use std::os::raw::{c_char, c_int};

/// Opaque handle. Real libpam clients reach into this via
/// `pam_get_item` / `pam_set_item` — V2 will, V1 doesn't.
#[repr(C)]
pub struct PamHandle {
    _opaque: [u8; 0],
}

// Subset of the PAM return constants we care about. The full list
// lives in `/usr/include/security/_pam_types.h`. Picking only the
// ones we'll reference keeps the surface small and obviously correct.
const PAM_SUCCESS: c_int = 0;
const PAM_IGNORE: c_int = 25;
#[allow(dead_code)] // Used by V2.
const PAM_AUTH_ERR: c_int = 7;

/// PAM authentication entry point. Called by libpam when a service's
/// `auth` stack reaches a line referencing this module.
///
/// V1 contract: always return `PAM_IGNORE`. This is the safe default
/// for an unimplemented biometric — the user falls through to the
/// next module in the stack (typically `pam_unix.so`), which then
/// handles the actual password check. No regression.
///
/// # Safety
/// libpam owns `pamh` and the argv buffer. We don't dereference
/// either in V1; V2 will use libpam-supplied helpers (`pam_get_item`,
/// `pam_get_user`) instead of touching the pointers raw.
#[no_mangle]
pub unsafe extern "C" fn pam_sm_authenticate(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    PAM_IGNORE
}

/// PAM credential management — required by libpam to be exported
/// from every auth module. We have no credentials to set or refresh
/// (V2 won't either; biometric matches don't carry tokens), so this
/// is `PAM_SUCCESS` always.
///
/// # Safety
/// Same as `pam_sm_authenticate` — pointers are libpam-owned, we
/// don't touch them.
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

    /// Trivial: V1 must return PAM_IGNORE so it never disturbs the
    /// auth stack. This guards against a future "fix" that flips
    /// the V1 return to PAM_SUCCESS / PAM_AUTH_ERR without the
    /// underlying biometric check landing first.
    #[test]
    fn authenticate_returns_ignore_in_v1() {
        let r = unsafe {
            pam_sm_authenticate(std::ptr::null_mut(), 0, 0, std::ptr::null())
        };
        assert_eq!(r, PAM_IGNORE);
    }

    #[test]
    fn setcred_returns_success() {
        let r = unsafe { pam_sm_setcred(std::ptr::null_mut(), 0, 0, std::ptr::null()) };
        assert_eq!(r, PAM_SUCCESS);
    }
}
