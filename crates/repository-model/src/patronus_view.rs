//! What the robots land and landed of the branch table, the facts the rules of the holds and the starts decide over
//! (`derive::decide_landing`, `domain_policy::holds`, `domain_policy::land`): the one derivation of the app (the session
//! derives it, `facts::derive_patronus`) and of `bd` (`brd push`, `brd safe-push`, `brd status`),
//! openspec/specs/branch/patronus/spec.md and docs/design/git-operations.md, "Session facts".
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
//! - One function makes every view (`view_of`): over the table, the remote branches and the robots, by the hashes and the
//!   answers of the content the memo keeps, no git. A derivation asks git first what the memo lacks (`learn_content`),
//!   then makes the view of the same inputs (`patronus_view`); the session makes it again whenever the robots, the table
//!   or the remote branches change before the derivation of them answers, or after it failed
//!   (`facts::PatronusFact::publish`), so every reader reads the robots as they are.

use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use crate::derive::decide_landing;
use crate::facts::PatronusOf;
use domain_policy::facts::RemoteBranch;
use domain_policy::facts::Repository;
use domain_policy::holds::{holds_pushes, is_active, is_pushing};
use domain_policy::land::took_as_is;
use domain_policy::landing_needs::name_landing_robots;
use domain_policy::robot_match::{RobotIndex, RobotMatches, Tips, match_table};
use domain_policy::stack::build_ancestors;
use domain_types::landing::{LandingRefusal, Subject};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::{BranchPatronus, Carried, HoldKind, LandedByView, LandingNeeds, PatronusView, RobotLandedBranch, RobotOf, SpanView, StackPatronus};
use domain_types::robot_landing::{BranchLanding, BranchSpan, RobotLandingRequest};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};
use sync_core::robot_landing::{robot_landings_of_grouped, robot_spans_of_grouped};
use tracing::{debug, instrument};

/// How many answers a memo keeps of each kind before it starts again
const MEMO_LIMIT: usize = 512;

/// What the content of the robots' commits told the derivations (`learn_content`), by the robot and the state it covers
/// (`content_key`: the robot, its commit, the tip of its subject and the commits of the branches with their copies): a
/// derivation of the same state runs no git, and every view reads its answers (`view_of`); of each merged robot also its
/// latest answer, which a view of a state not asked yet reads. Shared by the views of a session and the checks of its
/// pushes, kept in the frame of the session (`warm_start`), locked only to look up and to keep answers, never across git
#[derive(Debug, Default)]
pub struct ContentMemo {
  answers: Mutex<MemoAnswers>,
}

/// The answers of a memo, as a frame keeps them
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoAnswers {
  /// What a merged robot landed, by `content_key`
  landings: HashMap<String, Vec<BranchLanding>>,
  /// What the commit of a running robot has of the branches it may land, by `content_key`
  spans: HashMap<String, Option<Vec<BranchSpan>>>,
  /// By merged robot id, the key of its latest answer in `landings`
  latest_landing: HashMap<String, String>,
  /// Raised by every answer kept: a view of the same inputs and version is the same view
  #[serde(skip)]
  version: u64,
}

impl MemoAnswers {
  /// What the merged robot `robot_id` landed of the branches as `key` names them, else of the branches as they were for
  /// its latest answer
  fn landing(&self, key: &str, robot_id: &str) -> Option<&Vec<BranchLanding>> {
    self
      .landings
      .get(key)
      .or_else(|| self.latest_landing.get(robot_id).and_then(|latest| self.landings.get(latest)))
  }
}

impl ContentMemo {
  /// A memo of the answers a frame kept
  #[must_use]
  pub const fn seeded(answers: MemoAnswers) -> Self {
    Self { answers: Mutex::new(answers) }
  }

