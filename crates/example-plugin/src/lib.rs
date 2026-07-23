//! A minimal real plugin, compiled as a `cdylib` (`plugin.so`, per
//! Plugins.md SS4's plugin directory layout). Exists to prove
//! `plugin_manager::dynamic`'s loader genuinely works against a real
//! compiled shared library, not just an idea of one.
//!
//! Exposes a plain C ABI (`extern "C"` functions, C strings) rather than
//! any Rust-level type: Rust has no stable ABI across compiler versions
//! or even different builds of the same version, so a real dynamic
//! plugin boundary has to go through something both sides can agree on
//! unconditionally. See `plugin_manager::dynamic`'s module docs for the
//! fuller rationale.

use std::ffi::{c_char, CString};

#[no_mangle]
pub extern "C" fn plugin_name() -> *mut c_char {
    CString::new("example-plugin")
        .expect("no interior NUL")
        .into_raw()
}

#[no_mangle]
pub extern "C" fn plugin_version() -> *mut c_char {
    CString::new(env!("CARGO_PKG_VERSION"))
        .expect("no interior NUL")
        .into_raw()
}

/// Every string returned by this plugin must be freed through this
/// function -- it was allocated by this library's allocator, and
/// freeing it with the *caller's* allocator instead (which is what
/// happens if a host tries to free it directly) is undefined behavior
/// whenever the two aren't guaranteed to be the same allocator.
#[no_mangle]
pub extern "C" fn plugin_free_string(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: `s` must have come from `CString::into_raw` in one of
        // this module's functions, and must not already have been freed.
        // Both are upheld by every caller in this codebase (`dynamic.rs`
        // frees each returned string exactly once, immediately after
        // copying its contents out).
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}
