//! Loading a real `plugin.so` (Plugins.md SS4's plugin directory layout)
//! via `dlopen`, using [`libloading`].
//!
//! This is deliberately narrower than [`crate::plugin::Plugin`]: it
//! queries two plain C strings (name, version) across the dynamic
//! boundary, nothing more. Rust has no stable ABI across compiler
//! versions or even different dependency graphs of the same version, so
//! passing a `Box<dyn Plugin>` (a Rust trait object, with a
//! compiler-internal vtable layout) across a `dlopen`ed boundary isn't
//! safely guaranteed to work unless the plugin and host happen to share
//! an exactly compatible build -- fragile in exactly the way a
//! "third-party plugin" ecosystem can't assume. A plain C ABI (`extern
//! "C"` functions, C strings, freed by the same allocator that allocated
//! them) is the boundary both sides can actually agree on unconditionally,
//! which is why it's what this module uses, and why it only goes as far
//! as metadata for now: extending it to carry richer data (tool
//! definitions, for instance) means designing a `#[repr(C)]` vtable
//! deliberately, which is real, valuable future work, not something to
//! improvise inline here.
//!
//! This *is* fully real and tested, unlike Phase 2/4's Slint/Candle gaps:
//! loading a compiled `.so` via `dlopen` is a Linux dynamic-linking
//! capability, not a toolchain or model-hosting limitation, so this
//! sandbox can build the example plugin, load it, and verify it end to
//! end.

use common::{AppError, AppResult};
use libloading::{Library, Symbol};
use std::ffi::{c_char, CStr};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicPluginInfo {
    pub name: String,
    pub version: String,
}

/// Loads `path` and queries its `plugin_name`/`plugin_version` exports.
pub fn load_dynamic_plugin_info(path: &Path) -> AppResult<DynamicPluginInfo> {
    // SAFETY: `Library::new` executes the shared object's initializers,
    // which is inherently trusting whatever code is in that file -- the
    // same trust boundary as running any other native code. The plugin
    // directory layout (Plugins.md SS4) is meant to be populated by
    // plugins the user has deliberately installed, the same trust model
    // as installing any other native package.
    let library = unsafe {
        Library::new(path).map_err(|e| AppError::Module {
            module: "plugin-manager",
            message: format!("loading {}: {e}", path.display()),
        })?
    };

    // SAFETY: both symbols are declared as taking no arguments and
    // returning an owned, NUL-terminated C string the callee allocated
    // (matching example-plugin's actual signatures); `call_c_string_fn`
    // frees each string exactly once via the plugin's own
    // `plugin_free_string`, immediately after copying its contents out.
    let name = unsafe { call_c_string_fn(&library, b"plugin_name\0")? };
    let version = unsafe { call_c_string_fn(&library, b"plugin_version\0")? };

    Ok(DynamicPluginInfo { name, version })
}

/// # Safety
/// `symbol` must name a real `extern "C" fn() -> *mut c_char` exported by
/// `library`, and the library must also export a `plugin_free_string`
/// matching `example-plugin`'s -- the exact contract every plugin using
/// this loader is expected to uphold.
unsafe fn call_c_string_fn(library: &Library, symbol: &[u8]) -> AppResult<String> {
    let func: Symbol<unsafe extern "C" fn() -> *mut c_char> =
        library.get(symbol).map_err(|e| AppError::Module {
            module: "plugin-manager",
            message: format!("missing symbol {:?}: {e}", String::from_utf8_lossy(symbol)),
        })?;
    let free_func: Symbol<unsafe extern "C" fn(*mut c_char)> = library
        .get(b"plugin_free_string\0")
        .map_err(|e| AppError::Module {
            module: "plugin-manager",
            message: format!("missing plugin_free_string: {e}"),
        })?;

    let ptr = func();
    if ptr.is_null() {
        return Err(AppError::Module {
            module: "plugin-manager",
            message: "plugin function returned a null string".to_string(),
        });
    }
    let value = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    free_func(ptr);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cargo builds `example-plugin` (a workspace dev-dependency) to a
    /// predictable path in the shared workspace `target/` directory
    /// before this test runs.
    fn example_plugin_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/libexample_plugin.so")
    }

    #[test]
    fn loads_a_real_compiled_plugin_and_reads_its_metadata() {
        let path = example_plugin_path();
        assert!(
            path.exists(),
            "expected {} to exist -- is example-plugin built?",
            path.display()
        );

        let info = load_dynamic_plugin_info(&path).unwrap();
        assert_eq!(info.name, "example-plugin");
        assert_eq!(info.version, "0.1.0");
    }

    #[test]
    fn loading_a_nonexistent_file_is_a_clear_error() {
        let result = load_dynamic_plugin_info(Path::new("/definitely/not/a/real/plugin.so"));
        assert!(result.is_err());
    }

    #[test]
    fn loading_a_file_missing_the_expected_symbols_is_a_clear_error() {
        // Any real, loadable .so without our specific exports should
        // fail cleanly on the symbol lookup rather than crash. libc.so.6
        // is guaranteed present on any Linux system that could run this
        // binary at all.
        for candidate in [
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/lib64/libc.so.6",
            "/usr/lib/libc.so.6",
        ] {
            let path = Path::new(candidate);
            if path.exists() {
                assert!(load_dynamic_plugin_info(path).is_err());
                return;
            }
        }
        panic!("no libc.so.6 found in any expected location -- update the candidate list for this system");
    }

    #[test]
    fn loading_twice_works_independently() {
        let path = example_plugin_path();
        let first = load_dynamic_plugin_info(&path).unwrap();
        let second = load_dynamic_plugin_info(&path).unwrap();
        assert_eq!(first, second);
    }
}
