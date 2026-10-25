//! What the robots land and landed of the branch table, the facts the rules of the holds and the starts decide over
//! (`derive::decide_landing`, `domain_policy::holds`, `domain_policy::land`): the one derivation of the app (the session
//! derives it, `facts::derive_patronus`) and of `bd` (`bd push`, `bd safe-push`, `bd status`),
//! spec/branch/patronus.spec.md and docs/design/git-operations.md, "Session facts".
//!
//! - A merged robot landed the branch it ran on and those it is built on (for the whole stack every branch of it), the
//!   commits added after the ones it took the work left; of several, the one that left the fewest, of equals the first
//!   merge.
//! - A running robot that pushes (not a dry run) lands the branches its commit has commits of: of a branch built on
//!   them, the first of the table that does; of the whole stack, the branches of it no such robot lands. The first
//!   running robot that lands a branch is the span of its hold (a canceling one lands it but holds nothing and hides
//!   none, `domain_policy::holds::branch_hold`).
//! - The hashes tell when the robot took its subject as it is (its tip, or its up-to-date remote branch) or a commit of
//!   it the branch built new ones on (its copy is a commit of the branch): no git. Otherwise the content does
//!   (`robot_landing`), once per state of the branches (`ContentMemo`).

use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::derive::decide_landing;
use crate::facts::PatronusOf;
use domain_policy::facts::RemoteBranch;
use domain_policy::holds::{holds_pushes, is_active, is_pushing};
use domain_policy::land::took_as_is;
use domain_policy::landing_needs::name_landing_robots;
use domain_policy::robot_match::{RobotIndex, RobotMatches, Tips, match_table};
use domain_types::landing::{LandingRefusal, Subject};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::{BranchPatronus, Carried, HoldKind, LandedByView, LandingNeeds, PatronusView, RobotLandedBranch, RobotOf, SpanView, StackPatronus};
use domain_types::robot_landing::{BranchLanding, BranchSpan, RobotLandingRequest};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};
use sync_core::robot_landing::{robot_landings_of_grouped, robot_spans_of_grouped};
use tracing::{debug, instrument};

/// How many answers a memo keeps of each kind before it starts again
const MEMO_LIMIT: usize = 512;

/// The answers by content of earlier views, by the state they depend on (`content_key`: the robot's commit, the tip of
/// its subject and the commits of the branches with their copies): a view of the same state runs no git
#[derive(Debug, Default)]
pub struct ContentMemo {
  landings: HashMap<String, Vec<BranchLanding>>,
  spans: HashMap<String, Option<Vec<BranchSpan>>>,
}

/// `memo`; a poisoned one (a panic in a content walk) starts again
pub fn lock_memo(memo: &Mutex<ContentMemo>) -> MutexGuard<'_, ContentMemo> {
  memo.lock().unwrap_or_else(|poisoned| {
    memo.clear_poison();
    let mut memo = poisoned.into_inner();
    *memo = ContentMemo::default();
    memo
  })
}

/// The robot of each branch and of each whole stack of the table (`domain_policy::robot_match`: started on the branch,
/// or of a temporary ref at its local or remote tip), which the view names (`robot_id`)
pub fn match_robots(table: &BranchTable, remote: &RemoteFacts, robots: &[PatronusRobot]) -> RobotMatches {
  fn remote_tip(fact: Option<&RemoteFact>) -> Option<&str> {
    fact.and_then(|fact| fact.remote_tip.as_deref())
  }
  let branches = table.branches.iter().map(|branch| Tips {
    of: &branch.name,
    local: table.tip_of(&branch.name),
    remote: remote_tip(remote.branches.get(&branch.name)),
  });
  let stacks = table.stack_branches.iter().map(|stack| Tips {
    of: &stack.root,
    local: stack.tip.as_deref(),
    remote: remote_tip(remote.stacks.get(&stack.root)),
  });
  match_table(&RobotIndex::new(robots), branches, stacks)
}

/// What the rules read of a remote branch the deriver knows
fn remote_branch(fact: Option<&RemoteFact>) -> Option<RemoteBranch<'_>> {
  fact.and_then(RemoteFact::branch)
}

