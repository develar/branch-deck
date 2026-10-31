//! The host slots (`lease_pool::host_slots`): a fill leases them as plain `ready` leases of `bd-host/<n>` that the other
//! commands of the pool leave alone, a run gets the slot of its affinity, else the one used longest ago, else waits in
//! line; a give-back stops the processes of the run and discards its changes, and quarantines the slot for an older
//! process; `reclaim` takes up what a gone process left; the clean rules run by size, by free space and after 50 runs;
//! a shrink returns its slots without `--confirm-preserved`, also without `bd-host serve`; a hand return is refused;
//! while a live process upgrades git, a fill, a clean, a shrink and `reclaim` skip, and a host-object call waits

use crate::lease_command::LeaseContext;
#[cfg(unix)]
use crate::lease_test::busy_in;
use crate::lease_test::{Fixture, assert_trash_deleted_once_free, path_of, slot_git};
use bd_cli::fixture_test::{assert_error, git};
use lease_pool::host_refs::have;
use lease_pool::host_slots::{
  Filled, HostSlotRefusal, HostSlots, HostSlotsParams, HostState, Lend, LendAnswer, LendRequest, Queued, ReclaimAction, ShrinkOutcome, SlotCheckout, changes_run, checkout,
  clean_slot, fill, give_back, give_back_run, lend, lend_waiting, read_host_slots, reclaim, set_limit, shrink, snapshot_run,
};
use lease_pool::lease::{LeaseState, Pool, canonical_path};
use lease_pool::lease_clean::CleanOverrides;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;
#[cfg(unix)]
use std::time::UNIX_EPOCH;
#[cfg(unix)]
use sync_utils::process_scan::ProcessScan;
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
pub(crate) fn context(fixture: &Fixture) -> LeaseContext {
  LeaseContext::resolve(&fixture.environment(fixture.repo.path()), None).unwrap()
}

