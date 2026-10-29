//! The reasons the rules give (`domain_types::reason::Reason`; spec/branch/branch-row.spec.md, an action disabled with
//! its reason; spec/branch/patronus.spec.md, a start refused), built from the facts and the robots they name: the push
//! mode and the target of a robot go with its code, so the words of either side need nothing else (the page:
//! `src/presenters/reasons.ts`; `bd` and the engine: `repository_model::refusal_words::start_refusal_text`,
//! `repository_model::push_guard`). `reason_catalog_test` writes every code with an example to `reasons.json`,
//! which the page's formatter is tested on.

use domain_types::patronus::PatronusRobot;
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldView, LandedByView, RobotOf, SpanView};
use domain_types::stack::{BranchStack, StackBranchState, StackBranchUpdate};

/// A robot runs on the commit the subject is at
#[must_use]
pub fn last_commit(robot: &PatronusRobot) -> Reason {
  Reason::LastCommit {
    robot_id: robot.id.clone(),
    mode: robot.push_mode,
  }
}

/// The running robot of `span` lands `branch` with more
#[must_use]
pub fn descendant(branch: &str, span: &SpanView, robot: &PatronusRobot) -> Reason {
  Reason::Descendant {
    branch: branch.to_string(),
    by: span.by.clone(),
    mode: robot.push_mode,
    carried: span.carried.clone(),
  }
}

/// The running robot of `landing` lands a part of what the start would land
#[must_use]
pub fn ancestor(landing: &RobotOf, robot: &PatronusRobot) -> Reason {
  Reason::Ancestor {
    landing: landing.clone(),
    mode: robot.push_mode,
  }
}

/// The merged robot of `landed` landed all of `branch`
#[must_use]
pub fn landed(branch: &str, landed: &LandedByView, robot: &PatronusRobot) -> Reason {
  Reason::Landed {
    branch: branch.to_string(),
    by: landed.by.clone(),
    own: landed.own,
    target: robot.target_branch.clone(),
  }
}

/// The robot of `hold` holds the push of `branch`
#[must_use]
pub fn held(branch: &str, hold: &HoldView, robot: &PatronusRobot) -> Reason {
  Reason::Held {
    branch: branch.to_string(),
    hold: hold.clone(),
    mode: robot.push_mode,
    target: robot.target_branch.clone(),
  }
}

/// Merged robots landed every branch of the whole stack of `root` whole (`landed`: what landed each member, with its
/// robot): each robot named once, in the order they merged, the target of the first; `own` when the robot of the whole
/// stack of `root` landed them all. `None` for no branch
#[must_use]
pub fn stack_landed(root: &str, landed: &[(&RobotOf, &PatronusRobot)]) -> Option<Reason> {
  let mut merged: Vec<&(&RobotOf, &PatronusRobot)> = landed.iter().collect();
  merged.sort_by_key(|(_, robot)| robot.finish_time.unwrap_or(0));
  let target = merged.first()?.1.target_branch.clone();
  let mut by: Vec<RobotOf> = Vec::new();
  for (of, _) in merged {
    if !by.iter().any(|known| known.robot_id == of.robot_id) {
      by.push((*of).clone());
    }
  }
  Some(Reason::StackLanded {
    root: root.to_string(),
    own: landed.iter().all(|(of, _)| of.whole_stack && of.branch == root),
    by,
    target,
  })
}

/// `name` is blocked: by the parent the sync could not build on, else the branch it is built on, else its declared
/// parent
#[must_use]
pub fn blocked(name: &str, stack: Option<&BranchStack>) -> Reason {
  let by = stack.and_then(|stack| {
    stack
      .blocked_by
      .as_ref()
      .map(|block| block.parent.clone())
      .or_else(|| stack.effective_base.clone())
      .or_else(|| stack.declared_parent.clone())
  });
  Reason::Blocked { branch: name.to_string(), by }
}

/// The stack branch of `stack` is no build of its commits: not `built`, or built without a tip; `None` for a built one
#[must_use]
pub fn stack_not_built(stack: &StackBranchUpdate) -> Option<Reason> {
  if stack.state == StackBranchState::Built && stack.tip.is_some() {
    return None;
  }
  Some(Reason::StackNotBuilt {
    root: stack.root.clone(),
    state: stack.state,
    blocked_by: stack.blocked_by.as_ref().map(|block| block.member.clone()),
    conflict: stack.conflict.clone(),
    error: stack.error.clone(),
  })
}
