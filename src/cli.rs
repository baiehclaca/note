use crate::NoteError;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Action {
    Help,
    Show {
        path: Option<PathBuf>,
    },
    Set {
        path: Option<PathBuf>,
        text: String,
    },
    Clear {
        path: Option<PathBuf>,
    },
    Find {
        path: Option<PathBuf>,
        query: String,
    },
}

pub fn parse<I, S>(args: I) -> Result<Action, NoteError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut path = None;
    let mut mode = Mode::Words;
    let mut words = Vec::new();
    let mut flags_done = false;
    let mut raw_args = args.into_iter();

    while let Some(raw) = raw_args.next() {
        let raw = raw.into();
        if !flags_done {
            if raw == "--" {
                flags_done = true;
                continue;
            }
            if let Some(flag) = raw.to_str() {
                if flag == "--help" || flag == "-h" {
                    return Ok(Action::Help);
                }
                if flag == "--clear" {
                    mode = set_mode(mode, Mode::Clear)?;
                    continue;
                }
                if flag == "--find" {
                    mode = set_mode(mode, Mode::Find)?;
                    continue;
                }
                if flag == "--path" {
                    let value = raw_args.next().ok_or_else(|| {
                        NoteError::Usage(
                            "note --path needs a folder. Run note --help for usage.".into(),
                        )
                    })?;
                    if path.is_some() {
                        return Err(NoteError::Usage(
                            "note takes one --path. Run note --help for usage.".into(),
                        ));
                    }
                    path = Some(PathBuf::from(value.into()));
                    continue;
                }
                if flag.starts_with('-') && flag != "-" {
                    return Err(NoteError::Usage(format!(
                        "Unknown option {flag}. Run note --help for usage."
                    )));
                }
            }
        }
        match raw.to_str() {
            Some(word) => words.push(word.to_string()),
            None => return Err(NoteError::Failed("Notes have to be text.".into())),
        }
    }

    // Words after `--` are note text, including the words clear, find, and help.
    if mode == Mode::Words && !flags_done {
        if words.first().map(String::as_str) == Some("help") && words.len() == 1 && path.is_none() {
            return Ok(Action::Help);
        }
        if words.first().map(String::as_str) == Some("clear") {
            if words.len() != 1 {
                return Err(NoteError::Usage(
                    "clear removes the note and takes no other words. To save the word \"clear\", run: note -- clear".into(),
                ));
            }
            mode = Mode::Clear;
            words.clear();
        } else if words.first().map(String::as_str) == Some("find") {
            mode = Mode::Find;
            words.remove(0);
        }
    }

    match mode {
        Mode::Words if words.is_empty() => Ok(Action::Show { path }),
        Mode::Words => Ok(Action::Set {
            path,
            text: words.join(" "),
        }),
        Mode::Clear if words.is_empty() => Ok(Action::Clear { path }),
        Mode::Clear => Err(NoteError::Usage(
            "clear removes the note and takes no other words. To save the word \"clear\", run: note -- clear".into(),
        )),
        Mode::Find => {
            let query = words.join(" ");
            if query.trim().is_empty() {
                return Err(NoteError::Usage(
                    "Give note find some words to search for.".into(),
                ));
            }
            Ok(Action::Find { path, query })
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Words,
    Clear,
    Find,
}

fn set_mode(current: Mode, next: Mode) -> Result<Mode, NoteError> {
    if current != Mode::Words {
        return Err(NoteError::Usage(
            "Use either clear or find, not both. Run note --help for usage.".into(),
        ));
    }
    Ok(next)
}

/// The full `note --help` text. `intro` and `details` come from the
/// platform backend because what stores and searches the note differs per OS.
pub fn help_text(intro: &str, details: &str) -> String {
    format!(
        "\
{intro}
Usage:
  note [words...]          Set the note on this folder
  note                     Show the note
  note clear               Remove the note
  note find [words...]     List folders whose note matches
  note --path PATH ...     Use PATH instead of the current folder

  note -- clear            Save a note whose text is \"clear\"
  note -- help             Save a note whose text is \"help\"

{details}
Examples:
  cd ~/Projects/client-work
  note prague pitch, march, the blue deck
  note find prague
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Action {
        parse(args.iter().copied()).unwrap()
    }

    #[test]
    fn words_set_a_note_and_no_words_show_it() {
        assert!(matches!(parse_args(&[]), Action::Show { path: None }));
        match parse_args(&["prague", "pitch,", "march"]) {
            Action::Set { path: None, text } => assert_eq!(text, "prague pitch, march"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn clear_and_find_are_commands() {
        assert!(matches!(
            parse_args(&["clear"]),
            Action::Clear { path: None }
        ));
        assert!(matches!(
            parse_args(&["--clear"]),
            Action::Clear { path: None }
        ));
        match parse_args(&["find", "prague", "pitch"]) {
            Action::Find { path: None, query } => assert_eq!(query, "prague pitch"),
            other => panic!("unexpected {other:?}"),
        }
        match parse_args(&["--", "clear"]) {
            Action::Set { text, .. } => assert_eq!(text, "clear"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn path_selects_the_folder() {
        match parse_args(&["--path", "/tmp/client", "blue", "deck"]) {
            Action::Set { path, text } => {
                assert_eq!(path.unwrap(), PathBuf::from("/tmp/client"));
                assert_eq!(text, "blue deck");
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse_args(&["--path", "/tmp/client"]) {
            Action::Show { path } => assert_eq!(path.unwrap(), PathBuf::from("/tmp/client")),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_flags_and_an_empty_search() {
        assert!(matches!(parse(["--nope"]), Err(NoteError::Usage(_))));
        assert!(matches!(parse(["find"]), Err(NoteError::Usage(_))));
        assert!(matches!(
            parse(["clear", "extra"]),
            Err(NoteError::Usage(_))
        ));
        assert!(matches!(parse(["help"]), Ok(Action::Help)));
    }

    #[test]
    fn path_and_mode_flags_are_checked() {
        assert!(matches!(parse(["--path"]), Err(NoteError::Usage(_))));
        assert!(matches!(
            parse(["--path", "a", "--path", "b"]),
            Err(NoteError::Usage(_))
        ));
        assert!(matches!(
            parse(["--clear", "--find", "x"]),
            Err(NoteError::Usage(_))
        ));
        assert!(matches!(
            parse(["--clear", "extra"]),
            Err(NoteError::Usage(_))
        ));
        match parse_args(&["--find", "--path", "/tmp", "blue"]) {
            Action::Find { path, query } => {
                assert_eq!(path.unwrap(), PathBuf::from("/tmp"));
                assert_eq!(query, "blue");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn words_after_double_dash_are_text() {
        match parse_args(&["--", "--help", "-x", "find"]) {
            Action::Set { text, .. } => assert_eq!(text, "--help -x find"),
            other => panic!("unexpected {other:?}"),
        }
        match parse_args(&["-", "dash"]) {
            Action::Set { text, .. } => assert_eq!(text, "- dash"),
            other => panic!("unexpected {other:?}"),
        }
        match parse_args(&["--path", "/tmp", "help"]) {
            Action::Set { text, .. } => assert_eq!(text, "help"),
            other => panic!("unexpected {other:?}"),
        }
        match parse_args(&["help", "me"]) {
            Action::Set { text, .. } => assert_eq!(text, "help me"),
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(parse_args(&["-h", "x"]), Action::Help));
    }

    #[test]
    fn find_needs_real_words() {
        assert!(matches!(parse(["find", " ", ""]), Err(NoteError::Usage(_))));
        assert!(matches!(parse(["--find"]), Err(NoteError::Usage(_))));
    }

    #[cfg(unix)]
    #[test]
    fn notes_must_be_text() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let bad = OsStr::from_bytes(&[0x66, 0xff, 0x6f]);
        assert!(matches!(parse([bad]), Err(NoteError::Failed(_))));
        // A non-UTF-8 folder name is fine.
        match parse([OsStr::new("--path"), bad]).unwrap() {
            Action::Show { path } => assert_eq!(path.unwrap().as_os_str(), bad),
            other => panic!("unexpected {other:?}"),
        }
    }
}
