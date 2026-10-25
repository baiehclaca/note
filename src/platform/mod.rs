//! Picks the native [`Backend`](crate::backend::Backend) for the OS `note`
//! was built for.

#[cfg(target_os = "linux")]
mod linux;
// The macOS backend only shells out to `xattr`, `osascript` and `mdfind`, so
// it builds everywhere. Compiling it on every OS keeps it under clippy and
// runs its pure tests (query escaping, plist codec) on every CI machine.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
pub type Native = macos::MacOs;
#[cfg(target_os = "linux")]
pub type Native = linux::Linux;
#[cfg(windows)]
pub type Native = windows::Windows;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub type Native = unsupported::Unsupported;

pub fn native() -> Native {
    Native::default()
}
