#![forbid(unsafe_code)]

mod bplist;
mod cli;
mod store;

use std::ffi::OsString;
use std::path::PathBuf;

pub use store::NoteError;

#[cfg(not(target_os = "macos"))]
compile_error!("note tags folders through macOS Spotlight and only builds on macOS");

pub fn run<I, S>(args: I) -> Result<(), NoteError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    match cli::parse(args)? {
        cli::Action::Help => {
            print!("{}", cli::help_text());
            Ok(())
        }
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
