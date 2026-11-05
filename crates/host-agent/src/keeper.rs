//! The keeper of the host slots (spec/cli/host.spec.md, "serve"): a thread of `serve` that wakes once a minute, and at
//! each start of a run of a repository. Per repository, one at a time, it keeps the git of the host at its floor
//! (`run_git::ensure`: the call blocks while the pool waits for the lends and while Homebrew runs), takes up what no
//! process acts on and deletes the refs that no run uses (`lease_pool::host_runs::reclaim_all`), then leases the missing
//! slots (`lease_pool::host_slots::fill`) on a thread of its own, one per repository at a time. After the repositories,
//! one relief keeps the minimum free space (`lease_pool::host_slots::keep_free`: below `branchdeck.leaseMinFree` the
//! steps of a get there, the trim of every pool of the host among them), on a thread of its own and once per volume: the
//! repositories of a host share one. A failed fill waits 1, 5, then 15 minutes before the next one, a relief that leaves
//! a volume below its minimum 15 minutes.
//! The repositories start from the
//! repository list and the runs on disk, so a restart of `serve` keeps them. A change of the outcome of the git is one
//! line of the log

use crate::home::Home;
use crate::log::log;
use crate::repo::Repository;
use crate::run::Runs;
use crate::{cleanup, run_git, tuning};
use lease_pool::host_runs::reclaim_all;
use lease_pool::host_slots::{fill, keep_free};
use lease_pool::lease::{ReliefRound, Relieved};
use lease_pool::lease_clean::describe_size;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::span::in_current_span;

/// The waits after a failed fill: the first, the second, and every later one
const FILL_BACKOFF: [Duration; 3] = [Duration::from_mins(1), Duration::from_mins(5), Duration::from_mins(15)];

/// The wait after a relief that left the volume below its minimum, or failed: what it could delete is gone, and the
/// next one would walk the pools again for nothing
const RELIEF_BACKOFF: Duration = Duration::from_mins(15);

/// The fills of each repository
#[derive(Debug, Default)]
struct Fill {
  running: bool,
  failures: usize,
  not_before: Option<Instant>,
}

/// The relief of the home: one at a time, and after one that left a volume low, not before `not_before`
#[derive(Debug, Default)]
struct Relief {
  running: bool,
  not_before: Option<Instant>,
}

/// Starts the keeper of `home`; the sender wakes it for a repository
pub(crate) fn start(home: Home) -> mpsc::Sender<PathBuf> {
  let (wake, woken) = mpsc::channel::<PathBuf>();
  thread::spawn(in_current_span(move || keep(&home, &woken)));
  wake
}

fn keep(home: &Home, woken: &mpsc::Receiver<PathBuf>) {
  let mut repos: BTreeSet<PathBuf> = Runs::new(home.runs_dir())
    .list()
    .iter()
    .filter_map(|run| run.spec().ok())
    .map(|spec| home.expand(&spec.repo))
    .chain(cleanup::repos(home))
    .collect();
  let fills: Arc<Mutex<HashMap<PathBuf, Fill>>> = Arc::default();
  let relief: Arc<Mutex<Relief>> = Arc::default();
  let mut said: HashMap<PathBuf, String> = HashMap::new();
  loop {
    let mut opened = Vec::new();
    for path in &repos {
      match Repository::open(home, path.clone()) {
        Ok(repo) => {
          ensure_git(&repo, &mut said);
          opened.push(repo.clone());
          tend(repo, &fills);
        }
        Err(refusal) => note(&mut said, path, format!("cannot keep the host slots of {}: {refusal}", path.display())),
      }
    }
    relieve(opened, &relief);
    match woken.recv_timeout(tuning::keeper_tick()) {
      Ok(repo) => {
        repos.insert(repo);
        repos.extend(woken.try_iter());
      }
      Err(RecvTimeoutError::Timeout) => {}
      Err(RecvTimeoutError::Disconnected) => return,
    }
  }
}

/// Logs `line` for `repo` when it differs from the last line of that repository
fn note(said: &mut HashMap<PathBuf, String>, repo: &Path, line: String) {
  if said.get(repo) != Some(&line) {
    log(&line);
    said.insert(repo.to_path_buf(), line);
  }
}

/// Keeps the git of the host at its floor for `repo`, and logs an upgrade or a change of the outcome. `said` holds the
/// last outcome of each repository
fn ensure_git(repo: &Repository, said: &mut HashMap<PathBuf, String>) {
  let line = match run_git::ensure(repo, &|line| log(&format!("the git of the host: {line}"))) {
    Ok(ensured) => {
      let version = ensured.found.version.map_or_else(|| ensured.found.text.clone(), |version| version.to_string());
      if ensured.upgraded {
        log(&format!("Homebrew upgraded the git of the host to {} {version}", ensured.found.path.display()));
      }
      format!("the git of the host is {} {version}", ensured.found.path.display())
    }
    Err(error) => format!("the git of the host stays below its floor: {error:#}"),
  };
  note(said, &repo.path, line);
}

