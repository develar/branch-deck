use crate::branch_stats::{BranchInput, BranchStats, branch_stats};
use crate::landed::{LandedBySummary, landed_branches};
use crate::patronus_facts::PatronusFacts;
use crate::robot_list::read_robots;
use bd_core::branch_view::{ParentSourceName, StackBlockSummary, StackIssue, read_grouping, stack_issues};
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::next_step_summary::NextStepSummary;
use bd_core::output::{ResolvedByWords, SchemaVersion, ShortIds};
use bd_core::stack_summary::StackSummary;
use domain_policy::facts::{Repository, Robots};
use domain_policy::landed::LandedFacts;
use domain_policy::next_step::PatronusStanding;
use domain_policy::parked::parked_state;
use domain_types::branch_names::{BranchKind, baseline_on_remote, strip_branch_namespace, to_final_branch_name};
use domain_types::landing::{LandingFix, LandingRefusal, Subject};
use domain_types::next_step::StepKind;
use domain_types::repository_view::LandingNeeds;
use domain_types::session::{Fact, FactState, LandedSection, PullRequestsView, RobotsHead, RobotsView, stack_readiness_key, stack_root_of_readiness_key};
use domain_types::settings::PrefixSource;
use domain_types::stack::{ParentState, StackBranchUpdate};
use domain_types::{BranchOrigin, GroupedBranchInfo};
use git_ops::commit_utils::get_commits_info;
use history_rewrite::store_lines::StoreLineProblem;
use repository_model::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use repository_model::derive::{StepSources, grouped_rows, landed_states, next_steps};
use repository_model::patronus_view::{landed_commits, with_landing_needs};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use sync_core::grouping::BranchGrouping;
use sync_core::stack_branch::read_stack_branches;
use tracing::debug;
use trial_check::landing_gate::landing_needs_of;

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
  /// Where `prefix` came from: `cli` (`--prefix`), `repository_config` (this repository's `branchdeck.branchPrefix`),
  /// `app_setting` (General → Branch prefix of the app), `global_config` (the global or system `branchdeck.branchPrefix`)
  pub prefix_source: PrefixSource,
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
  /// The parked branches (`refs/branch-deck/parked`) that group no commit ahead of the baseline on this branch. The map is
  /// shared by every branch and worktree, so they may park a branch of another one; a sync drops the line of a branch
  /// once it is integrated, `bd unpark` drops any
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_parked: Vec<String>,
  /// The branches `bd unapply` kept (`{prefix}/unapplied/<name>`), newest first: their commits are not in HEAD, they land
  /// nothing; `bd push --unapplied <name>` keeps a copy on the remote
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub unapplied: Vec<UnappliedSummary>,
}

/// A branch an unapply kept, as the listing of the branches under the prefix has it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UnappliedSummary {
  /// `{prefix}/unapplied/<name>`
  pub name: String,
  /// `<name>`, the branch it was (a `-N` suffix when that name was taken); `bd push --unapplied` takes it
  pub branch: String,
  pub tip: String,
  /// The subject of its tip
  pub subject: String,
  /// Its remote branch, when it was pushed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remote: Option<UnappliedRemote>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UnappliedRemote {
  /// The tip of the remote branch
  pub head: String,
  /// The remote branch is at the tip
  pub up_to_date: bool,
}

/// The unapplied branches of the listing, with the tips of their remote branches and the subjects of their tips: one
/// `log --no-walk` when there are any (the listing reads no commit)
fn unapplied_summaries(context: &RepoContext, grouping: &BranchGrouping) -> Result<Vec<UnappliedSummary>, CliError> {
  let listed: Vec<_> = grouping
    .listed
    .iter()
    .filter_map(|listed| Some((listed, strip_branch_namespace(&listed.name, &context.prefix, BranchKind::Unapplied)?)))
    .collect();
  let tips: Vec<&str> = listed.iter().map(|(listed, _)| listed.tip.as_str()).collect();
  let commits = get_commits_info(&context.git, &context.repository, &tips)?;
  let short = context.short_ids(
    listed
      .iter()
      .flat_map(|(listed, _)| std::iter::once(listed.tip.as_str()).chain(grouping.remote_tips.get(&listed.name).map(String::as_str))),
  );
  Ok(
    listed
      .into_iter()
      .map(|(listed, branch)| UnappliedSummary {
        name: listed.name.clone(),
        branch: branch.to_string(),
        tip: short.get(&listed.tip).to_string(),
        subject: commits.get(&listed.tip).map_or_default(|commit| commit.subject.clone()),
        remote: grouping.remote_tips.get(&listed.name).map(|head| UnappliedRemote {
          head: short.get(head).to_string(),
          up_to_date: *head == listed.tip,
        }),
      })
      .collect(),
  )
}

/// Why a branch is parked: kept local, `bd push` and `bd safe-push` refuse it until the user unparks it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ParkedSummary {
  /// Why, as the user wrote it; empty without a note
  pub note: String,
  /// The parked branch it is built on, whose commits it carries; omitted when its own line parks it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub with: Option<String>,
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
  /// As in `bd status`
  pub prefix_source: PrefixSource,
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
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_parked: Vec<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub unapplied: Vec<UnappliedSummary>,
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
  /// Kept local: never push it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parked: Option<ParkedSummary>,
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

