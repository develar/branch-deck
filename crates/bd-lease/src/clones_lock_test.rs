//! The clones lock of the pool (`Pool::lock_clones`): a get that must add a slot waits for the one that adds a slot,
//! a slot freed meanwhile ends the wait, the minimum free space is checked under the lock, the steps of a get
//! arrive in order, and a killed holder frees the lock with its process. The queue of the gets that wait
//! (`lease_pool::get_queue`): the clones lock and a freed slot go to the gets in the order they asked

use crate::lease_command::{LeaseContext, LeaseGetArgs, LeaseGetOutput, lease_get_reporting, progress_line};
#[cfg(unix)]
use crate::lease_test::pool_environment;
use crate::lease_test::{Fixture, path_of};
#[cfg(unix)]
use crate::lease_test::{held_environment, states, trash_of_a_run, wait_until_held};
#[cfg(unix)]
use crate::takeover_test::landed_lease;
#[cfg(unix)]
use bd_cli::fixture_test::{TestChild, git, print_child_answer, run_json};
use bd_cli::fixture_test::{assert_covered, gone_process};
use bd_core::error::CliError;
#[cfg(unix)]
use domain_types::error::ErrorCode;
use git_ops::lease_slot::STATE_DIR;
use lease_pool::get_queue::{Behind, QueueTicket, TicketPhase};
use lease_pool::host_slots::now_ms;
#[cfg(unix)]
use lease_pool::lease::AddingSlot;
use lease_pool::lease::{CloneHolder, GetStep, LeaseState, Pool, canonical_path};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Mutex;
use std::thread::ScopedJoinHandle;
use std::time::{Duration, Instant};
use sync_utils::process_scan::process_started;
use test_utils::git_test_utils::TestRepo;
use test_utils::killed::Killed;

/// `brd lease get --holder <holder>` in the repository of the fixture, each step recorded in `steps`
pub(crate) fn get_with_steps(fixture: &Fixture, holder: &str, steps: &Mutex<Vec<GetStep>>) -> Result<LeaseGetOutput, CliError> {
  let environment = fixture.environment(fixture.repo.path());
  let context = LeaseContext::resolve(&environment, None)?;
  let args = LeaseGetArgs {
    holder: Some(holder.to_string()),
    ..LeaseGetArgs::default()
  };
  lease_get_reporting(&context, &environment, &args, &|step| steps.lock().unwrap().push(step))
}

/// The output of a get as brd prints it, checked against `brd schema`
pub(crate) fn printed(fixture: &Fixture, output: &LeaseGetOutput) -> Value {
  let value = serde_json::to_value(output).unwrap();
  let (_, schema) = fixture.run(&["schema", "--all"]);
  assert_covered(&schema["outputs"]["lease_get"], &schema["outputs"]["lease_get"], &value, "lease_get");
  value
}

/// The kind of each step; a wait told again (another holder or get ahead, a minute later) counts once
pub(crate) fn kinds(steps: &Mutex<Vec<GetStep>>) -> Vec<&'static str> {
  let mut kinds: Vec<&'static str> = steps
    .lock()
    .unwrap()
    .iter()
    .map(|step| match step {
      GetStep::Reserve { adding: false } => "reserve",
      GetStep::Reserve { adding: true } => "reserve adding",
      GetStep::WaitForClone(_) => "wait",
      GetStep::WaitBehind(_) => "wait behind",
      GetStep::WaitForSlot { .. } => "wait for slot",
      GetStep::DeleteTrash => "delete trash",
      GetStep::Reclaim => "reclaim",
      GetStep::Trim => "trim",
      GetStep::Shrink => "shrink",
      GetStep::Evict => "evict",
      GetStep::WaitForPressure { .. } => "wait for pressure",
      GetStep::SweepCloneLeftovers { .. } => "sweep",
      GetStep::RemoveUnfinished { .. } => "remove unfinished",
      GetStep::StopServers { .. } => "stop servers",
      GetStep::Status { .. } => "status",
      GetStep::Clone { .. } => "clone",
      GetStep::Refresh { .. } => "refresh",
      GetStep::Checkout { .. } => "checkout",
      GetStep::CheckoutEveryFile { .. } => "checkout every file",
      GetStep::LfsPull { .. } => "lfs pull",
      GetStep::Seed { .. } => "seed",
      GetStep::TakeOver { .. } => "take over",
      GetStep::CleanTakenOver { .. } => "clean taken over",
    })
    .collect();
  kinds.dedup_by(|next, previous| next == previous && matches!(*next, "wait" | "wait behind"));
  kinds
}

/// The steps that add a new slot, after its reserve: on macOS an APFS clone of a template, its index refreshed, the
/// checkout of the diff and the status of the clone; elsewhere a registration and a full checkout
pub(crate) fn new_slot_steps() -> &'static [&'static str] {
  if cfg!(target_os = "macos") {
    &["clone", "refresh", "checkout", "status"]
  } else {
    &["checkout"]
  }
}

/// Until the get of `steps` waits for the clones lock: whom it waits for. The get must not end first
fn wait_for_the_wait<T>(steps: &Mutex<Vec<GetStep>>, get: &ScopedJoinHandle<'_, T>) -> CloneHolder {
  loop {
    // read before the steps: a get that ended told every step it took
    let ended = get.is_finished();
    let waited = steps.lock().unwrap().iter().find_map(|step| match step {
      GetStep::WaitForClone(holder) => Some(holder.clone()),
      _ => None,
    });
    if let Some(holder) = waited {
      return holder;
    }
    assert!(!ended, "the get ended without a wait: {:?}", steps.lock().unwrap());
    std::thread::sleep(Duration::from_millis(10));
  }
}

/// Lets the get that `held_environment` in the directory holds go on when dropped, also when an assertion fails
#[cfg(unix)]
struct Release<'a>(&'a Path);

#[cfg(unix)]
impl Release<'_> {
  /// The held get goes on now
  fn go(&self) {
    let _ = std::fs::write(self.0.join("go"), "");
  }
}

#[cfg(unix)]
impl Drop for Release<'_> {
  fn drop(&mut self) {
    self.go();
  }
}

/// What a test sees of a get held under the clones lock and a get beside it
#[cfg(unix)]
struct Beside<T> {
  /// The exit code and the output of the held get (`--holder first`)
  first: (i32, Value),
  /// The get beside it (`--holder second`)
  second: Result<LeaseGetOutput, CliError>,
  /// What `meanwhile` returned
  seen: T,
}

