//! `note find` for systems where no desktop search indexes the note.
//!
//! On Linux and Windows the note lives in folder metadata that the default
//! desktop search does not read (see each backend), so `note find` walks the
//! folder tree itself and reads every folder's note. It needs no indexer and
//! always sees the current note, but it is a full walk: it costs time on a
//! big tree, and it only finds what is under the folder it starts from.

use crate::NoteError;
use std::path::{Path, PathBuf};

/// Folders under `root` (including `root`) whose note contains `query`,
/// ignoring case. Hidden folders below `root` are skipped, as Spotlight,
/// Baloo and Windows Search skip them: a name starting with `.` everywhere,
/// plus the hidden or system attribute on Windows (which also keeps the
/// walk out of `AppData`). Symbolic links and junctions are never followed,
/// so the walk cannot loop.
/// Folders that cannot be read are skipped silently.
pub fn find_under<F>(root: &Path, query: &str, mut read_note: F) -> Vec<PathBuf>
where
    F: FnMut(&Path) -> Result<Option<String>, NoteError>,
{
    let mut hits = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        if let Ok(Some(note)) = read_note(&folder) {
            if note_matches(&note, query) {
                hits.push(folder.clone());
            }
        }
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            // `file_type` does not follow links: a link to a folder is not a dir.
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if is_dir && !is_hidden(&entry) {
                pending.push(entry.path());
            }
        }
    }
    hits.sort();
    hits
}

/// Whether `note` contains every character of `query`, in order and next
/// to each other, ignoring case. This is the same substring match
/// `note find` does through Spotlight on macOS, minus Spotlight's accent
/// folding: here `cafe` does not match `café`.
pub fn note_matches(note: &str, query: &str) -> bool {
    note.to_lowercase().contains(&query.trim().to_lowercase())
}

fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    if entry.file_name().to_string_lossy().starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        if let Ok(metadata) = entry.metadata() {
            return metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM)
                != 0;
        }
    }
    false
}

/// The folder `note find` walks when no `--path` is given: the user's home.
pub fn default_root() -> Result<PathBuf, NoteError> {
    let home = if cfg!(windows) {
        std::env::var_os("USERPROFILE")
    } else {
        std::env::var_os("HOME")
    };
    home.filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            NoteError::Failed(
                "Can't tell where your home folder is. Search a folder with: note --path FOLDER find WORDS"
                    .into(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::test_support::TempFolder;
    use std::collections::HashMap;

    #[test]
    fn matching_is_a_case_insensitive_substring() {
        assert!(note_matches("Prague pitch, the BLUE deck", "blue deck"));
        assert!(note_matches("Žilina Q3", "žILINA"));
        assert!(!note_matches("blue deck", "deck blue"));
        assert!(!note_matches("café", "cafe"));
    }

    #[test]
    fn walks_the_tree_and_skips_hidden_folders() {
        let root = TempFolder::new("scan");
        let a = root.0.join("a");
        let deep = a.join("b").join("c");
        let hidden = root.0.join(".hidden");
        let inside_hidden = hidden.join("x");
        for dir in [&deep, &inside_hidden] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(a.join("file"), "not a folder").unwrap();
        let notes: HashMap<PathBuf, &str> = [
            (root.0.clone(), "root blue"),
            (a.clone(), "nothing"),
            (deep.clone(), "deep BLUE"),
            (hidden.clone(), "blue"),
            (inside_hidden.clone(), "blue"),
        ]
        .into_iter()
        .collect();
        let hits = find_under(&root.0, "blue", |folder| {
            Ok(notes.get(folder).map(|text| text.to_string()))
        });
        let mut expected = vec![root.0.clone(), deep];
        expected.sort();
        assert_eq!(hits, expected);
    }

    #[test]
    fn a_hidden_root_is_still_searched() {
        let root = TempFolder::new("scan");
        let hidden = root.0.join(".config");
        std::fs::create_dir_all(&hidden).unwrap();
        let hits = find_under(&hidden, "x", |_| Ok(Some("x".into())));
        assert_eq!(hits, vec![hidden]);
    }

    #[test]
    fn unreadable_notes_are_skipped() {
        let root = TempFolder::new("scan");
        let hits = find_under(&root.0, "x", |_| Err(NoteError::Failed("no".into())));
        assert!(hits.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn folders_with_the_hidden_attribute_are_skipped() {
        let root = TempFolder::new("scan");
        let hidden = root.0.join("hidden");
        std::fs::create_dir_all(&hidden).unwrap();
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(&hidden)
            .status()
            .unwrap();
        assert!(status.success());
        let hits = find_under(&root.0, "x", |_| Ok(Some("x".into())));
        assert_eq!(hits, vec![root.0.clone()]);
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_not_followed() {
        let root = TempFolder::new("scan");
        let real = root.0.join("real");
        std::fs::create_dir_all(&real).unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(real.join("loop"))
            .arg(&root.0)
            .status()
            .unwrap();
        assert!(status.success());
        let mut visited = Vec::new();
        find_under(&root.0, "x", |folder| {
            visited.push(folder.to_path_buf());
            Ok(None)
        });
        visited.sort();
        assert_eq!(visited, vec![root.0.clone(), real]);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let root = TempFolder::new("scan");
        let real = root.0.join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&root.0, real.join("loop")).unwrap();
        let mut visited = Vec::new();
        find_under(&root.0, "x", |folder| {
            visited.push(folder.to_path_buf());
            Ok(None)
        });
        visited.sort();
        assert_eq!(visited, vec![root.0.clone(), real]);
    }
}
