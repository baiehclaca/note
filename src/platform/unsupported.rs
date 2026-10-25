//! Placeholder for systems without a native backend yet. Every command that
//! needs storage fails with a clear message instead of guessing.

use crate::backend::Backend;
use crate::NoteError;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy)]
pub struct Unsupported;

fn unsupported() -> NoteError {
    NoteError::Failed(
        "note stores notes as Finder comments for Spotlight, so it only works on macOS for now."
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
note tags a folder with a note you can search for. On this system it is
not supported yet; it works on macOS, where the note is the folder's
Finder comment and Spotlight searches it.
"
    }

    fn help_details(&self) -> &'static str {
        "A note can be up to 2048 bytes.\n"
    }
}