/// The get beside a held clone, as `meanwhile` sees it
#[cfg(unix)]
type SecondGet<'scope> = ScopedJoinHandle<'scope, Result<LeaseGetOutput, CliError>>;

/// A get held at the registration of its new slot, under the clones lock, and the get of `steps` beside it: once that
/// one waits for the clones lock, `meanwhile` runs with whom it waits for and that get, then the held get goes on
#[cfg(unix)]
fn beside_a_held_clone<T>(fixture: &Fixture, steps: &Mutex<Vec<GetStep>>, meanwhile: impl FnOnce(CloneHolder, &SecondGet<'_>) -> T) -> Beside<T> {
  let queued = queue_beside_a_held_clone(fixture, &["second"], std::slice::from_ref(steps), |gets, _| {
    meanwhile(wait_for_the_wait(steps, &gets[0]), &gets[0])
  });
  Beside {
    first: queued.first,
    second: queued.gets.into_iter().next().unwrap(),
    seen: queued.seen,
  }
}

/// Two gets that must add a slot never clone at the same time: the first holds the clones lock while it adds its slot
/// (held at its registration); the second reserves nothing meanwhile and says whom it waits for (the process, the new
/// slot and the holder of its lease), then adds its own slot once the first is done, and says how long it waited
#[cfg(unix)]
#[test_log::test]
fn a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot() {
  let fixture = Fixture::new();
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |holder, _| {
    // the held get's slot is the only one: the waiting get reserved nothing
    let states = states(&fixture.status());
    (holder, states, kinds(&steps))
  });
  let (holder, states_while_held, while_held) = beside.seen;
  assert_eq!(while_held, ["reserve", "wait"]);
  let adding = path_of(&beside.first.1);
  assert_eq!(states_while_held, [(adding.clone(), "preparing".to_string())]);
  assert_eq!(
    (holder.process.pid, &holder.adding),
    (
      std::process::id(),
      &Some(AddingSlot {
        slot: adding.clone(),
        holder: "first".to_string(),
        template: None,
      })
    )
  );
  let line = GetStep::WaitForClone(holder).to_string();
  assert!(
    line.starts_with("no free slot; waiting for `") && line.contains(&format!("(pid {}, for ", std::process::id())) && line.contains(&format!("which adds {adding} for first")),
    "{line}"
  );

  assert_eq!(beside.first.1["reused"], json!(false));
  let second = printed(&fixture, &beside.second.unwrap());
  assert_eq!(second["reused"], json!(false), "{second}");
  assert_ne!(path_of(&second), adding);
  assert!(second["waited_for_clone_ms"].is_u64(), "{second}");
  assert_eq!(kinds(&steps), [&["reserve", "wait", "reserve adding"][..], new_slot_steps()].concat());
}

/// Until `done` holds, polled every 10 ms; past a minute, which no run reaches, the test fails with `what`
pub(crate) fn wait_until(what: &str, done: impl Fn() -> bool) {
  let deadline = Instant::now() + Duration::from_mins(1);
  while !done() {
    assert!(Instant::now() < deadline, "{what} for a minute");
    std::thread::sleep(Duration::from_millis(10));
  }
}

/// A slot that a return frees while a get waits for the clones lock ends the wait: the get reserves that slot without
/// the lock and moves it (`reused: true`) while the clone it waited for is still held, and the pool does not grow
#[cfg(unix)]
#[test_log::test]
fn a_slot_freed_while_a_get_waits_for_the_clones_lock_ends_the_wait() {
  let fixture = Fixture::new();
  let leased = path_of(&fixture.get(&["--holder", "leased"]));
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |_, second| {
    fixture.give_back(&leased);
    wait_until("the get waited past the free slot", || second.is_finished());
  });
  let second = printed(&fixture, &beside.second.unwrap());
  assert_eq!((path_of(&second), &second["reused"]), (leased.clone(), &json!(true)), "{second}");
  assert!(second["waited_for_clone_ms"].is_u64(), "{second}");
  assert_eq!(kinds(&steps), ["reserve", "wait", "reserve", "status", "checkout"]);
  let mut expected = vec![(leased, "ready".to_string()), (path_of(&beside.first.1), "ready".to_string())];
  let mut actual = states(&fixture.status());
  expected.sort();
  actual.sort();
  assert_eq!(actual, expected);
}

/// A dirty slot freed while a get waits for the clones lock ends the wait once: the get reserves it, finds the local
/// change, marks it dirty and waits again, and that slot, tried, never ends the wait again; the get adds a new slot once
/// the clone it waited for is done
#[cfg(unix)]
#[test_log::test]
fn a_dirty_slot_freed_while_a_get_waits_ends_the_wait_once() {
  let fixture = Fixture::new();
  let got = fixture.get(&["--holder", "dirty"]);
  let dirty = path_of(&got);
  std::fs::write(Path::new(&dirty).join("left.txt"), "left\n").unwrap();
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |_, second| {
    // freed as the state says it, with its change: a return would refuse it, or discard the change
    let common_dir = canonical_path(&fixture.repo.path().join(".git"));
    Pool::new(Path::new(&common_dir))
      .update(Duration::from_secs(10), |store| {
        let slot = store.slots.iter_mut().find(|slot| slot.path == dirty).unwrap();
        slot.lease = None;
        Ok(())
      })
      .unwrap();
    wait_until("the get waited again after the dirty slot", || {
      assert!(!second.is_finished(), "the get ended while the clone was held: {:?}", steps.lock().unwrap());
      kinds(&steps).ends_with(&["status", "reserve", "wait"])
    });
  });
  let second = printed(&fixture, &beside.second.unwrap());
  assert_eq!(second["reused"], json!(false), "{second}");
  assert_eq!(
    second["skipped"],
    json!([{ "slot": got["slot"], "path": dirty, "reason": "dirty", "paths": ["left.txt"] }]),
    "{second}"
  );
  assert_eq!(
    kinds(&steps),
    [&["reserve", "wait", "reserve", "status", "reserve", "wait", "reserve adding"][..], new_slot_steps()].concat()
  );
}

