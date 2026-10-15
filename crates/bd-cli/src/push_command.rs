//! `bd push`: sync, then push the virtual branches in one atomic `git push --porcelain` with an explicit lease per ref,
//! parents first, and print a ready `gh pr create` per branch.
//!
//! The lease is the one the app's push button uses too (`sync_core::push_lease`): the remote head the sync in the same
//! process compared with (`remote.head`), or the one `--expect-remote` names. The push replaces exactly that remote
//! state; the refusal of a branch behind its remote without `--force` decides whether it may drop commits. A rejected
//! push fetches the branches and reports what the remote has now. With `--remote` other than `origin`, which the sync
//! compares with, the remote status of the pushed branches is computed the same way against that remote.

use crate::context::RepoContext;
use crate::describe::read_descriptions;
use crate::error::{CliError, ErrorBranch, ErrorCode};
use crate::output::SchemaVersion;
use crate::status::read_grouping;
use crate::sync_command::{RemoteSummary, SyncState, SyncedBranch, lock, sync_locked};
use git_ops::model::to_final_branch_name;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashSet;
use sync_core::grouping::branch_tips;
use sync_core::pull_requests::{parse_github_repository, resolve_github_repository};
use sync_core::push_lease::{PushRef, fetch_remote_branches, parse_push_porcelain, prepare_push_expecting};
use sync_core::remote_command::execute_remote_command_accepting;
use sync_core::remote_status::compute_remote_status_for_branch;

#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct PushArgs {
  /// The branches to push; their parents must be on the remote or pushed with them
  pub branches: Vec<String>,
  /// Push every branch that syncs cleanly; needs --yes
  #[arg(long, conflicts_with = "branches")]
  pub all: bool,
  /// Confirm --all
  #[arg(long, requires = "all")]
  pub yes: bool,
  /// Push the branches stacked on the named ones too, directly or further down
  #[arg(long, requires = "branches", conflicts_with = "expect_remote")]
  pub with_children: bool,
  /// Push branches the remote has commits for that they don't (behind > 0): the push drops the commits the sync
  /// counted, nothing pushed since
  #[arg(long)]
  pub force: bool,
  /// Lease on this remote commit: the push replaces the remote branch only if it is still there. Pass the
  /// `remote.head` of the status the user saw, e.g. of a `lease_rejected` error. Needs exactly one branch
  #[arg(long, value_name = "COMMIT", conflicts_with = "all")]
  pub expect_remote: Option<String>,
  /// The remote; default: the remote of the baseline (`origin` for `origin/master`)
  #[arg(long)]
  pub remote: Option<String>,
  /// Ask the remote, change nothing (`git push --dry-run`)
  #[arg(long)]
  pub dry_run: bool,
}

/// `bd push`
#[derive(Debug, Serialize, JsonSchema)]
pub struct PushOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub remote: String,
  pub dry_run: bool,
  /// Parents first
  pub branches: Vec<PushedBranch>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PushedBranch {
  pub name: String,
  /// The branch on the remote, the same name as the local virtual branch
  pub remote_branch: String,
  pub commit: String,
  /// `new`, `updated`, `forced` or `up_to_date`
  pub status: String,
  /// The commit the remote branch had to be at; `null` when it had not to exist
  pub lease: Option<String>,
  /// `expected` (the remote head of the status: the sync's, or --expect-remote) or `absent` (no remote branch: it must
  /// not exist)
  pub lease_source: String,
  pub pull_request: PullRequestDraft,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PullRequestDraft {
  /// The remote branch the pull request merges into: the parent's, or the baseline without the remote
  pub base: String,
  pub head: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  /// Pipe it into the command (`--body-file -`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub body: Option<String>,
  /// A ready `gh pr create`
  pub command: String,
}

/// A branch to push, with what decides its lease
struct Candidate<'a> {
  synced: &'a SyncedBranch,
  full_name: String,
  parent: Option<String>,
  /// The status against the remote pushed to; `None` without one
  remote: Option<RemoteSummary>,
}

