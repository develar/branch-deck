//! The clones lock of the pool (`Pool::lock_clones`): a get that must add a slot waits for the one that adds a slot,
//! a slot freed meanwhile wins over a new one, the minimum free space is checked under the lock, the steps of a get
//! arrive in order, and a killed holder frees the lock with its process

use crate::lease_command::{LeaseContext, LeaseGetArgs, LeaseGetOutput, lease_get_reporting};
use crate::lease_test::{Fixture, Killed, path_of};
#[cfg(unix)]
use crate::lease_test::{held_environment, states, wait_until_held};
use bd_cli::fixture_test::assert_covered;
#[cfg(unix)]
use bd_cli::fixture_test::{git, run_json};
use bd_core::error::CliError;
#[cfg(unix)]
use domain_types::error::ErrorCode;
#[cfg(unix)]
use lease_pool::lease::AddingSlot;
use lease_pool::lease::{CloneHolder, GetStep, Pool, canonical_path};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Mutex;
use std::thread::ScopedJoinHandle;
use std::time::Duration;

/// `bd lease get --holder <holder>` in the repository of the fixture, each step recorded in `steps`
fn get_with_steps(fixture: &Fixture, holder: &str, steps: &Mutex<Vec<GetStep>>) -> Result<LeaseGetOutput, CliError> {
  let environment = fixture.environment(fixture.repo.path());
  let context = LeaseContext::resolve(&environment, None)?;
  let args = LeaseGetArgs {
    holder: Some(holder.to_string()),
    ..LeaseGetArgs::default()
  };
  lease_get_reporting(&context, &environment, &args, &|step| steps.lock().unwrap().push(step))
}

/// The output of a get as bd prints it, checked against `bd schema`
fn printed(fixture: &Fixture, output: &LeaseGetOutput) -> Value {
  let value = serde_json::to_value(output).unwrap();
  let (_, schema) = fixture.run(&["schema", "--all"]);
  assert_covered(&schema["outputs"]["lease_get"], &schema["outputs"]["lease_get"], &value, "lease_get");
  value
}

/// The kind of each step; a wait told again (another holder, a minute later) counts once
fn kinds(steps: &Mutex<Vec<GetStep>>) -> Vec<&'static str> {
  let mut kinds: Vec<&'static str> = steps
    .lock()
    .unwrap()
    .iter()
    .map(|step| match step {
      GetStep::Reserve { adding: false } => "reserve",
      GetStep::Reserve { adding: true } => "reserve adding",
      GetStep::WaitForClone(_) => "wait",
      GetStep::StopServers { .. } => "stop servers",
      GetStep::Status { .. } => "status",
      GetStep::Clone { .. } => "clone",
      GetStep::Refresh { .. } => "refresh",
      GetStep::Checkout { .. } => "checkout",
      GetStep::CheckoutEveryFile { .. } => "checkout every file",
      GetStep::Seed { .. } => "seed",
      GetStep::TakeOver { .. } => "take over",
      GetStep::CleanTakenOver { .. } => "clean taken over",
    })
    .collect();
  kinds.dedup_by(|next, previous| next == previous && *next == "wait");
  kinds
}