pub(crate) fn params<'a>(fixture: &'a Fixture, context: &'a LeaseContext) -> HostSlotsParams<'a> {
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
pub(crate) fn lend_to(params: &HostSlotsParams<'_>, run: &str, affinity: Option<&str>) -> LendAnswer {
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

/// Rewrite the table as a process that crashed, or time, would have left it: by a rename, as bd writes it, so a bd
/// that reads it meanwhile sees one whole table
pub(crate) fn edit_table(fixture: &Fixture, edit: impl FnOnce(&mut Value)) {
  let mut value: Value = serde_json::from_slice(&std::fs::read(table_file(fixture)).unwrap()).unwrap();
  edit(&mut value);
  let written = table_file(fixture).with_extension("json.edit");
  std::fs::write(&written, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
  std::fs::rename(&written, table_file(fixture)).unwrap();
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
/// signalled, also one that started in the millisecond of the lend: it quarantines the slot, no run gets it, and
/// `reclaim` ends the quarantine once the process is gone
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
  // the lend in the millisecond the process started, after it: the lend is known to the millisecond (`since_ms`), so
  // the process seems to have started after it
  let started = ProcessScan::scan()
    .inside(Path::new(&slot.path))
    .iter()
    .find(|process| process.pid == older.id())
    .and_then(|process| process.started_at())
    .expect("the process works in the slot");
  let started_ms = u64::try_from(started.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap();
  edit_table(fixture, |table| table["slots"][0]["state"]["since_ms"] = Value::from(started_ms));
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

/// The fsmonitor daemon of a worktree, stopped when the test ends
#[cfg(target_os = "macos")]
struct FsmonitorDaemon(String);

#[cfg(target_os = "macos")]
impl FsmonitorDaemon {
  /// Start it in `dir` without `HOME`, so it keeps `dir` as its working directory, in a process group of its own, so
  /// it is no process of the session that runs bd
  fn start(dir: &str) -> Self {
    use std::os::unix::process::CommandExt;
    let status = test_utils::git_test_utils::git_command()
      .args(["fsmonitor--daemon", "start"])
      .current_dir(dir)
      .env_remove("HOME")
      .process_group(0)
      .status()
      .unwrap();
    assert!(status.success(), "git fsmonitor--daemon start in {dir}: {status}");
    Self(dir.to_string())
  }

  fn git(&self, subcommand: &str) -> bool {
    test_utils::git_test_utils::git_command()
      .args(["fsmonitor--daemon", subcommand])
      .current_dir(&self.0)
      .stdout(std::process::Stdio::null())
      .status()
      .is_ok_and(|status| status.success())
  }
}

#[cfg(target_os = "macos")]
impl Drop for FsmonitorDaemon {
  fn drop(&mut self) {
    self.git("stop");
  }
}

/// With the repository's `core.fsmonitor`, git's fsmonitor daemon of a host slot outlives the give-back, also when it
/// started after the lend with its working directory in the slot: it is no process of the run, so the slot is idle
/// again, not in quarantine. The `status` of the next give-back asks the daemon and still finds the changes of its run
#[cfg(target_os = "macos")]
#[test_log::test]
fn the_fsmonitor_daemon_of_a_host_slot_outlives_the_give_back_and_the_changes_of_a_run_are_still_found() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  git(&fixture.repo, &["config", "core.fsmonitor", "true"]);
  git(&fixture.repo, &["config", "core.untrackedCache", "true"]);

  let slot = lent(lend_to(&params, "run-1", None));
  let daemon = FsmonitorDaemon::start(&slot.path);
  let ProcessScan::Done { processes, .. } = ProcessScan::scan() else {
    panic!("macOS has a process scan")
  };
  let canonical_slot = std::fs::canonicalize(&slot.path).unwrap();
  assert!(
    processes
      .iter()
      .any(|process| { process.argv.iter().any(|arg| arg == "fsmonitor--daemon") && process.cwd.as_ref().is_some_and(|cwd| cwd.starts_with(&canonical_slot)) }),
    "the daemon works in the slot"
  );
  let path = Path::new(&slot.path);
  std::fs::write(path.join("a.txt"), "changed\n").unwrap();
  std::fs::write(path.join("untracked.txt"), "x\n").unwrap();
  let gave_back = give_back(&context.git, &params, 1, "run-1").unwrap();
  let mut discarded = gave_back.discarded.clone();
  discarded.sort();
  assert_eq!(
    (gave_back.stopped.len(), gave_back.quarantined.len(), discarded),
    (0, 0, vec!["a.txt".to_string(), "untracked.txt".to_string()])
  );
  assert_eq!(state_of(fixture, 1), HostState::Idle);
  assert!(daemon.git("status"), "the give-back leaves the daemon");

  lent(lend_to(&params, "run-2", None));
  std::fs::write(path.join("a.txt"), "changed again\n").unwrap();
  let gave_back = give_back(&context.git, &params, 1, "run-2").unwrap();
  assert_eq!(gave_back.discarded, ["a.txt"]);
  assert_eq!(slot_git(&slot.path, &["-c", "core.fsmonitor=false", "status", "--porcelain"]), "");
  assert!(daemon.git("status"));
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

/// A give-back holds the slot only for the git of its measure: `ls-files` runs while the slot is given back, and the
/// walk of the sizes once it is idle; the clean that follows deletes its trash once the slot is idle again
#[test]
fn a_give_back_walks_the_sizes_and_a_clean_deletes_its_trash_once_the_slot_is_idle() {
  let log = test_utils::captured_log::CapturedLog::install(true);
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  git(&fixture.repo, &["config", "branchdeck.hostSlotMaxSize", "1M"]);
  let slot = lent(lend_to(&params, "run-1", None));
  std::fs::create_dir_all(Path::new(&slot.path).join("out")).unwrap();
  std::fs::write(Path::new(&slot.path).join("out/cache"), vec![b'x'; 2 << 20]).unwrap();
  let start = log.text().len();

  let gave_back = give_back(&context.git, &params, 1, "run-1").unwrap();
  let cleaned = gave_back.cleaned.expect("over the size");
  assert_eq!((cleaned.cleaned[0].freed_bytes > 0, cleaned.trash_left), (true, Vec::<String>::new()));
  assert!(!Path::new(&slot.path).join("out/cache").exists());
  let text = log.text().split_off(start);
  let at = |line: &str| text.find(line).unwrap_or_else(|| panic!("no {line:?} in {text}"));
  let measured = at("ls-files");
  let idle = at("a give-back left the host slot idle");
  let walked = at("measuring what the clean rules of the idle host slot would free");
  let deleted = at("a clean left the host slot idle: deleting its trash");
  assert!(measured < idle && idle < walked && walked < deleted, "{text}");
  assert_eq!(state_of(fixture, 1), HostState::Idle);
}

/// A shrink returns the host slots past the count without `--confirm-preserved`: a slot whose run committed a snapshot
/// and left changes, and whose lend is abandoned (no `bd-host serve`), goes back to the pool; a live lend is refused.
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

/// A shrink returns each slot without deleting its trash, and deletes what the clean rules of all of them moved there
/// once the last one is free; nothing is left
#[test]
fn a_shrink_deletes_the_trash_once_every_slot_it_returns_is_free() {
  let log = test_utils::captured_log::CapturedLog::install(false);
  let host = Host::filled(2);
  let (fixture, context, params) = host.parts();
  for slot in &host.filled {
    std::fs::create_dir_all(Path::new(&slot.path).join("out")).unwrap();
    std::fs::write(Path::new(&slot.path).join("out/cache"), vec![b'x'; 1 << 16]).unwrap();
  }

  let shrunk = shrink(&context.git, &params, 0).unwrap();
  for (slot, filled) in shrunk.iter().zip(&host.filled) {
    let ShrinkOutcome::Returned(returned) = &slot.outcome else {
      panic!("not returned: {slot:?}")
    };
    assert_eq!((returned.cleaned[0].freed_bytes > 0, &returned.trash_left), (true, &Vec::new()), "{returned:?}");
    assert!(!Path::new(&filled.path).join("out/cache").exists());
  }
  assert_trash_deleted_once_free(fixture, &log.text(), ("a shrink is done with the host slot", 2), "deleting the trash of the shrink");
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

/// The upgrade of git that a live process records, `brew upgrade git` since the millisecond 1: this process
pub(crate) fn record_live_upgrade(fixture: &Fixture) {
  let pid = std::process::id();
  edit_table(fixture, |table| {
    table["git"] = json!({
      "kind": "upgrading",
      "owner": { "pid": pid, "process_started": process_started(pid) },
      "command": "brew upgrade git",
      "since_ms": 1,
    });
  });
}

/// Remove the record of the upgrade of git from the table
pub(crate) fn remove_upgrade(fixture: &Fixture) {
  edit_table(fixture, |table| {
    table.as_object_mut().unwrap().remove("git");
  });
}

/// Whether `error` is the refusal of a step while git is upgrading by `brew upgrade git`
fn is_upgrading(error: &anyhow::Error) -> bool {
  matches!(error.downcast_ref::<HostSlotRefusal>(), Some(HostSlotRefusal::GitUpgrading(command)) if command == "brew upgrade git")
}

/// While a live process upgrades git, a fill adds no slot, a clean and the steps of `reclaim` skip
/// with the refusal, `bd lease clean --host` says that git is upgrading, and a shrink refuses each slot. The upgrade of
/// a gone process holds nothing back
#[test_log::test]
fn a_fill_a_clean_a_shrink_and_reclaim_skip_while_git_is_upgrading() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  git(&fixture.repo, &["config", "branchdeck.hostSlots", "2"]);
  lend_to_gone(&params, "run-1");
  record_live_upgrade(fixture);

  assert!(is_upgrading(&fill(&context.git, &params, None).unwrap_err()));
  assert_eq!(table(fixture).slots.len(), 1, "no slot added");
  assert!(is_upgrading(&clean_slot(&context.git, &params, 1, CleanOverrides::default()).unwrap_err()));
  let reclaimed = reclaim(&context.git, &params).unwrap();
  assert_eq!(
    reclaimed.iter().map(|done| (done.n, done.action, done.error.clone())).collect::<Vec<_>>(),
    [(1, ReclaimAction::GaveBack, Some("The git of the host is upgrading (`brew upgrade git`)".to_string()))]
  );
  assert!(matches!(state_of(fixture, 1), HostState::Lent(_)), "the abandoned lend waits for a later reclaim");
  let (exit_code, cleaned) = fixture.run(&["lease", "clean", "--host"]);
  assert_eq!(exit_code, 0, "{cleaned}");
  assert_eq!(
    cleaned["targets"][0]["skipped"],
    json!("The git of the host is upgrading (`brew upgrade git`)"),
    "{cleaned}"
  );
  let shrunk = shrink(&context.git, &params, 0).unwrap();
  assert!(matches!(&shrunk[..], [slot] if matches!(&slot.outcome, ShrinkOutcome::Refused(reason) if reason.contains("upgrading"))));
  assert_eq!(table(fixture).slots.len(), 1, "the slot stays");

  edit_table(fixture, |table| table["git"]["owner"]["pid"] = Value::from(gone_process().0));
  set_limit(&params, None).unwrap();
  let reclaimed = reclaim(&context.git, &params).unwrap();
  assert_eq!(
    reclaimed.iter().map(|done| (done.n, done.action, done.error.clone())).collect::<Vec<_>>(),
    [(1, ReclaimAction::GaveBack, None)]
  );
  assert_eq!(fill(&context.git, &params, None).unwrap().iter().map(|slot| slot.n).collect::<Vec<_>>(), [2]);
}

/// A host-object call waits while a live process upgrades git and no slot is busy, and goes on once the upgrade is gone;
/// it leaves `users` at its end. While a slot is lent, Homebrew can't start, so a scan of the lent run goes on at once:
/// the run and the upgrade never wait for each other
#[test_log::test]
fn a_host_object_call_waits_for_an_upgrade_of_git_but_a_scan_of_a_lent_run_goes_on() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  let head = fixture.repo.head();
  record_live_upgrade(fixture);

  std::thread::scope(|scope| {
    let (done, answered) = std::sync::mpsc::channel();
    let (context, head) = (&context, &head);
    scope.spawn(move || done.send(have(&context.git, &context.dirs, std::slice::from_ref(head), context.lock_timeout).map_err(|error| format!("{error:#}"))));
    // a loaded machine can make this pass later, never fail: the call must not answer while the upgrade is recorded
    assert_eq!(
      answered.recv_timeout(Duration::from_millis(300)),
      Err(RecvTimeoutError::Timeout),
      "it waits for the upgrade"
    );
    remove_upgrade(fixture);
    assert_eq!(answered.recv().unwrap(), Ok(vec![head.clone()]));
  });
  assert_eq!(table(fixture).users, [], "the call left the users");

  lent(lend_to(&params, "run-1", None));
  record_live_upgrade(fixture);
  assert_eq!(snapshot_run(&context.git, &params, "run-1").unwrap().changed, 0);
  assert_eq!(table(fixture).users, []);
}

/// The refusal of the host slots in `error`
fn refusal(error: &anyhow::Error) -> &HostSlotRefusal {
  error.downcast_ref::<HostSlotRefusal>().unwrap_or_else(|| panic!("no refusal of the host slots: {error:#}"))
}

/// checkout: the slot of a run moves to the commit of the run by the diff, stays at it on a repeat, and checks out every
/// file with git's reason when an untracked file is in a path of the diff; a run without a slot is `NoSlot`
#[test_log::test]
fn a_run_checks_out_its_commit_in_its_slot_by_the_diff() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  let path = PathBuf::from(lent(lend_to(&params, "run-1", None)).path);
  let check_out = |commit: &str| checkout(&context.git, &params, "run-1", commit).unwrap();

  let first = fixture.repo.create_commit("Add c", "c.txt", "c\n");
  let moved = check_out(&first);
  assert_eq!((moved.n, moved.moved), (1, SlotCheckout::Switched));
  assert_eq!(std::fs::read_to_string(path.join("c.txt")).unwrap(), "c\n");
  assert_eq!(check_out(&first).moved, SlotCheckout::Kept);

  let second = fixture.repo.create_commit("Add d", "d.txt", "d\n");
  std::fs::write(path.join("d.txt"), "in the way\n").unwrap();
  let SlotCheckout::Forced(reason) = check_out(&second).moved else {
    panic!("git took the diff over an untracked file in its way");
  };
  assert!(reason.contains("d.txt"), "{reason}");
  assert_eq!(std::fs::read_to_string(path.join("d.txt")).unwrap(), "d\n");

  let refused = checkout(&context.git, &params, "run-2", &second).unwrap_err();
  assert!(matches!(refusal(&refused), HostSlotRefusal::NoSlot(run) if run == "run-2"), "{refused:#}");
}

/// A process that sleeps for 30 s
fn sleeping() -> std::process::Child {
  let mut command = if cfg!(windows) {
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-Command", "Start-Sleep 30"]);
    command
  } else {
    let mut command = std::process::Command::new("sleep");
    command.arg("30");
    command
  };
  command.spawn().unwrap()
}

/// `lend_waiting`: a run that waits behind a lent slot gets it once the slot is given back, without a second call; a
/// wait whose time is up answers its place in the queue; a process that ends while it waits is `ProcessGone`, and its
/// ticket leaves the queue
#[test_log::test]
fn a_lend_that_waits_gets_the_slot_once_it_is_given_back() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  lent(lend_to(&params, "run-1", None));
  let pid = std::process::id();
  let request = |run| LendRequest {
    run,
    pid,
    process_started: process_started(pid),
    affinity: None,
  };
  let queue = || table(fixture).queue.into_iter().map(|ticket| ticket.run).collect::<Vec<_>>();
  let quiet = |_: &Queued, _: Duration| {};

  let timed_out = lend_waiting(&context.git, &params, &request("run-2"), Duration::from_secs(1), &quiet).unwrap();
  assert!(matches!(timed_out, LendAnswer::Queued(Queued { ahead: 0, .. })), "{timed_out:?}");
  assert_eq!(queue(), ["run-2"], "the ticket stays while its process lives");

  let (params, context) = (&params, &context);
  let waited = std::thread::scope(|scope| {
    let waiting = scope.spawn(|| lend_waiting(&context.git, params, &request("run-3"), Duration::from_secs(120), &quiet).unwrap());
    while queue() != ["run-2", "run-3"] {
      std::thread::yield_now();
    }
    // run-2 leaves the queue, then the slot comes free
    give_back_run(&context.git, params, "run-2").unwrap();
    give_back_run(&context.git, params, "run-1").unwrap();
    waiting.join().unwrap()
  });
  assert_eq!(lent(waited).n, 1);

  let mut child = crate::lease_test::Killed(sleeping());
  let child_pid = child.0.id();
  let gone = LendRequest {
    run: "run-4",
    pid: child_pid,
    process_started: process_started(child_pid),
    affinity: None,
  };
  let refused = std::thread::scope(|scope| {
    let waiting = scope.spawn(|| lend_waiting(&context.git, params, &gone, Duration::from_secs(120), &quiet));
    while queue() != ["run-4"] {
      std::thread::yield_now();
    }
    let _ = child.0.kill();
    let _ = child.0.wait();
    waiting.join().unwrap().unwrap_err()
  });
  assert!(matches!(refusal(&refused), HostSlotRefusal::ProcessGone(gone) if *gone == child_pid), "{refused:#}");
  assert_eq!(queue(), Vec::<String>::new(), "the ticket of a gone process left the queue");
}

/// A waiting lend gives back a slot that no process acts on itself and gets it at once, without the tick of `reclaim`:
/// an abandoned lend, its changes discarded, and a quarantine whose processes are gone
#[test_log::test]
fn a_waiting_lend_gives_back_an_abandoned_slot_itself() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  let pid = std::process::id();
  let request = |run| LendRequest {
    run,
    pid,
    process_started: process_started(pid),
    affinity: None,
  };
  let quiet = |_: &Queued, _: Duration| {};
  // a limit no run reaches: no file event tells that a process ended, so without its own give-back the wait would end
  // queued
  let wait = Duration::from_mins(2);
  let abandoned = lend_to_gone(&params, "run-1");
  std::fs::write(Path::new(&abandoned.path).join("a.txt"), "changed\n").unwrap();

  let got = lent(lend_waiting(&context.git, &params, &request("run-2"), wait, &quiet).unwrap());
  assert_eq!((got.n, got.generation), (1, 1), "the give-back discarded the changes of the gone run");
  assert_eq!(slot_git(&got.path, &["status", "--porcelain"]), "");
  give_back(&context.git, &params, 1, "run-2").unwrap();

  let (gone, started_at) = gone_process();
  edit_table(fixture, |table| {
    table["slots"][0]["state"] = json!({
      "kind": "quarantined",
      "processes": [{ "pid": gone, "started": started_at, "name": "sleep", "command": "sleep 30" }],
    });
  });
  assert_eq!(lent(lend_waiting(&context.git, &params, &request("run-3"), wait, &quiet).unwrap()).n, 1);
}

/// snapshot and changes: the tree of the slot of a run before its command is the tree of its commit, without a change of
/// the slot index; after the command, the patch from that tree to the new one applies to the commit elsewhere, and a
/// tree that did not change writes no patch; a run without a slot is `NoSlot`
#[test_log::test]
fn the_scans_of_a_slot_name_its_tree_and_the_patch_of_its_command() {
  let host = Host::filled(1);
  let (fixture, context, params) = host.parts();
  let path = PathBuf::from(lent(lend_to(&params, "run-1", None)).path);
  let commit = fixture.repo.create_commit("Add c", "c.txt", "c\n");
  checkout(&context.git, &params, "run-1", &commit).unwrap();

  let before = snapshot_run(&context.git, &params, "run-1").unwrap();
  let tree = git(&fixture.repo, &["rev-parse", &format!("{commit}^{{tree}}")]);
  assert_eq!((before.tree.as_str(), before.head.as_str(), before.changed), (tree.as_str(), commit.as_str(), 0));

  std::fs::write(path.join("c.txt"), "c\nchanged by the command\n").unwrap();
  std::fs::write(path.join("made.txt"), "made\n").unwrap();
  let dir = tempfile::tempdir().unwrap();
  let patch = dir.path().join("run-1.patch");
  let changes = changes_run(&context.git, &params, "run-1", &tree, &patch).unwrap();
  assert_eq!(changes.patch_bytes, Some(std::fs::metadata(&patch).unwrap().len()));
  let slot_status = slot_git(path.to_str().unwrap(), &["status", "--porcelain"]);
  assert_eq!(slot_status, " M c.txt\n?? made.txt".trim(), "the slot index stays: {slot_status}");

  let elsewhere = test_utils::git_test_utils::TestRepo::new_empty();
  elsewhere.clone_from(fixture.repo.path()).unwrap();
  elsewhere.git(&["apply", patch.to_str().unwrap()]);
  assert_eq!(std::fs::read_to_string(elsewhere.path().join("made.txt")).unwrap(), "made\n");

  let none = dir.path().join("none.patch");
  assert_eq!(changes_run(&context.git, &params, "run-1", &changes.tree, &none).unwrap().patch_bytes, None);
  assert!(!none.exists());
  let refused = snapshot_run(&context.git, &params, "run-2").unwrap_err();
  assert!(matches!(refusal(&refused), HostSlotRefusal::NoSlot(run) if run == "run-2"), "{refused:#}");
}
