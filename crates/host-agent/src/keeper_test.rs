use super::*;
use lease_pool::lease::{Ahead, SlotState};
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
    walk: None,
    parts: 0,
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

/// A free or leased slot of a shrink that freed `bytes`
fn shrunk_slot(name: &str, holder: Option<&str>, bytes: u64) -> ShrunkSlot {
  ShrunkSlot {
    name: name.to_owned(),
    path: format!("/pool/{name}"),
    holder: holder.map(str::to_owned),
    git_common_dir: "/repo/.git".to_owned(),
    cleaned: vec![lease_pool::lease_clean::Cleaned {
      rule: "cargo target".to_owned(),
      freed_bytes: bytes,
      elapsed: Duration::ZERO,
      error: None,
      trashed: Vec::new(),
      deferred: Vec::new(),
      profiles: lease_pool::lease_clean::Profiles::default(),
    }],
    trash_left: Vec::new(),
  }
}

/// The line of a prune ahead of the minimum names the threshold, the leased slots it pruned with their holders and what
/// each freed, the leased slots where cargo builds, and the free slots it cleaned; a leased slot that freed nothing is
/// not named. A relief below the minimum names the leased slots the same way
#[test]
fn the_line_of_a_prune_ahead_names_the_threshold_and_each_leased_slot_with_its_holder() {
  let shrunk = Shrunk {
    leased: vec![
      shrunk_slot("branch-deck1", Some("lease-keeper"), 12 * GIB),
      shrunk_slot("branch-deck2", Some("idle-agent"), 0),
    ],
    building: vec![lease_pool::shrink::BuildingSlot {
      name: "branch-deck3".to_owned(),
      path: "/pool/branch-deck3".to_owned(),
      holder: "debuginfo".to_owned(),
      profiles: vec!["target/debug".to_owned()],
    }],
    slots: vec![shrunk_slot("idea-15", None, 30 * GIB)],
    busy: Vec::new(),
    process_scan: true,
  };
  let relieved = Relieved {
    required: 100 * GIB,
    before: 120 * GIB,
    after: Some(162 * GIB),
    swept: false,
    reclaimed: Vec::new(),
    trimmed: None,
    shrunk: Some(shrunk),
    evicted: None,
    host_slots: Vec::new(),
    ahead: Some(Ahead {
      threshold: 150 * GIB,
      trash_freed: 0,
    }),
  };
  assert_eq!(
    relieved_line(Path::new("/repo"), &relieved, false),
    "the volume of the pools of /repo had 120.0 GiB free, less than 150.0 GiB, one and a half of `branchdeck.leaseMinFree` 100.0 GiB, below which the keeper prunes ahead of it: the prune of the leased slots by the cargo rules of their leases freed branch-deck1 (lease-keeper) 12.0 GiB, left the leased slot(s) branch-deck3 (debuginfo) as cargo builds there, the shrink freed 30.0 GiB of 1 free slot(s); now 162.0 GiB free"
  );
  let below = Relieved {
    before: 95 * GIB,
    after: Some(112 * GIB),
    ahead: None,
    ..relieved
  };
  assert!(
    relieved_line(Path::new("/repo"), &below, false).starts_with(
      "the volume of the pools of /repo had 95.0 GiB free, less than `branchdeck.leaseMinFree` 100.0 GiB: the prune of the leased slots by the cargo rules of their leases freed branch-deck1 (lease-keeper) 12.0 GiB,"
    ),
    "{}",
    relieved_line(Path::new("/repo"), &below, false)
  );
}

/// A prune ahead that ended below the minimum, as builds wrote faster than it deleted, is not a volume still low after
/// every step: it starts no wait of the relief, and its line has neither the wait nor what holds the space, so the next
/// round runs the whole relief. A relief below the minimum that leaves the volume below it is still low. The line of a
/// prune ahead names the trash that earlier deletes left, which it deleted first
#[test]
fn a_prune_ahead_that_ends_below_the_minimum_leaves_the_relief_to_the_next_round() {
  let ahead = Relieved {
    required: 100 * GIB,
    before: 101 * GIB,
    after: Some(99 * GIB),
    swept: false,
    reclaimed: Vec::new(),
    trimmed: None,
    shrunk: None,
    evicted: None,
    host_slots: Vec::new(),
    ahead: Some(Ahead {
      threshold: 150 * GIB,
      trash_freed: 3 * GIB,
    }),
  };
  assert!(!still_low(&ahead));
  assert_eq!(ahead_freed(&ahead), 3 * GIB);
  assert_eq!(
    relieved_line(Path::new("/repo"), &ahead, still_low(&ahead)),
    "the volume of the pools of /repo had 101.0 GiB free, less than 150.0 GiB, one and a half of `branchdeck.leaseMinFree` 100.0 GiB, below which the keeper prunes ahead of it: deleted the trash that earlier deletes left, 3.0 GiB; now 99.0 GiB free"
  );
  let below = Relieved {
    before: 95 * GIB,
    ahead: None,
    ..ahead
  };
  assert!(still_low(&below));
}

/// After a prune ahead of the minimum that freed bytes the next one may run at once, after ones that freed nothing it
/// waits 10 minutes, and a round with no prune ahead keeps the wait it had
#[test]
fn a_prune_ahead_that_freed_nothing_makes_the_next_one_wait() {
  let now = Instant::now();
  let waited = Some(now + Duration::from_mins(3));
  assert_eq!(next_ahead(waited, &[0, 5], now), None);
  assert_eq!(next_ahead(None, &[0], now), Some(now + Duration::from_mins(10)));
  assert_eq!(next_ahead(waited, &[], now), waited);
}
