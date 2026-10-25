//! macOS: the note is the folder's Finder comment, and Spotlight searches it.
//!
//! A directory has no body for Spotlight to read. The Finder comment is
//! metadata on the folder itself, and a plain Spotlight search returns the
//! folder when the query words are in that comment. A file inside the folder
//! would show up as a file, and would also dirty git checkouts.

//!
//! The comment is written straight to the `kMDItemFinderComment` extended
//! attribute (what Spotlight indexes) and then also set through Finder, so
//! Get Info shows it. `note find` asks Spotlight with `mdfind`, so it finds
//! a note anywhere Spotlight indexes, and nowhere else: hidden folders,
//! temporary folders and volumes with indexing off are not searched.

mod bplist;

use crate::backend::Backend;
use crate::NoteError;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const FINDER_COMMENT: &str = "com.apple.metadata:kMDItemFinderComment";

const FINDER_SCRIPT: &str = r#"on run argv
  set folderPath to item 1 of argv
  set noteText to item 2 of argv
  tell application "Finder"
    set comment of (POSIX file folderPath as alias) to noteText
  end tell
  return
end run"#;

/// Finder comments plus Spotlight. See the module docs.
#[derive(Debug, Default, Clone, Copy)]
pub struct MacOs;

impl Backend for MacOs {
    fn read(&self, folder: &Path) -> Result<Option<String>, NoteError> {
        read_xattr(folder)
    }

    fn write(&self, folder: &Path, text: &str) -> Result<Vec<String>, NoteError> {
        write_xattr(folder, text)?;
        let mut warnings = Vec::new();
        match sync_finder(folder, text) {
            Ok(()) => {
                // Finder rewrites the attribute itself; make sure it ended up
                // holding exactly our text.
                if read_xattr(folder)?.as_deref() != Some(text) {
                    write_xattr(folder, text)?;
                }
            }
            Err(error) => warnings.push(format!(
                "Saved the note for Spotlight. Finder did not update Get Info: {error}"
            )),
        }
        if spotlight_skips(folder) {
            warnings.push(
                "Saved the note, but Spotlight does not index hidden or temporary folders, so search may not show this one."
                    .into(),
            );
        }
        Ok(warnings)
    }

    fn stored_len(&self, text: &str) -> usize {
        bplist::string_len(text)
    }

    fn clear(&self, folder: &Path) -> Result<(), NoteError> {
        let _ = sync_finder(folder, "");
        delete_xattr(folder)
    }

