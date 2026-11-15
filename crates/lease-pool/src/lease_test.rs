use crate::epoch::slot_epochs;
#[cfg(unix)]
use crate::lease::stop_servers;
use crate::lease::{
  DirtyCheck, GetStep, Lease, LeaseState, LeaseStore, Pool, RECLAIM_ROOM, Reclaimed, Slot, SlotMark, commit_takeover, glob_matches, repository_name, seed_candidates,
  stamp_reclaimed, stamp_returned,
};
use crate::shrink::{next_free, shrink_order};
use crate::trim::trim_order;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;
#[cfg(unix)]
use sync_utils::process_scan::{ProcessInfo, is_running, process_started};

#[test]
fn a_glob_matches_by_stars_question_marks_and_classes() {
  for (pattern, name) in [
    ("*.local.json", "settings.local.json"),
    ("*", ".env"),
    ("a?c", "abc"),
    ("[ab]x", "bx"),
    ("[a-c]x", "cx"),
    ("[!a]x", "bx"),
    ("[^a]x", "bx"),
    ("[]]x", "]x"),
    ("a[", "a["),
  ] {
    assert!(glob_matches(pattern, name), "{pattern} {name}");
  }
  for (pattern, name) in [("*.json", "a.txt"), ("a?c", "ac"), ("[ab]x", "cx"), ("[!a]x", "ax"), ("x", "xy")] {
    assert!(!glob_matches(pattern, name), "{pattern} {name}");
  }
}

/// Anchored literal paths, a glob in the last component of one directory, a directory; comments, negations, `**`, a
/// glob in a directory component and a path out of the root are left out
#[test]
fn the_seed_candidates_are_literal_paths_and_globs_of_one_directory() {
  let root = tempfile::tempdir().unwrap();
  let write = |path: &str| {
    let path = root.path().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "x").unwrap();
  };
  write(".claude/settings.local.json");
  write("config/a.local.json");
  write("config/b.local.json");
  write("config/c.json");
  write(".env");
  write("cache/data.bin");
  write("deep/config/d.local.json");
  let text =
    "# local files\n/.claude/settings.local.json\n\n/config/*.local.json\n.env\n/cache/\n!/config/a.local.json\n**/d.local.json\n/*/config/*.json\n../outside\n/missing.txt\n";
  assert_eq!(
    seed_candidates(root.path(), text),
    [".claude/settings.local.json", "config/a.local.json", "config/b.local.json", ".env", "cache"]
  );
}

/// A dirty verdict holds while one dirty path has its stat of then: the other paths may change, a path deleted or
/// rewritten counts as changed, and once every path changed the verdict is gone. A path gone then and gone still is
/// unchanged (a deleted tracked file); `.gitignore` written anew ends it
#[test]
fn a_dirty_verdict_holds_while_one_dirty_path_has_its_stat() {
  let dir = tempfile::tempdir().unwrap();
  let slot = dir.path();
  std::fs::write(slot.join("a.txt"), "a\n").unwrap();
  std::fs::write(slot.join("b.txt"), "b\n").unwrap();
  let check = DirtyCheck::of(slot, vec!["a.txt".to_string(), "b.txt".to_string()]);
  assert!(check.holds(slot));
  std::fs::remove_file(slot.join("a.txt")).unwrap();
  assert!(check.holds(slot), "b.txt proves it");
  std::fs::remove_file(slot.join("b.txt")).unwrap();
  std::fs::write(slot.join("b.txt"), "other\n").unwrap();
  assert!(!check.holds(slot), "every path changed");

  let gone = DirtyCheck::of(slot, vec!["deleted.txt".to_string()]);
  assert!(gone.holds(slot));
  std::fs::write(slot.join(".gitignore"), "/deleted.txt\n").unwrap();
  assert!(!gone.holds(slot), "a new .gitignore");
}

#[test]
fn the_repository_name_is_the_directory_of_its_main_worktree() {
  assert_eq!(repository_name(Path::new("/projects/idea-1/.git")), "idea-1");
  assert_eq!(repository_name(Path::new("/srv/ultimate.git")), "ultimate");
  assert_eq!(repository_name(Path::new("/srv/bare")), "bare");
}

