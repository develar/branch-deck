use crate::branch_stats::{BranchInput, BranchStats, branch_stats};
use crate::context::RepoContext;
use crate::describe::read_descriptions;
use crate::error::{CliError, ErrorCode};
use crate::output::{SchemaVersion, ShortIds};
use git_ops::model::to_final_branch_name;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use sync_core::grouping::{BranchGrouping, group_commits_ahead_of_baseline};
use sync_core::regroup::{describe_problem, problem_commits};
use sync_types::GroupedBranchInfo;
use sync_types::stack::{BlockReason, BranchStack, ParentState};

#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "independent command line flags")]
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
  /// Per branch: files, lines, top directories, authors, dates, issue keys and a diffstat of the synced branch
  #[arg(long)]
  pub stats: bool,
  /// Count lock files and generated files in the --stats lines
  #[arg(long, requires = "stats")]
  pub include_generated: bool,
  /// A compact view: per branch the name, commit count, issue key, newest subject and stack state, then the
  /// unassigned commits
  #[arg(long, conflicts_with_all = ["branch", "stats"])]
  pub brief: bool,
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
  /// With stacked branches on (`branchdeck.stacks`): the branches with commits, each after its parent
  #[serde(skip_serializing_if = "Option::is_none")]
  pub forest: Option<Vec<String>>,
  /// Newest first; omitted with --unassigned
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branches: Option<Vec<BranchStatus>>,
  /// Newest first; omitted with --branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unassigned: Option<Vec<CommitSummary>>,
}

