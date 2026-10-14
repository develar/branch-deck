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

use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ref_transaction::RefTransaction;
use std::collections::HashMap;
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
  /// Neither: the remote branch must not exist
  Absent,
}

impl LeaseSource {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Recorded => "recorded",
      Self::Tracking => "tracking",
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
  let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
  let refs: HashMap<String, String> = git_executor
    .execute_command_lines(&arg_refs, repository_path)?
    .iter()
    .filter_map(|line| line.split_once(' ').map(|(id, reference)| (reference.to_string(), id.to_string())))
    .collect();

  full_names
    .iter()
    .map(|full_name| {
      let tip = refs
        .get(&format!("refs/heads/{full_name}"))
        .ok_or_else(|| anyhow!("Branch {full_name} does not exist, sync first"))?;
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
