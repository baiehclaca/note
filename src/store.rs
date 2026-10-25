//! Store a folder note where Spotlight already looks.
//!
//! A directory has no body for Spotlight to read. The Finder comment is
//! metadata on the folder itself, and a plain Spotlight search returns the
//! folder when the query words are in that comment. A file inside the folder
//! would show up as a file, and would also dirty git checkouts.

use crate::bplist;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const FINDER_COMMENT: &str = "com.apple.metadata:kMDItemFinderComment";
pub const MAX_NOTE_BYTES: usize = 2048;

const FINDER_SCRIPT: &str = r#"on run argv
  set folderPath to item 1 of argv
  set noteText to item 2 of argv
  tell application "Finder"
    set comment of (POSIX file folderPath as alias) to noteText
  end tell
  return
end run"#;

#[derive(Debug, PartialEq, Eq)]
pub enum NoteError {
    Usage(String),
    Missing(String),
    Failed(String),
}

impl NoteError {
    pub fn code(&self) -> i32 {
        match self {
            NoteError::Usage(_) => 2,
            NoteError::Missing(_) | NoteError::Failed(_) => 1,
        }
    }
}

impl std::fmt::Display for NoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteError::Usage(message)
            | NoteError::Missing(message)
            | NoteError::Failed(message) => f.write_str(message),
        }
    }
}

#[derive(Debug)]
pub struct Tag {
    pub finder_warning: Option<String>,
    pub hidden_from_spotlight: bool,
}

pub fn resolve_folder(path: Option<&Path>) -> Result<PathBuf, NoteError> {
    let raw = match path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => current_dir()?.join(path),
        None => current_dir()?,
    };
    let canonical = raw
        .canonicalize()
        .map_err(|_| NoteError::Failed(format!("No folder at {}.", display_path(&raw))))?;
    if !canonical.is_dir() {
        return Err(NoteError::Failed(format!(
            "{} is a file. note tags folders.",
            canonical.display()
        )));
    }
    Ok(canonical)
}

pub fn spotlight_skips(path: &Path) -> bool {
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

pub fn set_note(folder: &Path, text: &str) -> Result<Tag, NoteError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NoteError::Failed("The note is empty.".into()));
    }
    if text.len() > MAX_NOTE_BYTES {
        return Err(NoteError::Failed(format!(
            "That note is {} bytes. A folder note can be at most {} bytes, which is what Spotlight will keep with the folder.",
            text.len(),
            MAX_NOTE_BYTES
        )));
    }
    if text
        .chars()
        .any(|c| c != '\n' && c != '\t' && c.is_control())
    {
        return Err(NoteError::Failed(
            "The note contains a control character Spotlight cannot store.".into(),
        ));
    }
    write_xattr(folder, text)?;
    let finder_warning = match sync_finder(folder, text) {
        Ok(()) => {
            if read_xattr(folder)?.as_deref() != Some(text) {
                write_xattr(folder, text)?;
            }
            None
        }
        Err(error) => Some(format!(
            "Saved the note for Spotlight. Finder did not update Get Info: {error}"
        )),
    };
    Ok(Tag {
        finder_warning,
        hidden_from_spotlight: spotlight_skips(folder),
    })
}

pub fn get_note(folder: &Path) -> Result<Option<String>, NoteError> {
    read_xattr(folder)
}

pub fn clear_note(folder: &Path) -> Result<(), NoteError> {
    let _ = sync_finder(folder, "");
    delete_xattr(folder)
}

pub fn find_notes(query: &str, only_in: Option<&Path>) -> Result<Vec<PathBuf>, NoteError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(NoteError::Usage(
            "Give note find some words to search for.".into(),
        ));
    }
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

pub fn comment_query(term: &str) -> String {
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

fn current_dir() -> Result<PathBuf, NoteError> {
    std::env::current_dir()
        .map_err(|error| NoteError::Failed(format!("Can't see the current folder: {error}")))
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    #[cfg(target_os = "macos")]
    fn stores_and_reads_the_exact_note() {
        let folder = temp_folder();
        let text = "Praha — client \"Q3\" files";
        write_xattr(&folder, text).unwrap();
        assert_eq!(read_xattr(&folder).unwrap().as_deref(), Some(text));
        delete_xattr(&folder).unwrap();
        assert_eq!(read_xattr(&folder).unwrap(), None);
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn rejects_an_empty_note() {
        let folder = temp_folder();
        let error = set_note(&folder, "   \n").unwrap_err();
        assert_eq!(error, NoteError::Failed("The note is empty.".into()));
        std::fs::remove_dir_all(&folder).unwrap();
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
    fn spotlight_skips_hidden_and_temporary_folders() {
        assert!(spotlight_skips(Path::new("/tmp/work")));
        assert!(spotlight_skips(Path::new("/Users/me/.hidden/work")));
        assert!(spotlight_skips(Path::new("/Users/me/work/.git")));
        assert!(!spotlight_skips(Path::new("/Users/me/Projects/client")));
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let folder = temp_folder();
        let file = folder.join("file.txt");
        std::fs::write(&file, "x").unwrap();
        let error = resolve_folder(Some(&file)).unwrap_err();
        assert!(error.to_string().contains("is a file"));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    fn temp_folder() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let path = std::env::temp_dir().join(format!("note-store-{millis}-{n}"));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
