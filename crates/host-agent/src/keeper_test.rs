use super::*;
use lease_pool::lease::SlotState;
use pool_disk::report::{PoolReport, SlotReport};

const GIB: u64 = 1 << 30;

fn slot(name: &str, bytes: u64) -> SlotReport {
  SlotReport {
    slot: name.to_owned(),
    path: format!("/pool/{name}"),
    state: SlotState::Ready,
    holder: None,
    returned: None,
    bytes,
    worktree_bytes: 0,
    build: Vec::new(),
    output_bases: Vec::new(),
    measured_at: 1,
    cached: true,
    trims_next: false,
  }
}

fn pool(repository: &str, slots: Vec<SlotReport>) -> PoolReport {
  PoolReport {
    git_common_dir: format!("{repository}/.git"),
    repository: Some(repository.to_owned()),
    max_slots: 10,
    past_limit: 0,
    bytes: slots.iter().map(|slot| slot.bytes).sum(),
    slots,
    checkout_bases: Vec::new(),
    trash_bytes: 0,
    notes_bytes: 0,
  }
}

/// A volume still below its minimum after the relief names the pools that hold the most, the largest slots of each, and
/// what a person can do, since the relief deleted all it may; a report without a measured byte names no pool
#[test]
fn the_line_of_a_volume_still_low_names_the_largest_pools_and_slots_and_what_a_person_can_do() {
  let report = DiskReport {
    free_bytes: Some(12 * GIB),
    min_free: Some(100 * GIB),
    pools: vec![
      pool("/Users/u/projects/accio", vec![slot("accio1", 90 * GIB), slot("accio2", 98 * GIB)]),
      pool(
        "/Users/u/projects/branch-deck",
        vec![
          slot("branch-deck1", 101 * GIB),
          slot("branch-deck2", 99 * GIB),
          slot("branch-deck3", 98 * GIB),
          slot("branch-deck4", 104 * GIB),
        ],
      ),
      pool("/Users/u/projects/empty", Vec::new()),
    ],
    orphans: Vec::new(),
    bytes: 0,
    ms: 1,
  };
  assert_eq!(
    holders_line(&report),
    "; the pools that hold the most: /Users/u/projects/branch-deck 402.0 GiB in 4 slot(s) (largest: branch-deck4 104.0 GiB, branch-deck1 101.0 GiB, branch-deck2 99.0 GiB); /Users/u/projects/accio 188.0 GiB in 2 slot(s) (largest: accio2 98.0 GiB, accio1 90.0 GiB). The relief deleted all it may: the rest is held by leases, the host slots of runs among them. A person frees more by returning the leases that `brd lease status` names on the host, by `brd-host trim <host> --max <n>`, which deletes the free slots past n, or by moving a pool to a larger volume: `git -C <repository> config branchdeck.leasePool <dir>` on the host, whose new slots go there"
  );
  let unmeasured = DiskReport { pools: Vec::new(), ..report };
  assert!(
    holders_line(&unmeasured).starts_with("; no pool has measured bytes yet (`brd-host disk <host> --fresh` measures them). The relief deleted all it may"),
    "{}",
    holders_line(&unmeasured)
  );
}
