//! `bd push`: sync, then push the virtual branches in one atomic `git push --porcelain` with an explicit lease per ref,
//! parents first, and print a ready `gh pr create` per branch.
//!
//! The lease is the commit this command last pushed for the ref, kept in `refs/branch-deck/pushed/<branch>`: one
//! `for-each-ref` reads all of them and one `update-ref --stdin` records a push, atomically and batched as every other
//! ref change; the ref keeps the pushed commit reachable, and it never leaves the repository (no refspec fetches or
//! pushes `refs/branch-deck/`). A config key (`branch.<name>.branchDeckPushed`) costs one process per branch, and a
//! notes ref would be fetched and pushed along with the user's notes. A background fetch moves the remote-tracking ref,
//! never this one, so it can't defeat the lease.

use crate::context::RepoContext;
use crate::describe::read_descriptions;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use crate::status::read_grouping;
use crate::sync_command::{SyncState, SyncedBranch, lock, sync_locked};
use git_ops::model::to_final_branch_name;
use git_ops::ref_transaction::RefTransaction;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use sync_core::grouping::branch_tips;
use sync_core::remote_command::execute_remote_command_accepting;

/// The namespace of the recorded leases
pub const PUSHED_NAMESPACE: &str = "refs/branch-deck/pushed/";

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
  /// Push branches the remote has commits for that they don't (behind > 0), leased on the fetched remote branch
  #[arg(long)]
  pub force: bool,
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
  /// `recorded` (the last push of bd), `tracking` (the fetched remote branch: never pushed by bd, or --force) or
  /// `absent`
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
  tip: String,
  parent: Option<String>,
}

