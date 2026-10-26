//! The remote branches someone else pushed to, as a session finds them (spec/repository/outgoing.spec.md, "Remote
//! commits of others"): a listing of the pull requests shows the head of each branch on GitHub, and a branch whose open
//! pull request shows another head than the tip the session knows is fetched (`sync_core::moved_remote`), by the rule
//! of `domain_policy::refresh::moved_remote_fetch_due`. `MovedFetches` keeps the last fetch of each branch, so one head
//! is fetched once, and again only while the remote lacks it (a mirror that lags). The fetch runs as a push of the
//! branch runs in the session (`Sessions::fetch_moved_remotes`): a sync meanwhile shows no remote status of it, and the
//! status the fetch left goes into the table after it; a push of the same remote ref waits for it (`push_guard`), and a
//! branch the app pushes is not fetched.

use crate::branch_table::BranchTable;
use crate::facts::session_remote;
use crate::push_guard::claim_free_refs;
use crate::session::{SessionKey, Sessions};
use crate::table::{RemoteSubject, TableState};
use domain_policy::refresh::{MovedRemoteFetch, moved_remote_fetch_due};
use domain_types::RemoteStatusUpdate;
use domain_types::branch_names::to_final_branch_name;
use domain_types::pull_request::{PullRequest, PullRequestState};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::git_common_dir;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;
use sync_core::moved_remote::{MovedBranch, fetch_moved_remotes};
use sync_utils::span::in_current_span;
use tracing::{instrument, warn};

/// The remote the app pushes the virtual branches to and compares them with
const REMOTE: &str = "origin";

/// A virtual branch whose open pull request shows `head`, which its remote-tracking ref lacks
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovedRemoteAsk {
  pub branch: String,
  pub head: String,
}

/// What the fetch of an ask found: the status against the tip it fetched (`None`: it fetched nothing, the tip had the
/// head, or it failed), and whether the head is in the remote-tracking ref now
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchedRemote {
  pub branch: String,
  pub head: String,
  pub status: Option<RemoteStatusUpdate>,
  pub settled: bool,
  /// The authors of the dropped commits of the status that are not the branch's author (`my_email`), each once, newest
  /// first: whose work the remote has that the branch lacks
  pub others: Vec<String>,
}

/// The last fetch of each branch the session asked, by branch name
#[derive(Clone, Debug, Default)]
pub struct MovedFetches {
  last: HashMap<String, MovedRemoteFetch>,
}

impl MovedFetches {
  /// The branches of `table` to fetch at `now` on the session clock, by the open pull requests listed and the remote
  /// branches as `state` knows them; each is in flight from now
  pub fn asks(&mut self, now: Duration, table: &BranchTable, state: &TableState, pull_requests: &BTreeMap<String, PullRequest>) -> Vec<MovedRemoteAsk> {
    let remote = session_remote(table, state);
    let mut asks = Vec::new();
    for branch in &table.branches {
      let Some(pull_request) = pull_requests.get(&branch.name).filter(|pull_request| pull_request.state == PullRequestState::Open) else {
        continue;
      };
      let head = pull_request.head_ref_oid.as_str();
      let remote_tip = remote.branches.get(&branch.name).and_then(|fact| fact.remote_tip.as_deref());
      let pushing = state.is_pushing(&RemoteSubject::Branch(branch.name.clone()));
      let last = self.last.get(&branch.name);
      if !moved_remote_fetch_due(now, head, remote_tip, pushing, last) {
        continue;
      }
      let attempts = last.filter(|last| last.head == head && last.tip.as_deref() == remote_tip).map_or(0, |last| last.attempts);
      self.last.insert(
        branch.name.clone(),
        MovedRemoteFetch {
          head: head.to_string(),
          tip: remote_tip.map(str::to_string),
          settled: false,
          attempts: attempts + 1,
          in_flight: true,
          asked_at: now,
        },
      );
      asks.push(MovedRemoteAsk {
        branch: branch.name.clone(),
        head: head.to_string(),
      });
    }
    asks
  }

  /// The fetches of `branches` ended with `fetched` (none when dropped): the tip each left, and whether its head is in it
  pub fn answered(&mut self, branches: impl IntoIterator<Item = impl AsRef<str>>, fetched: &[FetchedRemote]) {
    for branch in branches {
      if let Some(last) = self.last.get_mut(branch.as_ref()) {
        last.in_flight = false;
      }
    }
    for answer in fetched {
      if let Some(last) = self.last.get_mut(&answer.branch).filter(|last| last.head == answer.head) {
        if let Some(status) = &answer.status {
          last.tip.clone_from(&status.remote_tip);
        }
        last.settled = answer.settled;
      }
    }
  }
}

