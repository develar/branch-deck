use crate::dir_watch::{Fingerprints, Wake, WatchedDir};
use notify::Event;
use notify::event::{EventKind, Flag, ModifyKind};
use pretty_assertions::assert_eq;
use std::path::Path;

/// An event of the system that names `paths`
fn event(paths: &[&Path]) -> Event {
  paths
    .iter()
    .fold(Event::new(EventKind::Modify(ModifyKind::Any)), |event, path| event.add_path(path.to_path_buf()))
}

const NAMES: [&str; 3] = ["operations.json", "stamp", "journal.jsonl"];

/// An event of a file the waiter looks at names it; an event of another name of the directory (a temporary file renamed
/// over one of them), of the directory itself, without a path, with a lost event (`need_rescan`) or an error wakes for
/// all of them; an event below the directory wakes none
#[test]
fn an_event_names_its_files_and_anything_else_of_the_directory_wakes_for_all() {
  let dir = tempfile::tempdir().unwrap();
  let watched = WatchedDir::new(dir.path());
  let stamp = dir.path().join("stamp");

  assert_eq!(watched.wake_of(&Ok(event(&[&stamp])), &NAMES), Wake::Named(0b10));
  assert_eq!(watched.wake_of(&Ok(event(&[&dir.path().join("operations.json"), &stamp])), &NAMES), Wake::Named(0b11));
  for any in [
    Ok(event(&[&dir.path().join(".stamp.tmpA1b2")])),
    Ok(event(&[dir.path()])),
    Ok(event(&[])),
    Ok(Event::new(EventKind::Other).add_path(stamp).set_flag(Flag::Rescan)),
    Err(notify::Error::generic("the queue of the watcher overflowed")),
  ] {
    assert_eq!(watched.wake_of(&any, &NAMES), Wake::Any, "{any:?}");
  }
  assert_eq!(watched.wake_of(&Ok(event(&[&dir.path().join("notes/lease/log")])), &NAMES), Wake::None);
  assert!(Wake::Any.names(2) && Wake::Named(0b10).names(1) && !Wake::Named(0b10).names(0) && !Wake::None.wakes());
}

/// `FSEvents` names the real path of the directory (`/private/var/…` for `/var/…` on macOS): an event of it names the
/// file as one of the directory as given
#[test]
fn an_event_of_the_real_path_of_the_directory_names_its_file() {
  let dir = tempfile::tempdir().unwrap();
  let real = std::fs::canonicalize(dir.path()).unwrap();
  let watched = WatchedDir::new(dir.path());

  assert_eq!(watched.wake_of(&Ok(event(&[&real.join("journal.jsonl")])), &NAMES), Wake::Named(0b100));
  assert_eq!(watched.wake_of(&Ok(event(&[&real])), &NAMES), Wake::Any);
}

/// A look tells which files changed since the last one: made, renamed over (a new file id), written in place (a new
/// size) or removed; a look that finds nothing new tells nothing
#[test]
fn a_look_tells_the_files_made_renamed_over_written_or_removed_since_the_last_one() {
  let dir = tempfile::tempdir().unwrap();
  let (registry, stamp, journal) = (dir.path().join("operations.json"), dir.path().join("stamp"), dir.path().join("journal.jsonl"));
  std::fs::write(&registry, "{}").unwrap();
  let mut fingerprints = Fingerprints::new(vec![registry.clone(), stamp.clone(), journal.clone()]);
  assert_eq!(fingerprints.look(), 0, "nothing since the first look");

  std::fs::write(&stamp, "1 42\n").unwrap();
  assert_eq!(fingerprints.look(), 0b10, "made");
  assert_eq!(fingerprints.look(), 0, "told once");

  // the same size, renamed over: the file id tells it
  let temporary = dir.path().join(".operations.json.tmp");
  std::fs::write(&temporary, "[]").unwrap();
  std::fs::rename(&temporary, &registry).unwrap();
  std::fs::write(&journal, "{}\n").unwrap();
  assert_eq!(fingerprints.look(), 0b101, "renamed over and made");

  use std::io::Write as _;
  writeln!(std::fs::OpenOptions::new().append(true).open(&journal).unwrap(), "{{}}").unwrap();
  std::fs::remove_file(&stamp).unwrap();
  assert_eq!(fingerprints.look(), 0b110, "written in place and removed");
}
