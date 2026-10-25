//! Rules shared by every platform: which folder a command acts on, and what
//! counts as a valid note.

use crate::NoteError;
use std::path::{Path, PathBuf};

/// The longest note `note` accepts, on every OS. The limit comes from
/// Spotlight, which drops a Finder comment that does not stay inline; using
/// the same limit everywhere keeps a note portable between machines.
pub const MAX_NOTE_BYTES: usize = 2048;

/// Turns `--path` (or the current folder) into an absolute, existing folder.
pub fn resolve_folder(path: Option<&Path>) -> Result<PathBuf, NoteError> {
    let raw = match path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => current_dir()?.join(path),
        None => current_dir()?,
    };
    let canonical = raw
        .canonicalize()
        .map_err(|_| NoteError::Failed(format!("No folder at {}.", raw.display())))?;
    if !canonical.is_dir() {
        return Err(NoteError::Failed(format!(
            "{} is a file. note tags folders.",
            canonical.display()
        )));
    }
    Ok(without_verbatim_prefix(canonical))
}

/// On Windows `canonicalize` returns `\\?\C:\...` paths. They work, but they
/// are not what people type or what Explorer shows, so print the plain form.
/// Rust's std adds the prefix back by itself when a path is too long.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let Some(text) = path.to_str() else {
        return path;
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

/// Trims the note and checks it can be stored. Returns the text to store.
///
/// `stored_len` measures the note the way the OS stores it (see
/// [`crate::backend::Backend::stored_len`]), so the limit is in stored bytes.
pub fn validate_note<'a>(
    text: &'a str,
    stored_len: &dyn Fn(&str) -> usize,
) -> Result<&'a str, NoteError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(NoteError::Failed("The note is empty.".into()));
    }
    let stored = stored_len(text);
    if stored > MAX_NOTE_BYTES {
        return Err(NoteError::Failed(format!(
            "That note takes {stored} bytes as stored on this system. A folder note can be at most {MAX_NOTE_BYTES} bytes; see note --help."
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
    Ok(text)
}

fn current_dir() -> Result<PathBuf, NoteError> {
    std::env::current_dir()
        .map_err(|error| NoteError::Failed(format!("Can't see the current folder: {error}")))
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A fresh, empty folder that is removed when dropped.
    pub struct TempFolder(pub PathBuf);

    impl TempFolder {
        pub fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let path = std::env::temp_dir()
                .join(format!("note-{label}-{}-{millis}-{n}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            TempFolder(super::resolve_folder(Some(&path)).unwrap())
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TempFolder;
    use super::*;

    #[test]
    fn a_file_is_not_a_folder() {
        let folder = TempFolder::new("folder");
        let file = folder.0.join("file.txt");
        std::fs::write(&file, "x").unwrap();
        let error = resolve_folder(Some(&file)).unwrap_err();
        assert!(error.to_string().contains("is a file"));
    }

    #[test]
    fn a_missing_folder_is_named() {
        let folder = TempFolder::new("folder");
        let gone = folder.0.join("gone");
        let error = resolve_folder(Some(&gone)).unwrap_err();
        assert!(error.to_string().starts_with("No folder at"), "{error}");
        assert_eq!(resolve_folder(Some(&folder.0)).unwrap(), folder.0);
    }

    #[test]
    fn verbatim_prefixes_are_dropped_on_windows() {
        let plain = without_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\me"));
        let unc = without_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\x"));
        let device = without_verbatim_prefix(PathBuf::from(r"\\?\Volume{1}\x"));
        if cfg!(windows) {
            assert_eq!(plain, PathBuf::from(r"C:\Users\me"));
            assert_eq!(unc, PathBuf::from(r"\\server\share\x"));
        } else {
            assert_eq!(plain, PathBuf::from(r"\\?\C:\Users\me"));
            assert_eq!(unc, PathBuf::from(r"\\?\UNC\server\share\x"));
        }
        assert_eq!(device, PathBuf::from(r"\\?\Volume{1}\x"));
    }

    fn utf8(text: &str) -> usize {
        text.len()
    }

    #[test]
    fn notes_are_trimmed_and_checked() {
        assert_eq!(validate_note("  blue deck\n", &utf8).unwrap(), "blue deck");
        assert_eq!(validate_note("a\tb\nc", &utf8).unwrap(), "a\tb\nc");
        assert_eq!(
            validate_note("   \n", &utf8).unwrap_err(),
            NoteError::Failed("The note is empty.".into())
        );
        let long = "a".repeat(MAX_NOTE_BYTES + 1);
        let error = validate_note(&long, &utf8).unwrap_err();
        assert!(error.to_string().contains("takes 2049 bytes"), "{error}");
        assert!(error.to_string().contains("at most 2048 bytes"), "{error}");
        assert!(validate_note(&"a".repeat(MAX_NOTE_BYTES), &utf8).is_ok());
        let error = validate_note("bell\u{7}", &utf8).unwrap_err();
        assert!(error.to_string().contains("control character"), "{error}");
    }

    #[test]
    fn the_limit_is_in_stored_bytes() {
        // 1025 characters: 1026 bytes of UTF-8, 2050 bytes of UTF-16.
        let text = format!("{}\u{e9}", "a".repeat(1024));
        assert!(validate_note(&text, &utf8).is_ok());
        let utf16 = |text: &str| text.encode_utf16().count() * 2;
        let error = validate_note(&text, &utf16).unwrap_err();
        assert!(error.to_string().contains("takes 2050 bytes"), "{error}");
        // Trimmed text is what gets measured.
        assert!(validate_note(&format!("  {}  ", "a".repeat(MAX_NOTE_BYTES)), &utf8).is_ok());
    }
}
