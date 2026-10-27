//! The host slots (`lease_pool::host_slots`): a fill leases them as plain `ready` leases of `bd-host/<n>` that the other
//! commands of the pool leave alone, a run gets the slot of its affinity, else the one used longest ago, else waits in
//! line; a give-back stops the processes of the run and discards its changes, and quarantines the slot for an older
//! process; `reclaim` takes up what a gone process left; the clean rules run by size, by free space and after 50 runs;
//! a shrink returns its slots without `--confirm-preserved`, also without `bd host serve`; a hand return is refused

use crate::lease_command::LeaseContext;
#[cfg(unix)]
use crate::lease_test::busy_in;
use crate::lease_test::{Fixture, path_of, slot_git};
use bd_cli::fixture_test::{assert_error, git};
use lease_pool::host_slots::{
  Filled, HostSlots, HostSlotsParams, HostState, Lend, LendAnswer, LendRequest, ReclaimAction, ShrinkOutcome, fill, give_back, lend, read_host_slots, reclaim, set_limit, shrink,
};
use lease_pool::lease::{LeaseState, Pool, canonical_path};
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::path::{Path, PathBuf};
use sync_utils::process_scan::process_started;

/// The rules of the fixture: `out/` is ignored, and a clean deletes `out/cache`
const RULES: &str = "[lease]\nclean = [\n  { delete = \"out/cache\" },\n]\n";

/// A fixture whose repository keeps `slots` host slots, with the rules and the `.gitignore` of `RULES`
pub(crate) fn host_fixture(slots: u32) -> Fixture {
  let fixture = Fixture::new();
  fixture
    .repo
    .create_commit_with_files("Add the clean rules", &[(".gitignore", "out/\n"), (".branchdeck.toml", RULES)]);
  git(&fixture.repo, &["config", "branchdeck.hostSlots", &slots.to_string()]);
  fixture
}

/// The context of bd in the repository of the fixture
fn context(fixture: &Fixture) -> LeaseContext {
  LeaseContext::resolve(&fixture.environment(fixture.repo.path()), None).unwrap()
}

fn params<'a>(fixture: &'a Fixture, context: &'a LeaseContext) -> HostSlotsParams<'a> {
  HostSlotsParams {
    repository: &context.dirs,
    home: Some(fixture.home.path()),
    lock_timeout: context.lock_timeout,
    server_timeout: context.server_stop_timeout,
  }
}

/// A fixture with its host slots filled
struct Host {
  fixture: Fixture,
  context: LeaseContext,
  filled: Vec<Filled>,
}

impl Host {
  fn filled(slots: u32) -> Self {
    let fixture = host_fixture(slots);
    let context = context(&fixture);
    let filled = fill(&context.git, &params(&fixture, &context), None).unwrap();
    Self { fixture, context, filled }
  }

  /// The fixture, the context of bd and the parameters of the host slots
  fn parts(&self) -> (&Fixture, &LeaseContext, HostSlotsParams<'_>) {
    (&self.fixture, &self.context, params(&self.fixture, &self.context))
  }
}

/// The lend of `run` to this process, which lives as long as the test
fn lend_to(params: &HostSlotsParams<'_>, run: &str, affinity: Option<&str>) -> LendAnswer {
  let pid = std::process::id();
  lend(
    params,
    &LendRequest {
      run,
      pid,
      process_started: process_started(pid),
      affinity,
    },
  )
  .unwrap()
}

fn lent(answer: LendAnswer) -> Lend {
  match answer {
    LendAnswer::Lent(lend) => lend,
    LendAnswer::Queued(queued) => panic!("queued: {queued:?}"),
  }
}

/// The pid and the start of a process that has exited
pub(crate) fn gone_process() -> (u32, Option<u64>) {
  let mut command = if cfg!(windows) {
    let mut command = std::process::Command::new("cmd");
    command.args(["/C", "exit"]);
    command
  } else {
    std::process::Command::new("true")
  };
  let mut child = command.spawn().unwrap();
  let pid = child.id();
  let started = process_started(pid);
  child.wait().unwrap();
  (pid, started)
}

/// A lend of `run` to a process that is gone
fn lend_to_gone(params: &HostSlotsParams<'_>, run: &str) -> Lend {
  let (pid, process_started) = gone_process();
  lent(
    lend(
      params,
      &LendRequest {
        run,
        pid,
        process_started,
        affinity: None,
      },
    )
    .unwrap(),
  )
}

fn table(fixture: &Fixture) -> HostSlots {
  read_host_slots(&fixture.repo.path().join(".git")).unwrap()
}