/// Until the get of `steps` waits for the return or the clean of a slot (`GetStep::WaitForSlot`): that step. The get
/// must not end first
pub(crate) fn wait_for_the_slot<T>(steps: &Mutex<Vec<GetStep>>, get: &ScopedJoinHandle<'_, T>) -> GetStep {
  wait_until("the get waited for the slot", || {
    assert!(!get.is_finished(), "the get ended without waiting for the slot: {:?}", steps.lock().unwrap());
    kinds(steps).contains(&"wait for slot")
  });
  let step = steps.lock().unwrap().last().unwrap().clone();
  let line = step.to_string();
  assert!(
    line.starts_with("no free slot; waiting for the ") && line.ends_with("the slot it frees is leased rather than a new one"),
    "{line}"
  );
  step
}

/// Set the state of the lease of `path` (of this process: the fixture runs brd in it) as a return or a clean marks it
fn mark_lease(fixture: &Fixture, path: &str, state: LeaseState) {
  let common_dir = canonical_path(&fixture.repo.path().join(".git"));
  Pool::new(Path::new(&common_dir))
    .update(Duration::from_secs(10), |store| {
      let lease = store.slots.iter_mut().find(|slot| slot.path == path).and_then(|slot| slot.lease.as_mut()).unwrap();
      assert_eq!(lease.pid, std::process::id());
      lease.state = state;
      Ok(())
    })
    .unwrap();
}

/// A get under the clones lock that waited for the return of a slot (`a_running_return_reserves_the_slot_against_a_get`)
/// adds a slot when that return fails: the lease is `ready` again, and the get waits for it no more
#[test_log::test]
fn a_get_that_waited_for_a_failed_return_adds_a_slot() {
  let fixture = Fixture::new();
  let returning = path_of(&fixture.get(&["--holder", "returning"]));
  mark_lease(&fixture, &returning, LeaseState::Returning);
  let steps = Mutex::new(Vec::new());
  let got = std::thread::scope(|scope| {
    let get = scope.spawn(|| get_with_steps(&fixture, "after", &steps));
    let wait = wait_for_the_slot(&steps, &get);
    assert_eq!(
      wait,
      GetStep::WaitForSlot {
        slot: returning.clone(),
        pid: std::process::id(),
        clean: false
      }
    );
    mark_lease(&fixture, &returning, LeaseState::Ready);
    printed(&fixture, &get.join().unwrap().unwrap())
  });
  assert_eq!(got["reused"], json!(false), "{got}");
  assert_eq!(
    kinds(&steps),
    [&["reserve", "reserve adding", "wait for slot", "reserve adding"][..], new_slot_steps()].concat()
  );
}

/// The minimum free space is checked again under the clones lock, so a queue of gets never adds slots past it: a get
/// that waited reads `branchdeck.leaseMinFree` and the free space as its turn finds them, trims, shrinks and evicts the
/// free slots of the pools without the lock, each followed by its turn again, and refuses with nothing reserved once the
/// space is still too low. The minimum is the size of the volume: one it holds, which no free space reaches
#[cfg(unix)]
#[test_log::test]
fn a_get_that_waited_for_the_clones_lock_checks_the_minimum_free_space_under_it() {
  let fixture = Fixture::new();
  let steps = Mutex::new(Vec::new());
  let total = lease_pool::free_space::volume_space(fixture.repo.path()).unwrap().total;
  let beside = beside_a_held_clone(&fixture, &steps, |_, _| git(&fixture.repo, &["config", "branchdeck.leaseMinFree", &total.to_string()]));
  let error = beside.second.unwrap_err();
  assert_eq!(error.code, ErrorCode::LowDiskSpace, "{}", error.message);
  assert!(!error.message.contains("less than the minimum, so"), "{}", error.message);
  assert_eq!(
    kinds(&steps),
    ["reserve", "wait", "trim", "reserve", "shrink", "reserve", "evict", "reserve"],
    "refused before its reserve under the lock"
  );
  assert_eq!(states(&fixture.status()), [(path_of(&beside.first.1), "ready".to_string())]);
}

/// A get whose turn at the clones lock finds the volume below the minimum and a trash of a gone run deletes that trash
/// without the lock, which the gets behind it take meanwhile: it releases the lock (its next reserve is one of a free
/// slot), deletes the trash, takes its turn again, trims without the lock as well, and refuses once the space is still
/// too low, the trash deleted: a minimum past the size of the volume shrinks and evicts nothing
#[cfg(unix)]
#[test_log::test]
fn a_get_deletes_the_trash_without_the_clones_lock() {
  let fixture = Fixture::new();
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |_, _| {
    // what a takeover left while the get waited, and a minimum no volume has
    let gone = trash_of_a_run(&fixture, true);
    git(&fixture.repo, &["config", "branchdeck.leaseMinFree", "1P"]);
    gone
  });
  let gone = beside.seen;
  let error = beside.second.unwrap_err();
  assert_eq!(error.code, ErrorCode::LowDiskSpace, "{}", error.message);
  assert!(!gone.exists(), "the trash of the gone run is deleted");
  assert_eq!(kinds(&steps), ["reserve", "wait", "delete trash", "reserve", "trim", "reserve"]);
  assert_eq!(
    GetStep::DeleteTrash.to_string(),
    "deleting the trash of runs that are gone: the free space is below the minimum"
  );
}

/// An ignored `local.txt` in the source that `.worktreeinclude` names: a get seeds it
fn add_a_seed(repo: &TestRepo) {
  repo.create_commit_with_files("Add the seed lists", &[(".gitignore", "/local.txt\n"), (".worktreeinclude", "/local.txt\n")]);
  std::fs::write(repo.path().join("local.txt"), "local\n").unwrap();
}