pub fn push(context: &RepoContext, args: &PushArgs) -> Result<PushOutput, CliError> {
  if args.branches.is_empty() && !args.all {
    return Err(CliError::new(ErrorCode::InvalidArguments, "Name the branches to push, or pass --all --yes"));
  }
  if args.all && !args.yes {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--all pushes every branch: confirm with --all --yes"));
  }
  if args.expect_remote.is_some() && args.branches.len() != 1 {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      "--expect-remote is the remote head of one branch: name exactly one",
    ));
  }

  let _lock = lock(context)?;
  let synced = sync_locked(context)?;
  let remote = match &args.remote {
    Some(remote) => remote.clone(),
    None => synced
      .baseline
      .split_once('/')
      .map(|(remote, _)| remote.to_string())
      .ok_or_else(|| CliError::new(ErrorCode::NoRemote, format!("The baseline {} is local: pass --remote", synced.baseline)))?,
  };
  check_remote(context, &remote)?;
  let base_branch = synced.baseline.strip_prefix(&format!("{remote}/")).unwrap_or(&synced.baseline).to_string();

  let grouping = read_grouping(context, None)?;
  let placement = |name: &str| grouping.placements.as_ref().and_then(|placements| placements.get(name));
  let parent_of = |name: &str| placement(name).and_then(|stack| stack.effective_base.clone());
  // parents first: the stack order with stacked branches, else the sync order
  let order: Vec<&str> = grouping.forest.as_ref().map_or_else(
    || synced.branches.iter().map(|branch| branch.name.as_str()).collect(),
    |forest| forest.order.iter().map(String::as_str).collect(),
  );

  let selected: Vec<&str> = if args.all {
    order.clone()
  } else {
    for name in &args.branches {
      if !synced.branches.iter().any(|branch| &branch.name == name) {
        return Err(CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}")));
      }
    }
    let mut selected: Vec<&str> = Vec::new();
    for name in &order {
      // a blocked child has no effective base: its declared parent places it, and the push refuses it as not synced
      let parent = placement(name).and_then(|stack| stack.effective_base.as_deref().or(stack.declared_parent.as_deref()));
      // the order is parents first: a child comes after its parent was selected
      if args.branches.iter().any(|named| named == name) || (args.with_children && parent.is_some_and(|parent| selected.contains(&parent))) {
        selected.push(name);
      }
    }
    selected
  };

  let tips = branch_tips(&context.git, &context.repository, &context.prefix)?;
  let on_remote = remote_branches(context, &remote)?;
  let mut candidates = Vec::with_capacity(selected.len());
  let mut refusals = Vec::new();
  let mut behind = Vec::new();
  for name in &selected {
    let Some(branch) = synced.branches.iter().find(|branch| branch.name == *name) else {
      continue;
    };
    if !matches!(branch.status, SyncState::Created | SyncState::Updated | SyncState::Unchanged) {
      let reason = branch
        .error
        .clone()
        .or_else(|| branch.conflict.as_ref().map(|conflict| format!("conflict in {}", conflict.files.join(", "))));
      refusals.push(format!(
        "{name} is not synced ({})",
        reason.unwrap_or_else(|| format!("{:?}", branch.status).to_lowercase())
      ));
      continue;
    }
    let full_name = final_name(context, name)?;
    if !tips.contains_key(&full_name) {
      refusals.push(format!("{name} has no virtual branch"));
      continue;
    }
    let parent = parent_of(name);
    let parent_ref = parent.as_deref().map(|parent| final_name(context, parent)).transpose()?;
    let remote_state = remote_status(context, &remote, &synced.baseline, branch, &full_name, parent_ref.as_deref())?;
    if let Some(remote_state) = &remote_state
      && remote_state.behind > 0
      && !args.force
    {
      refusals.push(format!(
        "{name}: the remote branch has {} commits the branch doesn't; pass --force to replace them",
        remote_state.behind
      ));
      behind.push(ErrorBranch {
        name: (*name).to_string(),
        remote: remote_state.clone(),
      });
    }
    if let (Some(parent), Some(parent_ref)) = (&parent, &parent_ref)
      && !on_remote.contains(parent_ref)
      && !selected.contains(&parent.as_str())
    {
      refusals.push(format!("{name}: its parent {parent} is not on the remote; push {parent} first or with it"));
    }
    candidates.push(Candidate {
      synced: branch,
      full_name,
      parent,
      remote: remote_state,
    });
  }
  if !refusals.is_empty() {
    let mut error = CliError::new(ErrorCode::PushRefused, format!("Nothing was pushed: {}", refusals.join("; ")));
    error.branches = behind;
    return Err(error);
  }

  let expected: Vec<(String, Option<String>)> = match &args.expect_remote {
    Some(expected) => vec![(candidates[0].full_name.clone(), Some(resolve_commit(context, expected)?))],
    // the remote head the status compared with: a remote commit the branch has by content goes, and --force drops what
    // the status counted, but nothing pushed since
    None => candidates
      .iter()
      .map(|candidate| (candidate.full_name.clone(), candidate.remote.as_ref().and_then(|remote| remote.head_id.clone())))
      .collect(),
  };
  let refs = prepare_push_expecting(&context.git, &context.repository, &expected)?;
  let lease_args: Vec<String> = refs.iter().map(PushRef::lease_arg).collect();
  let refspecs: Vec<String> = refs.iter().map(PushRef::refspec).collect();
  let mut push_args = vec!["push", "--porcelain", "--atomic"];
  if args.dry_run {
    push_args.push("--dry-run");
  }
  push_args.push(&remote);
  push_args.extend(lease_args.iter().map(String::as_str));
  push_args.extend(refspecs.iter().map(String::as_str));
  // 1: a ref was rejected; the porcelain output still reports every ref
  let output = execute_remote_command_accepting(&context.git, &push_args, &context.repository, &[1]).map_err(|e| CliError::new(ErrorCode::NetworkError, format!("{e:#}")))?;
  let statuses = parse_push_porcelain(&output);

  let rejected: Vec<&Candidate> = candidates
    .iter()
    .filter(|candidate| statuses.get(&format!("refs/heads/{}", candidate.full_name)).is_none_or(|status| !status.accepted()))
    .collect();
  if !rejected.is_empty() {
    return Err(lease_rejected(context, &remote, &synced.baseline, &rejected, args.dry_run));
  }

  let descriptions = read_descriptions(context)?;
  let repository_option = github_repository_option(context)?;
  let short = context.short_ids(refs.iter().flat_map(|pushed| std::iter::once(pushed.tip.as_str()).chain(pushed.lease.as_deref())));
  let branches = candidates
    .iter()
    .zip(&refs)
    .map(|(candidate, pushed)| {
      let flag = statuses.get(&format!("refs/heads/{}", candidate.full_name)).map_or(' ', |status| status.flag);
      let base = candidate
        .parent
        .as_ref()
        .and_then(|parent| to_final_branch_name(&context.prefix, parent).ok())
        .unwrap_or_else(|| base_branch.clone());
      let description = descriptions.get(&format!("refs/heads/{}", candidate.full_name));
      let command = match description {
        Some(description) => format!(
          "gh pr create{repository_option} --base {} --head {} --title {} --body-file -",
          shell_quote(&base),
          shell_quote(&candidate.full_name),
          shell_quote(&description.title)
        ),
        None => format!(
          "gh pr create{repository_option} --base {} --head {} --fill",
          shell_quote(&base),
          shell_quote(&candidate.full_name)
        ),
      };
      PushedBranch {
        name: candidate.synced.name.clone(),
        remote_branch: candidate.full_name.clone(),
        commit: short.get(&pushed.tip).to_string(),
        status: match flag {
          '*' => "new",
          '+' => "forced",
          '=' => "up_to_date",
          _ => "updated",
        }
        .to_string(),
        lease: pushed.lease.as_deref().map(|lease| short.get(lease).to_string()),
        lease_source: pushed.source.as_str().to_string(),
        pull_request: PullRequestDraft {
          base,
          head: candidate.full_name.clone(),
          title: description.map(|description| description.title.clone()),
          body: description.map(|description| description.body.clone()).filter(|body| !body.is_empty()),
          command,
        },
      }
    })
    .collect();

  Ok(PushOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    remote,
    dry_run: args.dry_run,
    branches,
  })
}

