#![forbid(unsafe_code)]

mod bplist;
mod cli;
mod store;

use std::ffi::OsString;
use std::path::PathBuf;

pub use store::NoteError;

pub fn run<I, S>(args: I) -> Result<(), NoteError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let action = cli::parse(args)?;
    if let cli::Action::Help = action {
        print!("{}", cli::help_text());
        return Ok(());
    }
    if !cfg!(target_os = "macos") {
        // The crate builds and its pure tests run everywhere, but tagging goes
        // through Finder comments and Spotlight, which only exist on macOS.
        return Err(NoteError::Failed(
            "note stores notes as Finder comments for Spotlight, so it only works on macOS for now."
                .into(),
        ));
    }
    match action {
        cli::Action::Help => Ok(()),
        cli::Action::Show { path } => show(path),
        cli::Action::Set { path, text } => set(path, &text),
        cli::Action::Clear { path } => clear(path),
        cli::Action::Find { path, query } => find(path, &query),
    }
}

fn show(path: Option<PathBuf>) -> Result<(), NoteError> {
    let folder = store::resolve_folder(path.as_deref())?;
    match store::get_note(&folder)? {
        Some(text) => {
            println!("{text}");
            Ok(())
        }
        None => Err(NoteError::Missing(format!(
            "No note on {}.\nSet one with: note whatever you want to remember",
            folder.display()
        ))),
    }
}

fn set(path: Option<PathBuf>, text: &str) -> Result<(), NoteError> {
    let folder = store::resolve_folder(path.as_deref())?;
    let tag = store::set_note(&folder, text)?;
    println!("{}", folder.display());
    println!("{}", text.trim());
    if let Some(warning) = tag.finder_warning {
        eprintln!("{warning}");
    }
    if tag.hidden_from_spotlight {
        eprintln!(
            "Saved the note, but Spotlight does not index hidden or temporary folders, so search may not show this one."
        );
    }
    Ok(())
}

fn clear(path: Option<PathBuf>) -> Result<(), NoteError> {
    let folder = store::resolve_folder(path.as_deref())?;
    store::clear_note(&folder)?;
    println!("{}", folder.display());
    Ok(())
}

fn find(path: Option<PathBuf>, query: &str) -> Result<(), NoteError> {
    let folder = match path {
        Some(path) => Some(store::resolve_folder(Some(&path))?),
        None => None,
    };
    let hits = store::find_notes(query, folder.as_deref())?;
    if hits.is_empty() {
        return Err(NoteError::Missing(format!(
            "No folders with a note matching \"{}\".",
            query.trim()
        )));
    }
    for hit in hits {
        println!("{}", hit.display());
    }
    Ok(())
}
