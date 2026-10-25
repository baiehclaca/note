//! Live Spotlight check. It needs macOS with indexing turned on.
#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The product invariant: after `note <words>` on a folder Spotlight already
/// knows, a search for those words returns that folder.
///
/// A directory created in the same second is a bad fixture. Spotlight often
/// imports it before the comment is written, then ignores the update for a
/// while. An ordinary folder you `cd` into is already in the index, and that
/// is the case this test pins.
#[test]
fn spotlight_returns_the_tagged_folder() {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap();
    if let Some(reason) = spotlight_unavailable(&folder) {
        eprintln!("skipping: {reason}");
        return;
    }
    let bin = env!("CARGO_BIN_EXE_note");
    let id = format!(
        "zxqnote{}{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let show = Command::new(bin).current_dir(&folder).output().unwrap();
    let previous = if show.status.success() {
        Some(
            String::from_utf8_lossy(&show.stdout)
                .trim_end_matches('\n')
                .to_string(),
        )
    } else {
        None
    };
    let _restore = Restore {
        bin,
        folder: folder.clone(),
        previous,
    };

    let text = format!("prague pitch, the blue deck {id}");
    let set = Command::new(bin)
        .arg(&text)
        .current_dir(&folder)
        .output()
        .unwrap();
    assert!(
        set.status.success(),
        "set failed: {}",
        String::from_utf8_lossy(&set.stderr)
    );

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut found = false;
    let mut last = String::new();
    while Instant::now() < deadline {
        let search = Command::new(bin).args(["find", &id]).output().unwrap();
        last = format!(
            "status={:?}\nstdout={}\nstderr={}",
            search.status.code(),
            String::from_utf8_lossy(&search.stdout),
            String::from_utf8_lossy(&search.stderr)
        );
        if search.status.success()
            && String::from_utf8_lossy(&search.stdout)
                .lines()
                .any(|line| Path::new(line) == folder)
        {
            found = true;
            break;
        }
        thread::sleep(Duration::from_millis(300));
    }
    assert!(
        found,
        "Spotlight did not return {}\n{last}",
        folder.display()
    );
}

/// CI machines often run with Spotlight indexing turned off. The invariant
/// can only be checked where Spotlight is actually indexing this volume.
fn spotlight_unavailable(folder: &Path) -> Option<String> {
    let status = match Command::new("mdutil").arg("-s").arg(folder).output() {
        Ok(output) => output,
        Err(error) => return Some(format!("mdutil failed: {error}")),
    };
    let text = String::from_utf8_lossy(&status.stdout).to_lowercase();
    if !status.status.success() || text.contains("disabled") || text.contains("unknown") {
        return Some(format!(
            "Spotlight is not indexing {}: {}",
            folder.display(),
            text.trim()
        ));
    }
    None
}

struct Restore {
    bin: &'static str,
    folder: PathBuf,
    previous: Option<String>,
}

impl Drop for Restore {
    fn drop(&mut self) {
        let mut command = Command::new(self.bin);
        command.current_dir(&self.folder);
        match &self.previous {
            Some(text) => {
                command.arg(text);
            }
            None => {
                command.arg("clear");
            }
        }
        let _ = command.output();
    }
}