/// `bd status --brief`: the branches of `bd status` without their commits, small enough to read whole
#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusBrief {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub head: Head,
  pub baseline: String,
  pub branch_count: usize,
  pub unassigned_count: usize,
  /// With stacked branches on: the branches with commits, each after its parent
  #[serde(skip_serializing_if = "Option::is_none")]
  pub forest: Option<Vec<String>>,
  /// Newest first; omitted with --unassigned
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branches: Option<Vec<BriefBranch>>,
  /// Newest first
  pub unassigned: Vec<CommitSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BriefBranch {
  pub name: String,
  pub commit_count: usize,
  /// The issue key of its subjects; omitted when it is `name`, an issue branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_key: Option<String>,
  /// The subject of its newest commit, without the `(prefix)`
  pub newest: String,
  /// The parent its commits declare, with stacked branches on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// Why it is not built: its parent can't be built on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<StackBlockSummary>,
  /// Why the branch can't be placed in the stack
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<StackIssue>,
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
  /// The parent its commits declare (`(parent/name)`), with stacked branches on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// Whether the declared parent has commits
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent_state: Option<ParentStateName>,
  /// The branch it is built on; omitted when it is built on the baseline
  #[serde(skip_serializing_if = "Option::is_none")]
  pub effective_base: Option<String>,
  /// Why it is not built: its parent can't be built on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<StackBlockSummary>,
  /// Why the branch can't be placed in the stack: conflicting parents, a cycle, a case collision, …
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<StackIssue>,
  /// The title of `branch.<name>.description` (`bd describe`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<DescriptionTitle>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_key: Option<String>,
  /// The newest committer time of its commits, Unix seconds
  pub latest_time: u32,
  pub commit_count: usize,
  /// The tip of the virtual branch as last synced; `null` if it was never synced
  pub tip: Option<String>,
  /// The pull request of the branch (not available yet: it will be the read-only status of `sync_core::pull_requests`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_request: Option<serde_json::Value>,
  /// With --stats
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stats: Option<BranchStats>,
  /// Newest first; subjects without the `(prefix)`
  pub commits: Vec<CommitSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParentStateName {
  /// The parent has commits
  Active,
  /// No commit is grouped into the parent and its archived branch is integrated: the branch is built on the nearest
  /// active ancestor or the baseline, and the base of its pull request changes
  Integrated,
  /// The parent was unapplied
  Unapplied,
  /// No commit is grouped into the parent
  Missing,
}

impl From<ParentState> for ParentStateName {
  fn from(state: ParentState) -> Self {
    match state {
      ParentState::Active => Self::Active,
      ParentState::Integrated => Self::Integrated,
      ParentState::Unapplied => Self::Unapplied,
      ParentState::Missing => Self::Missing,
    }
  }
}

/// Why a branch is not built: its parent can't be built on
#[derive(Debug, Serialize, JsonSchema)]
pub struct StackBlockSummary {
  pub parent: String,
  /// `missing`, `unapplied`, `error`, `conflict` or `blocked`
  pub reason: String,
}

impl StackBlockSummary {
  pub fn of(parent: &str, reason: BlockReason) -> Self {
    let reason = match reason {
      BlockReason::Missing => "missing",
      BlockReason::Unapplied => "unapplied",
      BlockReason::Error => "error",
      BlockReason::Conflict => "conflict",
      BlockReason::Blocked => "blocked",
    };
    Self {
      parent: parent.to_string(),
      reason: reason.to_string(),
    }
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StackIssue {
  pub message: String,
  /// The commits that declare it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DescriptionTitle {
  pub title: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitSummary {
  pub id: String,
  pub subject: String,
}

/// The stack problems of a branch, with their commits
pub fn stack_issues(branch: &str, stack: &BranchStack, short: &ShortIds) -> Vec<StackIssue> {
  stack
    .problems
    .iter()
    .map(|problem| StackIssue {
      message: describe_problem(branch, problem),
      commits: problem_commits(problem).iter().map(|id| short.get(id).to_string()).collect(),
    })
    .collect()
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
  let grouping = group_commits_ahead_of_baseline(&context.git, &context.repository, &context.prefix).map_err(|e| {
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

  // the brief view prints neither
  let tips = if args.brief { HashMap::new() } else { read_virtual_tips(context)? };
  let descriptions = if args.brief { HashMap::new() } else { read_descriptions(context)? };
  let forest = grouping.forest.as_ref();
  let placement = |name: &str| grouping.placements.as_ref().and_then(|placements| placements.get(name));
  let short = context.short_ids(
    grouping
      .branches
      .iter()
      .flat_map(|branch| branch.commits.iter())
      .chain(&grouping.unassigned)
      .map(|commit| commit.id.as_str())
      .chain(tips.values().map(String::as_str))
      .chain(grouping.base.as_deref())
      .chain(std::iter::once(head_id.as_str())),
  );
  let summary = |commit: &git_ops::commit_list::Commit| CommitSummary {
    id: short.get(&commit.id).to_string(),
    subject: commit.stripped_subject.clone(),
  };

  // --branch and --unassigned page the commits they list, else the branches are paged
  let paged = (args.offset, args.limit);
  let whole = (0, None);
  let (branch_page, commit_page) = if args.branch.is_some() { (whole, paged) } else { (paged, whole) };
  let reference_of = |name: &str| to_final_branch_name(&context.prefix, name).map_or_else(|_| String::new(), |name| format!("refs/heads/{name}"));
  let listed: Vec<&GroupedBranchInfo> = if args.unassigned {
    Vec::new()
  } else {
    let selected = grouping.branches.iter().filter(|branch| args.branch.as_ref().is_none_or(|name| &branch.name == name));
    page(selected, branch_page).collect()
  };
  let mut stats = if args.stats {
    let inputs: Vec<BranchInput> = listed
      .iter()
      .map(|branch| BranchInput {
        name: &branch.name,
        commits: &branch.commits,
        tip: tips.get(&reference_of(&branch.name)).map(String::as_str),
      })
      .collect();
    branch_stats(context, grouping.base.as_deref(), &inputs, args.include_generated, &short)?
  } else {
    HashMap::new()
  };
  let branches = (!args.unassigned).then(|| {
    listed
      .iter()
      .map(|branch| {
        let reference = reference_of(&branch.name);
        BranchStatus {
          tip: tips.get(&reference).map(|tip| short.get(tip).to_string()),
          name: branch.name.clone(),
          reference,
          parent: placement(&branch.name).and_then(|stack| stack.declared_parent.clone()),
          parent_state: placement(&branch.name).and_then(|stack| stack.parent_state).map(ParentStateName::from),
          effective_base: placement(&branch.name).and_then(|stack| stack.effective_base.clone()),
          blocked_by: placement(&branch.name)
            .and_then(|stack| stack.blocked_by.as_ref())
            .map(|block| StackBlockSummary::of(&block.parent, block.reason)),
          problems: placement(&branch.name).map_or_else(Vec::new, |stack| stack_issues(&branch.name, stack, &short)),
          description: descriptions
            .get(&reference_of(&branch.name))
            .map(|description| DescriptionTitle { title: description.title.clone() }),
          pull_request: None,
          issue_key: branch.issue_key.clone(),
          latest_time: branch.latest_commit_time,
          commit_count: branch.commits.len(),
          stats: stats.remove(&branch.name),
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
    forest: forest.map(|forest| forest.order.clone()),
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

/// `bd status --brief`: `bd status` cut down to one line of facts per branch
pub fn status_brief(context: &RepoContext, args: &StatusArgs) -> Result<StatusBrief, CliError> {
  let full = status(context, args)?;
  let branches = full.branches.map(|branches| {
    branches
      .into_iter()
      .map(|branch| BriefBranch {
        newest: branch.commits.into_iter().next().map_or_default(|commit| commit.subject),
        issue_key: branch.issue_key.filter(|key| *key != branch.name),
        name: branch.name,
        commit_count: branch.commit_count,
        parent: branch.parent,
        blocked_by: branch.blocked_by,
        problems: branch.problems,
      })
      .collect()
  });
  Ok(StatusBrief {
    schema: SchemaVersion,
    repository: full.repository,
    prefix: full.prefix,
    head: full.head,
    baseline: full.baseline,
    branch_count: full.branch_count,
    unassigned_count: full.unassigned_count,
    forest: full.forest,
    branches,
    unassigned: full.unassigned.unwrap_or_default(),
  })
}
