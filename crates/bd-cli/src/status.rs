use crate::branch_stats::{BranchInput, BranchStats, branch_stats};
use crate::context::{Environment, RepoContext};
use crate::error::{CliError, ErrorCode};
use crate::landed::{LandedBySummary, LandingTable, find_landed};
use crate::output::{ResolvedByWords, SchemaVersion, ShortIds};
use crate::robot_list::read_robots;
use crate::stack_summary::StackSummary;
use git_ops::branch_names::to_final_branch_name;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use sync_core::grouping::{BranchGrouping, group_commits_ahead_of_baseline_with};
use sync_core::regroup::{describe_problem, problem_commits};
use sync_core::stack_branch::read_stack_branches;
use sync_core::store_lines::StoreLineProblem;
use sync_types::stack::{BlockReason, BranchStack, ParentSource, ParentState};
use sync_types::{BranchOrigin, GroupedBranchInfo};

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
  /// The first item of the page, counted from 0
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
  /// With stacked branches on: the stack branch of each stack that forks (`{prefix}/stack/<root>`, all its commits in
  /// one branch), then those kept for their remote branch; as the last sync left them, by what the object cache knows
  /// of their builds; omitted with --branch or --unassigned
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stacks: Vec<StackSummary>,
  /// The keys of the issue groups (`refs/branch-deck/issue-groups`) that group no commit ahead of the baseline on this
  /// branch. The map is shared by every branch and worktree, so they may group the commits of another one: nothing
  /// removes them on its own
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_issue_groups: Vec<String>,
  /// The lines of the issue groups that are not used: not `KEY<TAB>name`, an invalid key or name, a key listed twice
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub issue_group_problems: Vec<StoreLine>,
  /// With stacked branches on, the children of the stack parents (`refs/branch-deck/stack-parents`) that stack no
  /// branch ahead of the baseline on this branch. The map is shared by every branch and worktree, so they may stack a
  /// branch of another one; a sync drops the line of a branch once it is integrated
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_stack_parents: Vec<String>,
  /// The lines of the stack parents that are not used: not `child<TAB>parent`, an invalid name, a branch that is its
  /// own parent, a child listed twice
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stack_parent_problems: Vec<StoreLine>,
}

/// A line of a store that is not used, and why
#[derive(Debug, Serialize, JsonSchema)]
pub struct StoreLine {
  pub line: String,
  pub message: String,
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
  /// The stack branches, as `bd status` lists them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stacks: Vec<StackSummary>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_issue_groups: Vec<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub issue_group_problems: Vec<StoreLine>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_stack_parents: Vec<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stack_parent_problems: Vec<StoreLine>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BriefBranch {
  pub name: String,
  pub commit_count: usize,
  /// The issue key of its subjects; omitted when it is `name`, an issue branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_key: Option<String>,
  /// The issue keys whose issue group it is
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub issue_keys: Vec<String>,
  /// The subject of its newest commit, without the `(prefix)`
  pub newest: String,
  /// The parent its commits declare, else its stack parent, with stacked branches on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// Where `parent` comes from
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent_source: Option<ParentSourceName>,
  /// The merged Safe Push that landed it, as `bd status` names it (`landed_by`), without the commits
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landed_by: Option<BriefLandedBy>,
  /// Why it is not built: its parent can't be built on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<StackBlockSummary>,
  /// The declared parent has no commits: the branch is built on the nearest active ancestor or the baseline
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub pr_base_changed: bool,
  /// Why the branch can't be placed in the stack
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<StackIssue>,
}