/// A fake Bazel server of `pid` with the start time `started`
#[cfg(unix)]
fn server(pid: u32, started: Option<u64>) -> ProcessInfo {
  ProcessInfo {
    pid,
    parent: 0,
    group: 0,
    name: "bazel(fake)".to_string(),
    cwd: None,
    argv: vec!["bazel(fake)".to_string(), "--workspace_directory=/slot".to_string()],
    started,
  }
}

/// A server whose start time is unknown is never signalled and stays running; one whose pid now has another start time
/// (the system reused the pid) is gone, and the process with that pid is not signalled either
#[cfg(unix)]
#[test]
fn a_server_is_signalled_only_while_its_pid_has_the_start_time_of_the_scan() {
  let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let pid = child.id();
  let started = process_started(pid);
  assert!(started.is_some());

  let unknown = server(pid, None);
  assert_eq!(stop_servers(&[&unknown], None, Duration::from_millis(200)).stopped, Vec::new());
  assert!(child.try_wait().unwrap().is_none(), "a server without a start time was signalled");

  let reused = server(pid, started.map(|started| started + 1));
  let stopped = stop_servers(&[&reused], None, Duration::from_millis(200)).stopped;
  assert_eq!(stopped.iter().map(|stopped| stopped.pid).collect::<Vec<_>>(), [pid], "the scanned server is gone");
  assert!(child.try_wait().unwrap().is_none(), "the process that got the pid was signalled");
  assert!(is_running(pid, started));

  child.kill().unwrap();
  child.wait().unwrap();
}

/// The commit point of a takeover holds only while the slot is as the check found it: a commit of a resumed session in
/// the slot meanwhile (another mark) stops it and leaves the state alone; the mark the check saw moves the idle lease
/// into the record of the slot
#[test]
fn a_commit_during_the_check_stops_the_takeover() {
  let dir = tempfile::tempdir().unwrap();
  let pool = Pool::new(dir.path());
  let lease = |id: &str, state: LeaseState| Lease {
    id: id.to_string(),
    holder: id.to_string(),
    leased_at: 1,
    state,
    pid: std::process::id(),
    process_started: None,
    source_head: "a".repeat(40),
    source: String::new(),
    source_branch: Some("master".to_string()),
    branch: None,
    seeded: Vec::new(),
    reused: true,
    holder_pid: None,
    holder_started: None,
    taking_over: None,
    idle_check: None,
    reclaim_failed_at: None,
    pruned_at: None,
  };
  let get = Lease {
    taking_over: Some(Box::new(lease("idle", LeaseState::Ready))),
    ..lease("get", LeaseState::Preparing)
  };
  // no such directory: the mark of the slot is the empty one
  let path = dir.path().join("slot").to_string_lossy().into_owned();
  let before = LeaseStore {
    slots: vec![Slot {
      name: "slot".to_string(),
      path: path.clone(),
      dirty: false,
      dirty_check: None,
      returned: None,
      returned_at: None,
      lease: Some(get.clone()),
      reclaimed: None,
    }],
    last_returned: None,
  };
  pool
    .update(Duration::from_secs(5), |store| {
      *store = before.clone();
      Ok(())
    })
    .unwrap();
  let moved = SlotMark {
    head: "ref: refs/heads/lane".to_string(),
    log_len: 1,
    log_modified: 1,
  };
  assert!(!commit_takeover(&pool, Duration::from_secs(5), &path, &get, &moved).unwrap());
  assert_eq!(pool.read().unwrap(), before);

  assert!(commit_takeover(&pool, Duration::from_secs(5), &path, &get, &SlotMark::default()).unwrap());
  let slot = pool.read().unwrap().slots.remove(0);
  assert_eq!(slot.lease.as_ref().map(|lease| (lease.id.as_str(), lease.taking_over.is_none())), Some(("get", true)));
  let reclaimed = slot.reclaimed.unwrap();
  assert_eq!((reclaimed.lease_id.as_str(), reclaimed.by_lease_id.as_deref()), ("idle", Some("get")));
}

