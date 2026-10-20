//! `bd pick` and `bd undo pick:…`: the commits of another worktree landed on the branch checked out here, without
//! `git cherry-pick` (spec/cli/pick.spec.md). The core (`sync_core::pick`) takes the repository lock, which every
//! worktree shares, replays the commits in memory, and moves the branch (and, with --follow, the source worktree)
//! through the guarded working-tree update; a conflict or local changes in the way change nothing

use crate::update_command::{core_error, uncommitted_overlap};
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CONFLICT_EXIT, CliError, ErrorCode};
use bd_core::output::{SchemaVersion, ShortIds};
use bd_core::suggest_plan::SuggestPlan;
use git_ops::pick::{FollowState, PickRefusal, PickState, PresentBy};
use history_rewrite::repository_lock::RewriteOptions;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use sync_core::pick::{DependencyFix, GroupedBy, PickDependency, PickFrom, PickParams, PickResult, PickUndoToken, UndonePick, pick_core, undo_pick_core};

#[derive(Debug, Default, clap::Args)]
pub struct PickArgs {
  /// A directory of the worktree whose commits land here: those its HEAD has that this branch lacks
  #[arg(long, value_name = "DIR", conflicts_with = "revisions", required_unless_present = "revisions")]
  pub from: Option<PathBuf>,
  /// One commit, `<rev>`, or a range, `<a>..<b>`, instead of --from
  #[arg(value_name = "REVISIONS")]
  pub revisions: Option<String>,
  /// Move the worktree of --from onto the new tip too, its files following; its local changes on other paths stay
  #[arg(long, requires = "from")]
  pub follow: bool,
  /// Pick only if HEAD is still this commit
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Print what the pick would do; nothing is written but git objects
  #[arg(long)]
  pub dry_run: bool,
}

/// `bd pick`
#[derive(Debug, Serialize, JsonSchema)]
pub struct PickOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// The checked out branch the commits land on
  pub branch: String,
  /// `picked` (with --dry-run: the commits would be picked), `up_to_date` (the branch has every commit already), or
  /// `conflict` (exit 1: a commit does not apply; nothing was changed)
  pub state: PickOutcome,
  pub dry_run: bool,
  /// The worktree of --from, its top level; omitted for revisions
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source_worktree: Option<String>,
  /// The commit the range starts after: the fork point of the worktree from the branch, or `<a>` of `<a>..<b>`
  pub base: String,
  /// HEAD before the pick
  pub previous_head: String,
  /// HEAD afterwards (with --dry-run: would be); `previous_head` when nothing moves
  pub new_head: String,
  /// The commits replayed onto the branch, oldest first
  pub picked: Vec<PickedOutput>,
  /// The commits whose change the branch has already: left out, never an empty commit; with `conflict` only those left
  /// out by their patch id
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub already_present: Vec<PresentOutput>,
  /// Picked commits whose prefix names a branch that is not built on the branches they need (their hunks change what
  /// commits of those branches wrote): that branch can't sync. Also with --dry-run, to fix the prefix before the pick
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dependencies: Vec<DependencyOutput>,
  /// Branch -> its parent now: a `(parent/leaf)` prefix of a picked commit stacked a branch the branch had already, a
  /// root or on another parent before
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub reparented: BTreeMap<String, String>,
  /// With --follow: how the worktree of --from followed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub followed: Option<FollowedOutput>,
  /// With `conflict`: the first commit that does not apply, and what to do
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<PickConflictOutput>,
  /// `pick:<previous_head>..<new_head>` with full ids, `+follow:<worktree>:<previous>..<new>` when the worktree moved:
  /// `bd undo` moves both back; omitted when nothing moved
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
}