/// `landed_by` of `bd status --brief`
#[derive(Debug, Serialize, JsonSchema)]
pub struct BriefLandedBy {
  /// The branch whose Safe Push landed it, or the root of the whole stack (`whole_stack`)
  pub branch: String,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub whole_stack: bool,
  /// How many commits were added after the landed ones: the only work left; 0 when all of it landed
  pub after: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Head {
  pub id: String,
  /// The checked out branch; `null` when HEAD is detached
  pub branch: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchStatus {
  /// The group name: the `(prefix)` of the subjects, their issue group or their issue key
  pub name: String,
  /// What groups its commits: `prefix` when a commit has the `(prefix)`, else `issue_group` when the issue groups put
  /// commits of other keys into it, else `issue`
  pub origin: OriginName,
  /// The issue keys whose issue group it is (`refs/branch-deck/issue-groups`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub issue_keys: Vec<String>,
  /// The virtual branch, `refs/heads/{prefix}/virtual/{name}`
  #[serde(rename = "ref")]
  pub reference: String,
  /// The parent its commits declare (`(parent/name)`), else its line in the stack parents
  /// (`refs/branch-deck/stack-parents`), with stacked branches on
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent: Option<String>,
  /// Where `parent` comes from: `prefix` when a prefix declares it, `store` when only the stack parents do
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent_source: Option<ParentSourceName>,
  /// Whether the declared parent has commits, else how it stands; a parent without commits in any state is passed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parent_state: Option<ParentStateName>,
  /// The branch it is built on; omitted when it is built on the baseline
  #[serde(skip_serializing_if = "Option::is_none")]
  pub effective_base: Option<String>,
  /// Why it is not built: its parent can't be built (a problem, a conflict, a failure or a block)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blocked_by: Option<StackBlockSummary>,
  /// The declared parent has no commits (any `parent_state` but `active`): the branch is built on the nearest active
  /// ancestor (`effective_base`) or the baseline, and the base of its pull request is not the declared parent
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub pr_base_changed: bool,
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
  /// A merged Safe Push landed it, until the update archives it: its own, one of a branch built on it, or of the whole
  /// stack, decided by content (the commits the robot tested against the branch's). With an empty `after` all of it is
  /// in the target: `bd push` leaves it out (`skipped`) and `bd safe-push` refuses it (`landed`); else `after` is the
  /// only work left. Asked of Patronus on Space with a Space token; omitted otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landed_by: Option<LandedBySummary>,
  /// With --stats
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stats: Option<BranchStats>,
  /// Newest first; subjects without the `(prefix)`
  pub commits: Vec<CommitSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OriginName {
  Prefix,
  Issue,
  IssueGroup,
}

impl From<BranchOrigin> for OriginName {
  fn from(origin: BranchOrigin) -> Self {
    match origin {
      BranchOrigin::Prefix => Self::Prefix,
      BranchOrigin::Issue => Self::Issue,
      BranchOrigin::IssueGroup => Self::IssueGroup,
    }
  }
}

/// Where the parent of a branch comes from
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParentSourceName {
  /// A prefix of its commits declares it (`(parent/name)`)
  Prefix,
  /// Its line in the stack parents (`refs/branch-deck/stack-parents`)
  Store,
}

impl From<ParentSource> for ParentSourceName {
  fn from(source: ParentSource) -> Self {
    match source {
      ParentSource::Prefix => Self::Prefix,
      ParentSource::Store => Self::Store,
    }
  }
}

/// How the declared parent stands; every state but `active` is passed: the branch is built on the nearest active
/// ancestor or the baseline (`pr_base_changed`)
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParentStateName {
  /// The parent has commits
  Active,
  /// No commit is grouped into the parent and its archived branch is integrated
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
  /// `error` (a stack problem or a failed build), `conflict` or `blocked`
  pub reason: String,
}

impl StackBlockSummary {
  pub fn of(parent: &str, reason: BlockReason) -> Self {
    Self {
      parent: parent.to_string(),
      reason: block_reason_name(reason).to_string(),
    }
  }
}

/// `error`, `conflict` or `blocked`
pub const fn block_reason_name(reason: BlockReason) -> &'static str {
  match reason {
    BlockReason::Error => "error",
    BlockReason::Conflict => "conflict",
    BlockReason::Blocked => "blocked",
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
  /// The conflict of git that its virtual copy resolved by words, as its mapping note says; omitted otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resolved_by_words: Option<ResolvedByWords>,
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

/// Group the commits ahead of the baseline; a missing baseline is `no_baseline`, a `branch` that is not a group is
/// `unknown_branch`
pub fn read_grouping(context: &RepoContext, branch: Option<&str>) -> Result<BranchGrouping, CliError> {
  // the stack setting of the command's config
  let stacks = context.config()?.stacks()?;
  let grouping = group_commits_ahead_of_baseline_with(&context.git, &context.repository, &context.prefix, stacks)?;
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

/// Where the sync places `name` among `branches` (`GroupedBranchInfo::stack`); `None` with stacked branches off
pub(crate) fn placement<'a>(branches: &'a [GroupedBranchInfo], name: &str) -> Option<&'a BranchStack> {
  branches.iter().find(|branch| branch.name == name)?.stack.as_ref()
}

/// The branches `name` is built on, root first: the chain of `effective_base` as a sync places them (the grouped
/// branches of a `BranchGrouping` or a `SyncReport`); empty with stacked branches off or for a root branch
pub(crate) fn stack_ancestors(branches: &[GroupedBranchInfo], name: &str) -> Vec<String> {
  let parent_of = |name: &str| placement(branches, name).and_then(|placement| placement.effective_base.clone());
  let mut ancestors = Vec::new();
  let mut parent = parent_of(name);
  while let Some(ancestor) = parent {
    if ancestors.contains(&ancestor) || ancestor == name {
      break;
    }
    parent = parent_of(&ancestor);
    ancestors.push(ancestor);
  }
  ancestors.reverse();
  ancestors
}

/// `(offset, limit)` of a listing
type Page = (usize, Option<usize>);

fn page<T>(items: impl IntoIterator<Item = T>, (offset, limit): Page) -> impl Iterator<Item = T> {
  items.into_iter().skip(offset).take(limit.unwrap_or(usize::MAX))
}

pub fn status(context: &RepoContext, args: &StatusArgs, environment: &Environment) -> Result<StatusOutput, CliError> {
  let grouping = read_grouping(context, args.branch.as_deref())?;
  let (head_id, head_branch) = (&grouping.head, grouping.head_branch.clone());

  // the brief view prints neither
  let tips = if args.brief { HashMap::new() } else { virtual_tips(context, &grouping) };
  let all_stacks = read_stack_branches(&context.prefix, &grouping, &context.cache(), context.config()?.resolve_mode()?);
  // what merged Safe Pushes landed, until the update archives it, by content: the robots kept in the object cache while
  // they hold, else one list and one poll (`robot_list`)
  let landed = if args.unassigned || grouping.branches.is_empty() {
    HashMap::new()
  } else {
    let listed_tips: HashMap<String, String> = grouping.listed.iter().map(|branch| (branch.name.clone(), branch.tip.clone())).collect();
    let table = LandingTable {
      branches: &grouping.branches,
      stacks: &all_stacks,
      tips: &listed_tips,
      baseline: &grouping.baseline_branch,
    };
    find_landed(context, &read_robots(context, environment), &table)?
  };
  let stacks = if args.branch.is_some() || args.unassigned { Vec::new() } else { all_stacks };
  let descriptions = if args.brief { HashMap::new() } else { context.config()?.descriptions().clone() };
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
      .chain(stacks.iter().flat_map(StackSummary::ids))
      .chain(grouping.base.as_deref())
      .chain(std::iter::once(head_id.as_str())),
  );
  let summary = |commit: &git_ops::commit_list::Commit| CommitSummary {
    id: short.get(&commit.id).to_string(),
    subject: commit.stripped_subject.clone(),
    resolved_by_words: ResolvedByWords::of_commit(commit),
  };

  // --branch and --unassigned page the commits they list, else the branches are paged
  let paged = (args.offset, args.limit);
  let whole = (0, None);
  let (branch_page, commit_page) = if args.branch.is_some() { (whole, paged) } else { (paged, whole) };
  let reference_of = |name: &str| to_final_branch_name(&context.prefix, name).map_or_else(|_| String::new(), |name| format!("refs/heads/{name}"));
  let issue_groups = &grouping.grouping.issue_groups;
  let issue_keys_of = |name: &str| -> Vec<String> {
    issue_groups
      .entries
      .iter()
      .filter(|(key, group)| *group == name && !grouping.idle_issue_groups.contains(key))
      .map(|(key, _)| key.clone())
      .collect()
  };
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
          origin: branch.origin.into(),
          issue_keys: issue_keys_of(&branch.name),
          reference,
          parent: placement(&branch.name).and_then(|stack| stack.declared_parent.clone()),
          parent_source: placement(&branch.name).and_then(|stack| stack.parent_source).map(ParentSourceName::from),
          parent_state: placement(&branch.name).and_then(|stack| stack.parent_state).map(ParentStateName::from),
          effective_base: placement(&branch.name).and_then(|stack| stack.effective_base.clone()),
          blocked_by: placement(&branch.name)
            .and_then(|stack| stack.blocked_by.as_ref())
            .map(|block| StackBlockSummary::of(&block.parent, block.reason)),
          pr_base_changed: placement(&branch.name).is_some_and(|stack| stack.pr_base_changed),
          problems: placement(&branch.name).map_or_else(Vec::new, |stack| stack_issues(&branch.name, stack, &short)),
          description: descriptions
            .get(&reference_of(&branch.name))
            .map(|description| DescriptionTitle { title: description.title.clone() }),
          issue_key: branch.issue_key.clone(),
          latest_time: branch.latest_commit_time,
          commit_count: branch.commits.len(),
          stats: stats.remove(&branch.name),
          landed_by: landed.get(&branch.name).map(|landed| LandedBySummary::new(context, landed, &short)),
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
      id: short.get(head_id).to_string(),
      branch: head_branch,
    },
    baseline: grouping.baseline_branch.clone(),
    branch_count: grouping.branches.len(),
    unassigned_count: grouping.unassigned.len(),
    forest: forest.map(|forest| forest.order.clone()),
    branches,
    unassigned,
    stacks: stacks
      .iter()
      .map(|stack| StackSummary::new(stack, grouping.branches.iter().flat_map(|branch| &branch.commits), &short))
      .collect(),
    idle_issue_groups: grouping.idle_issue_groups.clone(),
    issue_group_problems: store_lines(&issue_groups.problems),
    idle_stack_parents: grouping.idle_stack_parents.clone(),
    stack_parent_problems: if grouping.grouping.stacks {
      store_lines(&grouping.grouping.stack_parents.problems)
    } else {
      Vec::new()
    },
  })
}