/// The wait for the shrink or the eviction of another get names the holder, how long it holds the lock, and the free
/// space of the volume against the minimum, so a wait of minutes reads as disk pressure, never as a hang
#[test]
fn the_wait_for_the_shrink_of_another_names_the_holder_its_time_and_the_free_space() {
  let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - 600;
  let holder = history_rewrite::repository_lock::LockHolder {
    command: "brd lease get --holder win-native -C ~/projects/idea-1".to_string(),
    pid: 4242,
    process_started: None,
    since,
    subject: None,
  };
  let step = GetStep::WaitForPressure {
    holder,
    free: Some(195 << 30),
    minimum: 200 << 30,
  };
  let words = step.to_string();
  assert!(
    words.starts_with("waiting for the shrink or the eviction of another get, `brd lease get --holder win-native -C ~/projects/idea-1` (pid 4242, for 10 min"),
    "{words}"
  );
  assert!(
    words.ends_with("the volume at 195.0 GiB of the 200.0 GiB this get needs: the free space is measured again after it, or once it has the minimum"),
    "{words}"
  );
}

/// A free slot `n` of a pool under `root`, its directory there, returned as the `returned`-th at `returned_at`
fn returned_slot(root: &Path, n: u64, returned: Option<u64>, returned_at: Option<u64>) -> Slot {
  let path = root.join(n.to_string());
  std::fs::create_dir_all(&path).unwrap();
  Slot {
    name: format!("slot{n}"),
    path: path.to_string_lossy().into_owned(),
    dirty: false,
    dirty_check: None,
    returned,
    returned_at,
    lease: None,
    reclaimed: None,
  }
}

/// The names of the free slots of one pool in the order of each reader: a get (`next_free` first), the trim
/// (`trim_order`), the shrink (`shrink_order`, the next slot of the get last), and the time first, as a brd of
/// `Slot::return_order` ordered a pool
fn orders(slots: &[Slot]) -> (Option<String>, Vec<String>, Vec<String>, Vec<String>) {
  let names = |order: Vec<&Slot>| order.into_iter().map(|slot| slot.name.clone()).collect::<Vec<_>>();
  let mut by_time: Vec<&Slot> = slots.iter().filter(|slot| slot.lease.is_none()).collect();
  by_time.sort_by_key(|slot| (slot.returned_at, slot.returned));
  (
    next_free(slots.iter().filter(|slot| slot.lease.is_none())).map(|slot| slot.name.clone()),
    names(trim_order(slots).1),
    names(shrink_order(&[slots], |_| true).into_iter().map(|(_, slot)| slot).collect()),
    names(by_time),
  )
}

/// `stamp_reclaimed` of the slot `index` of the pool `slots`
fn reclaim(slots: &mut Vec<Slot>, index: usize, quiet_at: u64) {
  let mut store = LeaseStore {
    slots: std::mem::take(slots),
    last_returned: None,
  };
  stamp_reclaimed(&mut store, index, quiet_at);
  *slots = store.slots;
}

/// `stamp_returned` of the slot `index` of the pool `slots`
fn give_back(slots: &mut Vec<Slot>, index: usize, now: u64) {
  let mut store = LeaseStore {
    slots: std::mem::take(slots),
    last_returned: None,
  };
  stamp_returned(&mut store, index, now);
  *slots = store.slots;
}

/// `slot` under a lease that its return or reclaim holds
fn returning(mut slot: Slot) -> Slot {
  slot.lease = Some(Lease::marker("lease", "agent", LeaseState::Returning));
  slot
}

/// Every reader orders the free slots of `slots` as `coldest_first`: a get takes the last first, the trim and the
/// shrink take the first first, and the time orders them as the number does
fn assert_coldest_first(slots: &[Slot], coldest_first: &[&str]) {
  let names: Vec<String> = coldest_first.iter().map(|name| (*name).to_owned()).collect();
  assert_eq!(orders(slots), (names.last().cloned(), names.clone(), names.clone(), names));
}