  /// The answers; poisoned ones (a panic while they were locked) start again
  fn answers(&self) -> MutexGuard<'_, MemoAnswers> {
    self.answers.lock().unwrap_or_else(|poisoned| {
      self.answers.clear_poison();
      let mut answers = poisoned.into_inner();
      *answers = MemoAnswers::default();
      answers
    })
  }

  /// The answers as a frame keeps them
  #[must_use]
  pub fn frame(&self) -> MemoAnswers {
    self.answers().clone()
  }

  /// The version of the answers: raised by every answer kept
  #[must_use]
  pub fn version(&self) -> u64 {
    self.answers().version
  }

  /// Keeps what merged robots landed, by robot id and key, each the robot's latest answer; past `MEMO_LIMIT` only the
  /// latest answer of each robot stays
  fn keep_landings(&self, answered: impl IntoIterator<Item = (String, String, Vec<BranchLanding>)>) {
    let mut kept = self.answers();
    if kept.landings.len() > MEMO_LIMIT {
      let latest: HashSet<String> = kept.latest_landing.values().cloned().collect();
      kept.landings.retain(|key, _| latest.contains(key));
      if kept.landings.len() > MEMO_LIMIT {
        kept.landings.clear();
        kept.latest_landing.clear();
      }
    }
    for (robot_id, key, branches) in answered {
      kept.landings.insert(key.clone(), branches);
      kept.latest_landing.insert(robot_id, key);
    }
    kept.version += 1;
  }

  /// Keeps what the commits of running robots have, by key; past `MEMO_LIMIT` it starts again
  fn keep_spans(&self, answered: impl IntoIterator<Item = (String, Option<Vec<BranchSpan>>)>) {
    let mut kept = self.answers();
    if kept.spans.len() > MEMO_LIMIT {
      kept.spans.clear();
    }
    kept.spans.extend(answered);
    kept.version += 1;
  }
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

/// What tells what a robot did: the hashes, else the content (its question)
enum Told<T> {
  Hashes(T),
  Content(Question),
}

/// A question by content about a robot (`content_request`), with the key of its answer (`content_key`): the robot, its
/// commit and the commits of the branches it covers as they are
struct Question {
  key: String,
  request: RobotLandingRequest,
}

impl Question {
  fn of(table: &BranchTable, robot: &PatronusRobot, subject: &Subject, source: &str, branches: Vec<String>) -> Self {
    let request = content_request(table, robot, subject, source, branches);
    Self {
      key: content_key(table, &request),
      request,
    }
  }
}

/// The questions of `told` whose answers `known` lacks, each once
fn unasked<'q, T: 'q>(told: impl IntoIterator<Item = &'q Told<T>>, known: impl Fn(&str) -> bool) -> Vec<&'q Question> {
  let mut asked: Vec<&Question> = Vec::new();
  for told in told {
    if let Told::Content(question) = told
      && !known(&question.key)
      && !asked.iter().any(|it| it.key == question.key)
    {
      asked.push(question);
    }
  }
  asked
}

/// A merged robot of a view: its subject, the question of what it landed of the branches as they are, and what the
/// hashes tell of it (`None`: only the content does)
struct MergedPlan<'r> {
  robot: &'r PatronusRobot,
  subject: Subject,
  question: Question,
  hashes: Option<Vec<BranchLanding>>,
}

/// What one merged robot landed, before the merge of all robots
struct RobotLanded<'r> {
  robot: &'r PatronusRobot,
  by: RobotOf,
  subject_branch: Option<String>,
  branches: Vec<BranchLanding>,
}

/// The merged robots of `robots` that landed a subject of `table`, in the order they merged, each with its question by
/// content and what the hashes tell it landed
fn merged_plans<'r>(table: &BranchTable, repo: &Repository<'_>, remote: &RemoteFacts, robots: &'r [PatronusRobot]) -> Vec<MergedPlan<'r>> {
  let mut merged: Vec<&PatronusRobot> = robots
    .iter()
    .filter(|robot| robot.status == PatronusRobotStatus::Success && is_pushing(robot) && robot.source_revision.is_some())
    .collect();
  merged.sort_by_key(|robot| robot.finish_time.unwrap_or(0));
  merged
    .into_iter()
    .filter_map(|robot| {
      let source = robot.source_revision.as_deref().unwrap_or_default();
      let subject = match (&robot.branch_name, &robot.stack_root) {
        (Some(name), _) if table.branch(name).is_some() => Subject::Branch { name: name.clone() },
        (None, Some(root)) if table.stack(root).is_some() => Subject::WholeStack { root: root.clone() },
        _ => return None,
      };
      let candidates: Vec<String> = match &subject {
        Subject::Branch { name } => ancestors(repo, name).into_iter().chain(std::iter::once(name.clone())).collect(),
        Subject::WholeStack { root } => table.stack(root).map_or_else(Vec::new, |stack| stack.members.clone()),
      };
      let hashes = hashes_tell(table, remote, &subject, source).map(|above| {
        let own = match &subject {
          Subject::Branch { name } => Some(name.as_str()),
          Subject::WholeStack { .. } => None,
        };
        candidates
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
          .collect()
      });
      let question = Question::of(table, robot, &subject, source, candidates);
      Some(MergedPlan { robot, subject, question, hashes })
    })
    .collect()
}

