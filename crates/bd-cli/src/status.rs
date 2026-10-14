use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use git_ops::model::to_final_branch_name;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use sync_core::grouping::{BranchGrouping, group_commits_ahead_of_baseline};

#[derive(Debug, Default, clap::Args)]
pub struct StatusArgs {
  /// Only this branch, its commits paged by --limit/--offset
  #[arg(long)]
  pub branch: Option<String>,
  /// Only the unassigned commits, paged by --limit/--offset
  #[arg(long, conflicts_with = "branch")]
  pub unassigned: bool,
  /// Page size: of the branches, or of the commits with --branch or --unassigned
  #[arg(long)]
  pub limit: Option<usize>,
  #[arg(long, default_value_t = 0)]
  pub offset: usize,
}

/// `bd status`: the branches Branch Deck would sync and the unassigned commits
#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub head: Head,
  /// The branch the commits are ahead of, e.g. `origin/master`
  pub baseline: String,
  pub branch_count: usize,
  pub unassigned_count: usize,
  /// Newest first; omitted with --unassigned
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branches: Option<Vec<BranchStatus>>,
  /// Newest first; omitted with --branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unassigned: Option<Vec<CommitSummary>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Head {
  pub id: String,
  /// The checked out branch; `null` when HEAD is detached
  pub branch: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchStatus {
  /// The group name: the `(prefix)` of the subjects or their issue key
  pub name: String,
  /// The virtual branch, `refs/heads/{prefix}/virtual/{name}`
  #[serde(rename = "ref")]
  pub reference: String,
  /// The parent branch of a stacked branch (not available yet)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_key: Option<String>,
  /// The newest committer time of its commits, Unix seconds
  pub latest_time: u32,
  pub commit_count: usize,
  /// The tip of the virtual branch as last synced; `null` if it was never synced
  pub tip: Option<String>,
  /// Newest first; subjects without the `(prefix)`
  pub commits: Vec<CommitSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitSummary {
  pub id: String,
  pub subject: String,
}

/// The checked out commit and branch, from one `rev-parse`
pub fn read_head(context: &RepoContext) -> Result<(String, Option<String>), CliError> {
  let (output, exit_code) = context
    .git
    .execute_command_with_status(&["rev-parse", "HEAD", "--symbolic-full-name", "HEAD"], &context.repository)?;
  let mut lines = output.lines();
  let (Some(id), 0) = (lines.next(), exit_code) else {
    return Err(CliError::new(ErrorCode::GitError, format!("Cannot read HEAD: {output}")));
  };
  let branch = lines.next().and_then(|reference| reference.strip_prefix("refs/heads/")).map(str::to_string);
  Ok((id.to_string(), branch))
}

/// Group the commits ahead of the baseline; a missing baseline is `no_baseline`, a `branch` that is not a group is
/// `unknown_branch`
pub fn read_grouping(context: &RepoContext, branch: Option<&str>) -> Result<BranchGrouping, CliError> {
  let grouping = group_commits_ahead_of_baseline(&context.git, &context.repository).map_err(|e| {
    let message = format!("{e:#}");
    if message.starts_with("No baseline branch found") {
      CliError::new(ErrorCode::NoBaseline, message)
    } else {
      CliError::from(e)
    }
  })?;
  if let Some(name) = branch
    && !grouping.branches.iter().any(|candidate| candidate.name == name)
  {
    return Err(CliError::new(
      ErrorCode::UnknownBranch,
      format!("No branch named {name:?} among the commits ahead of {}", grouping.baseline_branch),
    ));
  }
  Ok(grouping)
}

/// `(offset, limit)` of a listing
type Page = (usize, Option<usize>);

fn page<T>(items: impl IntoIterator<Item = T>, (offset, limit): Page) -> impl Iterator<Item = T> {
  items.into_iter().skip(offset).take(limit.unwrap_or(usize::MAX))
}

pub fn status(context: &RepoContext, args: &StatusArgs) -> Result<StatusOutput, CliError> {
  let (head_id, head_branch) = read_head(context)?;
  let grouping = read_grouping(context, args.branch.as_deref())?;

  let tips = read_virtual_tips(context)?;
  let short = context.short_ids(
    grouping
      .branches
      .iter()
      .flat_map(|branch| branch.commits.iter())
      .chain(&grouping.unassigned)
      .map(|commit| commit.id.as_str())
      .chain(tips.values().map(String::as_str))
      .chain(std::iter::once(head_id.as_str())),
  )?;
  let summary = |commit: &git_ops::commit_list::Commit| CommitSummary {
    id: short.get(&commit.id).to_string(),
    subject: commit.stripped_subject.clone(),
  };

  // --branch and --unassigned page the commits they list, else the branches are paged
  let paged = (args.offset, args.limit);
  let whole = (0, None);
  let (branch_page, commit_page) = if args.branch.is_some() { (whole, paged) } else { (paged, whole) };
  let branches = (!args.unassigned).then(|| {
    let selected = grouping.branches.iter().filter(|branch| args.branch.as_ref().is_none_or(|name| &branch.name == name));
    page(selected, branch_page)
      .map(|branch| {
        let reference = to_final_branch_name(&context.prefix, &branch.name).map_or_else(|_| String::new(), |name| format!("refs/heads/{name}"));
        BranchStatus {
          tip: tips.get(&reference).map(|tip| short.get(tip).to_string()),
          name: branch.name.clone(),
          reference,
          parent: None,
          issue_key: branch.issue_key.clone(),
          latest_time: branch.latest_commit_time,
          commit_count: branch.commits.len(),
          commits: page(&branch.commits, commit_page).map(summary).collect(),
        }
      })
      .collect()
  });
  let unassigned_page = if args.unassigned { paged } else { whole };
  let unassigned = args.branch.is_none().then(|| page(&grouping.unassigned, unassigned_page).map(summary).collect());

  Ok(StatusOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    head: Head {
      id: short.get(&head_id).to_string(),
      branch: head_branch,
    },
    baseline: grouping.baseline_branch.clone(),
    branch_count: grouping.branches.len(),
    unassigned_count: grouping.unassigned.len(),
    branches,
    unassigned,
  })
}

/// Full ref -> commit id of the existing virtual branches, from one `for-each-ref`
fn read_virtual_tips(context: &RepoContext) -> Result<HashMap<String, String>, CliError> {
  let namespace = format!("refs/heads/{}/virtual/", context.prefix.trim_end_matches('/'));
  let lines = context
    .git
    .execute_command_lines(&["for-each-ref", "--format=%(refname) %(objectname)", &namespace], &context.repository)?;
  Ok(
    lines
      .into_iter()
      .filter_map(|line| line.rsplit_once(' ').map(|(reference, id)| (reference.to_string(), id.to_string())))
      .collect(),
  )
}