/// A reclaim gives its slot a number below every other free slot of the pool and a time not after any of theirs
/// (`stamp_reclaimed`), so a get takes it last, and the trim and the shrink take it first, by the number alone or by the
/// time first: every brd that reads the state orders it the same. No other slot changes its number, and the reclaimed
/// slot does not get its number from before the lease again
#[test]
fn a_reclaimed_slot_is_the_coldest_free_slot_of_its_pool_by_number_and_by_time() {
  let dir = tempfile::tempdir().unwrap();
  // the reclaimed slot had 2 before its lease, which went quiet at 1000; slot 3 was returned before it, 4 and 5 after
  let mut slots = vec![
    returned_slot(dir.path(), 3, Some(3), Some(500)),
    returned_slot(dir.path(), 4, Some(4), Some(1_100)),
    returned_slot(dir.path(), 5, Some(5), Some(1_200)),
    returning(returned_slot(dir.path(), 2, Some(2), Some(400))),
    returning(returned_slot(dir.path(), 9, Some(1), Some(300))),
  ];
  reclaim(&mut slots, 3, 1_000);
  slots[3].lease = None;
  let numbers = |slots: &[Slot]| slots.iter().map(|slot| (slot.returned, slot.returned_at)).collect::<Vec<_>>();
  assert_eq!(
    numbers(&slots),
    [
      (Some(3), Some(500)),
      (Some(4), Some(1_100)),
      (Some(5), Some(1_200)),
      (Some(1), Some(500)),
      (Some(1), Some(300))
    ]
  );
  assert_coldest_first(&slots, &["slot2", "slot3", "slot4", "slot5"]);

  // a free slot of a brd before `returned_at`: the reclaimed slot has no time either, so it comes first by the time too
  let mut slots = vec![
    returned_slot(dir.path(), 3, Some(3), None),
    returned_slot(dir.path(), 4, Some(4), Some(1_100)),
    returning(returned_slot(dir.path(), 2, Some(7), Some(400))),
  ];
  reclaim(&mut slots, 2, 1_000);
  slots[2].lease = None;
  assert_eq!(numbers(&slots)[2], (Some(2), None));
  assert_coldest_first(&slots, &["slot2", "slot3", "slot4"]);

  // no other free slot that was returned: one past the last number of the pool, as of when its lease went quiet
  let mut slots = vec![
    returned_slot(dir.path(), 3, None, None),
    returning(returned_slot(dir.path(), 9, Some(6), Some(300))),
    returning(returned_slot(dir.path(), 2, Some(2), Some(400))),
  ];
  reclaim(&mut slots, 2, 1_000);
  assert_eq!(numbers(&slots)[2], (Some(7), Some(1_000)));
}

/// With no number left below the other free slots (the lowest is 0, or 1 and the reclaimed slot had 0), they move up
/// by `RECLAIM_ROOM` and the reclaimed slot takes the number below them; a leased slot keeps its number
#[test]
fn a_reclaim_with_no_number_left_below_moves_the_free_slots_up_once() {
  let dir = tempfile::tempdir().unwrap();
  for (own, lowest) in [(Some(0), 1), (Some(5), 0)] {
    let mut slots = vec![
      returned_slot(dir.path(), 3, Some(lowest), Some(500)),
      returned_slot(dir.path(), 4, Some(4), Some(1_100)),
      returning(returned_slot(dir.path(), 9, Some(2), Some(300))),
      returning(returned_slot(dir.path(), 2, own, Some(400))),
    ];
    reclaim(&mut slots, 3, 1_000);
    slots[3].lease = None;
    let numbers: Vec<Option<u64>> = slots.iter().map(|slot| slot.returned).collect();
    let room = RECLAIM_ROOM;
    assert_eq!(numbers, [Some(lowest + room), Some(4 + room), Some(2), Some(lowest + room - 1)], "own {own:?}");
    assert_coldest_first(&slots, &["slot2", "slot3", "slot4"]);
  }
}

