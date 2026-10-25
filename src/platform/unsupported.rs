//! Placeholder for systems without a native backend yet. Every command that
//! needs storage fails with a clear message instead of guessing.

use crate::backend::Backend;
use crate::NoteError;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy)]
pub struct Unsupported;

fn unsupported() -> NoteError {
    NoteError::Failed(
        "note has no way to store folder notes on this operating system yet. It works on macOS, Linux and Windows."
            .into(),
    )
}

impl Backend for Unsupported {
    fn read(&self, _folder: &Path) -> Result<Option<String>, NoteError> {
        Err(unsupported())
    }

    fn write(&self, _folder: &Path, _text: &str) -> Result<Vec<String>, NoteError> {
        Err(unsupported())
    }

    fn clear(&self, _folder: &Path) -> Result<(), NoteError> {
        Err(unsupported())
    }

    fn find(&self, _query: &str, _only_in: Option<&Path>) -> Result<Vec<PathBuf>, NoteError> {
        Err(unsupported())
    }

    fn help_intro(&self) -> &'static str {
        "\
note tags a folder with a note you can search for. This operating system
is not supported yet; note works on macOS (Finder comment, searched by
Spotlight), Linux (user.xdg.comment extended attribute) and Windows (NTFS
alternate data stream).
"
    }

    fn help_details(&self) -> &'static str {
        "A note can be up to 2048 bytes.\n"
    }
}
