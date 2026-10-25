//! Linux: the note is the folder's `user.xdg.comment` extended attribute.
//!
//! `user.xdg.comment` is the comment attribute from the freedesktop.org
//! "Common Extended Attributes" proposal. KDE Dolphin shows and edits it as
//! the folder's Comment, and KDE's Baloo indexes it, so with Baloo file
//! indexing on, `baloosearch userComment:WORD` finds the folder. GNOME's
//! search (Tracker / LocalSearch) does not read it, and there is no
//! system-wide Linux index that does.
//!
//! So `note find` does not depend on any indexer: it walks the folder tree
//! and reads the attribute itself (see [`crate::scan`]). That works on every
//! distro and desktop, but it is a walk of your home folder (or `--path`),
//! not an instant index lookup.
//!
//! The filesystem must support user extended attributes. ext4, btrfs, xfs
//! and (since Linux 6.6) tmpfs do; FAT, exFAT and many network mounts do not.
//! `cp`, `tar` and `rsync` drop the note unless asked to keep xattrs.

use crate::backend::Backend;
use crate::scan;
use crate::NoteError;
use std::io;
use std::path::{Path, PathBuf};

const COMMENT: &str = "user.xdg.comment";

/// `user.xdg.comment` plus a tree walk for search. See the module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct Linux;

impl Backend for Linux {
    fn read(&self, folder: &Path) -> Result<Option<String>, NoteError> {
        let bytes = match xattr::get(folder, COMMENT) {
            Ok(bytes) => bytes,
            Err(error) if unsupported(&error) => return Ok(None),
            Err(error) => {
                return Err(NoteError::Failed(format!(
                    "Can't read the note on {}: {error}",
                    folder.display()
                )))
            }
        };
        let Some(bytes) = bytes else {
            return Ok(None);
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| NoteError::Failed("The note on this folder is not valid text.".into()))?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(text))
    }

    fn write(&self, folder: &Path, text: &str) -> Result<Vec<String>, NoteError> {
        xattr::set(folder, COMMENT, text.as_bytes()).map_err(|error| {
            if unsupported(&error) {
                NoteError::Failed(format!(
                    "Can't tag {}: its filesystem does not support extended attributes (user xattrs).",
                    folder.display()
                ))
            } else {
                NoteError::Failed(format!("Can't tag {}: {error}", folder.display()))
            }
        })?;
        Ok(Vec::new())
    }

    fn clear(&self, folder: &Path) -> Result<(), NoteError> {
        if self.read(folder)?.is_none() && !has_attribute(folder) {
            return Ok(());
        }
        xattr::remove(folder, COMMENT).map_err(|error| {
            NoteError::Failed(format!(
                "Can't clear the note on {}: {error}",
                folder.display()
            ))
        })
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

The words you pass are stored on the folder as its user.xdg.comment
extended attribute, the comment KDE Dolphin shows and Baloo indexes.
Nothing is written inside the folder.
"
    }

    fn help_details(&self) -> &'static str {
        "\
A note can be up to 2048 bytes of UTF-8: one byte per ASCII character,
two to four for others. note find walks your home folder (or --path) and
reads each folder's note, skipping hidden folders, so it
needs no indexer but can take a while on a big tree. On KDE with file
indexing on, baloosearch userComment:WORD finds notes too; GNOME search
does not. The filesystem must support user xattrs (ext4, btrfs and xfs
do; FAT and exFAT do not), and cp, tar and rsync drop the note unless
told to keep xattrs.
"
    }
}

/// An attribute that is present but empty still needs removing.
fn has_attribute(folder: &Path) -> bool {
    matches!(xattr::get(folder, COMMENT), Ok(Some(_)))
}

/// EOPNOTSUPP: the filesystem has no user xattrs.
fn unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported || error.raw_os_error() == Some(95)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::test_support::TempFolder;

    #[test]
    fn stores_reads_and_clears_the_exact_note() {
        let folder = TempFolder::new("linux");
        let text = "Praha \u{2014} client \"Q3\"\nline two";
        assert_eq!(Linux.read(&folder.0).unwrap(), None);
        assert!(Linux.write(&folder.0, text).unwrap().is_empty());
        assert_eq!(Linux.read(&folder.0).unwrap().as_deref(), Some(text));
        assert_eq!(
            xattr::get(&folder.0, COMMENT).unwrap().unwrap(),
            text.as_bytes()
        );
        Linux.clear(&folder.0).unwrap();
        assert_eq!(Linux.read(&folder.0).unwrap(), None);
        // Clearing twice is fine.
        Linux.clear(&folder.0).unwrap();
    }

    #[test]
    fn reads_a_comment_dolphin_wrote() {
        let folder = TempFolder::new("linux");
        xattr::set(&folder.0, COMMENT, "from dolphin".as_bytes()).unwrap();
        assert_eq!(
            Linux.read(&folder.0).unwrap().as_deref(),
            Some("from dolphin")
        );
        xattr::set(&folder.0, COMMENT, b"").unwrap();
        assert_eq!(Linux.read(&folder.0).unwrap(), None);
        Linux.clear(&folder.0).unwrap();
        assert_eq!(xattr::get(&folder.0, COMMENT).unwrap(), None);
        xattr::set(&folder.0, COMMENT, &[0xff, 0xfe]).unwrap();
        assert!(Linux.read(&folder.0).is_err());
    }

    #[test]
    fn find_walks_from_the_given_folder() {
        let root = TempFolder::new("linux");
        let client = root.0.join("clients").join("prague");
        std::fs::create_dir_all(&client).unwrap();
        Linux.write(&client, "Prague pitch, blue deck").unwrap();
        Linux.write(&root.0, "unrelated").unwrap();
        assert_eq!(
            Linux.find("BLUE DECK", Some(&root.0)).unwrap(),
            vec![client]
        );
        assert!(Linux.find("missing", Some(&root.0)).unwrap().is_empty());
    }
}
