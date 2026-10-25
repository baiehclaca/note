use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
#[cfg(target_os = "macos")]
fn set_show_and_clear_roundtrip() {
    let folder = temp_folder().canonicalize().unwrap();
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
#[cfg(target_os = "macos")]
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
    assert!(String::from_utf8_lossy(&help.stdout).contains("Spotlight"));
    fs::remove_dir_all(&folder).unwrap();
}

#[test]
#[cfg(not(target_os = "macos"))]
fn other_systems_get_a_clear_error() {
    let folder = temp_folder();
    let set = note(&folder, &["hello"]);
    assert_eq!(set.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&set.stderr).contains("only works on macOS"));
    fs::remove_dir_all(&folder).unwrap();
}

fn note(folder: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_note"))
        .args(args)
        .current_dir(folder)
        .output()
        .unwrap()
}

#[cfg(target_os = "macos")]
fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_folder() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let path = std::env::temp_dir().join(format!("note-cli-{millis}-{n}"));
    fs::create_dir_all(&path).unwrap();
    path
}