/// A get tells each step as it starts, in order: the reserve of a free slot, then under the clones lock of a new one,
/// the steps of the new slot and the seed; the reuse of that slot: its status, the checkout of the diff and the seed.
/// A get that took the clones lock at once waited for nothing (`waited_for_clone_ms` omitted)
#[test_log::test]
fn the_steps_of_a_get_arrive_in_order() {
  let fixture = Fixture::new();
  let repo = &fixture.repo;
  add_a_seed(repo);
  let steps = Mutex::new(Vec::new());
  let added = printed(&fixture, &get_with_steps(&fixture, "first", &steps).unwrap());
  assert_eq!((&added["reused"], &added.get("waited_for_clone_ms")), (&json!(false), &None), "{added}");
  assert_eq!(kinds(&steps), [&["reserve", "reserve adding"][..], new_slot_steps(), &["seed"]].concat());
  let path = path_of(&added);
  let checkout = GetStep::Checkout {
    slot: path.clone(),
    commit: repo.head(),
  };
  assert!(steps.lock().unwrap().contains(&checkout), "{:?}", steps.lock().unwrap());
  assert_eq!(steps.lock().unwrap().last(), Some(&GetStep::Seed { slot: path.clone(), paths: 1 }));
  assert_eq!(checkout.to_string(), format!("checking out {} in {path}", &repo.head()[..8]));
  let told: Vec<String> = steps.lock().unwrap().iter().map(ToString::to_string).collect();
  let answered: Vec<&str> = added["steps"].as_array().unwrap().iter().map(|step| step["step"].as_str().unwrap()).collect();
  assert_eq!(answered, told, "{added}");
  let starts: Vec<u64> = added["steps"].as_array().unwrap().iter().map(|step| step["at_ms"].as_u64().unwrap()).collect();
  assert!(starts.is_sorted(), "{starts:?}");

  fixture.give_back(&path);
  steps.lock().unwrap().clear();
  let reused = printed(&fixture, &get_with_steps(&fixture, "second", &steps).unwrap());
  assert_eq!((path_of(&reused), &reused["reused"]), (path, &json!(true)), "{reused}");
  assert_eq!(kinds(&steps), ["reserve", "status", "checkout", "seed"]);
}

/// A get that takes the clones lock moves into the trash what gets killed during their clone left: a number directory
/// of the pool in no state, holding only `<repository>.cloning`, whose number `next_slot_path` would pass over for ever
/// (the idea-1 pool kept 51 GiB of one from 2026-11-07). An empty number directory, one with more than the clone and a
/// slot stay; a get that reuses a free slot takes no clones lock and sweeps nothing
#[test_log::test]
fn a_get_that_adds_a_slot_moves_the_leftovers_of_killed_clones_into_the_trash() {
  let fixture = Fixture::new();
  let held = path_of(&fixture.get(&["--holder", "held"]));
  let pool = Path::new(&held).parent().unwrap().parent().unwrap().to_path_buf();
  let cloning = format!("{}.cloning", Path::new(&held).file_name().unwrap().to_string_lossy());
  let leftover = pool.join("7");
  std::fs::create_dir_all(leftover.join(&cloning).join("src")).unwrap();
  std::fs::write(leftover.join(&cloning).join("src/file"), "cloned").unwrap();
  let empty = pool.join("8");
  std::fs::create_dir_all(&empty).unwrap();
  let more = pool.join("9");
  std::fs::create_dir_all(more.join(&cloning)).unwrap();
  std::fs::write(more.join("other"), "kept").unwrap();

  let steps = Mutex::new(Vec::new());
  let added = printed(&fixture, &get_with_steps(&fixture, "second", &steps).unwrap());
  assert_eq!(&added["reused"], &json!(false), "{added}");
  assert!(kinds(&steps).starts_with(&["reserve", "sweep", "reserve adding"]), "{:?}", kinds(&steps));
  assert!(
    steps.lock().unwrap().contains(&GetStep::SweepCloneLeftovers {
      dirs: vec![leftover.to_string_lossy().into_owned()],
    }),
    "{:?}",
    steps.lock().unwrap()
  );
  assert!(!leftover.exists());
  assert!(empty.is_dir() && more.join("other").is_file() && Path::new(&held).is_dir());
  assert_ne!(Path::new(&path_of(&added)).parent().unwrap(), leftover);

  fixture.give_back(&path_of(&added));
  steps.lock().unwrap().clear();
  std::fs::create_dir_all(leftover.join(&cloning)).unwrap();
  let reused = printed(&fixture, &get_with_steps(&fixture, "third", &steps).unwrap());
  assert_eq!(&reused["reused"], &json!(true), "{reused}");
  assert!(!kinds(&steps).contains(&"sweep"), "{:?}", kinds(&steps));
  assert!(leftover.is_dir(), "a reuse takes no clones lock and sweeps nothing");
}

/// The stderr line of a step names the time since the get began, so a slow step shows in the gap to the next line
#[test]
fn a_step_on_stderr_names_when_it_started() {
  let step = GetStep::Reserve { adding: false };
  assert_eq!(progress_line(Duration::from_millis(900), &step), "brd lease get: [0 s] reserving a free slot");
  assert_eq!(progress_line(Duration::from_secs(192), &step), "brd lease get: [3 min 12 s] reserving a free slot");
}

/// A get that adds a slot releases the clones lock once the slot is checked out: the seed, a copy of the ignored files
/// that `.worktreeinclude` names (gigabytes in idea-1), runs without it, so the next get that adds a slot goes on
#[test_log::test]
fn a_get_seeds_its_new_slot_without_the_clones_lock() {
  let fixture = Fixture::new();
  let repo = &fixture.repo;
  add_a_seed(repo);
  let pool = Pool::new(Path::new(&canonical_path(&repo.path().join(".git"))));
  let environment = fixture.environment(repo.path());
  let context = LeaseContext::resolve(&environment, None).unwrap();
  // at the seed: whether the clones lock could be taken at once (in this process a turn the get still held cancels)
  let free_at_seed = Mutex::new(None);
  let got = lease_get_reporting(&context, &environment, &LeaseGetArgs::default(), &|step| {
    if matches!(step, GetStep::Seed { .. }) {
      *free_at_seed.lock().unwrap() = Some(pool.lock_clones(Duration::from_secs(10), &mut |_| {}, &|| true).is_ok());
    }
  })
  .unwrap();
  assert!(!got.reused, "{got:?}");
  assert_eq!(got.seeded, ["local.txt"]);
  assert_eq!(*free_at_seed.lock().unwrap(), Some(true), "the clones lock was free at the seed");
}

/// The environment variable that makes `hold_the_clones_lock_in_a_child` run: `<git common dir>|<file it writes once
/// it holds the lock>`
const CHILD_ENV: &str = "BD_CLONES_LOCK_CHILD";

/// The child of `clones_lock_in_a_child`: it takes the clones lock of the pool, says
/// so, and holds it until it is killed (at most 5 minutes); does nothing in a normal run
#[test_log::test]
fn hold_the_clones_lock_in_a_child() {
  let Ok(spec) = std::env::var(CHILD_ENV) else { return };
  let (common_dir, held) = spec.split_once('|').unwrap();
  let _lock = Pool::new(Path::new(common_dir)).lock_clones(Duration::from_secs(10), &mut |_| {}, &|| false).unwrap();
  std::fs::write(held, "").unwrap();
  std::thread::sleep(Duration::from_mins(5));
}

