//! The actions of each subject of the view of a session (`actions_of`): every branch of the table, the whole stack of
//! every root that heads one (`stack:<root>`), every robot of the user (`robot:<id>`) and every start whose answer was lost
//! (`op:<id>`), as `domain_policy::actions`
//! decides them over the table, the next steps the view derived with the same patch, its pushes and decisions of the
//! starts, the open pull requests, what landed each subject, and the facts of the remote services
//! (docs/design/rust-domain-authority.md, s8-policy), with the operations of the session laid over them
//! (`with_operations`: what runs, what a start waits behind, what holds a remote action back). The view derives them
//! with every patch, a sync and a change of the operations included. A table of the cached frame of an earlier run, or
//! robots or pull requests that are values of it, hold the remote actions back (`on_cached_frame`, by what each reads:
//! `reads_cached_frame`, which the engine refuses their intents by too).

use crate::derive::{github_repository_known, landing_route_of, live_hold, patronus_lands, row_of_view};
use crate::facts::FactStore;
use crate::table::TableState;
use domain_policy::actions::{
  BranchFacts, Children, Of, Problems, RobotFacts, Row, StackFacts, StackShape, branch_actions, on_cached_frame, review_blocker, robot_actions, stack_actions, start_actions,
  with_operations,
};
use domain_policy::land::merges_on_github;
use domain_policy::next_step::{Listing, PatronusStanding};
use domain_types::actions::{ActionState, NothingBeyond};
use domain_types::branch_names::to_final_branch_name;
use domain_types::landing::{LandDecisions, Landed, StartRoute};
use domain_types::landing_route::LandingRoute;
use domain_types::next_step::NextStepView;
use domain_types::patronus::PatronusRobotStatus;
use domain_types::pull_request::PullRequestView;
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, PatronusView, RobotOf};
use domain_types::session::{BranchPush, BranchView, RobotsView, SyncState, WholeStackView, stack_readiness_key};
use std::collections::{BTreeMap, HashSet};

/// What of the session the remote actions of a subject read, and their intents with them, which the cached frame of an
/// earlier run may hold
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameReads {
  /// The pushes, the starts, the pull requests and their bases, the review requests and the auto-merge of a branch and of
  /// a whole stack: the robots, the pull requests and the Patronus view
  Landing,
  /// The restart and the cancel of a robot, the robot that needs the user: the robots
  Robots,
  /// The review of a pull request to review: the review requests
  Reviews,
}

/// Whether the remote actions that read `reads` of the session read a frame nobody checked yet: its table is the cached
/// frame of an earlier run, or one of the facts they read is a value of it (`on_cached_frame`)
#[must_use]
pub fn reads_cached_frame(state: &TableState, facts: &FactStore, reads: FrameReads) -> bool {
  let robots = &facts.robots.view().head.state;
  match reads {
    FrameReads::Landing => on_cached_frame(state.cached(), [robots, &facts.pull_requests.view().head.state, &facts.patronus.part().state]),
    FrameReads::Robots => on_cached_frame(state.cached(), [robots]),
    FrameReads::Reviews => on_cached_frame(state.cached(), [&facts.review_requests.view().head.state]),
  }
}

/// What the actions of a view are decided on beside the table and the facts: the parts of the view derived with the same
/// patch
pub struct ActionSources<'a> {
  /// The branch prefix of the session: the full names of the virtual branches its pushes write
  pub prefix: &'a str,
  pub steps: &'a BTreeMap<String, NextStepView>,
  pub pushes: &'a BTreeMap<String, BranchPush>,
  pub lands: &'a BTreeMap<String, LandDecisions>,
  pub open_pull_requests: &'a BTreeMap<String, PullRequestView>,
  pub landed: &'a BTreeMap<String, Landed>,
}