fn store_lines(problems: &[StoreLineProblem]) -> Vec<StoreLine> {
  problems
    .iter()
    .map(|problem| StoreLine {
      line: problem.line.clone(),
      message: problem.message.clone(),
    })
    .collect()
}

/// Full ref -> commit id of the existing virtual branches, from the branches the grouping listed
fn virtual_tips(context: &RepoContext, grouping: &BranchGrouping) -> HashMap<String, String> {
  let namespace = format!("{}/virtual/", context.prefix.trim_end_matches('/'));
  grouping
    .listed
    .iter()
    .filter(|branch| branch.name.starts_with(&namespace))
    .map(|branch| (format!("refs/heads/{}", branch.name), branch.tip.clone()))
    .collect()
}

/// `bd status --brief`: `bd status` cut down to one line of facts per branch
pub fn status_brief(context: &RepoContext, args: &StatusArgs, environment: &Environment) -> Result<StatusBrief, CliError> {
  let full = status(context, args, environment)?;
  let branches = full.branches.map(|branches| {
    branches
      .into_iter()
      .map(|branch| BriefBranch {
        newest: branch.commits.into_iter().next().map_or_default(|commit| commit.subject),
        issue_key: branch.issue_key.filter(|key| *key != branch.name),
        issue_keys: branch.issue_keys,
        name: branch.name,
        commit_count: branch.commit_count,
        parent: branch.parent,
        parent_source: branch.parent_source,
        landed_by: branch.landed_by.map(|landed| BriefLandedBy {
          branch: landed.branch,
          whole_stack: landed.whole_stack,
          after: landed.after.len(),
        }),
        blocked_by: branch.blocked_by,
        pr_base_changed: branch.pr_base_changed,
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
    stacks: full.stacks,
    idle_issue_groups: full.idle_issue_groups,
    issue_group_problems: full.issue_group_problems,
    idle_stack_parents: full.idle_stack_parents,
    stack_parent_problems: full.stack_parent_problems,
  })
}