/// `<prefix>/virtual/<name>`
fn final_name(context: &RepoContext, name: &str) -> Result<String, CliError> {
  to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))
}

/// `no_remote` unless `remote` is a configured remote: a URL has no remote-tracking refs to compare with
fn check_remote(context: &RepoContext, remote: &str) -> Result<(), CliError> {
  let (_, exit_code) = context
    .git
    .execute_command_with_status(&["config", "--get", &format!("remote.{remote}.url")], &context.repository)?;
  if exit_code == 0 {
    Ok(())
  } else {
    Err(CliError::new(ErrorCode::NoRemote, format!("No remote named {remote:?}: `git remote` lists them")))
  }
}

/// The virtual branches that have a remote-tracking ref of `remote`, by full name: one `for-each-ref`
fn remote_branches(context: &RepoContext, remote: &str) -> Result<HashSet<String>, CliError> {
  let namespace = format!("refs/remotes/{remote}/");
  let pattern = format!("{namespace}{}/virtual/", context.prefix.trim_end_matches('/'));
  let lines = context.git.execute_command_lines(&["for-each-ref", "--format=%(refname)", &pattern], &context.repository)?;
  Ok(lines.iter().filter_map(|line| line.strip_prefix(&namespace)).map(str::to_string).collect())
}

/// The status of `branch` against `remote`: the one of the sync for `origin`, which the sync compares with; for another
/// remote computed now the same way, from its remote-tracking ref
fn remote_status(context: &RepoContext, remote: &str, baseline: &str, branch: &SyncedBranch, full_name: &str, parent_ref: Option<&str>) -> Result<Option<RemoteSummary>, CliError> {
  if remote == "origin" {
    return Ok(branch.remote.clone());
  }
  let status = compute_remote_status_for_branch(&context.git, &context.repository, remote, full_name, &branch.name, None, baseline, parent_ref)?;
  let short = context.short_ids(RemoteSummary::ids(&status));
  Ok(Some(RemoteSummary::new(&status, branch.commit_count, &short)))
}

