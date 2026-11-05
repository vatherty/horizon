//! A few shell lines to the Apple System Log (`os_log`), so the start of a run and
//! the reason it ended show in Console.app without digging the log file out of the
//! app's Documents folder. The game's own lines go to `logs/latest.log`
//! (`logging::init`); this is only for this shell's own lifecycle lines, before and
//! around the game loop.
//!
//! `os_log` lives in libSystem, which every iOS binary links already, so no extra
//! library is needed: the two functions below are declared straight from
//! `<os/log.h>`. The format is `%{public}s`, which prints the message in release
//! builds too (plain `%s` would be redacted to `<private>` in the log).

use std::ffi::{CString, c_char, c_void};
use std::sync::OnceLock;

/// `os_log_type_t`: 0 default, 1 info, 2 debug, 16 error, 17 fault.
const OS_LOG_TYPE_INFO: u8 = 1;
const OS_LOG_TYPE_ERROR: u8 = 16;

unsafe extern "C" {
    fn os_log_create(subsystem: *const c_char, category: *const c_char) -> *mut c_void;
    fn os_log_with_type(log: *mut c_void, type_: u8, format: *const c_char, ...);
}

/// The log object as an address: a raw pointer is not `Sync`, so it cannot sit in
/// a `static` itself, but its address (a `usize`) can.
static LOG: OnceLock<usize> = OnceLock::new();

fn log_object() -> *mut c_void {
    let address = LOG.get_or_init(|| {
        let subsystem = c"dev.horizon.ios";
        let category = c"shell";
        // SAFETY: both pointers are NUL-terminated strings that outlive the call,
        // and `os_log_create` returns a retained log object (or NULL, which
        // `os_log_with_type` accepts as "use the default log").
        unsafe { os_log_create(subsystem.as_ptr(), category.as_ptr()) as usize }
    });
    *address as *mut c_void
}

fn write(type_: u8, message: &str) {
    let Ok(text) = CString::new(message.replace('\0', " ")) else {
        return;
    };
    // SAFETY: the format is a constant `%{public}s` with exactly one C-string
    // argument, which is what `os_log` expects for that specifier.
    unsafe {
        os_log_with_type(log_object(), type_, c"%{public}s".as_ptr(), text.as_ptr());
    }
}

/// A shell lifecycle line at info level.
pub fn info(message: &str) {
    write(OS_LOG_TYPE_INFO, message);
}

/// A shell lifecycle line at error level.
pub fn error(message: &str) {
    write(OS_LOG_TYPE_ERROR, message);
}