/// The actions of every subject of `state` by subject key (`domain_policy::actions`); none before the first sync, whose
/// next steps they read
#[must_use]
pub fn actions_of(state: &TableState, facts: &FactStore, sources: &ActionSources<'_>) -> BTreeMap<String, Vec<ActionState>> {
  let parts = &state.parts;
  let part = facts.patronus.part();
  let robots = facts.robots.view();
  let pull_requests = facts.pull_requests.view();
  let standing = PatronusStanding::of(&robots.head, &part.state);
  let patronus = standing.answer();
  let listing = Listing::of(&pull_requests.head);
  let repository = github_repository_known(pull_requests, robots);
  let sync_running = matches!(state.sync, SyncState::Running { .. });
  let operations = state.operations();
  let route = landing_route_of(&parts.header, &parts.sections.landed, facts);
  let push_route = matches!(route, LandingRoute::Push { .. });
  let merge_route = merges_on_github(&route, patronus_lands(facts.robots.view()));
  // what the pushes, the starts and the pull requests read; a restart and a cancel only the robots
  let cached = reads_cached_frame(state, facts, FrameReads::Landing);
  let robots_cached = reads_cached_frame(state, facts, FrameReads::Robots);
  let mut actions = BTreeMap::new();
  for (name, branch) in &parts.branches {
    let Some(step) = sources.steps.get(name) else {
      continue;
    };
    let row = row_of_view(branch);
    let hold = part.view.branches.get(name).and_then(|facts| live_hold(facts, robots));
    let hold_reason = hold.map(|(hold, robot)| Reason::Held {
      branch: name.clone(),
      hold: hold.clone(),
      mode: robot.push_mode,
      target: robot.target_branch.clone(),
    });
    let children = sources.pushes.get(name).and_then(|push| push.children.as_ref()).map(|push| Children {
      push,
      statuses_pending: sync_running && !push.offers && unparked_line(parts, name, branch).any(|it| parts.branches.get(it).is_none_or(|it| it.remote.status.is_none())),
      landed: landed_with_children(&part.view, robots, name, &branch.structure.descendants),
    });
    let facts = BranchFacts {
      row: Row {
        name,
        syncing: row.syncing,
        status: row.status,
        blocked_by: row.blocked_by.as_deref(),
        sync_failed: row.sync_failed,
        problem: branch.head.stack.as_ref().and_then(|stack| stack.problems.first()),
        commit_count: row.commit_count,
        parked: branch.structure.parked.as_ref(),
        declared_parent: branch.head.stack.as_ref().and_then(|stack| stack.declared_parent.as_deref()),
        all_referenced: branch.head.all_commits_have_issue_references,
      },
      patronus,
      step: &step.step,
      hold: hold_reason.as_ref(),
      hold_landed: hold.is_some_and(|(hold, _)| hold.kind == HoldKind::Landed),
      remote: branch.remote.status.as_ref(),
      children,
      pull_request: pull_requests.pull_requests.get(name),
      open_pull_request: sources.open_pull_requests.get(name),
      listing,
      repository,
      comment_route: sources.lands.get(name).and_then(|lands| match lands.safe_push.start_route() {
        Some(StartRoute::Comment { number }) => Some(*number),
        _ => None,
      }),
      stacks_enabled: parts.header.stacks_enabled,
      push_route,
      merge_route,
      cached,
    };
    // what a push of it writes, read only while an operation may hold it
    let pushes = (!operations.is_empty()).then(|| to_final_branch_name(sources.prefix, name).ok()).flatten();
    let of = Of::Branch {
      name,
      pushes: pushes.as_deref(),
      unparks: branch.structure.parked.as_ref().and_then(|parked| parked.with.as_deref()).unwrap_or(name),
      pull_request: sources.open_pull_requests.get(name).map(|open| open.number),
    };
    actions.insert(name.clone(), with_operations(branch_actions(&facts), of, operations));
  }
  for (root, branch) in &parts.branches {
    let Some(whole) = &branch.structure.whole_stack else {
      continue;
    };
    let key = stack_readiness_key(root);
    let main_line = &branch.structure.main_line;
    let leaf = main_line.last();
    let leaf_step = leaf.and_then(|leaf| sources.steps.get(leaf)).map(|view| &view.step);
    let leaf_review = leaf.and_then(|leaf| parts.branches.get(leaf)).and_then(|leaf| {
      let row = row_of_view(leaf);
      review_blocker(&Row {
        name: &leaf.head.name,
        syncing: row.syncing,
        status: row.status,
        blocked_by: row.blocked_by.as_deref(),
        sync_failed: row.sync_failed,
        problem: leaf.head.stack.as_ref().and_then(|stack| stack.problems.first()),
        commit_count: row.commit_count,
        parked: None,
        declared_parent: None,
        all_referenced: false,
      })
    });
    let (shape, step) = match whole {
      WholeStackView::Forking { not_built } => (
        StackShape::Forking {
          not_built: not_built.as_ref(),
          tip: parts.stacks.get(root).is_some_and(|stack| stack.tip.is_some()),
          main_line: leaf_step,
        },
        sources.steps.get(&key).map(|view| &view.step),
      ),
      WholeStackView::Chain { .. } => (
        StackShape::Chain {
          length: main_line.len() + 1,
          review: leaf_review.as_ref(),
        },
        leaf_step,
      ),
    };
    let facts = StackFacts {
      root,
      patronus,
      shape,
      step,
      pull_request: pull_requests.stack_pull_requests.get(root),
      open_pull_request: sources.open_pull_requests.get(&key),
      repository,
      landed: sources.landed.get(&key),
      push_route,
      merge_route,
      cached,
    };
    let of = Of::WholeStack {
      root,
      main_line: leaf.map(String::as_str),
      pull_request: sources.open_pull_requests.get(&key).map(|open| open.number),
    };
    let offered = with_operations(stack_actions(&facts), of, operations);
    actions.insert(key, offered);
  }
  let table_robots: HashSet<&str> = part
    .view
    .branches
    .values()
    .filter_map(|facts| facts.robot_id.as_deref())
    .chain(part.view.stacks.values().filter_map(|facts| facts.robot_id.as_deref()))
    .collect();
  for robot in robots.robots.values() {
    let row = robot.branch_name.as_deref().filter(|name| parts.branches.contains_key(*name));
    let of_table = row.is_some() || table_robots.contains(robot.id.as_str());
    let problems = if robot.status != PatronusRobotStatus::Failure || !of_table {
      Problems::Known
    } else if let Some(message) = robots.problems_failed.get(&robot.id) {
      Problems::Failed(message)
    } else if robots.needs_review.contains_key(&robot.id) {
      Problems::Known
    } else {
      Problems::Pending
    };
    let facts = RobotFacts {
      robot,
      restart: part.view.restarts.get(&robot.id),
      pushes: row.is_some() || (robot.branch_name.is_none() && robot.stack_root.is_some()),
      needs_review: robots.needs_review.get(&robot.id).copied(),
      failing: robots.failing.contains(&robot.id),
      problems,
      step: row.and_then(|name| sources.steps.get(name)).map(|view| &view.step),
      cached: robots_cached,
    };
    let offered = with_operations(robot_actions(&facts), Of::Robot { id: &robot.id }, operations);
    actions.insert(format!("robot:{}", robot.id), offered);
  }
  // a start whose answer was lost, of any process (`op:<id>`)
  for operation in operations {
    let offered = start_actions(operation);
    if !offered.is_empty() {
      actions.insert(format!("op:{}", operation.id), with_operations(offered, Of::Start { op: &operation.id }, operations));
    }
  }
  actions
}