/// A return after a step back of the clock is the warmest free slot of its pool for every reader: its number is one
/// past the last, and its time is that of the last return of the pool, not the earlier time of the clock
/// (`stamp_returned`)
#[test]
fn a_return_after_a_step_back_of_the_clock_is_the_warmest_free_slot_of_its_pool() {
  let dir = tempfile::tempdir().unwrap();
  let mut slots = vec![
    returned_slot(dir.path(), 1, Some(1), Some(1_900)),
    returned_slot(dir.path(), 2, Some(2), Some(2_000)),
    returned_slot(dir.path(), 3, Some(3), Some(1_000)),
  ];
  slots[2].lease = Some(Lease::marker("lease", "agent", LeaseState::Returning));
  give_back(&mut slots, 2, 1_500);
  slots[2].lease = None;
  assert_eq!((slots[2].returned, slots[2].returned_at), (Some(4), Some(2_000)));
  assert_coldest_first(&slots, &["slot1", "slot2", "slot3"]);
  // a clock that goes on records its own time
  slots[0].lease = Some(Lease::marker("lease", "agent", LeaseState::Returning));
  give_back(&mut slots, 0, 2_100);
  assert_eq!((slots[0].returned, slots[0].returned_at), (Some(5), Some(2_100)));
  // a state that a brd without the bound wrote after a step back: the number orders the pool for a get and the trim
  let slots = [returned_slot(dir.path(), 1, Some(1), Some(2_000)), returned_slot(dir.path(), 2, Some(2), Some(1_500))];
  let (next, trim, _, _) = orders(&slots);
  assert_eq!((next, trim), (Some("slot2".to_owned()), vec!["slot1".to_owned(), "slot2".to_owned()]));
}

/// The slots of a pool through gets, returns and reclaims, with the epoch of each free period of each slot so far
struct PoolLife {
  store: LeaseStore,
  epochs: Vec<HashSet<String>>,
  leases: u32,
  now: u64,
  moves: u32,
}

impl PoolLife {
  /// A pool of `count` leased slots under `root`
  fn new(root: &Path, count: u64) -> Self {
    let slots = (0..count).map(|n| returning(returned_slot(root, n, None, None))).collect();
    Self {
      store: LeaseStore { slots, last_returned: None },
      epochs: (0..count).map(|_| HashSet::new()).collect(),
      leases: 0,
      now: 1_000,
      moves: 0,
    }
  }

  fn get(&mut self, index: usize) {
    self.leases += 1;
    let slot = &mut self.store.slots[index];
    assert!(slot.lease.is_none(), "slot {index} is free");
    slot.lease = Some(Lease::marker(&format!("lease{}", self.leases), "agent", LeaseState::Ready));
    // as `reserve` does
    slot.reclaimed = None;
  }

  fn give_back(&mut self, index: usize) {
    self.now += 10;
    stamp_returned(&mut self.store, index, self.now);
    self.store.slots[index].lease = None;
    self.check_free_period(index);
  }

  /// A reclaim of the lease of the slot `index`, as `lease_reclaim_core` does
  fn reclaim(&mut self, index: usize) {
    self.now += 10;
    let others = |store: &LeaseStore| {
      store
        .slots
        .iter()
        .enumerate()
        .filter(|(other, slot)| *other != index && slot.lease.is_none())
        .map(|(_, slot)| slot.returned)
        .collect::<Vec<_>>()
    };
    let before = others(&self.store);
    stamp_reclaimed(&mut self.store, index, self.now - 5);
    if others(&self.store) != before {
      self.moves += 1;
    }
    let slot = &mut self.store.slots[index];
    let lease = slot.lease.take().expect("a reclaim takes a lease");
    slot.reclaimed = Some(Reclaimed::of(&lease, None));
    self.check_free_period(index);
  }

  /// The epoch of the free slot `index` is none of an earlier free period of it: a record of the app under that epoch,
  /// and a measure of `pool-disk`, which compares the same number and reclaimed lease, would read as current
  fn check_free_period(&mut self, index: usize) {
    let epochs = slot_epochs(&serde_json::to_vec(&self.store).unwrap()).unwrap();
    let epoch = epochs[&self.store.slots[index].path].clone();
    assert!(
      self.epochs[index].insert(epoch.clone()),
      "slot {index} has the epoch `{epoch}` of an earlier free period: {:?}",
      self.store
    );
  }
}