/// The fetch of `asks` in the repository of `repository_path` (the host's blocking step): the branches no push of this
/// process writes, fetched from `origin` with their statuses; a failure is logged and leaves every ask unsettled, to be
/// tried again by the rule. Answers each ask, in their order
#[instrument(skip_all, fields(repository_path = %repository_path, asks = asks.len()))]
pub fn fetch_asked(git_executor: &GitCommandExecutor, git_common_dir: &Path, repository_path: &str, table: &BranchTable, asks: &[MovedRemoteAsk]) -> Vec<FetchedRemote> {
  let unsettled = |ask: &MovedRemoteAsk| FetchedRemote {
    branch: ask.branch.clone(),
    head: ask.head.clone(),
    status: None,
    settled: false,
    others: Vec::new(),
  };
  let full_names: Vec<Option<String>> = asks.iter().map(|ask| to_final_branch_name(&table.prefix, &ask.branch).ok()).collect();
  let names: Vec<String> = full_names.iter().flatten().cloned().collect();
  // a push of the same remote ref writes it: its outcome is newer than this fetch, which would race it for the ref
  let (_claim, claimed) = claim_free_refs(git_common_dir, &names);
  let mut fetched: Vec<FetchedRemote> = asks.iter().map(unsettled).collect();
  let mut branches = Vec::new();
  let mut indexes = Vec::new();
  for (index, (ask, full_name)) in asks.iter().zip(&full_names).enumerate() {
    let (Some(full_name), Some(branch)) = (full_name, table.branch(&ask.branch)) else {
      continue;
    };
    if !claimed.contains(full_name) {
      continue;
    }
    let parent = branch
      .stack
      .as_ref()
      .and_then(|stack| stack.effective_base.as_deref())
      .and_then(|base| to_final_branch_name(&table.prefix, base).ok());
    branches.push((full_name.as_str(), branch.name.as_str(), parent, ask.head.as_str(), branch.my_email.as_deref()));
    indexes.push(index);
  }
  if branches.is_empty() {
    return fetched;
  }
  let moved: Vec<MovedBranch<'_>> = branches
    .iter()
    .map(|(full_name, branch_name, parent, head, my_email)| MovedBranch {
      full_name,
      branch_name,
      parent: parent.as_deref(),
      head,
      my_email: *my_email,
    })
    .collect();
  match fetch_moved_remotes(git_executor, repository_path, REMOTE, &table.baseline, &moved) {
    Ok(answers) => {
      for ((index, (_, _, _, _, my_email)), answer) in indexes.into_iter().zip(&branches).zip(answers) {
        fetched[index].others = answer.status.as_ref().map_or_default(|status| others_of(status, *my_email));
        fetched[index].status = answer.status;
        fetched[index].settled = answer.settled;
      }
    }
    Err(error) => warn!(error = format!("{error:#}"), "cannot fetch the remote branches whose pull requests moved"),
  }
  fetched
}

/// The fetch of `asks` in the open session of `key` of `sessions` (the hosts' `FactSources::fetch_moved_remotes`): over
/// the table of its last sync, in a blocking step, run as a push in the session (`Sessions::fetch_moved_remotes`). The
/// session always gets its end, which takes the asks out of flight: a session without a table fetches nothing
pub async fn fetch_in_session(sessions: &Sessions, git_executor: &GitCommandExecutor, key: &SessionKey, asks: Vec<MovedRemoteAsk>) {
  let Some(facts) = sessions.table(key).await else {
    skip_in_session(sessions, key, asks).await;
    return;
  };
  let (git, worktree, blocking_asks) = (git_executor.clone(), key.worktree.clone(), asks.clone());
  let fetch = async move {
    let fetched = tokio::task::spawn_blocking(in_current_span(move || {
      let git_common_dir = match git_common_dir(&git, &worktree) {
        Ok(dir) => dir,
        Err(error) => {
          warn!(error = format!("{error:#}"), "cannot read the git dir of the remote branches to fetch");
          return Vec::new();
        }
      };
      fetch_asked(&git, Path::new(&git_common_dir), &worktree, &facts.table, &blocking_asks)
    }))
    .await;
    fetched.unwrap_or_default()
  };
  sessions.fetch_moved_remotes(key, &asks, fetch).await;
}

/// The end of `asks` that a host does not fetch (the E2E server's repository without a remote of its own): they are out of
/// flight, unsettled, so the rule asks them again later
pub async fn skip_in_session(sessions: &Sessions, key: &SessionKey, asks: Vec<MovedRemoteAsk>) {
  sessions.fetch_moved_remotes(key, &asks, async { Vec::new() }).await;
}

/// The authors of the dropped commits of `status` that `my_email` didn't make, each once, newest first
fn others_of(status: &RemoteStatusUpdate, my_email: Option<&str>) -> Vec<String> {
  let mut others: Vec<String> = Vec::new();
  for commit in &status.dropped_commits {
    let mine = my_email.is_some_and(|email| commit.author_email.eq_ignore_ascii_case(email));
    if !mine && !others.contains(&commit.author) {
      others.push(commit.author.clone());
    }
  }
  others
}

#[cfg(test)]
#[path = "moved_remote_test.rs"]
mod moved_remote_test;