/// The branch and the branches stacked on it that aren't parked: what Push with Children reads the statuses of
fn unparked_line<'a>(parts: &'a crate::table::TableParts, name: &'a str, branch: &'a BranchView) -> impl Iterator<Item = &'a str> {
  std::iter::once(name).chain(
    branch
      .structure
      .descendants
      .iter()
      .map(String::as_str)
      .filter(|it| parts.branches.get(*it).is_none_or(|it| it.structure.parked.is_none())),
  )
}

/// Merged Safe Pushes landed `name` and every branch stacked on it whole: each robot once, in the order they merged;
/// `own` when the robot of the whole stack of `name` landed them all. `None` for a branch without descendants, or while
/// one of them is left
fn landed_with_children(view: &PatronusView, robots: &RobotsView, name: &str, descendants: &[String]) -> Option<NothingBeyond> {
  if descendants.is_empty() {
    return None;
  }
  let mut by: Vec<(&RobotOf, u32, &str)> = Vec::new();
  for branch in std::iter::once(name).chain(descendants.iter().map(String::as_str)) {
    let landed = view.branches.get(branch)?.landed_by.as_ref().filter(|landed| landed.after.is_empty())?;
    let robot = robots.robots.get(&landed.by.robot_id).filter(|robot| robot.status == PatronusRobotStatus::Success)?;
    if !by.iter().any(|(of, _, _)| of.robot_id == landed.by.robot_id) {
      by.push((&landed.by, robot.finish_time.unwrap_or(0), &robot.target_branch));
    }
  }
  by.sort_by_key(|(_, finish, _)| *finish);
  let own = by.iter().all(|(of, _, _)| of.whole_stack && of.branch == name);
  let target = by.first().map(|(_, _, target)| (*target).to_string())?;
  Some(NothingBeyond::Landed {
    by: by.into_iter().map(|(of, _, _)| of.clone()).collect(),
    own,
    target,
  })
}

#[cfg(test)]
#[path = "action_view_test.rs"]
mod action_view_test;
