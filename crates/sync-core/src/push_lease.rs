//! The lease of a virtual branch push, shared by the app's push and `bd push`.
//!
//! A virtual branch is rebuilt on every sync, so a push must force; the lease keeps it from overwriting a remote
//! branch someone else updated. The lease is the commit Branch Deck last pushed for the ref, recorded in
//! `refs/branch-deck/pushed/<branch>` at push time:
//! - one `for-each-ref` reads the records with the local tips and the remote-tracking refs, and one `update-ref --stdin`
//!   records a push of any number of branches;
//! - the record keeps the pushed commit reachable, and no default refspec fetches or pushes `refs/branch-deck/`;
//! - a background fetch moves the remote-tracking ref, never the record, so it can't defeat the lease the way a bare
//!   `--force-with-lease=<ref>` (leased on the remote-tracking ref) can.
//!
//! Without a record (a branch pushed before records existed, or by another tool) the lease is the remote-tracking ref,
//! and without that the branch must not exist on the remote.
//!
//! A caller that showed the user the remote status leases on the remote head of that status instead
//! (`prepare_push_expecting`): the app's push button. The push then replaces exactly the remote state the user saw,
//! also after someone else's commit was fetched and shown as dropped. A rejected push fetches the branches
//! (`fetch_remote_branches`), so the caller can show what the remote has now.

use crate::remote_command::{execute_remote_command, execute_remote_command_accepting};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ref_transaction::RefTransaction;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

/// The namespace of the recorded pushes
pub const PUSHED_NAMESPACE: &str = "refs/branch-deck/pushed/";

/// Where the lease of a push comes from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseSource {
  /// The commit Branch Deck last pushed
  Recorded,
  /// The remote-tracking ref: no push was recorded, or the caller replaces what it fetched (`force`)
  Tracking,
  /// The remote head the caller showed the user (`prepare_push_expecting`)
  Expected,
  /// No remote branch: it must not exist
  Absent,
}

impl LeaseSource {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Recorded => "recorded",
      Self::Tracking => "tracking",
      Self::Expected => "expected",
      Self::Absent => "absent",
    }
  }
}

/// One branch of a push
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushRef {
  /// The branch name without `refs/heads/`, the same locally and on the remote
  pub full_name: String,
  /// The commit pushed: the local branch when the push was prepared
  pub tip: String,
  /// The commit the remote branch must be at; `None` when it must not exist
  pub lease: Option<String>,
  pub source: LeaseSource,
}

impl PushRef {
  pub fn lease_arg(&self) -> String {
    format!("--force-with-lease=refs/heads/{}:{}", self.full_name, self.lease.as_deref().unwrap_or_default())
  }

  /// `<tip>:refs/heads/<branch>`: the commit the lease was prepared for, even if the local branch moves meanwhile
  pub fn refspec(&self) -> String {
    format!("{}:refs/heads/{}", self.tip, self.full_name)
  }
}

/// The push of `full_names` to `remote` with their leases, from one `for-each-ref`. With `force` the lease is the
/// remote-tracking ref when there is one: the caller replaces what it fetched and saw. A missing local branch is an
/// error
#[instrument(skip(git_executor, full_names))]
pub fn prepare_push(git_executor: &GitCommandExecutor, repository_path: &str, remote: &str, full_names: &[String], force: bool) -> Result<Vec<PushRef>> {
  let tracking_namespace = format!("refs/remotes/{remote}/");
  let mut args = vec!["for-each-ref".to_string(), "--format=%(objectname) %(refname)".to_string()];
  for full_name in full_names {
    args.push(format!("refs/heads/{full_name}"));
    args.push(format!("{PUSHED_NAMESPACE}{full_name}"));
    args.push(format!("{tracking_namespace}{full_name}"));
  }
  let refs = read_refs(git_executor, repository_path, &args)?;

  full_names
    .iter()
    .map(|full_name| {
      let tip = local_tip(&refs, full_name)?;
      let recorded = refs.get(&format!("{PUSHED_NAMESPACE}{full_name}"));
      let tracking = refs.get(&format!("{tracking_namespace}{full_name}"));
      let (lease, source) = match (recorded, tracking) {
        (_, Some(tracking)) if force => (Some(tracking.clone()), LeaseSource::Tracking),
        (Some(recorded), _) => (Some(recorded.clone()), LeaseSource::Recorded),
        (None, Some(tracking)) => (Some(tracking.clone()), LeaseSource::Tracking),
        (None, None) => (None, LeaseSource::Absent),
      };
      Ok(PushRef {
        full_name: full_name.clone(),
        tip: tip.clone(),
        lease,
        source,
      })
    })
    .collect()
}

/// The push of each `(full_name, expected)` leased on `expected`: the remote head of the status the caller showed the
/// user (`RemoteStatusUpdate::remote_tip`), `None` when it showed no remote branch. The push replaces the remote branch
/// only if it is still there, so it drops what the user saw dropped and nothing else. One `for-each-ref` reads the tips
#[instrument(skip(git_executor, branches))]
pub fn prepare_push_expecting(git_executor: &GitCommandExecutor, repository_path: &str, branches: &[(String, Option<String>)]) -> Result<Vec<PushRef>> {
  let mut args = vec!["for-each-ref".to_string(), "--format=%(objectname) %(refname)".to_string()];
  args.extend(branches.iter().map(|(full_name, _)| format!("refs/heads/{full_name}")));
  let refs = read_refs(git_executor, repository_path, &args)?;

  branches
    .iter()
    .map(|(full_name, expected)| {
      Ok(PushRef {
        full_name: full_name.clone(),
        tip: local_tip(&refs, full_name)?.clone(),
        lease: expected.clone(),
        source: if expected.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
      })
    })
    .collect()
}

