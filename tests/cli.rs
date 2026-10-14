/* fbarcalc: find account maximum value
 * Copyright 2026 Mario Finelli
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Tests that run the real binary
//!
//! The interactive prompts need a terminal so they can't be driven from here:
//! these tests cover everything that happens before them, and what happens
//! when there is no terminal. In particular none of these situations should
//! ever panic.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A directory that is removed when it goes out of scope.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "fbarcalc-cli-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn join(&self, p: &str) -> PathBuf {
        self.0.join(p)
    }

    /// Create a file in the directory.
    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run fbarcalc with no terminal attached to it.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fbarcalc"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn run_with_config(config: &Path, args: &[&str]) -> Output {
    let mut all = vec!["--config", config.to_str().unwrap()];
    all.extend_from_slice(args);
    run(&all)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The program failed with an error message, as opposed to crashing.
fn assert_clean_failure(o: &Output) {
    let err = stderr(o);
    assert_eq!(o.status.code(), Some(1), "stderr: {}", err);
    assert!(err.starts_with("error: "), "stderr: {}", err);
    assert!(!err.contains("panicked"), "stderr: {}", err);
    assert_eq!(stdout(o).trim(), "", "stdout: {}", stdout(o));
}

#[test]
fn prints_the_version() {
    let o = run(&["-V"]);

    assert!(o.status.success());
    assert_eq!(
        stdout(&o).trim(),
        format!("fbarcalc {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn prints_help() {
    let o = run(&["--help"]);

    assert!(o.status.success());
    let out = stdout(&o);
    assert!(out.contains("--config"), "{}", out);
    assert!(out.contains("config"), "{}", out);
}

#[test]
fn a_missing_config_file_says_how_to_create_it() {
    let dir = TempDir::new();
    let file = dir.join("missing.toml");
    let o = run_with_config(&file, &[]);

    assert_clean_failure(&o);
    let err = stderr(&o);
    assert!(err.contains(file.to_str().unwrap()), "{}", err);
    assert!(err.contains("fbarcalc config"), "{}", err);
}

#[test]
fn a_config_with_the_wrong_type_is_an_error() {
    let dir = TempDir::new();
    let file = dir.file("config.toml", "fca_api_key = 5\n");
    let o = run_with_config(&file, &[]);

    assert_clean_failure(&o);
    assert!(stderr(&o).contains("fca_api_key"), "{}", stderr(&o));
}

#[test]
fn a_config_that_is_not_toml_is_an_error() {
    let dir = TempDir::new();
    let file = dir.file("config.toml", "this is not toml\n");
    let o = run_with_config(&file, &[]);

    assert_clean_failure(&o);
    assert!(
        stderr(&o).contains(file.to_str().unwrap()),
        "{}",
        stderr(&o)
    );
}

#[test]
fn a_config_path_that_is_a_directory_is_an_error() {
    let dir = TempDir::new();
    let o = run_with_config(&dir.0, &[]);

    assert_clean_failure(&o);
}

#[test]
fn running_without_a_terminal_is_an_error() {
    let dir = TempDir::new();
    let file = dir.file(
        "config.toml",
        "default_input_currency = \"USD\"\nfca_api_key = \"k\"\n",
    );
    let o = run_with_config(&file, &[]);

    assert_clean_failure(&o);
    assert!(stderr(&o).contains("TTY"), "{}", stderr(&o));
}

#[test]
fn an_unsupported_default_currency_is_not_a_crash() {
    let dir = TempDir::new();
    let file = dir.file("config.toml", "default_input_currency = \"CAD\"\n");
    let o = run_with_config(&file, &[]);

    // it gets as far as the (non-interactive) prompt
    assert_clean_failure(&o);
    assert!(stderr(&o).contains("TTY"), "{}", stderr(&o));
}

#[test]
fn configuring_without_a_terminal_changes_nothing() {
    let dir = TempDir::new();
    let original = "default_input_currency = \"EUR\"\n# keep me\n";
    let file = dir.file("config.toml", original);
    let o = run_with_config(&file, &["config"]);

    assert_clean_failure(&o);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn configuring_without_a_terminal_creates_nothing() {
    let dir = TempDir::new();
    let file = dir.join("new/dir/config.toml");
    let o = run_with_config(&file, &["config"]);

    assert_clean_failure(&o);
    assert!(!dir.join("new").exists());
}

#[test]
fn configuring_refuses_to_overwrite_a_file_that_is_not_toml() {
    let dir = TempDir::new();
    let original = "this is not toml\n";
    let file = dir.file("config.toml", original);
    let o = run_with_config(&file, &["config"]);

    assert_clean_failure(&o);
    assert!(
        stderr(&o).contains(file.to_str().unwrap()),
        "{}",
        stderr(&o)
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}