/// What the merged robots of `plans` landed, by the hashes, else by the answer `kept` has (`MemoAnswers::landing`: of
/// these branches as they are, else the robot's latest), none without one
fn landed_of<'r>(plans: Vec<MergedPlan<'r>>, kept: &MemoAnswers) -> Vec<RobotLanded<'r>> {
  plans
    .into_iter()
    .map(|MergedPlan { robot, subject, question, hashes }| {
      let branches = hashes.unwrap_or_else(|| kept.landing(&question.key, &robot.id).cloned().unwrap_or_default());
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

/// A running robot that pushes, with the branches it may land and what tells which of them its commit has
/// (`Hashes(None)`: just started, its commit not known yet)
struct SpanPlan<'r> {
  robot: &'r PatronusRobot,
  subject: Subject,
  candidates: Vec<String>,
  told: Told<Option<HashMap<String, Carried>>>,
}

/// What a running robot lands of its candidates: the branches, in their order, and the commits of each its commit has;
/// `carried` is None when only the stack as it is now tells
struct RobotSpanFacts<'r> {
  robot: &'r PatronusRobot,
  subject: Subject,
  names: Vec<String>,
  carried: Option<HashMap<String, Carried>>,
}

/// The running robots that push and land a branch of `table`: the robot of each branch with ancestors and of each whole
/// stack, in the order of the table, the stacks after, each with what the hashes tell, else its question by content
fn span_plans<'r>(table: &BranchTable, repo: &Repository<'_>, remote: &RemoteFacts, lookups: &Lookups<'r>) -> Vec<SpanPlan<'r>> {
  let lands = |robot: &&PatronusRobot| is_active(robot) && is_pushing(robot);
  let mut subjects: Vec<(&PatronusRobot, Subject, Vec<String>)> = Vec::new();
  for branch in &table.branches {
    let ancestors = ancestors(repo, &branch.name);
    if let Some(robot) = lookups.robot_of(&branch.name).filter(lands)
      && !ancestors.is_empty()
    {
      subjects.push((robot, Subject::Branch { name: branch.name.clone() }, ancestors));
    }
  }
  for stack in &table.stack_branches {
    if let Some(robot) = lookups.stack_robot_of(&stack.root).filter(lands)
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
  subjects
    .into_iter()
    .map(|(robot, subject, candidates)| {
      let told = match robot.source_revision.as_deref() {
        // just started: its commit is not known yet
        None => Told::Hashes(None),
        Some(source) if hashes_tell(table, remote, &subject, source).is_some() => Told::Hashes(Some(all_commits(&candidates))),
        Some(source) => {
          let branches = match &subject {
            Subject::Branch { name } => candidates.iter().cloned().chain(std::iter::once(name.clone())).collect(),
            Subject::WholeStack { .. } => candidates.clone(),
          };
          Told::Content(Question::of(table, robot, &subject, source, branches))
        }
      };
      SpanPlan { robot, subject, candidates, told }
    })
    .collect()
}

/// What the running robots of `plans` land, by the hashes, else by the answer `kept` has of the branches as they are;
/// without one, what the stack as it is now says (`carried` None, every candidate)
fn spans_of<'r>(plans: Vec<SpanPlan<'r>>, kept: &MemoAnswers) -> Vec<RobotSpanFacts<'r>> {
  plans
    .into_iter()
    .map(|SpanPlan { robot, subject, candidates, told }| {
      let carried = match told {
        Told::Hashes(carried) => carried,
        Told::Content(question) => kept.spans.get(&question.key).cloned().flatten().map(|branches| {
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

/// The robots of a view by id, with the robot of each branch and whole stack (`match_robots`)
struct Lookups<'r> {
  by_id: HashMap<&'r str, &'r PatronusRobot>,
  matched: &'r RobotMatches,
}

impl<'r> Lookups<'r> {
  fn new(robots: &'r [PatronusRobot], matched: &'r RobotMatches) -> Self {
    Self {
      by_id: robots.iter().map(|robot| (robot.id.as_str(), robot)).collect(),
      matched,
    }
  }

  /// The robot of the branch `name`
  fn robot_of(&self, name: &str) -> Option<&'r PatronusRobot> {
    self.matched.branches.get(name).and_then(|id| self.by_id.get(id.as_str()).copied())
  }

  /// The robot of the whole stack of `root`
  fn stack_robot_of(&self, root: &str) -> Option<&'r PatronusRobot> {
    self.matched.stacks.get(root).and_then(|id| self.by_id.get(id.as_str()).copied())
  }
}

/// The branches `name` is built on in `repo` (the rules' input of a table, built once per view), root first
fn ancestors(repo: &Repository<'_>, name: &str) -> Vec<String> {
  build_ancestors(repo, name).into_iter().map(str::to_string).collect()
}

/// What the content of the commits of the robots of `of` tells that `memo` lacks (module docs): one call for the merged
/// robots and one for the running ones, their answers kept in `memo`; what the hashes tell a merged robot landed is kept
/// as its latest answer too, which a view of a state the hashes no longer answer reads. A git error is no answer: the
/// next derivation asks again, and the view reads what `memo` had (`view_of`)
fn learn_content(git_executor: &GitCommandExecutor, repository_path: &str, of: &PatronusOf<'_>, matched: &RobotMatches, memo: &ContentMemo) {
  let PatronusOf { table, remote, robots, .. } = *of;
  let (lookups, repo) = (Lookups::new(robots, matched), table.repository());
  let merged = merged_plans(table, &repo, remote, robots);
  let spans = span_plans(table, &repo, remote, &lookups);
  // the memo is locked for the lookups only, never across git
  let (told_by_hashes, landing_asks, span_asks) = {
    let kept = memo.answers();
    let told_by_hashes: Vec<(String, String, Vec<BranchLanding>)> = merged
      .iter()
      .filter(|plan| kept.latest_landing.get(&plan.robot.id) != Some(&plan.question.key))
      .filter_map(|plan| Some((plan.robot.id.clone(), plan.question.key.clone(), plan.hashes.clone()?)))
      .collect();
    let mut landing_asks: Vec<&Question> = Vec::new();
    for plan in merged.iter().filter(|plan| plan.hashes.is_none() && !kept.landings.contains_key(&plan.question.key)) {
      if !landing_asks.iter().any(|it| it.key == plan.question.key) {
        landing_asks.push(&plan.question);
      }
    }
    (
      told_by_hashes,
      landing_asks,
      unasked(spans.iter().map(|plan| &plan.told), |key| kept.spans.contains_key(key)),
    )
  };
  if !told_by_hashes.is_empty() {
    memo.keep_landings(told_by_hashes);
  }
  if !landing_asks.is_empty() {
    let requests: Vec<RobotLandingRequest> = landing_asks.iter().map(|question| question.request.clone()).collect();
    match robot_landings_of_grouped(git_executor, repository_path, &table.prefix, &table.baseline, &requests, &table.branches) {
      Ok(answers) => memo.keep_landings(landing_asks.iter().map(|question| {
        let branches = answers
          .iter()
          .find(|landing| landing.robot_id == question.request.robot_id)
          .map_or_else(Vec::new, |landing| landing.branches.clone());
        (question.request.robot_id.clone(), question.key.clone(), branches)
      })),
      Err(error) => debug!(error = %format!("{error:#}"), "no robot landings: the view reads what the memo had"),
    }
  }
  if !span_asks.is_empty() {
    let requests: Vec<RobotLandingRequest> = span_asks.iter().map(|question| question.request.clone()).collect();
    match robot_spans_of_grouped(git_executor, repository_path, &table.prefix, &table.baseline, &requests, &table.branches) {
      Ok(answers) => memo.keep_spans(span_asks.iter().zip(answers).map(|(question, answer)| (question.key.clone(), answer.branches))),
      // the stack as it is now holds, asked again by the next derivation
      Err(error) => debug!(error = %format!("{error:#}"), "no robot spans: the stack as it is now tells"),
    }
  }
}

/// What the robots land and landed of the branches of the table of `of` (module docs), with the robot of each branch and
/// whole stack (`matched`, `match_robots`), and what the rules decide over these facts (`derive::decide_landing`: the
/// holds, the starts, the restarts): the content of the robots' commits the hashes don't answer and `memo` lacks asked
/// of git and kept in `memo` (`learn_content`), then the view of them (`view_of`). The robots are the user's (only those
/// that land or landed change what is held), the remote branches what the deriver knows of them
#[instrument(skip_all, fields(robots = of.robots.len(), branches = of.table.branches.len()))]
pub fn patronus_view(git_executor: &GitCommandExecutor, repository_path: &str, of: &PatronusOf<'_>, matched: &RobotMatches, memo: &ContentMemo) -> PatronusView {
  learn_content(git_executor, repository_path, of, matched, memo);
  view_of(of, matched, memo)
}

/// The view of `of` (module docs), with the robot of each branch and whole stack (`matched`), by the hashes and what
/// the content of the robots' commits told as `memo` keeps it, no git: the one function of every view, of the
/// derivation after it asked git what `memo` lacked (`patronus_view`), and of the session over the robots, the table and
/// the remote branches as they are while a derivation of them runs or failed (`facts::PatronusFact::publish`). A merged
/// robot `memo` has no answer of for the branches as they are landed what it landed of them as they were (its latest
/// answer, which `LandedByView::knows` and `is_whole_of` read against the branches as they are), nothing without one; a
/// running one lands what the stack as it is now says (`carried` None)
#[must_use]
pub fn view_of(of: &PatronusOf<'_>, matched: &RobotMatches, memo: &ContentMemo) -> PatronusView {
  let PatronusOf {
    table,
    remote,
    robots,
    needs_review,
    failing,
  } = *of;
  let (lookups, repo) = (Lookups::new(robots, matched), table.repository());
  let robot_of = |name: &str| lookups.robot_of(name);
  let lands = |robot: &PatronusRobot| is_active(robot) && is_pushing(robot);
  let (landings, spans) = {
    let kept = memo.answers();
    (
      landed_of(merged_plans(table, &repo, remote, robots), &kept),
      spans_of(span_plans(table, &repo, remote, &lookups), &kept),
    )
  };

  // landed: of several robots the one that left the fewest commits after it, of equals the first merge
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
    let landing_ancestor = ancestors(&repo, name)
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
  decide_landing(&mut view, table, remote, robots, &hold_span_of, needs_review, failing);
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
  let landing = landing_robots(view);
  for branch in &table.branches {
    let mut refusals = needs.get(&branch.name).cloned().unwrap_or_default();
    name_landing_robots(&mut refusals, |declared_in| landing.get(declared_in).cloned());
    view.branches.entry(branch.name.clone()).or_default().landing_needs = LandingNeeds::Known { refusals };
  }
}

/// Branch -> the robot that lands it now, as `view` holds it: its own running Safe Push, else the one of a start that
/// lands it (of a branch built on it, of its whole stack)
#[must_use]
pub fn landing_robots(view: &PatronusView) -> HashMap<String, RobotOf> {
  view
    .branches
    .iter()
    .filter_map(|(name, facts)| {
      let running = facts.landing.hold.as_ref().filter(|hold| hold.kind != HoldKind::Landed).map(|hold| hold.by.clone());
      Some((name.clone(), running.or_else(|| facts.span.as_ref().map(|span| span.by.clone()))?))
    })
    .collect()
}

#[cfg(test)]
#[path = "patronus_view_test.rs"]
pub(crate) mod patronus_view_test;

#[cfg(test)]
#[path = "patronus_view_content_test.rs"]
pub(crate) mod patronus_view_content_test;
