//! Picks the native [`Backend`](crate::backend::Backend) for the OS `note`
//! was built for.

// The macOS backend only shells out to `xattr`, `osascript` and `mdfind`, so
// it builds everywhere. Compiling it on every OS keeps it under clippy and
// runs its pure tests (query escaping, plist codec) on every CI machine.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub type Native = macos::MacOs;
#[cfg(not(target_os = "macos"))]
pub type Native = unsupported::Unsupported;

pub fn native() -> Native {
    Native::default()
}
