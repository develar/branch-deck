//! The lease of a virtual branch push, shared by the app's push and `bd push`.
//!
//! A virtual branch is rebuilt on every sync, so a push must force; the lease keeps it from overwriting a remote
//! branch that changed since the user looked. The lease is the remote head of the status the user saw
//! (`prepare_push_expecting`): the app's row, the sync `bd push` runs first, or `bd push --expect-remote`. The push
//! replaces exactly that remote state, also after someone else's commit was fetched and shown as dropped, or brought
//! into the branch. A rejected push fetches the branches (`fetch_remote_branches`), so the caller can show what the
//! remote has now. The status, and so the lease, compares with the remote-tracking ref: a background fetch that
//! moves it shows up as the commits a push drops (`behind`), which a push replaces only once the user saw them.

use crate::remote_command::{execute_remote_command, execute_remote_command_accepting, execute_remote_command_output, execute_remote_command_with_progress};
use crate::transfer_progress::{TransferProgress, TransferProgressAnalyzer};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::ref_transaction::RefTransaction;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

/// Where the lease of a push comes from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseSource {
  /// The remote head the caller showed the user (`prepare_push_expecting`)
  Expected,
  /// No remote branch: it must not exist
  Absent,
}

impl LeaseSource {
  pub const fn as_str(self) -> &'static str {
    match self {
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

/// The push of each `(full_name, expected)` leased on `expected`: the remote head of the status the caller showed the
/// user (`RemoteStatusUpdate::remote_tip`), `None` when it showed no remote branch. The push replaces the remote branch
/// only if it is still there, so it drops what the user saw dropped and nothing else. One `for-each-ref` reads the tips
#[instrument(skip(git_executor, branches))]
pub fn prepare_push_expecting(git_executor: &GitCommandExecutor, repository_path: &str, branches: &[(String, Option<String>)]) -> Result<Vec<PushRef>> {
  let mut args = vec!["for-each-ref", "--format=%(objectname) %(refname:lstrip=2)"];
  let patterns: Vec<String> = branches.iter().map(|(full_name, _)| format!("refs/heads/{full_name}")).collect();
  args.extend(patterns.iter().map(String::as_str));
  let tips: HashMap<String, String> = git_executor
    .execute_command_lines(&args, repository_path)?
    .iter()
    .filter_map(|line| line.split_once(' ').map(|(id, full_name)| (full_name.to_string(), id.to_string())))
    .collect();
  leased_push_refs(&tips, branches)
}

/// `prepare_push_expecting` for a caller that knows the tips (full name -> commit), as the sync of `bd push` left them:
/// no git process
pub fn leased_push_refs(tips: &HashMap<String, String>, branches: &[(String, Option<String>)]) -> Result<Vec<PushRef>> {
  branches
    .iter()
    .map(|(full_name, expected)| {
      Ok(PushRef {
        full_name: full_name.clone(),
        tip: tips.get(full_name).ok_or_else(|| anyhow!("Branch {full_name} does not exist, sync first"))?.clone(),
        lease: expected.clone(),
        source: if expected.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
      })
    })
    .collect()
}

/// How `push_refs` pushes
#[derive(Debug, Clone, Copy, Default)]
pub struct PushOptions {
  /// All refs or none (`--atomic`): a stack is pushed whole
  pub atomic: bool,
  /// Ask the remote, change nothing (`--dry-run`)
  pub dry_run: bool,
}

/// One `git push --porcelain <remote>` of `refs`, each with its lease (`--force-with-lease=refs/heads/<b>:<lease>`) and
/// its tip (`<tip>:refs/heads/<b>`), through the remote policy (`remote_command`); `on_progress` gets how far the push is
/// (`--progress`). Exit 1, a rejected ref, still reports every ref, and what the remote said of it. The app's push and
/// `bd push` share it; what they do with a rejection is their own
#[instrument(skip(git_executor, refs, on_progress), fields(refs = refs.len()))]
pub fn push_refs(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  remote: &str,
  refs: &[PushRef],
  options: PushOptions,
  on_progress: Option<&mut dyn FnMut(TransferProgress)>,
) -> Result<PushRefsReport> {
  let lease_args: Vec<String> = refs.iter().map(PushRef::lease_arg).collect();
  let refspecs: Vec<String> = refs.iter().map(PushRef::refspec).collect();
  let mut args = vec!["push", "--porcelain"];
  if options.atomic {
    args.push("--atomic");
  }
  if options.dry_run {
    args.push("--dry-run");
  }
  args.extend(lease_args.iter().map(String::as_str));
  args.push(remote);
  args.extend(refspecs.iter().map(String::as_str));
  let output = match on_progress {
    Some(on_progress) => execute_remote_command_with_progress(git_executor, &args, repository_path, &[1], on_progress)?,
    None => execute_remote_command_output(git_executor, &args, repository_path, &[1])?,
  };
  let messages = remote_messages(&output.stderr);
  Ok(PushRefsReport {
    refs: parse_push_porcelain(&output.into_text()),
    messages,
  })
}

/// What a `push --porcelain` reported
#[derive(Debug, Default)]
pub struct PushRefsReport {
  /// Remote ref -> its report; a ref git didn't report is absent
  pub refs: HashMap<String, PushedRefStatus>,
  /// Why, when the remote refused: the lines of stderr a hook or the server wrote (`remote: …`) and git's own errors
  /// (`error: remote unpack failed: …`), without the progress lines. The report of a ref says only
  /// `n/a (unpacker error)` or `pre-receive hook declined`
  pub messages: Vec<String>,
}

/// The lines of a push's stderr that say why: `remote: ` lines that are no progress of a stage, and git's `error:` and
/// `fatal:` lines but its `failed to push some refs`, which the reports say; not `Enumerating objects`, `Total …` or
/// the other lines of git's own progress
pub(crate) fn remote_messages(stderr: &str) -> Vec<String> {
  stderr
    .lines()
    .map(str::trim_end)
    .filter(|line| match line.strip_prefix("remote:") {
      Some(said) => !said.trim().is_empty() && TransferProgressAnalyzer::new().analyze(line).is_none(),
      None => (line.starts_with("error: ") || line.starts_with("fatal: ")) && !line.starts_with("error: failed to push some refs"),
    })
    .map(str::to_string)
    .collect()
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
  pub const fn accepted(&self) -> bool {
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

#[cfg(test)]
#[path = "push_lease_test.rs"]
mod push_lease_test;
