//! What each command does, independent of the OS. Output goes to the given
//! writers so the logic is tested against an in-memory backend.

use crate::backend::Backend;
use crate::cli::{self, Action};
use crate::folder::{resolve_folder, validate_note};
use crate::NoteError;
use std::io::Write;
use std::path::PathBuf;

pub fn execute(
    action: Action,
    backend: &dyn Backend,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), NoteError> {
    match action {
        Action::Help => {
            let help = cli::help_text(backend.help_intro(), backend.help_details());
            write_all(out, &help)
        }
        Action::Show { path } => show(backend, path, out),
        Action::Set { path, text } => set(backend, path, &text, out, err),
        Action::Clear { path } => clear(backend, path, out),
        Action::Find { path, query } => find(backend, path, &query, out),
    }
}

fn show(
    backend: &dyn Backend,
    path: Option<PathBuf>,
    out: &mut dyn Write,
) -> Result<(), NoteError> {
    let folder = resolve_folder(path.as_deref())?;
    match backend.read(&folder)? {
        Some(text) => write_all(out, &format!("{text}\n")),
        None => Err(NoteError::Missing(format!(
            "No note on {}.\nSet one with: note whatever you want to remember",
            folder.display()
        ))),
    }
}

fn set(
    backend: &dyn Backend,
    path: Option<PathBuf>,
    text: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), NoteError> {
    let folder = resolve_folder(path.as_deref())?;
    let text = validate_note(text)?;
    let warnings = backend.write(&folder, text)?;
    write_all(out, &format!("{}\n{text}\n", folder.display()))?;
    for warning in warnings {
        write_all(err, &format!("{warning}\n"))?;
    }
    Ok(())
}

fn clear(
    backend: &dyn Backend,
    path: Option<PathBuf>,
    out: &mut dyn Write,
) -> Result<(), NoteError> {
    let folder = resolve_folder(path.as_deref())?;
    backend.clear(&folder)?;
    write_all(out, &format!("{}\n", folder.display()))
}

fn find(
    backend: &dyn Backend,
    path: Option<PathBuf>,
    query: &str,
    out: &mut dyn Write,
) -> Result<(), NoteError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(NoteError::Usage(
            "Give note find some words to search for.".into(),
        ));
    }
    let folder = match path {
        Some(path) => Some(resolve_folder(Some(&path))?),
        None => None,
    };
    let hits = backend.find(query, folder.as_deref())?;
    if hits.is_empty() {
        return Err(NoteError::Missing(format!(
            "No folders with a note matching \"{query}\"."
        )));
    }
    let mut listing = String::new();
    for hit in hits {
        listing.push_str(&format!("{}\n", hit.display()));
    }
    write_all(out, &listing)
}

fn write_all(stream: &mut dyn Write, text: &str) -> Result<(), NoteError> {
    stream
        .write_all(text.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|error| NoteError::Failed(format!("Can't write output: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::memory::MemoryBackend;
    use crate::folder::test_support::TempFolder;

    struct Run {
        result: Result<(), NoteError>,
        out: String,
        err: String,
    }

    fn run(backend: &MemoryBackend, args: &[&str]) -> Run {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = cli::parse(args.iter().copied())
            .and_then(|action| execute(action, backend, &mut out, &mut err));
        Run {
            result,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    fn path_arg(folder: &TempFolder) -> &str {
        folder.0.to_str().unwrap()
    }

    #[test]
    fn set_show_clear_roundtrip() {
        let backend = MemoryBackend::default();
        let folder = TempFolder::new("app");
        let path = path_arg(&folder);

        let set = run(&backend, &["--path", path, "  Praha", "\"Q3\"  "]);
        set.result.unwrap();
        assert_eq!(set.out, format!("{}\nPraha \"Q3\"\n", folder.0.display()));
        assert_eq!(set.err, "");
        assert_eq!(
            backend.notes.borrow().get(&folder.0).map(String::as_str),
            Some("Praha \"Q3\"")
        );

        let show = run(&backend, &["--path", path]);
        show.result.unwrap();
        assert_eq!(show.out, "Praha \"Q3\"\n");

        let clear = run(&backend, &["--path", path, "clear"]);
        clear.result.unwrap();
        assert_eq!(clear.out, format!("{}\n", folder.0.display()));
        assert!(backend.notes.borrow().is_empty());

        let missing = run(&backend, &["--path", path]);
        let error = missing.result.unwrap_err();
        assert_eq!(error.code(), 1);
        assert!(error.to_string().starts_with("No note on"), "{error}");
    }

    #[test]
    fn warnings_go_to_stderr_after_saving() {
        let backend = MemoryBackend {
            warnings: vec!["first".into(), "second".into()],
            ..MemoryBackend::default()
        };
        let folder = TempFolder::new("app");
        let set = run(&backend, &["--path", path_arg(&folder), "blue"]);
        set.result.unwrap();
        assert_eq!(set.err, "first\nsecond\n");
        assert!(set.out.ends_with("blue\n"));
    }

    #[test]
    fn invalid_notes_never_reach_the_backend() {
        let backend = MemoryBackend::default();
        let folder = TempFolder::new("app");
        let empty = run(&backend, &["--path", path_arg(&folder), "   "]);
        assert!(empty.result.unwrap_err().to_string().contains("empty"));
        let file = folder.0.join("file.txt");
        std::fs::write(&file, "x").unwrap();
        let on_file = run(&backend, &["--path", file.to_str().unwrap(), "x"]);
        assert!(on_file
            .result
            .unwrap_err()
            .to_string()
            .contains("is a file"));
        assert!(backend.notes.borrow().is_empty());
    }

    #[test]
    fn find_lists_matches_and_reports_none() {
        let backend = MemoryBackend::default();
        let one = TempFolder::new("app");
        let two = TempFolder::new("app");
        run(&backend, &["--path", path_arg(&one), "Prague pitch"])
            .result
            .unwrap();
        run(&backend, &["--path", path_arg(&two), "blue deck"])
            .result
            .unwrap();

        let hit = run(&backend, &["find", "prague"]);
        hit.result.unwrap();
        assert_eq!(hit.out, format!("{}\n", one.0.display()));

        let scoped = run(&backend, &["--path", path_arg(&two), "find", "prague"]);
        let error = scoped.result.unwrap_err();
        assert_eq!(error.code(), 1);
        assert_eq!(
            error.to_string(),
            "No folders with a note matching \"prague\"."
        );
    }

    #[test]
    fn backend_failures_are_reported() {
        let backend = MemoryBackend {
            fail_with: Some("disk on fire".into()),
            ..MemoryBackend::default()
        };
        let folder = TempFolder::new("app");
        let set = run(&backend, &["--path", path_arg(&folder), "x"]);
        assert_eq!(
            set.result.unwrap_err(),
            NoteError::Failed("disk on fire".into())
        );
        assert_eq!(set.out, "");
    }

    #[test]
    fn help_uses_the_platform_text() {
        let backend = MemoryBackend::default();
        let help = run(&backend, &["--help"]);
        help.result.unwrap();
        assert!(help
            .out
            .starts_with("note keeps notes in memory.\n\nUsage:\n"));
        assert!(help
            .out
            .contains("\nNotes vanish when the test ends.\n\nExamples:\n"));
    }
}
