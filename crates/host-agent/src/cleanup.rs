//! The hourly cleanup of `serve`: it ends each run that lost its supervisor and removes each run a day after its end. It
//! also keeps the repository list of the packs and the runs, from which the keeper of the host slots and
//! `Request::Slots` start. Branch Deck owns the refs of the packs and the git maintenance: the give-back of a run
//! deletes its ref, the keeper deletes a ref that no run uses, and an ingest and a give-back start the deferred
//! maintenance

use crate::files::{private_dir, write_private};
use crate::home::Home;
use crate::run::Runs;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

/// Ends the lost runs and removes the expired runs. Answers a log line for each change
pub(crate) fn clean(home: &Home, now: SystemTime) -> Vec<String> {
  let runs = Runs::new(home.runs_dir());
  let mut lines: Vec<String> = runs
    .list()
    .iter()
    .filter(|run| run.end_if_lost())
    .map(|run| format!("ended the run {}, which lost its supervisor", run.path().display()))
    .collect();
  lines.extend(runs.remove_expired(now).into_iter().map(|run| format!("removed the run {run}, which ended a day ago")));
  lines
}

/// One writer of the repository list in this process, so two threads never lose an entry of each other
static REPOS: Mutex<()> = Mutex::new(());

/// Adds `repo` to the repository list, once
pub(crate) fn remember_repo(home: &Home, repo: &Path) -> io::Result<()> {
  let _guard = REPOS.lock().unwrap_or_else(PoisonError::into_inner);
  let mut known = repos(home);
  if known.iter().any(|known| known == repo) {
    return Ok(());
  }
  known.push(repo.to_path_buf());
  private_dir(&home.cache_dir())?;
  let mut text = String::new();
  for repo in &known {
    let _ = writeln!(text, "{}", repo.display());
  }
  write_private(&home.repos_file(), text.as_bytes())
}

/// The repositories that a pack or a run went to: one path per line of the repository list
pub(crate) fn repos(home: &Home) -> Vec<PathBuf> {
  fs::read_to_string(home.repos_file()).map_or_else(|_| Vec::new(), |text| text.lines().filter(|line| !line.is_empty()).map(PathBuf::from).collect())
}

#[cfg(test)]
#[path = "cleanup_test.rs"]
mod cleanup_test;
