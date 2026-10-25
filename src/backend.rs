//! The OS layer. Each platform stores a folder note in its own native place
//! and searches it its own way; the rest of `note` only talks to [`Backend`].

use crate::NoteError;
use std::path::{Path, PathBuf};

/// Where notes live on one OS.
///
/// `folder` is always an existing, canonical folder (see
/// [`crate::folder::resolve_folder`]) and `text` passed to [`Backend::write`]
/// is already trimmed and validated (see [`crate::folder::validate_note`]).
pub trait Backend {
    /// The note on `folder`, or `None` when there is none.
    fn read(&self, folder: &Path) -> Result<Option<String>, NoteError>;

    /// Stores `text` as the note on `folder`. Returns warnings to show the
    /// user when the note was saved but something about it is degraded.
    fn write(&self, folder: &Path, text: &str) -> Result<Vec<String>, NoteError>;

    /// Removes the note. Clearing a folder without a note is not an error.
    fn clear(&self, folder: &Path) -> Result<(), NoteError>;

    /// Folders whose note contains `query`, optionally only under `only_in`.
    fn find(&self, query: &str, only_in: Option<&Path>) -> Result<Vec<PathBuf>, NoteError>;

    /// The top of `note --help`: what the note becomes on this OS and what
    /// can search it.
    fn help_intro(&self) -> &'static str;

    /// The paragraph after the usage in `note --help`: limits and caveats of
    /// this OS.
    fn help_details(&self) -> &'static str;
}

#[cfg(test)]
pub(crate) mod memory {
    use super::Backend;
    use crate::NoteError;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// An in-memory backend so the command logic is tested on every OS.
    #[derive(Default)]
    pub struct MemoryBackend {
        pub notes: RefCell<BTreeMap<PathBuf, String>>,
        pub warnings: Vec<String>,
        pub fail_with: Option<String>,
    }

    impl MemoryBackend {
        fn check(&self) -> Result<(), NoteError> {
            match &self.fail_with {
                Some(message) => Err(NoteError::Failed(message.clone())),
                None => Ok(()),
            }
        }
    }

    impl Backend for MemoryBackend {
        fn read(&self, folder: &Path) -> Result<Option<String>, NoteError> {
            self.check()?;
            Ok(self.notes.borrow().get(folder).cloned())
        }

        fn write(&self, folder: &Path, text: &str) -> Result<Vec<String>, NoteError> {
            self.check()?;
            self.notes
                .borrow_mut()
                .insert(folder.to_path_buf(), text.to_string());
            Ok(self.warnings.clone())
        }

        fn clear(&self, folder: &Path) -> Result<(), NoteError> {
            self.check()?;
            self.notes.borrow_mut().remove(folder);
            Ok(())
        }

        fn find(&self, query: &str, only_in: Option<&Path>) -> Result<Vec<PathBuf>, NoteError> {
            self.check()?;
            let query = query.to_lowercase();
            Ok(self
                .notes
                .borrow()
                .iter()
                .filter(|(path, _)| only_in.is_none_or(|root| path.starts_with(root)))
                .filter(|(_, text)| text.to_lowercase().contains(&query))
                .map(|(path, _)| path.clone())
                .collect())
        }

        fn help_intro(&self) -> &'static str {
            "note keeps notes in memory.\n"
        }

        fn help_details(&self) -> &'static str {
            "Notes vanish when the test ends.\n"
        }
    }
}