/// A reclaim gives its slot a number below the other free slots, which can be one that the slot had in an earlier free
/// period. The epoch of a reclaimed slot names the reclaimed lease, so it differs, and `last_returned` keeps the highest
/// number the reclaimed slot gave up, so a later return does not repeat it either
#[test]
fn no_free_period_of_a_slot_has_the_epoch_of_an_earlier_one() {
  let dir = tempfile::tempdir().unwrap();
  // the review of the reclaim: A is returned with 1, B with 2 (dirty), C with 3; gets take C and A; A is returned with
  // 4 and leased again, then reclaimed below B, with 1
  let mut life = PoolLife::new(dir.path(), 3);
  for index in 0..3 {
    life.give_back(index);
  }
  life.get(2);
  life.get(0);
  life.give_back(0);
  life.get(0);
  life.reclaim(0);
  assert_eq!(life.store.slots[0].returned, Some(1), "below the lowest other free slot");

  // the slot of the highest number gives it up to a reclaim; its next return does not get it again
  let mut life = PoolLife::new(dir.path(), 2);
  life.give_back(1);
  life.give_back(0);
  life.get(0);
  life.reclaim(0);
  life.get(1);
  life.get(0);
  life.give_back(0);
  assert_eq!((life.store.slots[0].returned, life.store.last_returned), (Some(3), Some(3)));

  // a long life with every move: a get takes any free slot, as a dirty one is skipped
  let mut life = PoolLife::new(dir.path(), 4);
  let mut seed: u64 = 7;
  for _ in 0..3_000 {
    seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let index = usize::try_from((seed >> 33) % 4).unwrap();
    if life.store.slots[index].lease.is_none() {
      life.get(index);
    } else if (seed >> 20) % 3 == 0 {
      life.give_back(index);
    } else {
      life.reclaim(index);
    }
  }
  assert!(life.moves > 0, "the life moved the free slots up");
}

/// A prune of a leased slot whose lease changed after the shrink read it (a return and a new get came between) prunes
/// nothing: under the locks of cargo the slot holds another lease
#[cfg(unix)]
#[test]
fn the_prune_of_a_leased_slot_whose_lease_changed_prunes_nothing() {
  let common = tempfile::tempdir().unwrap();
  let slot_dir = tempfile::tempdir().unwrap();
  let slot = slot_dir.path();
  let git = git_executor::git_command_executor::GitCommandExecutor::new();
  git.execute_command(&["init", "-q"], slot.to_str().unwrap()).unwrap();
  std::fs::write(slot.join(".branchdeck.toml"), "[lease]\nclean = [{ cargo = \"target\", keep = 0, age = \"0s\" }]\n").unwrap();
  let unit = slot.join("target/debug/build/app/old/fingerprint");
  std::fs::create_dir_all(&unit).unwrap();
  std::fs::write(unit.join("dep"), "x").unwrap();
  std::fs::write(slot.join("target/debug/.cargo-lock"), "").unwrap();
  let read = Lease::marker("old", "agent", LeaseState::Ready);
  let current = Lease::marker("new", "agent", LeaseState::Ready);
  let pool = Pool::new(common.path());
  let path = slot.to_string_lossy().into_owned();
  pool
    .update(Duration::from_secs(5), |store: &mut LeaseStore| {
      store.slots.push(Slot {
        name: "slot".to_string(),
        path: path.clone(),
        dirty: false,
        dirty_check: None,
        returned: None,
        returned_at: None,
        lease: Some(current.clone()),
        reclaimed: None,
      });
      Ok(())
    })
    .unwrap();
  let trash = crate::lease_clean::Trash::new(&pool.trash_dir());
  let pruned = crate::lease::prune_leased_slot(&git, &pool, Duration::from_secs(5), (&path, &read), crate::lease_clean::CleanMode::Trash(&trash)).unwrap();
  assert!(pruned.is_none(), "{pruned:?}");
  assert!(unit.exists());
  let stamped = pool.read().unwrap().slots[0].lease.as_ref().and_then(|lease| lease.pruned_at);
  assert_eq!(stamped, None);
  let pruned = crate::lease::prune_leased_slot(&git, &pool, Duration::from_secs(5), (&path, &current), crate::lease_clean::CleanMode::Trash(&trash))
    .unwrap()
    .unwrap();
  assert_eq!(pruned.cleaned[0].profiles.pruned, 1);
  assert!(!unit.exists());
  assert!(pool.read().unwrap().slots[0].lease.as_ref().and_then(|lease| lease.pruned_at).is_some());
}