/// `$GIT_COMMON_DIR/branch-deck/host-slots.json`
fn table_file(fixture: &Fixture) -> PathBuf {
  fixture.repo.path().join(".git/branch-deck/host-slots.json")
}

/// Rewrite the table as a process that crashed, or time, would have left it
fn edit_table(fixture: &Fixture, edit: impl FnOnce(&mut Value)) {
  let mut value: Value = serde_json::from_slice(&std::fs::read(table_file(fixture)).unwrap()).unwrap();
  edit(&mut value);
  std::fs::write(table_file(fixture), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn state_of(fixture: &Fixture, n: u32) -> HostState {
  table(fixture)
    .slots
    .into_iter()
    .find(|slot| slot.n == n)
    .unwrap_or_else(|| panic!("no host slot {n}"))
    .state
}

/// A fill leases `bd-host/1` and `bd-host/2` as `ready` leases, the first in the free slot of the pool. A plain get adds
/// a slot of its own, `clean --free` skips them, and a get with a holder `bd-host/…` is refused
#[test_log::test]
fn a_fill_leases_the_host_slots_as_ready_leases_that_the_other_commands_leave_alone() {
  let fixture = host_fixture(2);
  let free = path_of(&fixture.get(&["--holder", "lane-1"]));
  fixture.give_back(&free);
  let context = context(&fixture);
  let params = params(&fixture, &context);

  let filled = fill(&context.git, &params, None).unwrap();
  assert_eq!(filled.iter().map(|slot| (slot.n, slot.reused)).collect::<Vec<_>>(), [(1, true), (2, false)]);
  assert_eq!(filled[0].path, free);
  assert!(fill(&context.git, &params, None).unwrap().is_empty(), "a full table adds nothing");
  let store = Pool::new(Path::new(&canonical_path(&fixture.repo.path().join(".git")))).read().unwrap();
  let leases: Vec<(String, String, LeaseState)> = store
    .slots
    .iter()
    .filter_map(|slot| slot.lease.as_ref().map(|lease| (slot.path.clone(), lease.holder.clone(), lease.state)))
    .collect();
  assert_eq!(
    leases,
    [
      (filled[0].path.clone(), "bd-host/1".to_string(), LeaseState::Ready),
      (filled[1].path.clone(), "bd-host/2".to_string(), LeaseState::Ready)
    ]
  );
  assert!(table(&fixture).slots.iter().all(|slot| slot.state == HostState::Idle));

  let other = path_of(&fixture.get(&["--holder", "lane-2"]));
  assert!(filled.iter().all(|slot| slot.path != other), "a get never takes a host slot");
  fixture.give_back(&other);
  let (exit_code, cleaned) = fixture.run(&["lease", "clean", "--free"]);
  assert_eq!(exit_code, 0, "{cleaned}");
  let targets: Vec<&str> = cleaned["targets"].as_array().unwrap().iter().map(|target| target["path"].as_str().unwrap()).collect();
  assert_eq!(targets, [other.as_str()]);
  let (exit_code, refused) = fixture.run(&["lease", "get", "--holder", "bd-host/9"]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
}

/// A hand `bd lease return` of a host slot is refused with `host_slot`, and the slot stays leased
#[test_log::test]
fn a_hand_return_of_a_host_slot_is_refused() {
  let host = Host::filled(1);
  let (fixture, path) = (&host.fixture, &host.filled[0].path);
  let (exit_code, refused) = fixture.run_in(Path::new(&path), &["lease", "return"]);
  assert_error(&refused, exit_code, (4, "host_slot"));
  assert_eq!(refused["error"]["paths"], serde_json::json!([path]));
  assert_eq!(state_of(fixture, 1), HostState::Idle);
}

/// The slot that the run's checkout used last is lent to it; else the idle slot used longest ago; with every slot lent
/// a run waits in line, a run behind it waits for it, and the ticket of a gone process leaves the line
#[test_log::test]
fn a_run_gets_the_slot_of_its_affinity_else_the_one_used_longest_ago_else_waits_in_line() {
  let host = Host::filled(2);
  let (fixture, context, params) = host.parts();

  let first = lent(lend_to(&params, "run-1", Some("checkout-a")));
  assert_eq!((first.n, first.last_used_by_affinity), (1, None));
  assert_eq!(lent(lend_to(&params, "run-1", Some("checkout-a"))).n, 1, "a repeat gets the same slot");
  give_back(&context.git, &params, 1, "run-1").unwrap();
  let second = lent(lend_to(&params, "run-2", Some("checkout-b")));
  assert_eq!(second.n, 2, "the slot never used is the one used longest ago");
  give_back(&context.git, &params, 2, "run-2").unwrap();
  let third = lent(lend_to(&params, "run-3", Some("checkout-a")));
  assert_eq!(third.n, 1, "the slot of the affinity");
  assert!(third.last_used_by_affinity.is_some());
  let fourth = lent(lend_to(&params, "run-4", None));
  assert_eq!(fourth.n, 2);

  let LendAnswer::Queued(waiting) = lend_to(&params, "run-5", None) else {
    panic!("every slot is lent")
  };
  assert_eq!(
    (waiting.ahead, waiting.slots, waiting.lent),
    (0, 2, vec![(1, "run-3".to_string()), (2, "run-4".to_string())])
  );
  let LendAnswer::Queued(behind) = lend_to(&params, "run-6", None) else {
    panic!("run-5 waits before it")
  };
  assert_eq!(behind.ahead, 1);
  give_back(&context.git, &params, 1, "run-3").unwrap();
  assert!(matches!(lend_to(&params, "run-6", None), LendAnswer::Queued(queued) if queued.ahead == 1), "run-5 is first");
  assert_eq!(lent(lend_to(&params, "run-5", None)).n, 1);
  assert!(matches!(lend_to(&params, "run-6", None), LendAnswer::Queued(queued) if queued.ahead == 0));

  // a ticket whose process is gone leaves the line
  edit_table(fixture, |table| {
    let (pid, _) = gone_process();
    table["queue"][0]["pid"] = Value::from(pid);
  });
  give_back(&context.git, &params, 2, "run-4").unwrap();
  let LendAnswer::Lent(late) = lend_to(&params, "run-7", None) else {
    panic!("the line is empty")
  };
  assert_eq!(late.n, 2);
  assert_eq!(table(fixture).queue, []);
}

/// A give-back discards the changes of the run, untracked files too, and keeps the ignored ones; a lend whose process is
/// gone is not lent again until `reclaim` gave it back
#[test_log::test]
fn an_abandoned_lend_is_given_back_by_reclaim_before_another_run_gets_the_slot() {
  let host = Host::filled(1);
  let (_, context, params) = host.parts();
  let slot = lend_to_gone(&params, "run-1");
  let path = Path::new(&slot.path);
  std::fs::write(path.join("a.txt"), "changed\n").unwrap();
  std::fs::write(path.join("untracked.txt"), "x\n").unwrap();
  std::fs::create_dir_all(path.join("out")).unwrap();
  std::fs::write(path.join("out/keep"), "warm\n").unwrap();

  assert!(matches!(lend_to(&params, "run-2", None), LendAnswer::Queued(_)), "an abandoned lend is not lent again");
  let reclaimed = reclaim(&context.git, &params).unwrap();
  assert_eq!(
    reclaimed.iter().map(|done| (done.n, done.action, done.error.clone())).collect::<Vec<_>>(),
    [(1, ReclaimAction::GaveBack, None)]
  );
  assert_eq!(slot_git(&slot.path, &["status", "--porcelain"]), "");
  assert!(path.join("out/keep").exists(), "the ignored files stay as the warm cache");
  let next = lent(lend_to(&params, "run-2", None));
  assert_eq!((next.n, next.generation), (1, 1), "the give-back discarded changes: a new generation of the files");
}

/// A process that the run started in its slot is stopped by the give-back; a process older than the lend is never
/// signalled: it quarantines the slot, no run gets it, and `reclaim` ends the quarantine once the process is gone
#[cfg(unix)]
#[test_log::test]
fn the_give_back_stops_the_processes_of_the_run_and_quarantines_the_slot_for_an_older_one() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();

  let slot = lent(lend_to(&params, "run-1", None));
  // the lend a moment earlier: on Linux the start of a process is known to the second
  edit_table(fixture, |table| {
    let since = table["slots"][0]["state"]["since_ms"].as_u64().unwrap();
    table["slots"][0]["state"]["since_ms"] = Value::from(since - 5000);
  });
  let mut runs = busy_in(&slot.path);
  let pid = runs.id();
  let reaped = std::thread::spawn(move || runs.wait().unwrap());
  let gave_back = give_back(&context.git, &params, 1, "run-1").unwrap();
  assert!(!reaped.join().unwrap().success(), "the process of the run was stopped");
  assert_eq!(
    (gave_back.stopped.iter().map(|process| process.pid).collect::<Vec<_>>(), gave_back.quarantined),
    (vec![pid], vec![])
  );
  assert_eq!(state_of(fixture, 1), HostState::Idle);

  let mut older = busy_in(&slot.path);
  lent(lend_to(&params, "run-2", None));
  let gave_back = give_back(&context.git, &params, 1, "run-2").unwrap();
  assert_eq!(gave_back.quarantined.iter().map(|process| process.pid).collect::<Vec<_>>(), [older.id()]);
  assert_eq!(older.try_wait().unwrap(), None, "an older process is never signalled");
  let LendAnswer::Queued(waiting) = lend_to(&params, "run-3", None) else {
    panic!("the slot is in quarantine")
  };
  assert_eq!(
    waiting.quarantined.iter().map(|(n, processes)| (*n, processes[0].pid)).collect::<Vec<_>>(),
    [(1, older.id())]
  );
  assert!(reclaim(&context.git, &params).unwrap().is_empty(), "the process still works in it");

  older.kill().unwrap();
  older.wait().unwrap();
  let reclaimed = reclaim(&context.git, &params).unwrap();
  assert_eq!(
    reclaimed.iter().map(|done| (done.n, done.action, done.error.clone())).collect::<Vec<_>>(),
    [(1, ReclaimAction::Requarantined, None)]
  );
  assert_eq!(lent(lend_to(&params, "run-3", None)).n, 1);
}

/// With `branchdeck.hostSlotMaxSize`, a give-back cleans the slot when the rules would free more than it; without it,
/// after 50 runs, or when the volume has less than `branchdeck.leaseMinFree`. `bd lease clean --host` cleans the idle
/// host slots now, `--dry-run` only measures, and a lent one is skipped
#[test_log::test]
fn a_host_slot_is_cleaned_by_size_by_free_space_and_after_fifty_runs() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  let run = |name: &str, bytes: usize| {
    let slot = lent(lend_to(&params, name, None));
    std::fs::create_dir_all(Path::new(&slot.path).join("out")).unwrap();
    std::fs::write(Path::new(&slot.path).join("out/cache"), vec![b'x'; bytes]).unwrap();
    let gave_back = give_back(&context.git, &params, 1, name).unwrap();
    (slot.path, gave_back.cleaned.is_some())
  };

  git(&fixture.repo, &["config", "branchdeck.hostSlotMaxSize", "1M"]);
  let (path, cleaned) = run("small", 10);
  assert!(!cleaned, "under the size");
  assert!(Path::new(&path).join("out/cache").exists());
  let (path, cleaned) = run("large", 2 << 20);
  assert!(cleaned, "over the size");
  assert!(!Path::new(&path).join("out/cache").exists());
  assert_eq!(table(fixture).slots[0].runs_since_clean, 0);

  git(&fixture.repo, &["config", "--unset", "branchdeck.hostSlotMaxSize"]);
  edit_table(fixture, |table| table["slots"][0]["runs_since_clean"] = Value::from(48));
  assert!(!run("run-49", 10).1);
  assert!(run("run-50", 10).1, "the 50th run since the clean");

  git(&fixture.repo, &["config", "branchdeck.leaseMinFree", "1000P"]);
  // where the system tells no free space (Windows), there is no minimum to fall under
  let tells_free_space = lease_pool::free_space::available_bytes(Path::new(&path)).is_some();
  assert_eq!(run("low-disk", 10).1, tells_free_space, "the volume has less than the minimum");
  git(&fixture.repo, &["config", "--unset", "branchdeck.leaseMinFree"]);

  std::fs::create_dir_all(Path::new(&path).join("out")).unwrap();
  std::fs::write(Path::new(&path).join("out/cache"), "x\n").unwrap();
  let (exit_code, measured) = fixture.run(&["lease", "clean", "--host", "--dry-run"]);
  assert_eq!((exit_code, &measured["targets"][0]["path"]), (0, &Value::from(path.clone())), "{measured}");
  assert!(Path::new(&path).join("out/cache").exists(), "a dry run deletes nothing");
  lent(lend_to(&params, "busy", None));
  let (exit_code, skipped) = fixture.run(&["lease", "clean", "--host"]);
  assert_eq!(exit_code, 0, "{skipped}");
  assert!(skipped["targets"][0]["skipped"].as_str().unwrap().contains("lent to the run busy"), "{skipped}");
  give_back(&context.git, &params, 1, "busy").unwrap();
  std::fs::create_dir_all(Path::new(&path).join("out")).unwrap();
  std::fs::write(Path::new(&path).join("out/cache"), "x\n").unwrap();
  let (exit_code, cleaned) = fixture.run(&["lease", "clean", "--host"]);
  assert_eq!(exit_code, 0, "{cleaned}");
  assert!(!Path::new(&path).join("out/cache").exists());
  let (exit_code, refused) = fixture.run(&["lease", "clean", "--host", "--free"]);
  assert_error(&refused, exit_code, (2, "invalid_arguments"));
}

/// A shrink returns the host slots past the count without `--confirm-preserved`: a slot whose run committed a snapshot
/// and left changes, and whose lend is abandoned (no `bd host serve`), goes back to the pool; a live lend is refused.
/// The count stays the limit of the fills until it is lifted
#[test_log::test]
fn a_shrink_returns_the_host_slots_past_the_count_also_without_the_agent() {
  let host = Host::filled(2);
  let (fixture, context, params) = host.parts();
  let first = lend_to_gone(&params, "run-1");
  std::fs::write(Path::new(&first.path).join("snapshot.txt"), "x\n").unwrap();
  slot_git(&first.path, &["add", "snapshot.txt"]);
  slot_git(&first.path, &["commit", "-q", "-m", "A snapshot of the client"]);
  std::fs::write(Path::new(&first.path).join("a.txt"), "changed\n").unwrap();
  let second = lent(lend_to(&params, "run-2", None));

  let shrunk = shrink(&context.git, &params, 0).unwrap();
  let outcomes: Vec<(u32, &str)> = shrunk
    .iter()
    .map(|slot| {
      (
        slot.n,
        match &slot.outcome {
          ShrinkOutcome::Returned(_) => "returned",
          ShrinkOutcome::Dropped => "dropped",
          ShrinkOutcome::Refused(_) => "refused",
        },
      )
    })
    .collect();
  assert_eq!(outcomes, [(1, "returned"), (2, "refused")]);
  assert!(matches!(&shrunk[1].outcome, ShrinkOutcome::Refused(reason) if reason.contains("lent to the run run-2")));
  let store = Pool::new(Path::new(&canonical_path(&fixture.repo.path().join(".git")))).read().unwrap();
  assert!(store.slots.iter().any(|slot| slot.path == first.path && slot.lease.is_none()), "back in the pool");
  assert_eq!(table(fixture).slots.iter().map(|slot| slot.n).collect::<Vec<_>>(), [2]);
  assert_eq!(table(fixture).limit, Some(0));

  give_back(&context.git, &params, 2, "run-2").unwrap();
  assert_eq!(second.n, 2);
  assert!(matches!(shrink(&context.git, &params, 0).unwrap()[0].outcome, ShrinkOutcome::Returned(_)));
  assert!(fill(&context.git, &params, None).unwrap().is_empty(), "the limit of the shrink holds");
  set_limit(&params, None).unwrap();
  assert_eq!(fill(&context.git, &params, None).unwrap().len(), 2);
}

/// `reclaim` drops the record of a fill that died before its get and returns a `bd-host/<n>` lease that the table does
/// not name (a fill that died after its get)
#[test_log::test]
fn reclaim_drops_a_dead_fill_and_returns_a_lease_the_table_does_not_name() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  let path = &host.filled[0].path;
  edit_table(fixture, |table| {
    table["slots"] = serde_json::json!([{ "n": 2, "state": { "kind": "filling", "pid": 0 } }]);
  });

  let mut reclaimed: Vec<(u32, ReclaimAction, Option<String>)> = reclaim(&context.git, &params).unwrap().into_iter().map(|done| (done.n, done.action, done.error)).collect();
  reclaimed.sort_by_key(|done| done.0);
  assert_eq!(reclaimed, [(1, ReclaimAction::Returned, None), (2, ReclaimAction::DroppedFill, None)]);
  assert_eq!(table(fixture).slots, []);
  let store = Pool::new(Path::new(&canonical_path(&fixture.repo.path().join(".git")))).read().unwrap();
  assert!(store.slots.iter().any(|slot| &slot.path == path && slot.lease.is_none()), "back in the pool");
}

/// The lease of a host slot in `leases.json` has the fields of any other lease: an older bd and the app read the pool
/// as before
#[test_log::test]
fn a_host_slot_lease_keeps_the_format_of_leases_json() {
  let host = Host::filled(1);
  let fixture = &host.fixture;
  fixture.get(&["--holder", "lane-1"]);
  let state: Value = serde_json::from_slice(&std::fs::read(fixture.state_file()).unwrap()).unwrap();
  let keys: Vec<Vec<String>> = state["slots"]
    .as_array()
    .unwrap()
    .iter()
    .map(|slot| {
      let mut keys: Vec<String> = slot["lease"].as_object().unwrap().keys().cloned().collect();
      keys.sort();
      keys
    })
    .collect();
  assert_eq!(keys[0], keys[1]);
}
