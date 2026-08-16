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
///
/// `unsafe extern "C" fn`, not just `extern "C" fn`: calling this with
/// anything other than a pointer this module itself handed out via
/// `CString::into_raw`, or calling it twice on the same pointer, is
/// undefined behavior that Rust's own type system can't rule out --
/// `clippy::not_unsafe_ptr_arg_deref` catches exactly this shape (a
/// public function taking a raw pointer it goes on to dereference
/// without being marked `unsafe` itself). `plugin_manager::dynamic`
/// already declared this symbol's expected type as `Symbol<unsafe
/// extern "C" fn(*mut c_char)>` (its own call to it was already inside
/// an `unsafe` block, since calling anything through `libloading::
/// Symbol` requires that regardless of the callee's own signature), so
/// this brings the exported function's actual signature in line with
/// what its only real caller already assumed rather than changing any
/// calling convention.
///
/// # Safety
///
/// `s` must be a pointer previously returned by `CString::into_raw`
/// from one of this module's functions (`plugin_name`/
/// `plugin_version`), and must not have already been passed to this
/// function once (double-free). A null pointer is explicitly allowed --
/// handled as a no-op below.
#[no_mangle]
pub unsafe extern "C" fn plugin_free_string(s: *mut c_char) {
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