/// Ref name -> id from a `for-each-ref --format=%(objectname) %(refname)`
fn read_refs(git_executor: &GitCommandExecutor, repository_path: &str, args: &[String]) -> Result<HashMap<String, String>> {
  let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
  Ok(
    git_executor
      .execute_command_lines(&arg_refs, repository_path)?
      .iter()
      .filter_map(|line| line.split_once(' ').map(|(id, reference)| (reference.to_string(), id.to_string())))
      .collect(),
  )
}

fn local_tip<'a>(refs: &'a HashMap<String, String>, full_name: &str) -> Result<&'a String> {
  refs
    .get(&format!("refs/heads/{full_name}"))
    .ok_or_else(|| anyhow!("Branch {full_name} does not exist, sync first"))
}

/// The report of one ref of a `push --porcelain`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedRefStatus {
  /// `*` new, ` ` fast-forward, `+` forced, `=` up to date, `-` deleted, `!` rejected
  pub flag: char,
  /// What git says about it, e.g. `[rejected] (stale info)`; for a log or an error message, never parsed
  pub summary: String,
}

impl PushedRefStatus {
  pub fn accepted(&self) -> bool {
    self.flag != '!'
  }
}

/// Remote ref -> its report, from the `push --porcelain` lines `<flag> TAB <from>:<to> TAB <summary>`
pub fn parse_push_porcelain(output: &str) -> HashMap<String, PushedRefStatus> {
  output
    .lines()
    .filter_map(|line| {
      let mut fields = line.splitn(3, '\t');
      let flag = fields.next()?.chars().next()?;
      let (_, to) = fields.next()?.split_once(':')?;
      let summary = fields.next().unwrap_or_default().to_string();
      Some((to.to_string(), PushedRefStatus { flag, summary }))
    })
    .collect()
}

/// Fetch `full_names` from `remote` into their remote-tracking refs, after a rejected push: the remote branches moved,
/// and the caller shows what they have now. Only these refs move, as `fetch_baseline` moves only the baseline's.
/// A branch deleted on the remote can't be fetched: `ls-remote` tells it from a failed connection, and its
/// remote-tracking ref is deleted
#[instrument(skip(git_executor, full_names))]
pub fn fetch_remote_branches(git_executor: &GitCommandExecutor, repository_path: &str, remote: &str, full_names: &[String]) -> Result<()> {
  let fetch = |names: &[&String]| {
    let refspecs: Vec<String> = names.iter().map(|full_name| format!("+refs/heads/{full_name}:refs/remotes/{remote}/{full_name}")).collect();
    let mut args = vec![
      "fetch",
      "--porcelain",
      "--no-tags",
      "--no-write-fetch-head",
      "--no-recurse-submodules",
      "--no-auto-maintenance",
      remote,
    ];
    args.extend(refspecs.iter().map(String::as_str));
    execute_remote_command(git_executor, &args, repository_path).map(|_| ())
  };
  let all: Vec<&String> = full_names.iter().collect();
  let Err(fetch_error) = fetch(&all) else {
    return Ok(());
  };

  // 2: none of the refs is on the remote
  let mut args = vec!["ls-remote", "--exit-code", remote];
  let patterns: Vec<String> = full_names.iter().map(|full_name| format!("refs/heads/{full_name}")).collect();
  args.extend(patterns.iter().map(String::as_str));
  let Ok(output) = execute_remote_command_accepting(git_executor, &args, repository_path, &[2]) else {
    return Err(fetch_error);
  };
  let existing: HashSet<&str> = output.lines().filter_map(|line| line.split_once('\t')).map(|(_, reference)| reference).collect();
  let (present, deleted): (Vec<&String>, Vec<&String>) = full_names.iter().partition(|full_name| existing.contains(format!("refs/heads/{full_name}").as_str()));
  if deleted.is_empty() {
    return Err(fetch_error);
  }
  let mut transaction = RefTransaction::new();
  for full_name in &deleted {
    transaction.delete(&format!("refs/remotes/{remote}/{full_name}"), None);
  }
  transaction.commit(git_executor, repository_path, "remote branch deleted")?;
  if present.is_empty() { Ok(()) } else { fetch(&present) }
}

/// Record a successful push of `refs` in one ref transaction, as the leases of the next push
pub fn record_push<'a>(git_executor: &GitCommandExecutor, repository_path: &str, refs: impl IntoIterator<Item = &'a PushRef>) -> Result<()> {
  let mut transaction = RefTransaction::new();
  for pushed in refs {
    transaction.update(&format!("{PUSHED_NAMESPACE}{}", pushed.full_name), &pushed.tip, None);
  }
  transaction.commit(git_executor, repository_path, "record push")
}

#[cfg(test)]
#[path = "push_lease_test.rs"]
mod push_lease_test;
