#[cfg(target_os = "macos")]
use std::fs::{self, OpenOptions};
#[cfg(target_os = "macos")]
use std::io::Write;
#[cfg(target_os = "macos")]
use std::sync::mpsc;
use std::time::Instant;

use super::*;
use std::time::Duration;

/// The wait of a test: no run reaches it, so a wait that ends before it ended by a change
const UNHURRIED: Duration = Duration::from_mins(1);

/// Waits with `watch` while a thread runs `change` once the wait has started (100 ms later), and asserts that the change
/// ended the wait: before its time, by the kqueue, which a wait leaves only for an event or its time
#[cfg(target_os = "macos")]
fn assert_ended_by(watch: &mut Watch, change: impl FnOnce() + Send + 'static) {
  let (sender, changed) = mpsc::channel();
  let changer = thread::spawn(move || {
    thread::sleep(Duration::from_millis(100));
    change();
    let _ = sender.send(());
  });
  let started = Instant::now();
  watch.wait(UNHURRIED);
  let waited = started.elapsed();
  changer.join().expect("the change");
  changed.recv().expect("the change happened");
  assert!(watch.is_pushed() && waited < UNHURRIED, "{waited:?}, pushed: {}", watch.is_pushed());
}

#[test]
#[cfg(target_os = "macos")]
fn a_write_to_the_log_and_a_new_file_in_the_run_directory_end_a_wait_at_once() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let log = dir.path().join("events.log");
  fs::write(&log, "").expect("the log");
  let mut watch = Watch::new(&[&log, dir.path()]);
  assert!(watch.is_pushed());

  assert_ended_by(&mut watch, move || {
    let mut file = OpenOptions::new().append(true).open(&log).expect("opened");
    file.write_all(b"line\n").expect("appended");
  });

  let exit = dir.path().join("exit.json");
  assert_ended_by(&mut watch, move || {
    let temp = exit.with_extension("tmp");
    fs::write(&temp, "{}").expect("written");
    sync_utils::fs_retry::rename(&temp, &exit).expect("renamed");
  });
}

#[test]
#[cfg(target_os = "macos")]
fn a_change_before_the_wait_ends_it_at_once() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let log = dir.path().join("events.log");
  fs::write(&log, "").expect("the log");
  let mut watch = Watch::new(&[&log]);
  fs::write(&log, "line\n").expect("written");
  let started = Instant::now();
  watch.wait(UNHURRIED);
  let waited = started.elapsed();
  assert!(watch.is_pushed() && waited < UNHURRIED, "{waited:?}, pushed: {}", watch.is_pushed());
}

#[test]
fn a_watch_of_a_missing_file_polls() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let mut watch = Watch::new(&[&dir.path().join("missing")]);
  assert!(!watch.is_pushed());
  let started = Instant::now();
  watch.wait(UNHURRIED);
  let waited = started.elapsed();
  assert!(waited >= tuning::watch_poll() && waited < UNHURRIED, "one poll, not the whole wait: {waited:?}");
}
