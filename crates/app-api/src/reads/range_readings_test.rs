//! The readings of ranges kept (`range_readings::keep_readings`, `kept_readings`): a final one in memory and on disk, so
//! the next process shows it at once; one that waits for the index nowhere; one whose renames no index proved, in a
//! repository without one, in both until the repository has an index, which may prove them

use crate::reads::range_readings::{keep_readings, kept_readings};
use crate::reads::review_plan::ReviewPlans;
use domain_types::range_reading::{FileRole, QuietProof, RangeReading, RoleReading, Unproven, UnprovenFile};
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;

fn reading(why: Option<Unproven>) -> RangeReading {
  RangeReading {
    roles: vec![RoleReading {
      role: FileRole::Code,
      files: 1,
      files_to_read: u32::from(why.is_some()),
      changes: 1,
      to_read: u32::from(why.is_some()),
      // the change to read is in the file no proof is made of
      files_to_read_proven: 0,
      to_read_proven: 0,
    }],
    unproven: why
      .map(|why| UnprovenFile {
        path: "Report.kt".to_string(),
        role: FileRole::Code,
        why,
      })
      .into_iter()
      .collect(),
    proofs: vec![QuietProof::Formatting],
  }
}

fn range(name: &str) -> (String, String) {
  (format!("{name}-base"), format!("{name}-head"))
}

/// openspec/specs/review/range-reading/spec.md, "Memory: A final reading is kept on disk"
#[test]
fn a_final_reading_is_kept_on_disk_and_shows_in_the_next_process_at_once() {
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  let (proven, pending, unanswered) = (reading(None), reading(Some(Unproven::RenamesPending)), reading(Some(Unproven::RenamesUnanswered)));
  let ranges = [range("proven"), range("pending"), range("unanswered")];
  let plans = ReviewPlans::default();
  let no_index = &mut || false;
  keep_readings(&plans, &cache, "/repo", &[(&ranges[0], &proven), (&ranges[1], &pending), (&ranges[2], &unanswered)], false);
  assert_eq!(
    kept_readings(&plans, &ObjectCache::disabled(), "/repo", &ranges, no_index),
    [Some(proven.clone()), None, Some(unanswered.clone())],
    "memory: what is final without an index"
  );

  // the next start of the app: another memory, the same store
  let restarted = ReviewPlans::default();
  assert_eq!(
    kept_readings(&restarted, &cache, "/repo", &ranges, no_index),
    [Some(proven.clone()), None, Some(unanswered)],
    "the store: what is final without an index"
  );
  assert_eq!(
    kept_readings(&restarted, &ObjectCache::disabled(), "/repo", &ranges[..1], no_index),
    [Some(proven.clone())],
    "memory keeps what the store had"
  );
  assert_eq!(
    kept_readings(&restarted, &ObjectCache::disabled(), "/other", &ranges[..1], no_index),
    [None],
    "memory keeps it by its repository"
  );

  // an index of the repository built since may prove the renames: the reading without it is dropped, from memory too
  let mut asked = 0;
  let mut built = || {
    asked += 1;
    true
  };
  assert_eq!(kept_readings(&restarted, &cache, "/repo", &ranges, &mut built), [Some(proven), None, None]);
  assert_eq!(asked, 1, "the repository is asked once");
  assert_eq!(
    kept_readings(&restarted, &ObjectCache::disabled(), "/repo", &ranges[2..], no_index),
    [None],
    "memory forgot it"
  );

  // a reading the index of the repository left unanswered (its clock cut the query) is kept nowhere
  let fresh = ReviewPlans::default();
  let cut = reading(Some(Unproven::RenamesUnanswered));
  keep_readings(&fresh, &ObjectCache::disabled(), "/repo", &[(&range("cut"), &cut)], true);
  assert_eq!(kept_readings(&fresh, &ObjectCache::disabled(), "/repo", &[range("cut")], no_index), [None]);
}