/// The full id of the commit `rev` names
fn resolve_commit(context: &RepoContext, rev: &str) -> Result<String, CliError> {
  let (id, exit_code) = context
    .git
    .execute_command_with_status(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")], &context.repository)?;
  if exit_code == 0 {
    Ok(id)
  } else {
    Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!("--expect-remote {rev} is not a commit: pass the `remote.head` bd printed"),
    ))
  }
}

/// `lease_rejected` for the `rejected` branches: nothing was pushed. The branches are fetched (not on a dry run), and
/// the error lists what each has now, so the user can see what a push with `--force --expect-remote` would drop
fn lease_rejected(context: &RepoContext, remote: &str, baseline: &str, rejected: &[&Candidate], dry_run: bool) -> CliError {
  let names = rejected.iter().map(|candidate| candidate.synced.name.as_str()).collect::<Vec<_>>().join(", ");
  let full_names: Vec<String> = rejected.iter().map(|candidate| candidate.full_name.clone()).collect();
  if dry_run {
    return CliError::new(ErrorCode::LeaseRejected, format!("The remote would reject {names}: it is not where the lease expects it"));
  }
  if let Err(e) = fetch_remote_branches(&context.git, &context.repository, remote, &full_names) {
    return CliError::new(
      ErrorCode::NetworkError,
      format!("The remote rejected {names}: it is not where the lease expects it. Nothing was pushed, and fetching it failed: {e:#}"),
    );
  }

  let computed: anyhow::Result<Vec<_>> = rejected
    .iter()
    .map(|candidate| {
      let parent_ref = candidate.parent.as_deref().map(|parent| to_final_branch_name(&context.prefix, parent)).transpose()?;
      compute_remote_status_for_branch(
        &context.git,
        &context.repository,
        remote,
        &candidate.full_name,
        &candidate.synced.name,
        None,
        baseline,
        parent_ref.as_deref(),
      )
    })
    .collect();
  let remote_statuses = match computed {
    Ok(computed) => computed,
    Err(e) => return CliError::from(e),
  };
  let short = context.short_ids(remote_statuses.iter().flat_map(RemoteSummary::ids));
  let mut error = CliError::new(
    ErrorCode::LeaseRejected,
    format!(
      "The remote rejected {names}: it is not where the lease expects it. Nothing was pushed; bd fetched it, and \
       `branches` says what the remote has now (`dropped_commits`: the commits a push drops). Show them to the user; to \
       replace them, push again with --force --expect-remote <remote.head>"
    ),
  );
  error.branches = rejected
    .iter()
    .zip(&remote_statuses)
    .map(|(candidate, status)| ErrorBranch {
      name: candidate.synced.name.clone(),
      remote: RemoteSummary::new(status, candidate.synced.commit_count, &short),
    })
    .collect();
  error
}

/// ` --repo owner/name` when the GitHub repository of the branches is not the one `origin` pushes to (a mirror:
/// `branchdeck.githubRepository` or the Space settings): `gh` finds no GitHub repository among the remotes; empty
/// otherwise
fn github_repository_option(context: &RepoContext) -> Result<String, CliError> {
  let Some(repository) = resolve_github_repository(&context.git, &context.repository)? else {
    return Ok(String::new());
  };
  let (url, exit_code) = context.git.execute_command_with_status(&["remote", "get-url", "--push", "origin"], &context.repository)?;
  if exit_code == 0 && parse_github_repository(&url).as_ref() == Some(&repository) {
    return Ok(String::new());
  }
  Ok(format!(" --repo {}", shell_quote(&format!("{}/{}", repository.owner, repository.name))))
}

fn shell_quote(word: &str) -> String {
  if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@')) {
    word.to_string()
  } else {
    format!("'{}'", word.replace('\'', r"'\''"))
  }
}
