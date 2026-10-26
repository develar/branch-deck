//! The words of the refusals the rules give (`domain_types::reason::Reason`), as `bd` and the engine of the operations
//! say them in their errors: why a start, a restart, a push or a review request is not possible ("A Safe Push of the last commit is
//! running"), and why the stack branch of a whole stack is no build of its commits. The page words the same codes
//! itself (`src/presenters/reasons.ts`).

use crate::push_guard::{landed_words, own_hold_words, parked_words, span_hold_words};
use domain_types::GroupedBranchInfo;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use domain_types::reason::Reason;
use domain_types::repository_view::HoldKind;
use domain_types::session::Parked;
use domain_types::stack::{BlockReason, StackBranchUpdate};
use sync_core::patronus_start::{carried_ids, carried_words, join_names, push_mode_name};
use sync_core::push::not_synced_refusal;

/// Why a start of `subject` ("auth", "the whole auth stack") is not possible, in the words of `bd` and of the guards
/// of the app, for the robot part of the decision (`domain_policy::land::start_refusal`): the commits a descendant lands
/// named by their ids, the robot of a hold by its URL (`robots`)
pub fn start_refusal_text(refusal: &Reason, subject: &str, robots: &[PatronusRobot]) -> String {
  match refusal {
    Reason::LastCommit { mode, .. } => format!("A {} of the last commit is running", push_mode_name(*mode)),
    Reason::Descendant { by, mode, carried, .. } => format!(
      "A {} of {} is running: it lands {} with it{}",
      push_mode_name(*mode),
      by.subject().words(),
      carried_words(subject, carried.as_ref()),
      carried_ids(carried.as_ref())
    ),
    Reason::Ancestor { landing, mode } => format!("A {} of {} is running: wait until it merges", push_mode_name(*mode), landing.subject().words()),
    Reason::Held { hold, .. } => match robots.iter().find(|robot| robot.id == hold.by.robot_id) {
      Some(robot) if hold.kind == HoldKind::Own => own_hold_words(subject, robot, None, "restart it on the current branch"),
      Some(robot) if hold.kind == HoldKind::Landed => format!("{}: nothing of it is left to push", landed_words(subject, &hold.by.subject(), robot)),
      Some(robot) => span_hold_words(subject, &hold.by.subject(), robot, hold.carried.as_ref()),
      None => format!("a running Safe Push of {} lands {subject}: a push won't land", hold.by.subject().words()),
    },
    Reason::Blocked { by: Some(by), .. } => format!("{subject} is blocked by {by}: fix it first, then sync"),
    Reason::Blocked { by: None, .. } => format!("{subject} is blocked: fix its parent first, then sync"),
    Reason::Landed { .. } | Reason::StackLanded { .. } => format!("{subject} landed: nothing of it is left to land"),
    Reason::LandedIn { upstream } => format!("{subject} is in {upstream}: the update archives it"),
    Reason::PullRequestMerged { number } => format!("pull request #{number} of {subject} is merged"),
    Reason::NoStackBranch { .. } => format!("{subject} has no stack branch: the stack doesn't fork"),
    Reason::StackNotBuilt { root, .. } => format!("the stack branch of {root} is not the build of its commits; fix it, then sync"),
    Reason::StackParked { member, .. } => format!("{subject} holds parked {member}; land the stack in pieces"),
    Reason::StackHeld { mode, .. } => format!("a {} of {subject} is running on its stack branch", push_mode_name(*mode)),
    // "auth is not synced (a merge conflict)", as `bd push` says it
    Reason::CantPush { branch, block } => not_synced_refusal(branch, *block),
    Reason::DropsRemote { branch, behind } => format!("the remote branch of {branch} has {behind} commits {branch} lacks"),
    Reason::RewrittenSincePush { branch } => format!("{branch} was rewritten since its push: it pushes whole"),
    Reason::Parked { branch, with, note } => parked_words(
      branch,
      &Parked {
        note: note.clone(),
        with: with.clone(),
      },
    ),
    Reason::NeedsPush { branch } => format!("{branch} has commits its remote branch lacks, which the reviewers would not see; --push pushes them first"),
    Reason::NoReviewer { groups } => format!(
      "Patronus names no code owner with a GitHub login to ask for {}: pass --reviewer <login>",
      join_names(groups)
    ),
    Reason::Unprotected { target } => format!("Patronus does not protect {target}: a robot can't be started into it"),
    Reason::Draft { number } => format!("Pull request #{number} is a draft: Patronus merges it once it is ready for review"),
    Reason::OtherBase { number, base, target } => {
      format!("pull request #{number} of {subject} goes into {base}, and Patronus merges a pull request into its base: its approvals count for no Safe Push into {target}")
    }
    Reason::ApprovalsMissing { groups } => format!(
      "The code owners haven't approved {subject}: a Safe Push fails without these approvals (needs review by {})",
      join_names(groups)
    ),
    Reason::DismissesApprovals { number, target } => format!(
      "the push dismisses the approvals of pull request #{number} that Patronus counts (the rules of {target} dismiss stale reviews on push), and the robot would fail without them"
    ),
    Reason::LandingRefused { refusal } => format!("{subject} would not build: {}", refusal.reason()),
    Reason::TargetConflict { files, .. } => format!("{subject} conflicts with the newest target in {}: Patronus would fail it at once", join_names(files)),
  }
}

/// "the whole auth stack doesn't build as one branch: 1a2b3c4d (Add login, of login) conflicts in a.rs with the commits
/// of the stack before it, though each branch builds": why the conflicted stack branch of `update` lands nothing, the
/// commit by `short` of its id with its subject and branch as `branches` (the table of the sync) have it. `bd` adds its
/// commands, the app its words
pub fn stack_conflict_words(branches: &[GroupedBranchInfo], update: &StackBranchUpdate, short: &dyn Fn(&str) -> String) -> String {
  let root = &update.root;
  let Some(conflict) = &update.conflict else {
    return format!("the stack branch of {root} conflicts");
  };
  let found = branches.iter().find_map(|branch| {
    branch
      .commits
      .iter()
      .find(|commit| commit.id == conflict.commit)
      .map(|commit| (branch.name.as_str(), commit))
  });
  let commit = match found {
    Some((branch, commit)) => format!("{} ({}, of {branch})", short(&conflict.commit), commit.stripped_subject),
    None => short(&conflict.commit),
  };
  format!(
    "{} doesn't build as one branch: {commit} conflicts in {} with the commits of the stack before it, though each branch builds",
    Subject::whole_stack(root).words(),
    join_names(&conflict.files)
  )
}

/// "the stack branch of auth is not the build of its commits: login conflicts, so the stack branch was not rebuilt": why
/// a stack branch that is not built can't land, the member that keeps it from being built or the reason
pub fn stack_not_built_words(update: &StackBranchUpdate) -> String {
  let why = match (&update.blocked_by, &update.error) {
    (Some(block), _) => {
      let reason = match block.reason {
        BlockReason::Conflict => "conflicts",
        BlockReason::Problem => "has a stack problem",
        BlockReason::Error => "failed",
        BlockReason::Blocked => "is blocked by its parent",
      };
      format!("{} {reason}, so the stack branch was not rebuilt", block.member)
    }
    (None, Some(error)) => error.clone(),
    (None, None) => "the stack branch was not rebuilt".to_string(),
  };
  format!("the stack branch of {} is not the build of its commits: {why}", update.root)
}
