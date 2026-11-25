use crate::idle_changes::{DEFAULT_IDLE_CHANGES_AFTER, DEFAULT_PRESERVE_MAX, PreserveConfig, latest_change};
use git_ops::lease_slot::LeaseConfig;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn config(after: Option<&str>, max: Option<&str>) -> PreserveConfig {
  PreserveConfig::of(&LeaseConfig {
    idle_changes_after: after.map(str::to_string),
    preserve_max: max.map(str::to_string),
    ..LeaseConfig::default()
  })
}

/// `branchdeck.leaseIdleChangesAfter`: 12 h without it, `0` never, `0s` at once; a value that can't be read keeps the
/// default. `branchdeck.leasePreserveMax`: 64 MiB without it
#[test]
fn the_quiet_time_and_the_bound_of_a_preserve_come_from_the_config() {
  assert_eq!((config(None, None).after, config(None, None).max), (Some(DEFAULT_IDLE_CHANGES_AFTER), DEFAULT_PRESERVE_MAX));
  assert_eq!(config(Some("0"), None).quiet_at(100), None);
  assert_eq!(config(Some("0s"), None).quiet_at(100), Some(100));
  assert_eq!(config(Some("2h"), None).quiet_at(100), Some(100 + 7200));
  assert_eq!(config(Some("soon"), None).after, Some(DEFAULT_IDLE_CHANGES_AFTER));
  assert_eq!(config(None, Some("1M")).max, 1 << 20);
  assert_eq!(config(None, Some("big")).max, DEFAULT_PRESERVE_MAX);
}

/// The change time of `path` as `latest_change` reads it: the later of its mtime and its ctime
#[cfg(unix)]
fn change_of(path: &Path) -> u64 {
  use std::os::unix::fs::MetadataExt;
  let metadata = std::fs::symlink_metadata(path).unwrap();
  let modified = metadata.modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs();
  modified.max(u64::try_from(metadata.ctime()).unwrap())
}

/// A file written now with an old mtime (`cp -p`, `unzip`, `tar x`) changed now by its ctime; a deleted path changed
/// when the nearest directory above it that exists did
#[cfg(unix)]
#[test]
fn the_latest_change_of_a_path_is_its_ctime_too_and_of_a_deleted_one_its_nearest_directory() {
  let slot = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(slot.path().join("dir")).unwrap();
  let copied = slot.path().join("dir").join("copied.txt");
  std::fs::write(&copied, "copied\n").unwrap();
  let two_hours_ago = SystemTime::now() - Duration::from_hours(2);
  std::fs::File::open(&copied).unwrap().set_modified(two_hours_ago).unwrap();
  let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
  let latest = latest_change(slot.path(), &["dir/copied.txt".to_string()]);
  assert!(latest + 60 >= now, "the ctime of the copy, not its mtime of two hours ago: {latest} at {now}");
  assert_eq!(latest_change(slot.path(), &["dir/gone/deep.txt".to_string()]), change_of(&slot.path().join("dir")));
}