/// What the hashes tell of the commit a robot took of its subject: the commits of the branch above it (original ids,
/// newest first; empty when it took the subject as it is), or None when only the content tells
fn hashes_tell(table: &BranchTable, remote: &RemoteFacts, subject: &Subject, source: &str) -> Option<Vec<String>> {
  match subject {
    Subject::Branch { name } => {
      let branch = table.branch(name)?;
      if took_as_is(source, table.tip_of(name), remote_branch(remote.branches.get(name))) {
        return Some(Vec::new());
      }
      // a commit of the branch the robot took, new ones built on it since: the copies below it are the ones it took
      let at = branch.commits.iter().position(|commit| table.copy_of(commit) == Some(source))?;
      if branch.commits[at..].iter().any(|commit| table.copy_of(commit).is_none()) {
        return None;
      }
      Some(branch.commits[..at].iter().map(|commit| commit.id.clone()).collect())
    }
    Subject::WholeStack { root } => {
      let stack = table.stack(root)?;
      took_as_is(source, stack.tip.as_deref(), remote_branch(remote.stacks.get(root))).then(Vec::new)
    }
  }
}

/// The tip of the ref the robot of `subject` ran on
fn subject_tip(table: &BranchTable, subject: &Subject) -> Option<String> {
  match subject {
    Subject::Branch { name } => table.tip_of(name).map(str::to_string),
    Subject::WholeStack { root } => table.stack(root).and_then(|stack| stack.tip.clone()),
  }
}

/// The question by content about the robot of `subject` and its `branches` (root first)
fn content_request(table: &BranchTable, robot: &PatronusRobot, subject: &Subject, source: &str, branches: Vec<String>) -> RobotLandingRequest {
  let (branch_name, stack_root) = match subject {
    Subject::Branch { name } => (Some(name.clone()), None),
    Subject::WholeStack { root } => (None, Some(root.clone())),
  };
  RobotLandingRequest {
    robot_id: robot.id.clone(),
    branch_name,
    stack_root,
    source_revision: source.to_string(),
    branches,
    tip: subject_tip(table, subject),
  }
}

/// What the answer to `request` depends on: the robot's commit, the tip it ran on, the commits of the branches with
/// their copies
fn content_key(table: &BranchTable, request: &RobotLandingRequest) -> String {
  let mut key = format!(
    "{}\n{}\n{}\n{}",
    request.robot_id,
    request.source_revision,
    request.branch_name.as_deref().or(request.stack_root.as_deref()).unwrap_or_default(),
    request.tip.as_deref().unwrap_or_default()
  );
  for name in &request.branches {
    key.push('\n');
    key.push_str(name);
    for commit in table.branch(name).map_or(&[][..], |branch| &branch.commits[..]) {
      key.push(' ');
      key.push_str(&commit.id);
      key.push('>');
      key.push_str(table.copy_of(commit).unwrap_or_default());
    }
  }
  key
}

/// The commits of `name`, original ids, newest first
fn commit_ids(table: &BranchTable, name: &str) -> Vec<String> {
  table
    .branch(name)
    .map_or_else(Vec::new, |branch| branch.commits.iter().map(|commit| commit.id.clone()).collect())
}

/// What the hashes told, or the key of the answer of the content (`content_key`) that tells it
enum Told<T> {
  Hashes(T),
  Content(String),
}

/// A robot with its subject, the candidates it may land and what tells what it does
struct Planned<'r, T> {
  robot: &'r PatronusRobot,
  subject: Subject,
  candidates: Vec<String>,
  told: Told<T>,
}

