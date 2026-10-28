//! The keeper of the host slots (spec/cli/host.spec.md, "serve"): a thread of `serve` that wakes once a minute, and at
//! each start of a run of a repository. Per repository, one at a time, it keeps the git of the host at its floor
//! (`run_git::ensure`: the call blocks while the pool waits for the lends and while Homebrew runs), takes up what no
//! process acts on and deletes the refs that no run uses (`lease_pool::host_runs::reclaim_all`), then leases the missing
//! slots (`lease_pool::host_slots::fill`). A fill can clone the whole checkout, so it runs on a thread of its own, one per
//! repository at a time. A failed fill waits 1, 5, then 15 minutes before the next one. The repositories start from the
//! repository list and the runs on disk, so a restart of `serve` keeps them. A change of the outcome of the git is one
//! line of the log

use crate::home::Home;
use crate::log::log;
use crate::repo::Repository;
use crate::run::Runs;
use crate::{cleanup, run_git, tuning};
use lease_pool::host_runs::reclaim_all;
use lease_pool::host_slots::fill;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::span::in_current_span;

/// The waits after a failed fill: the first, the second, and every later one
const FILL_BACKOFF: [Duration; 3] = [Duration::from_mins(1), Duration::from_mins(5), Duration::from_mins(15)];

/// The fills of each repository
#[derive(Debug, Default)]
struct Fill {
  running: bool,
  failures: usize,
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
  let mut said: HashMap<PathBuf, String> = HashMap::new();
  loop {
    for path in &repos {
      match Repository::open(home, path.clone()) {
        Ok(repo) => {
          ensure_git(&repo, &mut said);
          tend(repo, &fills);
        }
        Err(refusal) => note(&mut said, path, format!("cannot keep the host slots of {}: {refusal}", path.display())),
      }
    }
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
