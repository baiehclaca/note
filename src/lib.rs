//! `note` tags a folder with a short note in the OS's own folder metadata,
//! so the folder can be found by searching for the note.

#![forbid(unsafe_code)]

mod app;
mod backend;
mod cli;
mod error;
mod folder;
mod platform;
// The tree-walk search used by the Linux and Windows backends; its tests
// run everywhere.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
mod scan;

use std::ffi::OsString;

pub use error::NoteError;

/// Runs `note` with `args` (without the program name) against this OS.
pub fn run<I, S>(args: I) -> Result<(), NoteError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let action = cli::parse(args)?;
    let backend = platform::native();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    app::execute(action, &backend, &mut stdout.lock(), &mut stderr.lock())
}
