use super::{DiskReport, PoolReport, SlotReport};
use lease_pool::lease::SlotState;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn lines_name_the_slot_cache_source_age_and_elapsed_time() {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
  let report = DiskReport {
    free_bytes: None,
    min_free: None,
    pools: vec![PoolReport {
      git_common_dir: "/repo/.git".to_owned(),
      repository: Some("repo".to_owned()),
      max_slots: 10,
      past_limit: 0,
      bytes: 512,
      slots: vec![SlotReport {
        slot: "idea-1".to_owned(),
        path: "/pool/idea-1/repo".to_owned(),
        state: SlotState::Free,
        holder: None,
        returned: Some(1),
        bytes: 512,
        worktree_bytes: 512,
        build: Vec::new(),
        output_bases: Vec::new(),
        measured_at: now.saturating_sub(7200),
        cached: true,
        trims_next: false,
      }],
      checkout_bases: Vec::new(),
      trash_bytes: 0,
    }],
    orphans: Vec::new(),
    bytes: 512,
    ms: 17,
  };

  let lines = report.lines("This Mac");

  assert!(lines.iter().any(|line| line.starts_with("    idea-1: ") && line.ends_with("(cached, 2h ago)")), "{lines:?}");
  assert!(lines.iter().any(|line| line == "  measured in 17 ms"), "{lines:?}");
}