/// One tick of one repository: the reclaim on this thread, the fill on a thread of its own
fn tend(repo: Repository, fills: &Arc<Mutex<HashMap<PathBuf, Fill>>>) {
  match reclaim_all(&repo.git, &repo.params()) {
    Ok(reclaimed) => {
      for done in reclaimed.reclaimed {
        let action = format!("{:?}", done.action);
        match done.error {
          Some(error) => log(&format!("the host slots of {}: slot {}: {action} failed: {error}", repo.path.display(), done.n)),
          None => log(&format!("the host slots of {}: slot {}: {action}", repo.path.display(), done.n)),
        }
      }
      for deleted in reclaimed.deleted_refs {
        log(&format!("the host slots of {}: deleted {deleted}, which no run uses", repo.path.display()));
      }
    }
    Err(error) => log(&format!("cannot reclaim the host slots of {}: {error:#}", repo.path.display())),
  }
  {
    let mut fills = fills.lock().unwrap_or_else(PoisonError::into_inner);
    let fill = fills.entry(repo.path.clone()).or_default();
    if fill.running || fill.not_before.is_some_and(|not_before| Instant::now() < not_before) {
      return;
    }
    fill.running = true;
  }
  let fills = Arc::clone(fills);
  thread::spawn(in_current_span(move || {
    let filled = fill(&repo.git, &repo.params(), None);
    let mut fills = fills.lock().unwrap_or_else(PoisonError::into_inner);
    let state = fills.entry(repo.path.clone()).or_default();
    state.running = false;
    match filled {
      Ok(added) => {
        state.failures = 0;
        state.not_before = None;
        if !added.is_empty() {
          let numbers: Vec<u32> = added.iter().map(|slot| slot.n).collect();
          log(&format!("added the host slots {numbers:?} of {}", repo.path.display()));
        }
      }
      Err(error) => {
        let wait = FILL_BACKOFF[state.failures.min(FILL_BACKOFF.len() - 1)];
        state.failures += 1;
        state.not_before = Some(Instant::now() + wait);
        log(&format!(
          "cannot add the host slots of {}: {error:#}; again in {} min",
          repo.path.display(),
          wait.as_secs() / 60
        ));
      }
    }
  }));
}

/// The relief of one tick, when due and none runs: on a thread of its own, the repositories `repos` one after the other
/// in one round (`ReliefRound`), so each volume is measured and relieved once, by the first repository on it. A relief
/// that leaves a volume below its minimum, or fails, makes the next one wait `RELIEF_BACKOFF`
fn relieve(repos: Vec<Repository>, relief: &Arc<Mutex<Relief>>) {
  {
    let mut relief = relief.lock().unwrap_or_else(PoisonError::into_inner);
    if relief.running || relief.not_before.is_some_and(|not_before| Instant::now() < not_before) {
      return;
    }
    relief.running = true;
  }
  let relief = Arc::clone(relief);
  thread::spawn(in_current_span(move || {
    let mut round = ReliefRound::default();
    let mut low = false;
    for repo in repos {
      match keep_free(&repo.git, &repo.params(), &mut round) {
        Ok(None) => {}
        Ok(Some(relieved)) => {
          let still_low = relieved.after.is_some_and(|after| after < relieved.required);
          low |= still_low;
          log(&relieved_line(&repo.path, &relieved, still_low));
        }
        Err(error) => {
          low = true;
          log(&format!(
            "cannot keep the free space of the pools of {}: {error:#}; again in {} min",
            repo.path.display(),
            RELIEF_BACKOFF.as_secs() / 60
          ));
        }
      }
    }
    let mut relief = relief.lock().unwrap_or_else(PoisonError::into_inner);
    relief.running = false;
    relief.not_before = low.then(|| Instant::now() + RELIEF_BACKOFF);
  }));
}

/// The line of a relief below the minimum free space of the volume of `repo`: what each step deleted, and whether the
/// volume is `still_low` after all
fn relieved_line(repo: &Path, relieved: &Relieved, still_low: bool) -> String {
  let mut steps = Vec::new();
  if relieved.swept {
    steps.push("deleted the trash of gone runs".to_string());
  }
  if let Some(trimmed) = relieved.trimmed.as_ref().filter(|trimmed| trimmed.removed_slots() > 0 || trimmed.freed_bytes() > 0) {
    steps.push(format!(
      "the trim deleted {} free slot(s) past `branchdeck.leaseMaxSlots` and the Bazel output bases of gone slots, {}",
      trimmed.removed_slots(),
      describe_size(trimmed.freed_bytes())
    ));
  }
  if let Some(shrunk) = relieved.shrunk.as_ref().filter(|shrunk| !shrunk.slots.is_empty()) {
    steps.push(format!("the shrink freed {} of {} free slot(s)", describe_size(shrunk.freed_bytes()), shrunk.slots.len()));
  }
  if let Some(evicted) = relieved.evicted.as_ref().filter(|evicted| evicted.removed_slots() > 0) {
    steps.push(format!(
      "the eviction deleted {} free slot(s) below `branchdeck.leaseMaxSlots`, {}",
      evicted.removed_slots(),
      describe_size(evicted.freed_bytes())
    ));
  }
  let after = relieved.after.map_or_else(|| "unknown".to_string(), describe_size);
  let still = if still_low {
    format!(", still less than the minimum; the next relief in {} min", RELIEF_BACKOFF.as_secs() / 60)
  } else {
    String::new()
  };
  format!(
    "the volume of the pools of {} had {} free, less than `branchdeck.leaseMinFree` {}: {}; now {after} free{still}",
    repo.display(),
    describe_size(relieved.before),
    describe_size(relieved.required),
    if steps.is_empty() { "nothing to delete".to_string() } else { steps.join(", ") }
  )
}
