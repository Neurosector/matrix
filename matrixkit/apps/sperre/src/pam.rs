//! Minimales PAM-FFI: die vier Funktionen, die ein Sperrschirm braucht.
//! Prüft das Passwort des ANGEMELDETEN Nutzers gegen den PAM-Dienst
//! `matrix-sperre` (der system-auth einbindet) — genau wie swaylock/
//! gtklock. Kein Crate; die C-API ist winzig und seit Jahrzehnten stabil.

use std::ffi::{c_char, c_int, c_void, CString};

const PAM_SUCCESS: c_int = 0;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}
#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}
#[repr(C)]
struct PamConv {
    conv: extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int,
    appdata_ptr: *mut c_void,
}
type PamHandle = c_void;

extern "C" {
    fn pam_start(
        service: *const c_char,
        user: *const c_char,
        conv: *const PamConv,
        pamh: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_authenticate(pamh: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(pamh: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut PamHandle, status: c_int) -> c_int;
    fn strdup(s: *const c_char) -> *mut c_char;
    fn malloc(n: usize) -> *mut c_void;
}

/// Die Konversation: PAM fragt (verdeckt), wir reichen das Passwort.
/// Die Antworten MÜSSEN mit malloc/strdup belegt werden — PAM gibt sie
/// selbst mit free() frei.
extern "C" fn konversation(
    num_msg: c_int,
    msg: *mut *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata: *mut c_void,
) -> c_int {
    if num_msg <= 0 {
        return 19; // PAM_CONV_ERR
    }
    unsafe {
        let n = num_msg as usize;
        let antworten =
            malloc(n * std::mem::size_of::<PamResponse>()) as *mut PamResponse;
        if antworten.is_null() {
            return 5; // PAM_BUF_ERR
        }
        let passwort = appdata as *const c_char;
        for i in 0..n {
            let m = *msg.add(i);
            let r = antworten.add(i);
            (*r).resp_retcode = 0;
            (*r).resp = match (*m).msg_style {
                PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON => strdup(passwort),
                _ => std::ptr::null_mut(),
            };
        }
        *resp = antworten;
    }
    PAM_SUCCESS
}

/// true, wenn Nutzer+Passwort gültig sind (auth UND Kontostatus).
pub fn pruefen(nutzer: &str, passwort: &str) -> bool {
    let dienst = CString::new("matrix-sperre").unwrap();
    let cnutzer = match CString::new(nutzer) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let cpass = match CString::new(passwort) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let conv = PamConv {
        conv: konversation,
        appdata_ptr: cpass.as_ptr() as *mut c_void,
    };
    unsafe {
        let mut pamh: *mut PamHandle = std::ptr::null_mut();
        if pam_start(dienst.as_ptr(), cnutzer.as_ptr(), &conv, &mut pamh) != PAM_SUCCESS {
            return false;
        }
        let auth = pam_authenticate(pamh, 0);
        let konto = if auth == PAM_SUCCESS {
            pam_acct_mgmt(pamh, 0)
        } else {
            auth
        };
        pam_end(pamh, konto);
        // cpass lebt bis hier — die Konversation lief synchron.
        let _ = &cpass;
        auth == PAM_SUCCESS && konto == PAM_SUCCESS
    }
}