    fn find(&self, query: &str, only_in: Option<&Path>) -> Result<Vec<PathBuf>, NoteError> {
        let mut command = Command::new("mdfind");
        if let Some(folder) = only_in {
            command.arg("-onlyin").arg(folder);
        }
        command.arg(comment_query(query));
        let output = command
            .output()
            .map_err(|error| NoteError::Failed(format!("Search failed: {error}")))?;
        if !output.status.success() {
            return Err(NoteError::Failed(format!(
                "Search failed: {}",
                stderr(&output)
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect())
    }

    fn help_intro(&self) -> &'static str {
        "\
note tags the current folder so Spotlight can find it.

The words you pass become the folder's Finder comment. Spotlight indexes
that comment, so a search for those words returns the folder. Nothing is
written inside the folder.
"
    }

    fn help_details(&self) -> &'static str {
        "\
A note can be up to 2048 bytes as macOS stores it: one byte per character
when it is plain ASCII, but two bytes per character (four for most emoji)
once it has any other character, so about 1024 characters with an accent.
Search can take a few seconds. Spotlight skips hidden folders and
temporary folders. The note is metadata on the
folder, so it stays with the folder on this Mac and is not a file in git.
"
    }
}

fn spotlight_skips(path: &Path) -> bool {
    let text = path.to_string_lossy();
    if text == "/tmp"
        || text.starts_with("/tmp/")
        || text.starts_with("/private/tmp/")
        || text.starts_with("/var/folders/")
        || text.starts_with("/private/var/folders/")
    {
        return true;
    }
    path.components().any(|component| match component {
        Component::Normal(name) => name.to_string_lossy().starts_with('.'),
        _ => false,
    })
}

fn comment_query(term: &str) -> String {
    format!(
        "kMDItemFinderComment == \"*{}*\"cd && kMDItemContentTypeTree == \"public.folder\"",
        escape_query(term)
    )
}

fn escape_query(term: &str) -> String {
    let mut escaped = String::with_capacity(term.len());
    for character in term.chars() {
        if character.is_control() {
            escaped.push(' ');
            continue;
        }
        if matches!(character, '\\' | '"' | '\'' | '*' | '?') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn write_xattr(folder: &Path, text: &str) -> Result<(), NoteError> {
    let plist = bplist::encode_string(text).map_err(|error| NoteError::Failed(error.0))?;
    let hex = plist
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let output = run(
        "xattr",
        &[
            OsStr::new("-wx"),
            OsStr::new(FINDER_COMMENT),
            OsStr::new(&hex),
            folder.as_os_str(),
        ],
    )?;
    if output.status.success() {
        return Ok(());
    }
    Err(NoteError::Failed(format!(
        "Can't tag {}: {}",
        folder.display(),
        stderr(&output)
    )))
}

fn read_xattr(folder: &Path) -> Result<Option<String>, NoteError> {
    let output = run(
        "xattr",
        &[
            OsStr::new("-px"),
            OsStr::new(FINDER_COMMENT),
            folder.as_os_str(),
        ],
    )?;
    if !output.status.success() {
        let message = stderr(&output);
        if message.contains("No such xattr") {
            return Ok(None);
        }
        return Err(NoteError::Failed(format!(
            "Can't read the note on {}: {}",
            folder.display(),
            message
        )));
    }
    let bytes = decode_hex(&output.stdout)?;
    let text = bplist::decode_string(&bytes).map_err(|error| NoteError::Failed(error.0))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(text))
}

fn delete_xattr(folder: &Path) -> Result<(), NoteError> {
    let output = run(
        "xattr",
        &[
            OsStr::new("-d"),
            OsStr::new(FINDER_COMMENT),
            folder.as_os_str(),
        ],
    )?;
    if output.status.success() {
        return Ok(());
    }
    let message = stderr(&output);
    if message.contains("No such xattr") {
        return Ok(());
    }
    Err(NoteError::Failed(format!(
        "Can't clear the note on {}: {}",
        folder.display(),
        message
    )))
}

fn sync_finder(folder: &Path, text: &str) -> Result<(), NoteError> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(FINDER_SCRIPT)
        .arg(folder)
        .arg(text)
        .output()
        .map_err(|error| NoteError::Failed(format!("Finder is unavailable: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let message = stderr(&output);
    if message.is_empty() {
        return Err(NoteError::Failed("Finder did not take the comment.".into()));
    }
    Err(NoteError::Failed(message))
}

fn run(program: &str, args: &[&OsStr]) -> Result<Output, NoteError> {
    Command::new(program)
        .args(args)
        .output()
        .map_err(|error| NoteError::Failed(format!("{program} failed: {error}")))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .trim()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_hex(bytes: &[u8]) -> Result<Vec<u8>, NoteError> {
    let text = String::from_utf8_lossy(bytes);
    let hex: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if !hex.len().is_multiple_of(2) {
        return Err(NoteError::Failed(
            "The note on this folder is unreadable.".into(),
        ));
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let digits: Vec<char> = hex.chars().collect();
    let mut index = 0;
    while index < digits.len() {
        let pair: String = digits[index..index + 2].iter().collect();
        let byte = u8::from_str_radix(&pair, 16)
            .map_err(|_| NoteError::Failed("The note on this folder is unreadable.".into()))?;
        out.push(byte);
        index += 2;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    use crate::folder::test_support::TempFolder;

    #[test]
    #[cfg(target_os = "macos")]
    fn stores_and_reads_the_exact_note() {
        let folder = TempFolder::new("macos");
        let text = "Praha \u{2014} client \"Q3\" files";
        write_xattr(&folder.0, text).unwrap();
        assert_eq!(read_xattr(&folder.0).unwrap().as_deref(), Some(text));
        delete_xattr(&folder.0).unwrap();
        assert_eq!(read_xattr(&folder.0).unwrap(), None);
        // Clearing twice is fine.
        delete_xattr(&folder.0).unwrap();
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn the_backend_roundtrips_and_warns_about_temporary_folders() {
        let folder = TempFolder::new("macos");
        let warnings = MacOs.write(&folder.0, "blue deck").unwrap();
        assert_eq!(MacOs.read(&folder.0).unwrap().as_deref(), Some("blue deck"));
        // Temporary folders live under /var/folders, which Spotlight skips.
        assert!(
            warnings.iter().any(|w| w.contains("does not index")),
            "{warnings:?}"
        );
        MacOs.clear(&folder.0).unwrap();
        assert_eq!(MacOs.read(&folder.0).unwrap(), None);
    }

    #[test]
    fn query_escapes_spotlight_syntax() {
        let query = comment_query("say \"hi\" *now*");
        assert_eq!(
            query,
            "kMDItemFinderComment == \"*say \\\"hi\\\" \\*now\\**\"cd && kMDItemContentTypeTree == \"public.folder\""
        );
    }

    #[test]
    fn query_turns_control_characters_into_spaces() {
        assert_eq!(escape_query("a\tb\\c?'"), "a b\\\\c\\?\\'");
    }

    #[test]
    fn spotlight_skips_hidden_and_temporary_folders() {
        assert!(spotlight_skips(Path::new("/tmp/work")));
        assert!(spotlight_skips(Path::new("/Users/me/.hidden/work")));
        assert!(spotlight_skips(Path::new("/Users/me/work/.git")));
        assert!(!spotlight_skips(Path::new("/Users/me/Projects/client")));
    }

    #[test]
    fn the_limit_counts_what_the_comment_stores() {
        let backend = MacOs;
        assert_eq!(backend.stored_len(&"a".repeat(2048)), 2048);
        assert_eq!(
            backend.stored_len(&format!("{}\u{e9}", "a".repeat(1023))),
            2048
        );
        assert_eq!(backend.stored_len("\u{1F44B}"), 4);
        assert!(backend
            .help_details()
            .contains("2048 bytes as macOS stores it"));
    }

    #[test]
    fn refuses_a_comment_spotlight_would_drop() {
        // Mostly ASCII plus one accent is stored as UTF-16, which doubles it.
        // The plist is checked before any command runs, so this is portable.
        let mixed = format!("{}\u{e9}", "a".repeat(2000));
        let error = write_xattr(Path::new("/nonexistent"), &mixed).unwrap_err();
        assert!(error.to_string().contains("too long"), "{error}");
    }

    #[test]
    fn hex_from_xattr_is_decoded() {
        assert_eq!(decode_hex(b"62 70\n6c").unwrap(), b"bpl");
        assert!(decode_hex(b"abc").is_err());
        assert!(decode_hex(b"zz").is_err());
    }
}