pub fn push(context: &RepoContext, args: &PushArgs) -> Result<PushOutput, CliError> {
  if args.branches.is_empty() && !args.all {
    return Err(CliError::new(ErrorCode::InvalidArguments, "Name the branches to push, or pass --all --yes"));
  }
  if args.all && !args.yes {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--all pushes every branch: confirm with --all --yes"));
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
  let base_branch = synced.baseline.strip_prefix(&format!("{remote}/")).unwrap_or(&synced.baseline).to_string();

  let grouping = read_grouping(context, None)?;
  let parent_of = |name: &str| {
    grouping
      .placements
      .as_ref()
      .and_then(|placements| placements.get(name))
      .and_then(|stack| stack.effective_base.clone())
  };
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
    order.iter().copied().filter(|name| args.branches.iter().any(|selected| selected == name)).collect()
  };

  let tips = branch_tips(&context.git, &context.repository, &context.prefix)?;
  let mut candidates = Vec::with_capacity(selected.len());
  let mut refusals = Vec::new();
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
    if let Some(remote_state) = &branch.remote
      && remote_state.behind > 0
      && !args.force
    {
      refusals.push(format!(
        "{name}: the remote branch has {} commits the branch doesn't; pass --force to replace them",
        remote_state.behind
      ));
    }
    let parent = parent_of(name);
    if let Some(parent) = &parent {
      let parent_on_remote = synced
        .branches
        .iter()
        .any(|other| &other.name == parent && other.remote.as_ref().is_some_and(|remote| remote.exists));
      if !parent_on_remote && !selected.contains(&parent.as_str()) {
        refusals.push(format!("{name}: its parent {parent} is not on the remote; push {parent} first or with it"));
      }
    }
    let full_name = to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
    let Some(tip) = tips.get(&full_name).cloned() else {
      refusals.push(format!("{name} has no virtual branch"));
      continue;
    };
    candidates.push(Candidate {
      synced: branch,
      full_name,
      tip,
      parent,
    });
  }
  if !refusals.is_empty() {
    return Err(CliError::new(ErrorCode::PushRefused, format!("Nothing was pushed: {}", refusals.join("; "))));
  }

  let recorded = read_refs(context, PUSHED_NAMESPACE)?;
  let tracking = read_refs(context, &format!("refs/remotes/{remote}/"))?;
  let leases: Vec<(Option<String>, &str)> = candidates
    .iter()
    .map(|candidate| {
      let tracked = tracking.get(&candidate.full_name);
      match (recorded.get(&candidate.full_name), tracked) {
        // --force replaces what was fetched, not what bd pushed last
        (_, Some(tracked)) if args.force => (Some(tracked.clone()), "tracking"),
        (Some(pushed), _) => (Some(pushed.clone()), "recorded"),
        (None, Some(tracked)) => (Some(tracked.clone()), "tracking"),
        (None, None) => (None, "absent"),
      }
    })
    .collect();

  let lease_args: Vec<String> = candidates
    .iter()
    .zip(&leases)
    .map(|(candidate, (lease, _))| format!("--force-with-lease=refs/heads/{}:{}", candidate.full_name, lease.as_deref().unwrap_or_default()))
    .collect();
  let refspecs: Vec<String> = candidates.iter().map(|candidate| format!("refs/heads/{0}:refs/heads/{0}", candidate.full_name)).collect();
  let mut push_args = vec!["push", "--porcelain", "--atomic"];
  if args.dry_run {
    push_args.push("--dry-run");
  }
  push_args.push(&remote);
  push_args.extend(lease_args.iter().map(String::as_str));
  push_args.extend(refspecs.iter().map(String::as_str));
  // 1: a ref was rejected; the porcelain output still reports every ref
  let output = execute_remote_command_accepting(&context.git, &push_args, &context.repository, &[1]).map_err(|e| CliError::new(ErrorCode::NetworkError, format!("{e:#}")))?;
  let statuses = parse_porcelain(&output);

  let rejected: Vec<&str> = candidates
    .iter()
    .filter(|candidate| statuses.get(&format!("refs/heads/{}", candidate.full_name)).is_none_or(|flag| *flag == '!'))
    .map(|candidate| candidate.synced.name.as_str())
    .collect();
  if !rejected.is_empty() {
    return Err(CliError::new(
      ErrorCode::LeaseRejected,
      format!(
        "The remote rejected {}: it changed since the last push. Nothing was pushed; fetch, run `bd status` and retry",
        rejected.join(", ")
      ),
    ));
  }

  if !args.dry_run {
    let mut transaction = RefTransaction::new();
    for candidate in &candidates {
      transaction.update(&format!("{PUSHED_NAMESPACE}{}", candidate.full_name), &candidate.tip, None);
    }
    transaction.commit(&context.git, &context.repository, "record push")?;
  }

  let descriptions = read_descriptions(context)?;
  let short = context.short_ids(
    candidates
      .iter()
      .map(|candidate| candidate.tip.as_str())
      .chain(leases.iter().filter_map(|(lease, _)| lease.as_deref())),
  );
  let branches = candidates
    .iter()
    .zip(leases)
    .map(|(candidate, (lease, source))| {
      let flag = statuses.get(&format!("refs/heads/{}", candidate.full_name)).copied().unwrap_or(' ');
      let base = candidate
        .parent
        .as_ref()
        .and_then(|parent| to_final_branch_name(&context.prefix, parent).ok())
        .unwrap_or_else(|| base_branch.clone());
      let description = descriptions.get(&format!("refs/heads/{}", candidate.full_name));
      let command = match description {
        Some(description) => format!(
          "gh pr create --base {} --head {} --title {} --body-file -",
          shell_quote(&base),
          shell_quote(&candidate.full_name),
          shell_quote(&description.title)
        ),
        None => format!("gh pr create --base {} --head {} --fill", shell_quote(&base), shell_quote(&candidate.full_name)),
      };
      PushedBranch {
        name: candidate.synced.name.clone(),
        remote_branch: candidate.full_name.clone(),
        commit: short.get(&candidate.tip).to_string(),
        status: match flag {
          '*' => "new",
          '+' => "forced",
          '=' => "up_to_date",
          _ => "updated",
        }
        .to_string(),
        lease: lease.as_deref().map(|lease| short.get(lease).to_string()),
        lease_source: source.to_string(),
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

/// Branch full name (without the namespace) -> commit of the refs in `namespace`, from one `for-each-ref`
fn read_refs(context: &RepoContext, namespace: &str) -> Result<HashMap<String, String>, CliError> {
  let lines = context
    .git
    .execute_command_lines(&["for-each-ref", "--format=%(objectname) %(refname)", namespace], &context.repository)?;
  Ok(
    lines
      .iter()
      .filter_map(|line| {
        let (id, reference) = line.split_once(' ')?;
        Some((reference.strip_prefix(namespace)?.to_string(), id.to_string()))
      })
      .collect(),
  )
}

/// Remote ref -> flag of the `push --porcelain` lines `<flag> TAB <from>:<to> TAB <summary>`
pub fn parse_porcelain(output: &str) -> HashMap<String, char> {
  output
    .lines()
    .filter_map(|line| {
      let mut fields = line.splitn(3, '\t');
      let flag = fields.next()?.chars().next()?;
      let (_, to) = fields.next()?.split_once(':')?;
      Some((to.to_string(), flag))
    })
    .collect()
}

/// A word for a POSIX shell: single-quoted unless it is plain
fn shell_quote(word: &str) -> String {
  if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '@')) {
    word.to_string()
  } else {
    format!("'{}'", word.replace('\'', r"'\''"))
  }
}