/// The steps that add a new slot, after its reserve: on macOS an APFS clone of a template, its index refreshed, the
/// checkout of the diff and the status of the clone; elsewhere a registration and a full checkout
fn new_slot_steps() -> &'static [&'static str] {
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
impl Drop for Release<'_> {
  fn drop(&mut self) {
    let _ = std::fs::write(self.0.join("go"), "");
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

/// A get held at the registration of its new slot, under the clones lock, and the get of `steps` beside it: once that
/// one waits for the clones lock, `meanwhile` runs with whom it waits for, then the held get goes on
#[cfg(unix)]
fn beside_a_held_clone<T>(fixture: &Fixture, steps: &Mutex<Vec<GetStep>>, meanwhile: impl FnOnce(CloneHolder) -> T) -> Beside<T> {
  let dir = tempfile::tempdir().unwrap();
  let held = held_environment(fixture, fixture.repo.path(), dir.path(), "*\" worktree add \"*");
  std::thread::scope(|scope| {
    let release = Release(dir.path());
    let first = scope.spawn(|| run_json(&held, &["lease", "get", "--holder", "first"]));
    wait_until_held(dir.path(), &first);
    let second = scope.spawn(|| get_with_steps(fixture, "second", steps));
    let seen = meanwhile(wait_for_the_wait(steps, &second));
    drop(release);
    let first = first.join().unwrap();
    assert_eq!(first.0, 0, "{}", first.1);
    Beside {
      first,
      second: second.join().unwrap(),
      seen,
    }
  })
}

/// Two gets that must add a slot never clone at the same time: the first holds the clones lock while it adds its slot
/// (held at its registration); the second reserves nothing meanwhile and says whom it waits for (the process, the new
/// slot and the holder of its lease), then adds its own slot once the first is done, and says how long it waited
#[cfg(unix)]
#[test_log::test]
fn a_get_that_must_add_a_slot_waits_for_the_one_that_adds_a_slot() {
  let fixture = Fixture::new();
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |holder| {
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

/// A slot that a return frees while a get waits for the clones lock wins over a new one: the get reserves again under
/// the lock and moves that slot (`reused: true`), and the pool does not grow
#[cfg(unix)]
#[test_log::test]
fn a_slot_freed_while_a_get_waits_for_the_clones_lock_wins_over_a_new_one() {
  let fixture = Fixture::new();
  let leased = path_of(&fixture.get(&["--holder", "leased"]));
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |_| fixture.give_back(&leased));
  let second = printed(&fixture, &beside.second.unwrap());
  assert_eq!((path_of(&second), &second["reused"]), (leased.clone(), &json!(true)), "{second}");
  assert!(second["waited_for_clone_ms"].is_u64(), "{second}");
  assert_eq!(kinds(&steps), ["reserve", "wait", "reserve adding", "status", "checkout"]);
  let mut expected = vec![(leased, "ready".to_string()), (path_of(&beside.first.1), "ready".to_string())];
  let mut actual = states(&fixture.status());
  expected.sort();
  actual.sort();
  assert_eq!(actual, expected);
}

/// The minimum free space is checked again under the clones lock, so a queue of gets never adds slots past it: a get
/// that waited reads `branchdeck.leaseMinFree` and the free space as its turn finds them, and refuses with nothing
/// reserved
#[cfg(unix)]
#[test_log::test]
fn a_get_that_waited_for_the_clones_lock_checks_the_minimum_free_space_under_it() {
  let fixture = Fixture::new();
  let steps = Mutex::new(Vec::new());
  let beside = beside_a_held_clone(&fixture, &steps, |_| git(&fixture.repo, &["config", "branchdeck.leaseMinFree", "1P"]));
  let error = beside.second.unwrap_err();
  assert_eq!(error.code, ErrorCode::LowDiskSpace, "{}", error.message);
  assert!(error.message.contains("1048576.0 GiB"), "{}", error.message);
  assert_eq!(kinds(&steps), ["reserve", "wait"], "refused before its reserve under the lock");
  assert_eq!(states(&fixture.status()), [(path_of(&beside.first.1), "ready".to_string())]);
}

/// A get tells each step as it starts, in order: the reserve of a free slot, then under the clones lock of a new one,
/// the steps of the new slot and the seed; the reuse of that slot: its status, the checkout of the diff and the seed.
/// A get that took the clones lock at once waited for nothing (`waited_for_clone_ms` omitted)
#[test_log::test]
fn the_steps_of_a_get_arrive_in_order() {
  let fixture = Fixture::new();
  let repo = &fixture.repo;
  repo.create_commit_with_files("Add the seed lists", &[(".gitignore", "/local.txt\n"), (".worktreeinclude", "/local.txt\n")]);
  std::fs::write(repo.path().join("local.txt"), "local\n").unwrap();
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

  fixture.give_back(&path);
  steps.lock().unwrap().clear();
  let reused = printed(&fixture, &get_with_steps(&fixture, "second", &steps).unwrap());
  assert_eq!((path_of(&reused), &reused["reused"]), (path, &json!(true)), "{reused}");
  assert_eq!(kinds(&steps), ["reserve", "status", "checkout", "seed"]);
}

/// The environment variable that makes `hold_the_clones_lock_in_a_child` run: `<git common dir>|<file it writes once
/// it holds the lock>`
const CHILD_ENV: &str = "BD_CLONES_LOCK_CHILD";

/// The child of `a_killed_holder_frees_the_clones_lock_with_its_process`: it takes the clones lock of the pool, says
/// so, and holds it until it is killed (at most 5 minutes); does nothing in a normal run
#[test_log::test]
fn hold_the_clones_lock_in_a_child() {
  let Ok(spec) = std::env::var(CHILD_ENV) else { return };
  let (common_dir, held) = spec.split_once('|').unwrap();
  let _lock = Pool::new(Path::new(common_dir)).lock_clones(Duration::from_secs(10), &mut |_| {}).unwrap();
  std::fs::write(held, "").unwrap();
  std::thread::sleep(Duration::from_mins(5));
}

/// A holder of the clones lock that is killed frees the lock with its process, on every platform (the system releases a
/// file lock of a process that ended): the get that waited for it, naming that process, adds its slot, and the record
/// the killed holder left behind goes with the lock of that get
#[test_log::test]
fn a_killed_holder_frees_the_clones_lock_with_its_process() {
  let fixture = Fixture::new();
  let common_dir = canonical_path(&fixture.repo.path().join(".git"));
  let dir = tempfile::tempdir().unwrap();
  let marker = dir.path().join("held");
  let child = std::process::Command::new(std::env::current_exe().unwrap())
    .args(["--exact", "clones_lock_test::hold_the_clones_lock_in_a_child", "--nocapture"])
    .env(CHILD_ENV, format!("{common_dir}|{}", marker.display()))
    .spawn()
    .unwrap();
  let steps = Mutex::new(Vec::new());
  let (got, holder, child_pid) = std::thread::scope(|scope| {
    let mut child = Killed(child);
    while !marker.exists() {
      assert!(child.0.try_wait().unwrap().is_none(), "the child ended before it held the lock");
      std::thread::sleep(Duration::from_millis(10));
    }
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
    !Path::new(&common_dir).join("branch-deck/clones.lock.holder").exists(),
    "the record of the killed holder is gone"
  );
}