/// The plan of a robot the hashes don't answer: its question by content (about `branches`), asked once per key unless
/// the memo has the answer (`known`)
#[expect(clippy::too_many_arguments, reason = "the robot, its subject and question, and where the questions gather")]
fn ask_content<'r, T>(
  table: &BranchTable,
  robot: &'r PatronusRobot,
  subject: Subject,
  source: &str,
  branches: Vec<String>,
  candidates: Vec<String>,
  known: impl Fn(&str) -> bool,
  asked: &mut Vec<(String, RobotLandingRequest)>,
) -> Planned<'r, T> {
  let request = content_request(table, robot, &subject, source, branches);
  let key = content_key(table, &request);
  if !known(&key) && !asked.iter().any(|(question, _)| *question == key) {
    asked.push((key.clone(), request));
  }
  Planned {
    robot,
    subject,
    candidates,
    told: Told::Content(key),
  }
}

/// What one merged robot landed, before the merge of all robots
struct RobotLanded<'r> {
  robot: &'r PatronusRobot,
  by: RobotOf,
  subject_branch: Option<String>,
  branches: Vec<BranchLanding>,
}

/// What the merged robots landed, in the order they merged: by the hashes, else by content (one call for every robot
/// the memo lacks)
fn merged_landings<'r>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  table: &BranchTable,
  remote: &RemoteFacts,
  robots: &'r [PatronusRobot],
  memo: &mut ContentMemo,
) -> Vec<RobotLanded<'r>> {
  let mut merged: Vec<&PatronusRobot> = robots
    .iter()
    .filter(|robot| robot.status == PatronusRobotStatus::Success && is_pushing(robot) && robot.source_revision.is_some())
    .collect();
  merged.sort_by_key(|robot| robot.finish_time.unwrap_or(0));

  // by robot: the subject, and what the hashes told or the key of the content
  let mut asked: Vec<(String, RobotLandingRequest)> = Vec::new();
  let mut planned: Vec<Planned<'_, Vec<BranchLanding>>> = Vec::new();
  for robot in merged {
    let source = robot.source_revision.as_deref().unwrap_or_default();
    let subject = match (&robot.branch_name, &robot.stack_root) {
      (Some(name), _) if table.branch(name).is_some() => Subject::Branch { name: name.clone() },
      (None, Some(root)) if table.stack(root).is_some() => Subject::WholeStack { root: root.clone() },
      _ => continue,
    };
    let candidates: Vec<String> = match &subject {
      Subject::Branch { name } => table.ancestors_of(name).into_iter().chain(std::iter::once(name.clone())).collect(),
      Subject::WholeStack { root } => table.stack(root).map_or_else(Vec::new, |stack| stack.members.clone()),
    };
    if let Some(above) = hashes_tell(table, remote, &subject, source) {
      let own = match &subject {
        Subject::Branch { name } => Some(name.as_str()),
        Subject::WholeStack { .. } => None,
      };
      let branches = candidates
        .iter()
        .filter(|name| table.branch(name).is_some())
        .map(|name| {
          let after = if Some(name.as_str()) == own { above.clone() } else { Vec::new() };
          BranchLanding {
            branch: name.clone(),
            landed: commit_ids(table, name).into_iter().filter(|id| !after.contains(id)).collect(),
            after,
          }
        })
        .collect();
      planned.push(Planned {
        robot,
        subject,
        candidates: Vec::new(),
        told: Told::Hashes(branches),
      });
    } else {
      let landings = &memo.landings;
      planned.push(ask_content(
        table,
        robot,
        subject,
        source,
        candidates,
        Vec::new(),
        |key| landings.contains_key(key),
        &mut asked,
      ));
    }
  }
  if !asked.is_empty() {
    if memo.landings.len() > MEMO_LIMIT {
      memo.landings.clear();
    }
    let requests: Vec<RobotLandingRequest> = asked.iter().map(|(_, request)| request.clone()).collect();
    match robot_landings_of_grouped(git_executor, repository_path, &table.prefix, &table.baseline, &requests, &table.branches) {
      Ok(answers) => {
        for (key, request) in asked {
          let branches = answers
            .iter()
            .find(|landing| landing.robot_id == request.robot_id)
            .map_or_else(Vec::new, |landing| landing.branches.clone());
          memo.landings.insert(key, branches);
        }
      }
      // nothing landed by content for this view; the next view asks again
      Err(error) => debug!(error = %format!("{error:#}"), "no robot landings: nothing landed by content"),
    }
  }
  planned
    .into_iter()
    .map(|Planned { robot, subject, told, .. }| {
      let branches = match told {
        Told::Hashes(branches) => branches,
        Told::Content(key) => memo.landings.get(&key).cloned().unwrap_or_default(),
      };
      RobotLanded {
        robot,
        by: subject.robot_of(&robot.id),
        subject_branch: match subject {
          Subject::Branch { name } => Some(name),
          Subject::WholeStack { .. } => None,
        },
        branches,
      }
    })
    .collect()
}