/// A child process that holds the clones lock of the pool of `fixture` (`hold_the_clones_lock_in_a_child`), once it
/// holds it
fn clones_lock_in_a_child(fixture: &Fixture) -> Killed {
  let common_dir = canonical_path(&fixture.repo.path().join(".git"));
  let dir = tempfile::tempdir().unwrap();
  let marker = dir.path().join("held");
  let mut child = Killed(
    std::process::Command::new(std::env::current_exe().unwrap())
      .args(["--exact", "clones_lock_test::hold_the_clones_lock_in_a_child", "--nocapture"])
      .env(CHILD_ENV, format!("{common_dir}|{}", marker.display()))
      .spawn()
      .unwrap(),
  );
  while !marker.exists() {
    assert!(child.try_wait().unwrap().is_none(), "the child ended before it held the lock");
    std::thread::sleep(Duration::from_millis(10));
  }
  child
}

/// A holder of the clones lock that is killed frees the lock with its process, on every platform (the system releases a
/// file lock of a process that ended): the get that waited for it, naming that process, adds its slot, and the record
/// the killed holder left behind goes with the lock of that get
#[test_log::test]
fn a_killed_holder_frees_the_clones_lock_with_its_process() {
  let fixture = Fixture::new();
  let common_dir = canonical_path(&fixture.repo.path().join(".git"));
  let child = clones_lock_in_a_child(&fixture);
  let steps = Mutex::new(Vec::new());
  let (got, holder, child_pid) = std::thread::scope(|scope| {
    let get = scope.spawn(|| get_with_steps(&fixture, "after", &steps));
    let holder = wait_for_the_wait(&steps, &get);
    let child_pid = child.0.id();
    drop(child);
    (get.join().unwrap(), holder, child_pid)
  });
  assert_eq!((holder.process.pid, holder.adding), (child_pid, None));
  let got = printed(&fixture, &got.unwrap());
  assert_eq!(got["reused"], json!(false), "{got}");
  assert!(got["waited_for_clone_ms"].is_u64(), "{got}");
  assert_eq!(kinds(&steps), [&["reserve", "wait", "reserve adding"][..], new_slot_steps()].concat());
  assert!(
    !Path::new(&common_dir).join(STATE_DIR).join("clones.lock.holder").exists(),
    "the record of the killed holder is gone"
  );
}

/// The pool of the repository of `fixture`
fn pool_of(fixture: &Fixture) -> Pool {
  Pool::new(Path::new(&canonical_path(&fixture.repo.path().join(".git"))))
}

/// The tickets of the queue of the pool of `fixture`
fn queue_of(fixture: &Fixture) -> Vec<QueueTicket> {
  pool_of(fixture).read().unwrap().queue
}

/// The number directory of the slot at `path` (`<pool>/<n>/<repository>`): the pool numbers its new slots in the order
/// it adds them
fn slot_number(path: &str) -> u32 {
  Path::new(path).parent().unwrap().file_name().unwrap().to_string_lossy().parse().unwrap()
}

/// Until the get of `steps` told a step of the kind `kind` (`kinds`); the get must not end first
fn wait_for_kind<T>(steps: &Mutex<Vec<GetStep>>, get: &ScopedJoinHandle<'_, T>, kind: &str) {
  wait_until(&format!("the get told no {kind}"), || {
    // read before the steps: a get that ended told every step it took
    let ended = get.is_finished();
    let told = kinds(steps).contains(&kind);
    assert!(told || !ended, "the get ended without {kind}: {:?}", steps.lock().unwrap());
    told
  });
}

/// Whether the steps `kinds` of a get reserve a new slot under the clones lock and no free slot before it, whose
/// `status` would come first
fn reserved_no_free_slot(kinds: &[&str]) -> bool {
  kinds
    .iter()
    .position(|kind| *kind == "reserve adding")
    .is_some_and(|adding| !kinds[..adding].contains(&"status"))
}

/// The first `GetStep::WaitBehind` of `steps`
fn behind_in(steps: &Mutex<Vec<GetStep>>) -> Behind {
  steps
    .lock()
    .unwrap()
    .iter()
    .find_map(|step| match step {
      GetStep::WaitBehind(behind) => Some(behind.clone()),
      _ => None,
    })
    .unwrap_or_else(|| panic!("no wait behind another get: {:?}", steps.lock().unwrap()))
}

/// What a test sees of a get held under the clones lock and the gets that wait beside it
#[cfg(unix)]
struct Queued<T> {
  /// The exit code and the output of the held get (`--holder first`)
  first: (i32, Value),
  /// The gets that waited, in the order they started
  gets: Vec<Result<LeaseGetOutput, CliError>>,
  /// What `meanwhile` returned
  seen: T,
}

