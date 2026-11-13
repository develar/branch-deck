//! The readings of ranges kept (`range_readings::keep_readings`, `kept_readings`): a final one in memory and on disk, so
//! the next process shows it at once; one that waits for the index nowhere; one whose renames no index proved in memory
//! alone, as an index built since may prove them

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
  keep_readings(&plans, &cache, "/repo", &[(&ranges[0], &proven), (&ranges[1], &pending), (&ranges[2], &unanswered)], false);
  assert_eq!(
    kept_readings(&plans, &ObjectCache::disabled(), "/repo", &ranges),
    [Some(proven.clone()), None, Some(unanswered)],
    "memory: what is final without an index"
  );

  // the next start of the app: another memory, the same store
  let restarted = ReviewPlans::default();
  assert_eq!(kept_readings(&restarted, &cache, "/repo", &ranges), [Some(proven.clone()), None, None]);
  assert_eq!(
    kept_readings(&restarted, &ObjectCache::disabled(), "/repo", &ranges[..1]),
    [Some(proven)],
    "memory keeps what the store had"
  );
  assert_eq!(
    kept_readings(&restarted, &ObjectCache::disabled(), "/other", &ranges[..1]),
    [None],
    "memory keeps it by its repository"
  );
}