/// What a running robot lands of its candidates: the branches, in their order, and the commits of each its commit has;
/// `carried` is None when only the stack as it is now tells
struct RobotSpanFacts<'r> {
  robot: &'r PatronusRobot,
  subject: Subject,
  names: Vec<String>,
  carried: Option<HashMap<String, Carried>>,
}

/// What the running robots that push land: the robot of each branch with ancestors and of each whole stack, by the
/// hashes, else by content (one call for every robot the memo lacks); in the order of the table, the stacks after
fn running_spans<'r>(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  table: &BranchTable,
  remote: &RemoteFacts,
  robot_of: impl Fn(&str) -> Option<&'r PatronusRobot>,
  stack_robot_of: impl Fn(&str) -> Option<&'r PatronusRobot>,
  memo: &mut ContentMemo,
) -> Vec<RobotSpanFacts<'r>> {
  let lands = |robot: &&PatronusRobot| is_active(robot) && is_pushing(robot);
  let mut subjects: Vec<(&PatronusRobot, Subject, Vec<String>)> = Vec::new();
  for branch in &table.branches {
    let ancestors = table.ancestors_of(&branch.name);
    if let Some(robot) = robot_of(&branch.name).filter(lands)
      && !ancestors.is_empty()
    {
      subjects.push((robot, Subject::Branch { name: branch.name.clone() }, ancestors));
    }
  }
  for stack in &table.stack_branches {
    if let Some(robot) = stack_robot_of(&stack.root).filter(lands)
      && !stack.members.is_empty()
    {
      subjects.push((robot, Subject::WholeStack { root: stack.root.clone() }, stack.members.clone()));
    }
  }

  let all_commits = |names: &[String]| -> HashMap<String, Carried> {
    names
      .iter()
      .map(|name| {
        (
          name.clone(),
          Carried {
            commits: commit_ids(table, name),
            amended: Vec::new(),
          },
        )
      })
      .collect()
  };
  let mut asked: Vec<(String, RobotLandingRequest)> = Vec::new();
  let mut planned: Vec<Planned<'_, Option<HashMap<String, Carried>>>> = Vec::new();
  for (robot, subject, candidates) in subjects {
    let Some(source) = robot.source_revision.as_deref() else {
      // just started: its commit is not known yet
      planned.push(Planned {
        robot,
        subject,
        candidates,
        told: Told::Hashes(None),
      });
      continue;
    };
    if hashes_tell(table, remote, &subject, source).is_some() {
      let carried = all_commits(&candidates);
      planned.push(Planned {
        robot,
        subject,
        candidates,
        told: Told::Hashes(Some(carried)),
      });
      continue;
    }
    let branches = match &subject {
      Subject::Branch { name } => candidates.iter().cloned().chain(std::iter::once(name.clone())).collect(),
      Subject::WholeStack { .. } => candidates.clone(),
    };
    let spans = &memo.spans;
    planned.push(ask_content(table, robot, subject, source, branches, candidates, |key| spans.contains_key(key), &mut asked));
  }
  if !asked.is_empty() {
    if memo.spans.len() > MEMO_LIMIT {
      memo.spans.clear();
    }
    let requests: Vec<RobotLandingRequest> = asked.iter().map(|(_, request)| request.clone()).collect();
    match robot_spans_of_grouped(git_executor, repository_path, &table.prefix, &table.baseline, &requests, &table.branches) {
      Ok(answers) => {
        for ((key, _), answer) in asked.into_iter().zip(answers) {
          memo.spans.insert(key, answer.branches);
        }
      }
      // the stack as it is now holds, asked again by the next view
      Err(error) => debug!(error = %format!("{error:#}"), "no robot spans: the stack as it is now tells"),
    }
  }
  planned
    .into_iter()
    .map(|Planned { robot, subject, candidates, told }| {
      let carried = match told {
        Told::Hashes(carried) => carried,
        Told::Content(key) => memo.spans.get(&key).cloned().flatten().map(|branches| {
          branches
            .into_iter()
            .map(|span| {
              (
                span.branch,
                Carried {
                  commits: span.commits,
                  amended: span.amended,
                },
              )
            })
            .collect()
        }),
      };
      let names = match &carried {
        Some(carried) => candidates.into_iter().filter(|name| carried.contains_key(name)).collect(),
        None => candidates,
      };
      RobotSpanFacts { robot, subject, names, carried }
    })
    .collect()
}

