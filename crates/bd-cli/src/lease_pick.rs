//! The answer of `brd lease pick` (openspec/specs/cli/lease/spec.md, "Pick"): the pick of the lease's commits onto its
//! home branch, after a conflict the update of the lease onto that branch, and with `--return` the return of the lease
//! once its commits landed, in the same process: the pick (`bd_update::pick_command`) ends with the repository lock
//! released, and only then the return takes the lock of the pool (`leases.lock`, `lease_pool::lease`)

use bd_core::error::ErrorBody;
use bd_core::progress::CommandSteps;
use bd_lease::lease_command::{LeaseContext, LeasePickArgs, LeaseReturnArgs, LeaseReturnOutput, lease_return_after_pick};
use bd_update::pick_command::{PickOutcome, PickOutput};
use bd_update::update_command::UpdateOutput;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

/// `brd lease pick`: the pick of the lease's commits onto its home branch, and after a conflict the update of the lease
/// onto that branch
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeasePickOutput {
  #[serde(flatten)]
  pub pick: PickOutput,
  /// With a conflict of the pick: the update of the lease onto the tip of its home branch, as `brd update` prints it. A
  /// conflict there opens a resolve session in the lease (`state: conflict`, `session`): resolve it, `brd update
  /// --continue`, build and test, then `brd lease pick` again; `brd update --abort` closes it, nothing moved. An update
  /// that applies because its words resolved the conflict (`resolved`) moved the lease onto the tip and landed nothing:
  /// build and test the files of `verify.paths`, then `next`. The commit of its round has no `body`: its `patch` file
  /// holds the message
  #[serde(skip_serializing_if = "Option::is_none")]
  pub update: Option<UpdateOutput>,
  /// With --return, once the pick landed the commits (or found every one on the branch): the HEAD of the lease the pick
  /// took them from, a full id. The slot is free at it, and the next get moves it: after `brd undo <undo_token>`, which
  /// moves only the home branch back, this commit (and a branch of the lease, which stays) holds the commits, so
  /// `git branch <name> <source_head>` keeps them
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source_head: Option<String>,
  /// With --return: the return of the lease once its commits landed, as `brd lease return` prints it; the pick is the
  /// proof of its commits (`landed`, `proof: picked`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub returned: Option<LeaseReturnOutput>,
  /// With --return: the pick landed, but the return refused, as `brd lease return` refuses (`slot_busy`,
  /// `dirty_worktree`, `unlanded_commits` of a commit made after the pick read HEAD); the lease is still yours and did
  /// not follow. Act on it, then `brd lease return`. The exit code is the one of its code
  #[serde(skip_serializing_if = "Option::is_none")]
  pub return_refused: Option<ErrorBody>,
}

impl LeasePickOutput {
  /// The answer of a pick that returns nothing (no --return, a conflict, a dry run), and of an update after a conflict
  pub fn of(pick: PickOutput, update: Option<UpdateOutput>) -> Self {
    Self {
      pick,
      update: without_body(update),
      source_head: None,
      returned: None,
      return_refused: None,
    }
  }

  /// 1 while the commits did not land, the exit code of `return_refused` once they did, else 0
  pub const fn exit_code(&self) -> i32 {
    match &self.return_refused {
      _ if self.pick.exit_code() != 0 => self.pick.exit_code(),
      Some(refused) => refused.code.exit_code(),
      None => 0,
    }
  }
}

/// The answer of `brd lease pick` in `lease` with the return of `--return`: once the pick landed the commits or found
/// them all on the branch (no conflict, no dry run), the lease is returned as `brd lease return` returns it, the commits
/// of the pick's range proven landed by the pick (`lease_return_after_pick`), the HEAD the pick read named for the undo.
/// The return keeps every refusal of its own: a commit made after the pick read HEAD is checked as by any return. A
/// refusal keeps the lease (`return_refused`). The return is a step of `steps`, after the pick released the repository
/// lock
pub fn returned_after(lease: &LeaseContext, pick: PickOutput, update: Option<UpdateOutput>, args: &LeasePickArgs, steps: &CommandSteps<'_>) -> LeasePickOutput {
  if !args.return_lease || args.dry_run || pick.state == PickOutcome::Conflict {
    return LeasePickOutput::of(pick, update);
  }
  steps.tell(&format_args!("returning the lease {}", lease.dirs.top_level));
  let return_args = LeaseReturnArgs {
    workspace: Some(PathBuf::from(&lease.dirs.top_level)),
    stop_own: args.stop_own,
    ..LeaseReturnArgs::default()
  };
  let (returned, return_refused) = match lease_return_after_pick(lease, &return_args, &pick.landed_range) {
    Ok(returned) => (Some(returned), None),
    Err(mut error) => {
      error.message = format!("The commits landed, the lease stays: {}", error.message);
      (None, Some(error.to_output().error))
    }
  };
  LeasePickOutput {
    source_head: Some(pick.source_head.clone()),
    pick,
    update: without_body(update),
    returned,
    return_refused,
  }
}

/// `update` without the body of the commit of its round: the agent reads the message in its `patch` file, and the answer
/// of a conflict is read whole
fn without_body(mut update: Option<UpdateOutput>) -> Option<UpdateOutput> {
  if let Some(commit) = update.as_mut().and_then(|update| update.session.as_mut()).and_then(|session| session.commit.as_mut()) {
    commit.body.clear();
  }
  update
}
