use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn set_show_and_clear_roundtrip() {
    let folder = temp_folder();
    let note_text = "Praha — client \"Q3\" files";

    let set = note(&folder, &[note_text]);
    assert!(set.status.success(), "{}", stderr(&set));
    let stdout = String::from_utf8_lossy(&set.stdout);
    assert!(stdout.starts_with(&format!("{}\n", folder.display())));
    assert!(stdout.contains(note_text));

    let show = note(&folder, &[]);
    assert!(show.status.success(), "{}", stderr(&show));
    assert_eq!(
        String::from_utf8_lossy(&show.stdout),
        format!("{note_text}\n")
    );

    let cleared = note(&folder, &["clear"]);
    assert!(cleared.status.success(), "{}", stderr(&cleared));
    let missing = note(&folder, &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("No note"));

    fs::remove_dir_all(&folder).unwrap();
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn a_file_and_an_empty_note_fail() {
    let folder = temp_folder();
    let file = folder.join("file.txt");
    fs::write(&file, "x").unwrap();

    let tagged_file = note(&folder, &["--path", file.to_str().unwrap(), "hello"]);
    assert!(!tagged_file.status.success());
    assert!(String::from_utf8_lossy(&tagged_file.stderr).contains("is a file"));

    let empty = note(&folder, &["   "]);
    assert!(!empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stderr).contains("empty"));

    fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn help_exits_cleanly() {
    let folder = temp_folder();
    let help = note(&folder, &["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("note find [words...]"), "{text}");
    if cfg!(target_os = "macos") {
        assert!(text.contains("Spotlight"), "{text}");
    }
    if cfg!(windows) {
        assert!(text.contains("alternate data"), "{text}");
    }
    if cfg!(target_os = "linux") {
        assert!(text.contains("user.xdg.comment"), "{text}");
    }
    fs::remove_dir_all(&folder).unwrap();
}

#[test]
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn other_systems_get_a_clear_error() {
    let folder = temp_folder();
    let set = note(&folder, &["hello"]);
    assert_eq!(set.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&set.stderr).contains("not supported")
            || String::from_utf8_lossy(&set.stderr).contains("no way to store")
    );
    fs::remove_dir_all(&folder).unwrap();
}

/// Where no indexer is involved, `note find` sees a note right away.
#[test]
#[cfg(any(target_os = "linux", windows))]
fn find_returns_the_tagged_folder() {
    let root = temp_folder();
    let client = root.join("clients").join("prague");
    fs::create_dir_all(&client).unwrap();

    let set = note(&client, &["prague pitch, the blue deck"]);
    assert!(set.status.success(), "{}", stderr(&set));

    let root_arg = root.to_str().unwrap();
    let found = note(&root, &["--path", root_arg, "find", "Blue Deck"]);
    assert!(found.status.success(), "{}", stderr(&found));
    assert_eq!(
        String::from_utf8_lossy(&found.stdout),
        format!("{}\n", client.display())
    );

    let none = note(&root, &["--path", root_arg, "find", "green"]);
    assert_eq!(none.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&none.stderr).contains("No folders"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn usage_errors_exit_with_two() {
    let folder = temp_folder();
    for args in [&["--nope"][..], &["find"], &["clear", "extra"], &["--path"]] {
        let output = note(&folder, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("note"));
    }
    fs::remove_dir_all(&folder).unwrap();
}

fn note(folder: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_note"))
        .args(args)
        .current_dir(folder)
        .output()
        .unwrap()
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A fresh folder, as `note` prints it: canonical, and on Windows without
/// the `\\?\` prefix `canonicalize` adds.
fn temp_folder() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let path = std::env::temp_dir().join(format!("note-cli-{millis}-{n}"));
    fs::create_dir_all(&path).unwrap();
    let path = path.canonicalize().unwrap();
    match path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(rest) if cfg!(windows) => PathBuf::from(rest),
        _ => path,
    }
}
