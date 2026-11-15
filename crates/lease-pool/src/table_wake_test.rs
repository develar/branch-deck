use crate::host_slots::TableWake;
use notify::Event;
use notify::event::{EventKind, Flag, ModifyKind};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// The end of a wait that an event ends first: no run reaches it
const UNHURRIED: Duration = Duration::from_secs(60);

const FILES: [&str; 1] = ["heavy.json"];

fn modified(path: &Path) -> Event {
  Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.to_path_buf())
}

/// Whether `event`, sent before the wait, ends a wait of `dir` for `timeout`
fn wakes(dir: &Path, event: notify::Result<Event>, timeout: Duration) -> bool {
  let (sender, events) = mpsc::channel();
  let wake = TableWake::of_events(dir, &FILES, events);
  sender.send(event).unwrap();
  wake.wait(timeout)
}

/// A wait for the heavy turn ends at an event of its table, and at any event that may be one: the temporary file whose
/// rename over the table under load came as the only event, the directory itself, a lost event (`MUST_SCAN_SUBDIRS`)
/// and an error of the watcher; an event below the directory does not end it
#[test]
fn a_wait_ends_at_an_event_of_its_table_and_at_any_event_that_may_be_one() {
  let dir = tempfile::tempdir().unwrap();
  let heavy = dir.path().join("heavy.json");
  assert!(wakes(dir.path(), Ok(modified(&heavy)), UNHURRIED), "the table");
  for event in [
    Ok(modified(&dir.path().join(".heavy.json.tmpQ7x"))),
    Ok(modified(dir.path())),
    Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)),
    Err(notify::Error::generic("the queue of the watcher overflowed")),
  ] {
    assert!(wakes(dir.path(), event, UNHURRIED), "may be a change of the table");
  }
  // an event that does not wake leaves the wait to its end
  assert!(
    !wakes(dir.path(), Ok(modified(&dir.path().join("runs/run-1/exit.json"))), Duration::from_millis(50)),
    "below the directory"
  );
}