impl PickOutput {
  /// 1 for a conflict, else 0
  pub const fn exit_code(&self) -> i32 {
    if self.conflict.is_some() { CONFLICT_EXIT } else { 0 }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PickOutcome {
  Picked,
  UpToDate,
  Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FollowedState {
  Moved,
  AtTip,
  WouldMove,
  HeadMoved,
  Blocked,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PickedOutput {
  /// The commit in the source
  pub commit: String,
  /// Its copy on the branch
  pub new_id: String,
  pub subject: String,
  /// The branch Branch Deck groups the copy into; `null` for an unassigned commit
  pub group: Option<String>,
  /// What decides `group`
  pub grouped_by: GroupedByOutput,
}

/// What groups a picked commit on the branch
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupedByOutput {
  /// Its `(name)` prefix
  Prefix,
  /// The issue group of its issue key
  IssueGroup,
  /// Its issue key
  Issue,
  /// Nothing: a subject without a prefix or an issue key. Write the `(branch)` prefix when you commit
  None,
}

impl From<GroupedBy> for GroupedByOutput {
  fn from(grouped_by: GroupedBy) -> Self {
    match grouped_by {
      GroupedBy::Prefix => Self::Prefix,
      GroupedBy::IssueGroup => Self::IssueGroup,
      GroupedBy::Issue => Self::Issue,
      GroupedBy::None => Self::None,
    }
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PresentOutput {
  pub commit: String,
  pub subject: String,
  /// The branch's commit with the same patch; `null` when the branch has the change in another form
  pub on: Option<String>,
  /// `patch_id`: a commit of the branch has its patch id, so it was left out before the replay; `empty`: the replay
  /// dropped it as empty, the branch has the change in another form
  pub by: PresentByOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PresentByOutput {
  PatchId,
  Empty,
}

impl From<PresentBy> for PresentByOutput {
  fn from(by: PresentBy) -> Self {
    match by {
      PresentBy::PatchId => Self::PatchId,
      PresentBy::Empty => Self::Empty,
    }
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DependencyOutput {
  /// The copy on the branch (with --dry-run: the one the pick writes)
  pub commit: String,
  pub subject: String,
  /// The branch its prefix names
  pub prefix_branch: String,
  /// The branches it needs that `prefix_branch` is not built on
  pub depends_on: Vec<String>,
  /// The plan for `bd group --plan`: the commit moves into the branch of the stack that holds all of them. The plans
  /// of several commits go into one `bd group`, their `branches` merged: a rewrite gives the commits new ids
  #[serde(skip_serializing_if = "Option::is_none")]
  pub plan: Option<SuggestPlan>,
  /// The commit needs its own branch too: `bd restack <prefix_branch> <parent>` stacks the branch on one that holds
  /// all of them and its parent
  #[serde(skip_serializing_if = "Option::is_none")]
  pub restack: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FollowedOutput {
  /// The worktree, its top level
  pub worktree: String,
  pub previous_head: String,
  pub new_head: String,
  /// `moved`, `at_tip` (it was at the new tip already), `would_move` (--dry-run), or, after the branch moved:
  /// `head_moved` (its HEAD moved or switched since the check) or `blocked` (its files could not follow). The same
  /// `bd pick` again moves only the worktree then
  pub state: FollowedState,
  /// Why it did not move
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PickConflictOutput {
  pub commit: String,
  pub subject: String,
  /// The commits before it, which apply
  pub clean: Vec<String>,
  /// The files it conflicts on
  pub files: Vec<String>,
  /// How many files `files` leaves out
  #[serde(default, skip_serializing_if = "bd_core::sync_command::is_zero")]
  pub files_omitted: u32,
  /// What to do: the conflict is resolved in the source, on top of the branch
  pub next: String,
}

pub fn pick(context: &RepoContext, args: &PickArgs, environment: &Environment) -> Result<PickOutput, CliError> {
  let from = match (&args.from, &args.revisions) {
    (Some(dir), _) => PickFrom::Worktree(environment.cwd.join(dir).to_string_lossy().into_owned()),
    (None, Some(revisions)) => PickFrom::Revisions(revisions.clone()),
    (None, None) => return Err(CliError::new(ErrorCode::InvalidArguments, "Name the commits: --from <worktree dir> or <rev>, <a>..<b>")),
  };
  let params = PickParams {
    repository_path: context.repository.clone(),
    from,
    follow: args.follow,
    dry_run: args.dry_run,
  };
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_timeout: context.lock_timeout,
    held_lock: None,
  };
  let result = pick_core(&context.git, &params, &options, &context.cache(), context.file_diffs()).map_err(pick_error)?;
  output(context, &result, args.dry_run)
}

/// The code of a failed pick: a refusal of `git_ops::pick`, else as for an update (`core_error`)
fn pick_error(error: anyhow::Error) -> CliError {
  let code = match error.downcast_ref::<PickRefusal>() {
    Some(PickRefusal::InvalidSource(_)) => ErrorCode::InvalidArguments,
    Some(PickRefusal::RangeUnknown(_)) => ErrorCode::RangeUnknown,
    Some(PickRefusal::SourceOnNewerBaseline(_)) => ErrorCode::SourceOnNewerBaseline,
    None => return core_error(error, ErrorCode::HeadMoved),
  };
  CliError::new(code, format!("{error:#}"))
}

fn output(context: &RepoContext, result: &PickResult, dry_run: bool) -> Result<PickOutput, CliError> {
  let check = &result.check;
  if let PickState::Blocked { worktree, paths, path_count } = &check.state {
    let mut error = uncommitted_overlap(paths.clone(), *path_count as usize, "pick");
    error.message = format!("{worktree}: {}", error.message);
    return Err(error);
  }
  let applied = result.applied.as_ref();
  let new_head = applied.map_or(check.new_head.as_str(), |applied| applied.new_head.as_str());
  let mut ids = vec![check.head.as_str(), new_head, check.base.as_str()];
  ids.extend(check.picked.iter().flat_map(|commit| [commit.commit.as_str(), commit.new_id.as_str()]));
  ids.extend(check.present.iter().flat_map(|commit| std::iter::once(commit.commit.as_str()).chain(commit.on.as_deref())));
  ids.extend(check.follow.iter().map(|follow| follow.head.as_str()));
  ids.extend(result.dependencies.iter().map(|dependency| dependency.commit.as_str()));
  if let PickState::Conflict(conflict) = &check.state {
    ids.extend(std::iter::once(conflict.commit.id.as_str()).chain(conflict.clean.iter().map(|commit| commit.id.as_str())));
  }
  let short = context.short_ids(ids);
  let state = match &check.state {
    PickState::Conflict(_) => PickOutcome::Conflict,
    _ if check.picked.is_empty() => PickOutcome::UpToDate,
    _ => PickOutcome::Picked,
  };
  let followed = check.follow.as_ref().map(|follow| {
    let (state, message) = match applied.and_then(|applied| applied.followed.as_ref()) {
      _ if follow.head == check.new_head => (FollowedState::AtTip, None),
      Some(FollowState::Moved) => (FollowedState::Moved, None),
      Some(FollowState::HeadMoved(message)) => (FollowedState::HeadMoved, Some(message.clone())),
      Some(FollowState::Blocked(message)) => (FollowedState::Blocked, Some(message.clone())),
      None => (FollowedState::WouldMove, None),
    };
    FollowedOutput {
      worktree: follow.worktree.clone(),
      previous_head: short.get(&follow.head).to_string(),
      new_head: short.get(new_head).to_string(),
      state,
      message,
    }
  });
  let source_worktree = check.follow.as_ref().map(|follow| follow.worktree.clone());
  let conflict = match &check.state {
    PickState::Conflict(conflict) => Some(PickConflictOutput {
      commit: short.get(&conflict.commit.id).to_string(),
      subject: conflict.commit.subject.clone(),
      clean: conflict.clean.iter().map(|commit| short.get(&commit.id).to_string()).collect(),
      files: conflict.files.clone(),
      files_omitted: conflict.file_count.saturating_sub(u32::try_from(conflict.files.len()).unwrap_or(u32::MAX)),
      next: next_step(context, result, &short),
    }),
    _ => None,
  };
  Ok(PickOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: check.branch.clone(),
    state,
    dry_run,
    source_worktree,
    base: short.get(&check.base).to_string(),
    previous_head: short.get(&check.head).to_string(),
    new_head: short.get(new_head).to_string(),
    picked: check
      .picked
      .iter()
      .zip(&result.groups)
      .map(|(commit, group)| PickedOutput {
        commit: short.get(&commit.commit).to_string(),
        new_id: short.get(&commit.new_id).to_string(),
        subject: commit.subject.clone(),
        group: group.group.clone(),
        grouped_by: group.grouped_by.into(),
      })
      .collect(),
    already_present: check
      .present
      .iter()
      .map(|commit| PresentOutput {
        commit: short.get(&commit.commit).to_string(),
        subject: commit.subject.clone(),
        on: commit.on.as_deref().map(|on| short.get(on).to_string()),
        by: commit.by.into(),
      })
      .collect(),
    dependencies: result.dependencies.iter().map(|dependency| dependency_output(context, dependency, &short)).collect(),
    reparented: result.reparented.clone(),
    followed,
    conflict,
    undo_token: result.undo_token().map(|token| token.to_string()),
  })
}

fn dependency_output(context: &RepoContext, dependency: &PickDependency, short: &ShortIds) -> DependencyOutput {
  let commit = short.get(&dependency.commit).to_string();
  let (plan, restack) = match &dependency.fix {
    Some(DependencyFix::Move(branch)) => (
      Some(SuggestPlan {
        branches: BTreeMap::from([(branch.clone(), vec![commit.clone()])]),
        ..SuggestPlan::default()
      }),
      None,
    ),
    Some(DependencyFix::Restack(parent)) => (None, Some(format!("{} restack {} {parent}", context.bd, dependency.prefix_branch))),
    None => (None, None),
  };
  DependencyOutput {
    commit,
    subject: dependency.subject.clone(),
    prefix_branch: dependency.prefix_branch.clone(),
    depends_on: dependency.depends_on.clone(),
    plan,
    restack,
  }
}

/// What an agent does with a conflict: the source is moved onto the branch and the commit made again there, never with
/// a `git cherry-pick` in the shared checkout
fn next_step(context: &RepoContext, result: &PickResult, short: &ShortIds) -> String {
  let check = &result.check;
  let onto = short.get(&check.head);
  let base = short.get(&check.base);
  let resolve = "resolve it there: regenerate a generated file, never keep one side of a code hunk";
  let no_cherry_pick = "Never `git cherry-pick` here: it leaves CHERRY_PICK_HEAD, which blocks every bd rewrite";
  match &check.follow {
    Some(follow) => format!(
      "Move the worktree onto {onto} (`git -C {worktree} rebase --onto {onto} {base}`), {resolve}, then run `{bd} pick --from {worktree} --follow` \
       again. {no_cherry_pick}",
      worktree = follow.worktree,
      bd = context.bd,
    ),
    None => format!("Rebase the commits onto {onto} where they were made, {resolve}, then pick them again. {no_cherry_pick}"),
  }
}

/// `bd undo pick:<previous head>..<new head>[+follow:<worktree>:<previous>..<new>]`; `None` for a token of another kind
pub fn undo_pick(context: &RepoContext, token: &str) -> Option<Result<(PickUndoToken, UndonePick), CliError>> {
  if !PickUndoToken::is_pick_token(token) {
    return None;
  }
  Some(undo_pick_token(context, token))
}

fn undo_pick_token(context: &RepoContext, text: &str) -> Result<(PickUndoToken, UndonePick), CliError> {
  let token = PickUndoToken::parse(text).ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("{text:?} is not a pick token: pick:<previous head>..<new head>, with the full ids bd pick printed"),
    )
  })?;
  let undone = undo_pick_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(|e| core_error(e, ErrorCode::UndoStale))?;
  Ok((token, undone))
}