/// A commit outside what the Safe Push of a branch lands that its build needs (`landing_needs`)
#[derive(Debug, Serialize, JsonSchema)]
pub struct LandingNeed {
  /// `missing_declaration`: a commit of what lands uses a name that only this commit adds; `dangling_reference`: a
  /// commit of what lands removes or changes a declaration a use still has, which only this commit changes
  pub code: &'static str,
  pub identifier: String,
  /// Where the use is: `src/b/Use.kt:3`
  pub at: String,
  /// The commit the build needs
  pub needs: String,
  /// Its branch; omitted for an unassigned commit
  #[serde(skip_serializing_if = "Option::is_none")]
  pub needs_branch: Option<String>,
  /// What lands it with the branch: `bd restack <root> <branch>` (ask the user first), else `move <commit> into <branch>`
  pub fix: String,
  /// The robot id of the running Safe Push that lands `needs_branch`: it tests that branch without this one, so a start
  /// beside it fails all the same
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landing_in: Option<String>,
}

impl LandingNeed {
  fn new(context: &RepoContext, refusal: &LandingRefusal, short: &ShortIds) -> Self {
    let (code, identifier, file, line, fix) = match refusal {
      LandingRefusal::MissingDeclaration { identifier, file, line, fix, .. } => ("missing_declaration", identifier, file, line, fix),
      LandingRefusal::DanglingReference { identifier, file, line, fix, .. } => ("dangling_reference", identifier, file, line, fix),
    };
    let needs = refusal.needed();
    Self {
      code,
      identifier: identifier.clone(),
      at: format!("{file}:{line}"),
      needs: short.get(&needs.id).to_string(),
      needs_branch: (!needs.branch.is_empty()).then(|| needs.branch.clone()),
      fix: match fix {
        LandingFix::Restack { branch, onto } => format!("{} restack {branch} {onto}", context.bd),
        LandingFix::MoveCommit { commit, into } => format!("move {} into {into}", short.get(&commit.id)),
      },
      landing_in: match refusal {
        LandingRefusal::MissingDeclaration { landing_in, .. } => landing_in.as_ref().map(|robot| robot.robot_id.clone()),
        LandingRefusal::DanglingReference { .. } => None,
      },
    }
  }
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
  /// What its Safe Push needs of a commit outside what it lands (the branch and those it is built on): the refusals
  /// `bd safe-push` would give its start, from one trial check of every branch. Empty: its build has what it uses. On
  /// Space only, where a Safe Push lands it; omitted otherwise, with --brief, and when the check can't run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landing_needs: Option<Vec<LandingNeed>>,
  /// The user parked it (`bd park`), or a branch it is built on: kept local, `bd push` and `bd safe-push` refuse it.
  /// Never push or unpark it on your own
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parked: Option<ParkedSummary>,
  /// What the app's row offers next (`domain_policy::next_step`, spec/branch/next-step.spec.md): the row of the table, the
  /// step, the fact it waits for, its pull request, why a Safe Push or a Dry Run can't start. `bd status` reads no pull
  /// request, readiness or why a robot failed: a step that needs them waits for them (row 0); `bd safe-push --check`
  /// reads them. Omitted off Space (the row has no slot) and with --unassigned
  #[serde(skip_serializing_if = "Option::is_none")]
  pub next_step: Option<NextStepSummary>,
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

/// The table of a grouping without a sync (`bd status`, `bd robot`): the tips the refs hold, the remote tips the grouping
/// read, no remote status
pub(crate) fn grouping_table(context: &RepoContext, grouping: &BranchGrouping, stacks: &[StackBranchUpdate]) -> BranchTable {
  BranchTable {
    prefix: context.prefix.clone(),
    baseline: grouping.baseline_branch.clone(),
    branches: grouping.branches.clone(),
    tips: grouping.listed.iter().map(|branch| (branch.name.clone(), branch.tip.clone())).collect(),
    // a grouping without a sync: the tips are what the refs hold
    not_built: HashMap::new(),
    copies: HashMap::new(),
    stack_branches: stacks.to_vec(),
    parked: grouping.grouping.parked.entries.clone(),
    // no sync: the remote tips the grouping read, no remote status
    remote: RemoteFacts {
      branches: grouping
        .branches
        .iter()
        .map(|branch| {
          let full_name = to_final_branch_name(&context.prefix, &branch.name).unwrap_or_default();
          let fact = RemoteFact {
            remote_tip: grouping.remote_tips.get(&full_name).cloned(),
            ..RemoteFact::default()
          };
          (branch.name.clone(), fact)
        })
        .collect(),
      stacks: BTreeMap::new(),
    },
  }
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
  let (landed, landing_needs, mut steps) = if args.unassigned || grouping.branches.is_empty() {
    (HashMap::new(), None, HashMap::new())
  } else {
    let table = grouping_table(context, &grouping, &all_stacks);
    let read = read_robots(context, environment);
    let mut facts = PatronusFacts::read(context, &table, read.robots);
    // what the Safe Push of each branch needs, where a Safe Push lands it: one trial session of every branch
    let needs = if args.brief || context.space_repository()?.is_none() {
      None
    } else {
      landing_needs_of(&context.git, &context.cache(), &context.repository, &context.prefix, &landed_commits(&facts.view))
        .inspect_err(|error| debug!(error = %format!("{error:#}"), "no landing needs"))
        .ok()
    };
    let landing_needs = needs.map(|needs| {
      with_landing_needs(&mut facts.view, &table, &needs);
      facts
        .view
        .branches
        .iter()
        .filter_map(|(name, branch)| match &branch.landing_needs {
          LandingNeeds::Known { refusals } => Some((name.clone(), refusals.clone())),
          LandingNeeds::Unknown => None,
        })
        .collect::<HashMap<String, Vec<LandingRefusal>>>()
    });
    let steps = if args.brief {
      HashMap::new()
    } else {
      next_steps_of_status(&grouping, &table, &facts, read.head, landing_needs.is_some())
    };
    (landed_branches(&facts), landing_needs, steps)
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
  let summary = |commit: &domain_types::commit::Commit| CommitSummary {
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
  let repo = Repository::of_grouping(&grouping.branches, &grouping.grouping.parked.entries);
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
          landing_needs: landing_needs.as_ref().map(|needs| {
            needs
              .get(&branch.name)
              .map_or_default(|refusals| refusals.iter().map(|refusal| LandingNeed::new(context, refusal, &short)).collect())
          }),
          parked: parked_state(&repo, &branch.name).map(|parked| ParkedSummary {
            note: parked.note,
            with: parked.with,
          }),
          next_step: steps.remove(&branch.name),
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
    prefix_source: context.prefix_source,
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
      .map(|stack| StackSummary {
        next_step: steps.remove(&stack_readiness_key(&stack.root)),
        ..StackSummary::new(stack, grouping.branches.iter().flat_map(|branch| &branch.commits), &short)
      })
      .collect(),
    idle_issue_groups: grouping.idle_issue_groups.clone(),
    issue_group_problems: store_lines(&issue_groups.problems),
    idle_stack_parents: grouping.idle_stack_parents.clone(),
    stack_parent_problems: if grouping.grouping.stacks {
      store_lines(&grouping.grouping.stack_parents.problems)
    } else {
      Vec::new()
    },
    idle_parked: grouping
      .grouping
      .parked
      .entries
      .keys()
      .filter(|name| !grouping.branches.iter().any(|branch| &branch.name == *name))
      .cloned()
      .collect(),
    unapplied: unapplied_summaries(context, &grouping)?,
  })
}

/// The next step of every branch and whole stack of `table`, as the app's rows show it, over what `bd status` read:
/// the robots and how Patronus answered (`head`), the Patronus view and its landing needs (`needs_known`: the check ran);
/// no pull request, readiness or why a robot failed, which the steps that need them wait for
fn next_steps_of_status(grouping: &BranchGrouping, table: &BranchTable, facts: &PatronusFacts, head: Fact<RobotsHead>, needs_known: bool) -> HashMap<String, NextStepSummary> {
  let rows = grouped_rows(table);
  let robots = RobotsView {
    head,
    robots: facts.robots.iter().map(|robot| (robot.id.clone(), robot.clone())).collect(),
    ..RobotsView::default()
  };
  let view_state = FactState::Fresh;
  let patronus = PatronusStanding::of(&robots.head, &view_state);
  let landing_needs = if needs_known { FactState::Fresh } else { FactState::Unknown };
  let pull_requests = PullRequestsView {
    head: Fact {
      state: FactState::Unknown,
      ..Fact::default()
    },
    ..PullRequestsView::default()
  };
  let robots_of = Robots::new(&facts.robots);
  let baseline = LandedSection::default();
  let landed = LandedFacts {
    baseline: &baseline,
    view: Some(&facts.view),
    robots: &robots_of,
  };
  let landed = landed_states(table, &table.remote, &landed, &pull_requests);
  let readiness = |_: &str| None;
  let sources = StepSources {
    table,
    remote: &table.remote,
    rows: &rows,
    view: &facts.view,
    robots: &robots,
    patronus: &patronus,
    landing_needs: &landing_needs,
    pull_requests: &pull_requests,
    open_pull_requests: &BTreeMap::new(),
    landed: &landed,
    readiness: &readiness,
    target: baseline_on_remote(&grouping.baseline_branch),
  };
  next_steps(&sources)
    .into_iter()
    // off Space a row has no slot: nothing to say
    .filter(|(_, view)| view.step.kind != StepKind::Hidden)
    .map(|(key, view)| {
      let what = stack_root_of_readiness_key(&key).map_or_else(|| key.clone(), |root| Subject::whole_stack(root).words());
      (key, NextStepSummary::new(&view.step, &what, &facts.robots))
    })
    .collect()
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
        parked: branch.parked,
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
    prefix_source: full.prefix_source,
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
    idle_parked: full.idle_parked,
    unapplied: full.unapplied,
  })
}
