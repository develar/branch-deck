use crate::dir_watch::{Fingerprints, Wake, WatchedDir, watch_from_with};
use notify::Event;
use notify::event::{AccessKind, AccessMode, EventKind, Flag, ModifyKind};
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

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

/// An open, a read or a close without a write of a file of the directory changes nothing and wakes no waiter: inotify
/// tells every open (`IN_OPEN`), the read of the waiter itself too, which would else ask a look of it, whose read asks
/// the next, for ever. A close after a write (`IN_CLOSE_WRITE`) names its file, and a lost event wakes for all
#[test]
fn an_open_a_read_or_a_close_without_a_write_wakes_no_waiter() {
  let dir = tempfile::tempdir().unwrap();
  let watched = WatchedDir::new(dir.path());
  let registry = dir.path().join("operations.json");
  let of = |kind: EventKind, path: &Path| Ok(Event::new(kind).add_path(path.to_path_buf()));

  for nothing in [
    AccessKind::Open(AccessMode::Any),
    AccessKind::Read,
    AccessKind::Close(AccessMode::Read),
    AccessKind::Close(AccessMode::Execute),
  ] {
    assert_eq!(watched.wake_of(&of(EventKind::Access(nothing), &registry), &NAMES), Wake::None, "{nothing:?}");
    assert_eq!(
      watched.wake_of(&of(EventKind::Access(nothing), &dir.path().join("operations.lock")), &NAMES),
      Wake::None,
      "{nothing:?}"
    );
  }
  assert_eq!(
    watched.wake_of(&of(EventKind::Access(AccessKind::Close(AccessMode::Write)), &registry), &NAMES),
    Wake::Named(0b1)
  );
  assert_eq!(watched.wake_of(&of(EventKind::Access(AccessKind::Any), &registry), &NAMES), Wake::Named(0b1));
  let lost = Event::new(EventKind::Access(AccessKind::Open(AccessMode::Any))).add_path(registry).set_flag(Flag::Rescan);
  assert_eq!(watched.wake_of(&Ok(lost), &NAMES), Wake::Any);
}

/// The names of the files `event` names
fn names_of(event: &notify::Result<Event>) -> Vec<String> {
  event.as_ref().map_or_default(|event| {
    event
      .paths
      .iter()
      .filter_map(|path| path.file_name())
      .map(|name| name.to_string_lossy().into_owned())
      .collect()
  })
}

/// The watch of the system: the reads of a file of the directory wake no waiter, whatever it tells of them (inotify an
/// open each, `IN_OPEN`; `FSEvents` and `ReadDirectoryChangesW` nothing), and a file renamed in after them, whose
/// events come after theirs (inotify keeps the order), wakes. The reads begin once the stream told a write of its own
/// (a probe): `FSEvents` drops the changes of its first moments. Its wait ends at a deadline with "no watch event" where
/// the watch of the system runs behind the writes
#[test]
fn the_reads_of_a_file_of_the_directory_wake_no_waiter_on_the_watch_of_the_system() {
  let dir = tempfile::tempdir().unwrap();
  let registry = dir.path().join("operations.json");
  std::fs::write(&registry, "{}").unwrap();
  let (sender, events) = std::sync::mpsc::channel();
  let _watcher = crate::dir_watch::watch(dir.path(), sender).unwrap();
  let watched = WatchedDir::new(dir.path());
  let deadline = Instant::now() + Duration::from_secs(30);
  let no_event = || -> ! { panic!("no watch event within 30 s: the watch of the system runs behind the writes") };
  let probe = dir.path().join("probe");
  'live: loop {
    std::fs::write(&probe, "a write the stream tells once it runs").unwrap();
    loop {
      match events.recv_timeout(Duration::from_millis(100)) {
        Ok(event) if names_of(&event).iter().any(|name| name == "probe") => break 'live,
        Ok(_) => {}
        Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => continue 'live,
        Err(_) => no_event(),
      }
    }
  }

  for _ in 0..3 {
    std::fs::read(&registry).unwrap();
  }
  let temporary = dir.path().join(".anchor.tmp");
  std::fs::write(&temporary, "the change after the reads").unwrap();
  crate::fs_retry::rename(&temporary, dir.path().join("anchor")).unwrap();
  let mut of_reads = 0;
  loop {
    let event = events.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_else(|_| no_event());
    let names = names_of(&event);
    if names.iter().any(|name| name == "anchor" || name == ".anchor.tmp") {
      assert!(watched.wake_of(&event, &NAMES).wakes(), "the file renamed in wakes: {event:?}");
      break;
    }
    // the probe, told late, and the directory itself: no file of the reads
    if names.iter().any(|name| name == "operations.json") {
      of_reads += 1;
      assert_eq!(watched.wake_of(&event, &NAMES), Wake::None, "an event of a read: {event:?}");
    }
  }
  assert_eq!(of_reads > 0, cfg!(target_os = "linux"), "only inotify tells the reads of a file: {of_reads} events of them");
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

/// The first look of `watch_from` comes before its watcher starts: a write while the watcher starts, whose event the
/// first moments of an `FSEvents` stream drop, is a change at the next look. A first look after the watcher would take
/// it as where it starts and tell it at no look
#[test]
fn the_first_look_comes_before_the_watcher_so_a_write_while_it_starts_is_a_change_at_the_next_look() {
  let dir = tempfile::tempdir().unwrap();
  let journal = dir.path().join("journal.jsonl");
  let start = |_dir: &Path, (): ()| std::fs::write(&journal, "{}\n").map_err(notify::Error::io);
  let (mut fingerprints, started) = watch_from_with(dir.path(), vec![journal.clone()], (), start);
  started.unwrap();
  assert_eq!(fingerprints.look(), 0b1, "the write while the watcher started");
}
