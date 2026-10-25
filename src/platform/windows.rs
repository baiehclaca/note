//! Windows: the note is an NTFS alternate data stream on the folder,
//! `FOLDER:note.comment`, holding the text as UTF-8.
//!
//! Why a stream and not `desktop.ini`: a stream is metadata on the folder
//! itself, like the Finder comment on macOS, so nothing is written inside
//! the folder, git checkouts stay clean, and folder attributes are left
//! alone. `desktop.ini` would add a file to the folder and needs the folder
//! marked read-only or system before Explorer reads it.
//!
//! What can search it, honestly: Windows Search and Explorer do not index or
//! show alternate data streams, so the Start menu and Explorer's search box
//! will not find the note. `note find` walks your user folder (or `--path`)
//! and reads the stream itself (see [`crate::scan`]), so it always works,
//! but it is a walk, not an index lookup. You can read the stream yourself
//! with `Get-Content -Stream note.comment FOLDER` in PowerShell.
//!
//! The stream needs NTFS (or ReFS). It moves with the folder on the same
//! drive and is copied by Explorer between NTFS drives, but it is lost on
//! FAT/exFAT drives, in zip files, in git, and with most cloud sync.

use crate::backend::Backend;
use crate::scan;
use crate::NoteError;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

const STREAM: &str = "note.comment";

/// ERROR_INVALID_NAME: what FAT and exFAT answer for a `name:stream` path.
const ERROR_INVALID_NAME: i32 = 123;

/// An NTFS alternate data stream plus a tree walk for search. See the module
/// docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Windows;

impl Backend for Windows {
    fn read(&self, folder: &Path) -> Result<Option<String>, NoteError> {
        let bytes = match std::fs::read(stream_path(folder)) {
            Ok(bytes) => bytes,
            Err(error) if missing(&error) => return Ok(None),
            Err(error) => {
                return Err(NoteError::Failed(format!(
                    "Can't read the note on {}: {error}",
                    folder.display()
                )))
            }
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| NoteError::Failed("The note on this folder is not valid text.".into()))?;
        // Tools like Notepad may have added a byte order mark.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(text.to_string()))
    }

    fn write(&self, folder: &Path, text: &str) -> Result<Vec<String>, NoteError> {
        std::fs::write(stream_path(folder), text.as_bytes()).map_err(|error| {
            if error.raw_os_error() == Some(ERROR_INVALID_NAME) {
                NoteError::Failed(format!(
                    "Can't tag {}: its drive does not support alternate data streams. note needs an NTFS or ReFS drive.",
                    folder.display()
                ))
            } else {
                NoteError::Failed(format!("Can't tag {}: {error}", folder.display()))
            }
        })?;
        Ok(Vec::new())
    }

    fn clear(&self, folder: &Path) -> Result<(), NoteError> {
        match std::fs::remove_file(stream_path(folder)) {
            Ok(()) => Ok(()),
            Err(error) if missing(&error) => Ok(()),
            Err(error) => Err(NoteError::Failed(format!(
                "Can't clear the note on {}: {error}",
                folder.display()
            ))),
        }
    }

    fn find(&self, query: &str, only_in: Option<&Path>) -> Result<Vec<PathBuf>, NoteError> {
        let root = match only_in {
            Some(folder) => folder.to_path_buf(),
            None => scan::default_root()?,
        };
        Ok(scan::find_under(&root, query, |folder| self.read(folder)))
    }

    fn help_intro(&self) -> &'static str {
        "\
note tags the current folder with a note you can search for.

The words you pass are stored on the folder in an NTFS alternate data
stream named note.comment. Nothing is written inside the folder.
"
    }

    fn help_details(&self) -> &'static str {
        "\
A note can be up to 2048 bytes. Windows Search and Explorer do not see
the note: note find walks your user folder (or --path) and reads each
folder's note, skipping hidden and system folders, so it can take a while
on a big tree. The note needs an NTFS or ReFS drive and is lost on FAT or
exFAT drives, in zip files, in git and with most cloud sync.
"
    }
}

/// `C:\\path\\folder:note.comment`, the named stream on the folder itself.
fn stream_path(folder: &Path) -> PathBuf {
    let mut path = OsString::from(folder.as_os_str());
    path.push(":");
    path.push(STREAM);
    PathBuf::from(path)
}

/// No stream yet. Windows says "file not found" for a missing stream.
fn missing(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::test_support::TempFolder;

    #[test]
    fn stores_reads_and_clears_the_exact_note() {
        let folder = TempFolder::new("windows");
        let text = "Praha \u{2014} client \"Q3\"\nline two";
        assert_eq!(Windows.read(&folder.0).unwrap(), None);
        assert!(Windows.write(&folder.0, text).unwrap().is_empty());
        assert_eq!(Windows.read(&folder.0).unwrap().as_deref(), Some(text));
        assert_eq!(
            std::fs::read(stream_path(&folder.0)).unwrap(),
            text.as_bytes()
        );
        // The note is not a file inside the folder.
        assert_eq!(std::fs::read_dir(&folder.0).unwrap().count(), 0);
        Windows.clear(&folder.0).unwrap();
        assert_eq!(Windows.read(&folder.0).unwrap(), None);
        // Clearing twice is fine.
        Windows.clear(&folder.0).unwrap();
    }

    #[test]
    fn a_shorter_note_replaces_a_longer_one() {
        let folder = TempFolder::new("windows");
        Windows
            .write(&folder.0, "a much longer first note")
            .unwrap();
        Windows.write(&folder.0, "short").unwrap();
        assert_eq!(Windows.read(&folder.0).unwrap().as_deref(), Some("short"));
    }

    #[test]
    fn reads_a_stream_other_tools_wrote() {
        let folder = TempFolder::new("windows");
        std::fs::write(stream_path(&folder.0), "\u{feff}from notepad").unwrap();
        assert_eq!(
            Windows.read(&folder.0).unwrap().as_deref(),
            Some("from notepad")
        );
        std::fs::write(stream_path(&folder.0), [0xff, 0xfe, 0x00]).unwrap();
        assert!(Windows.read(&folder.0).is_err());
    }

    #[test]
    fn find_walks_from_the_given_folder() {
        let root = TempFolder::new("windows");
        let client = root.0.join("clients").join("prague");
        std::fs::create_dir_all(&client).unwrap();
        Windows.write(&client, "Prague pitch, blue deck").unwrap();
        Windows.write(&root.0, "unrelated").unwrap();
        assert_eq!(
            Windows.find("BLUE DECK", Some(&root.0)).unwrap(),
            vec![client]
        );
        assert!(Windows.find("missing", Some(&root.0)).unwrap().is_empty());
    }
}