/// A get held at the registration of its new slot, under the clones lock (`--holder first`), and a get of each holder
/// of `holders` beside it, each recorded in `steps` and started once the one before it waits: the first for the clones
/// lock, each other one behind the gets before it. Once all of them wait, `meanwhile` runs with them and the release of
/// the held get, which goes on at the latest after `meanwhile`
#[cfg(unix)]
fn queue_beside_a_held_clone<T>(fixture: &Fixture, holders: &[&str], steps: &[Mutex<Vec<GetStep>>], meanwhile: impl FnOnce(&[SecondGet<'_>], &Release<'_>) -> T) -> Queued<T> {
  let dir = tempfile::tempdir().unwrap();
  let held = held_environment(fixture, fixture.repo.path(), dir.path(), "*\" worktree add \"*");
  std::thread::scope(|scope| {
    let release = Release(dir.path());
    let first = scope.spawn(|| run_json(&held, &["lease", "get", "--holder", "first"]));
    wait_until_held(dir.path(), &first);
    let mut gets = Vec::new();
    for (index, (holder, steps)) in holders.iter().zip(steps).enumerate() {
      let get = scope.spawn(move || get_with_steps(fixture, holder, steps));
      wait_for_kind(steps, &get, if index == 0 { "wait" } else { "wait behind" });
      gets.push(get);
    }
    let seen = meanwhile(&gets, &release);
    drop(release);
    let first = first.join().unwrap();
    assert_eq!(first.0, 0, "{}", first.1);
    Queued {
      first,
      gets: gets.into_iter().map(|get| get.join().unwrap()).collect(),
      seen,
    }
  })
}

/// Three gets that wait while another adds a slot take the clones lock in the order they asked, not as the lock happens
/// to fall: the first waits for the lock, each later one behind the gets before it, naming the get right ahead, and the
/// pool numbers their new slots in that order
#[cfg(unix)]
#[test_log::test]
fn three_gets_that_wait_take_the_clones_lock_in_the_order_they_asked() {
  let fixture = Fixture::new();
  let steps: [Mutex<Vec<GetStep>>; 3] = Default::default();
  let queued = queue_beside_a_held_clone(&fixture, &["one", "two", "three"], &steps, |_, _| ());
  let first = slot_number(&path_of(&queued.first.1));
  let numbers: Vec<u32> = queued
    .gets
    .into_iter()
    .map(|got| {
      let got = printed(&fixture, &got.unwrap());
      assert!(got["waited_for_clone_ms"].is_u64(), "{got}");
      slot_number(&path_of(&got))
    })
    .collect();
  assert!(first < numbers[0] && numbers.is_sorted() && numbers[0] < numbers[2], "{first} then {numbers:?}");
  assert!(kinds(&steps[0]).starts_with(&["reserve", "wait", "reserve adding"]), "{:?}", kinds(&steps[0]));
  for later in &steps[1..] {
    let kinds = kinds(later);
    assert!(kinds.starts_with(&["reserve", "wait behind"]) && reserved_no_free_slot(&kinds), "{kinds:?}");
  }
  let behind = behind_in(&steps[2]);
  assert_eq!((behind.holder.as_str(), behind.pid, behind.ahead), ("two", std::process::id(), 2));
  let line = GetStep::WaitBehind(behind).to_string();
  assert!(
    line.starts_with("no free slot; waiting behind the get of two (pid ")
      && line.contains(", in the queue for ")
      && line.ends_with("), 2 get(s) ahead: the gets that wait take their slots in the order they asked"),
    "{line}"
  );
  assert_eq!(queue_of(&fixture), Vec::<QueueTicket>::new(), "every get left the queue");
}

/// A slot that a return frees while the head of the queue adds a slot goes to the next get that waits at once, before
/// that add ends; the get behind it does not take it and adds a slot of its own after the add
#[cfg(unix)]
#[test_log::test]
fn a_slot_freed_while_the_head_adds_goes_to_the_next_waiting_get_at_once() {
  let fixture = Fixture::new();
  let leased = path_of(&fixture.get(&["--holder", "leased"]));
  let steps: [Mutex<Vec<GetStep>>; 2] = Default::default();
  let queued = queue_beside_a_held_clone(&fixture, &["next", "after"], &steps, |gets, _| {
    fixture.give_back(&leased);
    wait_until("the next get took the freed slot", || gets[0].is_finished());
    assert!(!gets[1].is_finished(), "the get behind ended first: {:?}", steps[1].lock().unwrap());
  });
  let mut gets = queued.gets.into_iter();
  let next = printed(&fixture, &gets.next().unwrap().unwrap());
  assert_eq!((path_of(&next), &next["reused"]), (leased.clone(), &json!(true)), "{next}");
  assert_eq!(kinds(&steps[0]), ["reserve", "wait", "reserve", "status", "checkout"]);
  let after = printed(&fixture, &gets.next().unwrap().unwrap());
  assert_eq!(after["reused"], json!(false), "{after}");
  assert_ne!(path_of(&after), leased);
  let kinds = kinds(&steps[1]);
  assert!(kinds.starts_with(&["reserve", "wait behind"]) && reserved_no_free_slot(&kinds), "{kinds:?}");
}

/// A waiting ticket of this process, as a get writes it: entered `entered_ago` ms before now, its heartbeat an hour
/// ahead, so no load makes it stale
fn waiting_ticket(id: &str, entered_ago: u64) -> QueueTicket {
  let pid = std::process::id();
  QueueTicket {
    id: id.to_string(),
    holder: format!("{id}-agent"),
    pid,
    process_started: process_started(pid),
    entered_ms: now_ms() - entered_ago,
    heartbeat_ms: now_ms() + 3_600_000,
    phase: TicketPhase::Waiting,
    tried: Vec::new(),
    takes_over: true,
  }
}

/// Write `queue` into the state of the pool of `fixture`
fn set_queue(fixture: &Fixture, queue: Vec<QueueTicket>) {
  pool_of(fixture)
    .update(Duration::from_secs(10), |store| {
      store.queue = queue;
      Ok(())
    })
    .unwrap();
}

/// A new get does not pass a get that waits: a free slot that the waiting get has not tried is that get's, so the new
/// get waits behind it, and takes the slot once the waiting get tried it. Its ticket leaves the queue with its lease
#[test_log::test]
fn a_new_get_does_not_pass_a_get_that_waits() {
  let fixture = Fixture::new();
  let free = path_of(&fixture.get(&["--holder", "returned"]));
  fixture.give_back(&free);
  set_queue(&fixture, vec![waiting_ticket("waiting", 1_000)]);
  let steps = Mutex::new(Vec::new());
  let got = std::thread::scope(|scope| {
    let get = scope.spawn(|| get_with_steps(&fixture, "new", &steps));
    wait_for_kind(&steps, &get, "wait behind");
    assert_eq!(behind_in(&steps).holder, "waiting-agent");
    let store = pool_of(&fixture).read().unwrap();
    assert!(
      store.slots.iter().any(|slot| slot.path == free && slot.lease.is_none()),
      "the slot is free still: {store:?}"
    );
    let new = store.queue.iter().find(|ticket| ticket.holder == "new").unwrap();
    assert_eq!((new.phase, new.tried.as_slice()), (TicketPhase::Waiting, &[][..]), "{new:?}");
    pool_of(&fixture)
      .update(Duration::from_secs(10), |store| {
        store.queue[0].tried = vec![free.clone()];
        Ok(())
      })
      .unwrap();
    printed(&fixture, &get.join().unwrap().unwrap())
  });
  assert_eq!((path_of(&got), &got["reused"]), (free, &json!(true)), "{got}");
  assert_eq!(kinds(&steps), ["reserve", "wait behind", "reserve", "status", "checkout"]);
  let left: Vec<String> = queue_of(&fixture).into_iter().map(|ticket| ticket.holder).collect();
  assert_eq!(left, ["waiting-agent"]);
}

/// The queue passes over the ticket of a killed get and a ticket whose heartbeat is stale: a new get takes the free
/// slot at once. The ticket of the killed get leaves the queue in that reserve, and the stale one keeps its place
#[test_log::test]
fn the_ticket_of_a_killed_get_and_a_stale_one_are_passed_over() {
  let fixture = Fixture::new();
  let free = path_of(&fixture.get(&["--holder", "returned"]));
  fixture.give_back(&free);
  let (pid, process_started) = gone_process();
  let killed = QueueTicket {
    pid,
    process_started: Some(process_started.unwrap_or(1)),
    ..waiting_ticket("killed", 2_000)
  };
  let stale = QueueTicket {
    heartbeat_ms: now_ms() - 600_000,
    ..waiting_ticket("stale", 1_000)
  };
  set_queue(&fixture, vec![killed, stale]);
  let steps = Mutex::new(Vec::new());
  let got = printed(&fixture, &get_with_steps(&fixture, "new", &steps).unwrap());
  assert_eq!((path_of(&got), &got["reused"]), (free, &json!(true)), "{got}");
  assert_eq!(kinds(&steps), ["reserve", "status", "checkout"]);
  let left: Vec<String> = queue_of(&fixture).into_iter().map(|ticket| ticket.id).collect();
  assert_eq!(left, ["stale"]);
}

/// The lease of the slot at `path` idle, under the pool lock, as gets beside the test write the state: its holder
/// process gone, its get and the last move of its HEAD two hours ago
#[cfg(unix)]
fn make_idle_beside_gets(fixture: &Fixture, path: &str) {
  let (pid, started) = gone_process();
  let long_ago = std::time::SystemTime::now() - Duration::from_hours(2);
  pool_of(fixture)
    .update(Duration::from_secs(10), |store| {
      let lease = store.slots.iter_mut().find(|slot| slot.path == path).and_then(|slot| slot.lease.as_mut()).unwrap();
      lease.holder_pid = Some(pid);
      lease.holder_started = Some(started.unwrap_or(1));
      lease.leased_at = long_ago.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
      Ok(())
    })
    .unwrap();
  let git_dir = git_ops::lease_slot::linked_worktree(Path::new(path)).unwrap().git_dir;
  std::fs::File::options()
    .write(true)
    .open(git_dir.join("logs").join("HEAD"))
    .unwrap()
    .set_modified(long_ago)
    .unwrap();
}

/// The head of the queue waits for the clones lock while another get adds a slot, and a lease whose holder is gone
/// turns idle and quiet meanwhile: the head ends its wait and takes the lease over at once, before that add ends; the
/// get behind it does not take it
#[cfg(unix)]
#[test_log::test]
fn an_idle_lease_that_turns_quiet_while_gets_wait_goes_to_the_first_waiting_get_at_once() {
  let fixture = Fixture::new();
  let (old, _) = landed_lease(&fixture, "gone-agent");
  let idle = path_of(&old);
  let steps: [Mutex<Vec<GetStep>>; 2] = Default::default();
  let queued = queue_beside_a_held_clone(&fixture, &["head", "behind"], &steps, |gets, _| {
    make_idle_beside_gets(&fixture, &idle);
    wait_until("the head took the idle lease over", || gets[0].is_finished());
    assert!(!gets[1].is_finished(), "the get behind ended first: {:?}", steps[1].lock().unwrap());
  });
  let mut gets = queued.gets.into_iter();
  let head = printed(&fixture, &gets.next().unwrap().unwrap());
  assert_eq!((path_of(&head), &head["took_over"]["holder"]), (idle, &json!("gone-agent")), "{head}");
  let kinds_of_head = kinds(&steps[0]);
  assert!(
    kinds_of_head.starts_with(&["reserve", "wait", "reserve", "take over"]) && !kinds_of_head.contains(&"reserve adding"),
    "{kinds_of_head:?}"
  );
  let behind = printed(&fixture, &gets.next().unwrap().unwrap());
  assert_eq!(behind["reused"], json!(false), "{behind}");
  assert!(!kinds(&steps[1]).contains(&"take over"), "{:?}", kinds(&steps[1]));
}

/// The head of the queue takes the clones lock and waits for a return under it; the slot that the return frees is
/// dirty. The head keeps the lock across its `status` and adds a slot with no second wait, and the get behind it, which
/// waited meanwhile, takes neither the freed slot nor the lock first
#[cfg(unix)]
#[test_log::test]
fn a_dirty_slot_that_a_return_frees_under_the_lock_leaves_the_head_its_turn() {
  let fixture = Fixture::new();
  let returning = path_of(&fixture.get(&["--holder", "returning"]));
  let steps: [Mutex<Vec<GetStep>>; 2] = Default::default();
  let queued = queue_beside_a_held_clone(&fixture, &["head", "behind"], &steps, |gets, release| {
    mark_lease(&fixture, &returning, LeaseState::Returning);
    release.go();
    wait_for_kind(&steps[0], &gets[0], "wait for slot");
    // the head waits for the slot at its place, so no get behind it takes the slot first
    let head = queue_of(&fixture).into_iter().find(|ticket| ticket.holder == "head").unwrap();
    assert_eq!(head.phase, TicketPhase::Waiting, "{head:?}");
    std::fs::write(Path::new(&returning).join("left.txt"), "left\n").unwrap();
    pool_of(&fixture)
      .update(Duration::from_secs(10), |store| {
        store.slots.iter_mut().find(|slot| slot.path == returning).unwrap().lease = None;
        Ok(())
      })
      .unwrap();
  });
  let mut gets = queued.gets.into_iter();
  let head = printed(&fixture, &gets.next().unwrap().unwrap());
  assert_eq!(head["reused"], json!(false), "{head}");
  assert_eq!(head["skipped"][0]["reason"], json!("dirty"), "{head}");
  assert_eq!(
    kinds(&steps[0]),
    [
      &["reserve", "wait", "reserve adding", "wait for slot", "reserve adding", "status", "reserve adding"][..],
      new_slot_steps()
    ]
    .concat()
  );
  let behind = printed(&fixture, &gets.next().unwrap().unwrap());
  assert!(slot_number(&path_of(&head)) < slot_number(&path_of(&behind)), "{head} before {behind}");
  assert!(reserved_no_free_slot(&kinds(&steps[1])), "{:?}", kinds(&steps[1]));
}

/// A get that fails while it waits in the queue takes its ticket out of the state: here the clones lock of a process
/// whose record is gone, which refuses after the lock timeout
#[test_log::test]
fn a_get_that_fails_while_it_waits_leaves_no_ticket() {
  let fixture = Fixture::new();
  let common_dir = canonical_path(&fixture.repo.path().join(".git"));
  let child = clones_lock_in_a_child(&fixture);
  std::fs::remove_file(Path::new(&common_dir).join(STATE_DIR).join("clones.lock.holder")).unwrap();
  let environment = fixture.environment(fixture.repo.path());
  let mut context = LeaseContext::resolve(&environment, None).unwrap();
  context.lock_timeout = Duration::from_secs(1);
  let steps = Mutex::new(Vec::new());
  let args = LeaseGetArgs {
    holder: Some("failing".to_string()),
    ..LeaseGetArgs::default()
  };
  let error = lease_get_reporting(&context, &environment, &args, &|step| steps.lock().unwrap().push(step)).unwrap_err();
  drop(child);
  assert!(error.message.contains("clones.lock"), "{}", error.message);
  assert_eq!(kinds(&steps), ["reserve"]);
  assert_eq!(queue_of(&fixture), Vec::<QueueTicket>::new());
}

/// The environment variable that makes `lease_get_of_another_process` run: the JSON of the paths of its fixture and its
/// holder
#[cfg(unix)]
const GET_CHILD_ENV: &str = "BD_LEASE_GET_CHILD";

/// The child of `gets_of_other_processes_take_the_clones_lock_in_the_order_they_asked`: `brd lease get --holder
/// <holder>` in the repository of a fixture, in a process of its own, which prints its answer; does nothing in a normal
/// run
#[cfg(unix)]
#[test_log::test]
fn lease_get_of_another_process() {
  let Ok(spec) = std::env::var(GET_CHILD_ENV) else { return };
  let spec: Value = serde_json::from_str(&spec).unwrap();
  let path = |key: &str| std::path::PathBuf::from(spec[key].as_str().unwrap());
  let environment = pool_environment(&path("repo"), &path("home"), &path("global_config"));
  let (exit_code, output) = run_json(&environment, &["lease", "get", "--holder", spec["holder"].as_str().unwrap()]);
  print_child_answer(exit_code, &output);
}

/// Gets of three processes of their own wait while another adds a slot: each process polls the clones lock on its own,
/// so only the queue in the state orders them. They add their slots in the order they asked, and each one behind
/// another names the get right ahead in its steps, the lines of its stderr
#[cfg(unix)]
#[test_log::test]
fn gets_of_other_processes_take_the_clones_lock_in_the_order_they_asked() {
  let fixture = Fixture::new();
  let dir = tempfile::tempdir().unwrap();
  let held = held_environment(&fixture, fixture.repo.path(), dir.path(), "*\" worktree add \"*");
  let holders = ["a", "b", "c"];
  let (first, answers, pids) = std::thread::scope(|scope| {
    let release = Release(dir.path());
    let first = scope.spawn(|| run_json(&held, &["lease", "get", "--holder", "first"]));
    wait_until_held(dir.path(), &first);
    let mut children = Vec::new();
    for holder in holders {
      let spec = json!({
        "repo": fixture.repo.path(),
        "home": fixture.home.path(),
        "global_config": fixture.global_config.path(),
        "holder": holder,
      });
      children.push(TestChild::spawn("clones_lock_test::lease_get_of_another_process", GET_CHILD_ENV, &spec.to_string()));
      wait_until(&format!("the get of {holder} waits in the queue"), || {
        queue_of(&fixture).iter().any(|ticket| ticket.holder == holder)
      });
    }
    let pids: Vec<u32> = children.iter().map(TestChild::pid).collect();
    drop(release);
    let first = first.join().unwrap();
    (first, children.into_iter().map(TestChild::answer).collect::<Vec<_>>(), pids)
  });
  assert_eq!(first.0, 0, "{}", first.1);
  let numbers: Vec<u32> = answers
    .iter()
    .map(|(exit_code, output)| {
      assert_eq!(*exit_code, 0, "{output}");
      slot_number(&path_of(output))
    })
    .collect();
  assert!(
    slot_number(&path_of(&first.1)) < numbers[0] && numbers[0] < numbers[1] && numbers[1] < numbers[2],
    "{numbers:?}"
  );
  for (index, (_, output)) in answers.iter().enumerate().skip(1) {
    let behind = format!("no free slot; waiting behind the get of {} (pid {}, in the queue for ", holders[index - 1], pids[index - 1]);
    let steps: Vec<&str> = output["steps"].as_array().unwrap().iter().map(|step| step["step"].as_str().unwrap()).collect();
    assert!(steps.iter().any(|step| step.starts_with(&behind)), "{behind}: {steps:?}");
  }
  assert_eq!(queue_of(&fixture), Vec::<QueueTicket>::new());
}

/// A brd that knows no queue drops the queue from the state when it writes the state: the waiting get writes its
/// ticket again at its next look, with its time of entry, so it keeps its place
#[cfg(unix)]
#[test_log::test]
fn a_waiting_get_writes_its_ticket_again_once_a_brd_without_the_queue_dropped_it() {
  let fixture = Fixture::new();
  let steps: [Mutex<Vec<GetStep>>; 1] = Default::default();
  let queued = queue_beside_a_held_clone(&fixture, &["waiting"], &steps, |_, _| {
    let entered = queue_of(&fixture).iter().find(|ticket| ticket.holder == "waiting").unwrap().entered_ms;
    set_queue(&fixture, Vec::new());
    wait_until("the get wrote its ticket again", || queue_of(&fixture).iter().any(|ticket| ticket.holder == "waiting"));
    let again = queue_of(&fixture).into_iter().find(|ticket| ticket.holder == "waiting").unwrap();
    (entered, again)
  });
  let (entered, again) = queued.seen;
  assert_eq!((again.entered_ms, again.phase), (entered, TicketPhase::Waiting), "{again:?}");
  printed(&fixture, &queued.gets.into_iter().next().unwrap().unwrap());
  assert_eq!(queue_of(&fixture), Vec::<QueueTicket>::new());
}
