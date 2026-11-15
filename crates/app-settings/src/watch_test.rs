use crate::watch::Changed;
use notify::Event;
use notify::event::{EventKind, Flag, ModifyKind};
use pretty_assertions::assert_eq;
use std::path::Path;
use sync_utils::dir_watch::WatchedDir;

fn modified(path: &Path) -> Event {
  Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.to_path_buf())
}

/// An event of a settings file asks for it; an event of another name of the directory (the temporary file of a rename
/// over one of them), of the directory itself, a lost event and an error ask for both, as any of them may have changed;
/// an event below the directory asks for none
#[test]
fn an_event_asks_for_its_file_and_one_that_may_be_a_change_of_either_asks_for_both() {
  let directory = tempfile::tempdir().unwrap();
  let dir = WatchedDir::new(directory.path());
  let settings = Changed {
    settings: true,
    repository_state: false,
  };
  assert_eq!(Changed::of(&dir, &Ok(modified(&directory.path().join("settings.json")))), settings);
  for event in [
    Ok(modified(&directory.path().join(".settings.json.tmp4f2"))),
    Ok(modified(directory.path())),
    Ok(Event::new(EventKind::Other).set_flag(Flag::Rescan)),
    Err(notify::Error::generic("the stream failed")),
  ] {
    assert_eq!(Changed::of(&dir, &event), Changed::BOTH);
  }
  assert_eq!(Changed::of(&dir, &Ok(modified(&directory.path().join("logs/app.log")))), Changed::default());
}