/// By branch, the first robot of `spans` that lands it and passes `counts`: of a branch built on it in the order of the
/// table, then of its whole stack
fn spans_by_branch(spans: &[RobotSpanFacts<'_>], counts: impl Fn(&PatronusRobot) -> bool) -> HashMap<String, SpanView> {
  let mut by_branch: HashMap<String, SpanView> = HashMap::new();
  // the robots of branches come first in `spans`, then those of the whole stacks
  for facts in spans.iter().filter(|facts| counts(facts.robot)) {
    for name in &facts.names {
      by_branch.entry(name.clone()).or_insert_with(|| SpanView {
        by: facts.subject.robot_of(&facts.robot.id),
        carried: facts.carried.as_ref().and_then(|carried| carried.get(name).cloned()),
      });
    }
  }
  by_branch
}

/// What the robots land and landed of the branches of the table of `of` (module docs), with the robot of each branch and
/// whole stack (`matched`, `match_robots`), and what the rules decide over these facts (`derive::decide_landing`: the
/// holds, the starts, the restarts). The robots are the user's (only those that land or landed change what is held),
/// the remote branches what the deriver knows of them. The content of robots the hashes don't answer comes from `memo`, else from git (`robot_landing`); a
/// git error is an answer of nothing by content: a merged robot landed nothing, a running one lands what the stack as
/// it is now says.
#[instrument(skip_all, fields(robots = of.robots.len(), branches = of.table.branches.len()))]
pub fn patronus_view(git_executor: &GitCommandExecutor, repository_path: &str, of: &PatronusOf<'_>, matched: &RobotMatches, memo: &mut ContentMemo) -> PatronusView {
  let PatronusOf {
    table,
    remote,
    robots,
    needs_review,
  } = *of;
  let by_id: HashMap<&str, &PatronusRobot> = robots.iter().map(|robot| (robot.id.as_str(), robot)).collect();
  let robot_of = |name: &str| matched.branches.get(name).and_then(|id| by_id.get(id.as_str()).copied());
  let stack_robot_of = |root: &str| matched.stacks.get(root).and_then(|id| by_id.get(id.as_str()).copied());
  let lands = |robot: &PatronusRobot| is_active(robot) && is_pushing(robot);

  // landed: of several robots the one that left the fewest commits after it, of equals the first merge
  let landings = merged_landings(git_executor, repository_path, table, remote, robots, memo);
  let mut landed_by: HashMap<String, LandedByView> = HashMap::new();
  let mut landed_by_robot: BTreeMap<String, Vec<RobotLandedBranch>> = BTreeMap::new();
  for landing in &landings {
    for branch in &landing.branches {
      let own = landing.subject_branch.as_deref() == Some(branch.branch.as_str());
      landed_by_robot.entry(landing.robot.id.clone()).or_default().push(RobotLandedBranch {
        branch: branch.branch.clone(),
        own,
        after: branch.after.clone(),
      });
      if landed_by.get(&branch.branch).is_some_and(|known| known.after.len() <= branch.after.len()) {
        continue;
      }
      landed_by.insert(
        branch.branch.clone(),
        LandedByView {
          by: landing.by.clone(),
          own,
          landed: branch.landed.clone(),
          after: branch.after.clone(),
        },
      );
    }
  }

  // lands: what the commit of each running robot has; the spans of active robots, the holds of running ones
  let spans = running_spans(git_executor, repository_path, table, remote, robot_of, stack_robot_of, memo);
  let span_of = spans_by_branch(&spans, lands);
  let hold_span_of = spans_by_branch(&spans, holds_pushes);

  let mut view = PatronusView {
    landed_by_robot,
    ..PatronusView::default()
  };
  for branch in &table.branches {
    let name = branch.name.as_str();
    // the nearest ancestor whose robot lands, or that a robot of a side branch lands; never this branch's own robot,
    // which a start replaces
    let landing_ancestor = table
      .ancestors_of(name)
      .iter()
      .rev()
      .find_map(|ancestor| match robot_of(ancestor).filter(|robot| lands(robot)) {
        Some(robot) => Some(Subject::Branch { name: ancestor.clone() }.robot_of(&robot.id)),
        None => span_of.get(ancestor).filter(|span| span.by.branch != name).map(|span| span.by.clone()),
      });
    let facts = BranchPatronus {
      robot_id: matched.branches.get(name).cloned(),
      landed_by: landed_by.remove(name),
      span: span_of.get(name).cloned(),
      landing_ancestor,
      ..BranchPatronus::default()
    };
    if facts != BranchPatronus::default() {
      view.branches.insert(name.to_string(), facts);
    }
  }

  for stack in &table.stack_branches {
    let root = stack.root.as_str();
    let robot_id = matched.stacks.get(root).cloned();
    // the first branch of it whose robot lands a part of it; a stack that no longer forks has none, only its robot
    let landing_member = stack.members.iter().find_map(|member| {
      robot_of(member)
        .filter(|robot| lands(robot))
        .map(|robot| Subject::Branch { name: member.clone() }.robot_of(&robot.id))
    });
    let facts = StackPatronus {
      robot_id,
      landing_member,
      ..StackPatronus::default()
    };
    if facts != StackPatronus::default() {
      view.stacks.insert(root.to_string(), facts);
    }
  }
  decide_landing(&mut view, table, remote, robots, &hold_span_of, needs_review);
  view
}

/// The commits (original ids) merged Safe Pushes landed, as `view` names them (`landedBy`): the target has the names
/// they add, the landing needs count them as landed
pub fn landed_commits(view: &PatronusView) -> HashSet<String> {
  view
    .branches
    .values()
    .filter_map(|facts| facts.landed_by.as_ref())
    .flat_map(|landed| landed.landed.iter().cloned())
    .collect()
}

/// Every branch of `table` with its landing needs, known: `needs` lists the branches with a refusal. A missing
/// declaration names the running Safe Push that lands the branch adding the name, as `view` holds it: its own robot, or
/// one of a branch built on it or of its whole stack
pub fn with_landing_needs(view: &mut PatronusView, table: &BranchTable, needs: &BTreeMap<String, Vec<LandingRefusal>>) {
  let landing: HashMap<String, RobotOf> = view
    .branches
    .iter()
    .filter_map(|(name, facts)| {
      let running = facts.landing.hold.as_ref().filter(|hold| hold.kind != HoldKind::Landed).map(|hold| hold.by.clone());
      Some((name.clone(), running.or_else(|| facts.span.as_ref().map(|span| span.by.clone()))?))
    })
    .collect();
  for branch in &table.branches {
    let mut refusals = needs.get(&branch.name).cloned().unwrap_or_default();
    name_landing_robots(&mut refusals, |declared_in| landing.get(declared_in).cloned());
    view.branches.entry(branch.name.clone()).or_default().landing_needs = LandingNeeds::Known { refusals };
  }
}

#[cfg(test)]
#[path = "patronus_view_test.rs"]
pub(crate) mod patronus_view_test;

#[cfg(test)]
#[path = "patronus_view_content_test.rs"]
pub(crate) mod patronus_view_content_test;
