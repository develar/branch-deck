//! The fix of the conflicts of a sync: the regroup after which the conflicting branches apply cleanly, found by trial
//! builds (`check.rs`) and never written (spec/branch/conflict-fix.spec.md). Branch Deck offers it; the user, or an agent
//! with `bd group`, applies it.
//!
//! - **What a commit needs.** The commits it depends on by their lines (`dependencies.rs`), with their fixups and what
//!   they need in turn, that neither its branch nor the branches it is built on have; by the changed files (the missing
//!   commits) only when the lines name none, as for a modify/delete conflict.
//! - **Candidates**, each a move as `move_plan.rs` plans it (a whole issue by the issue groups, a part by a prefix):
//!   move the commit and the commits of its branch that depend on it to the branch whose stack holds what it needs;
//!   pull what it needs into its branch (from a branch stacked on it, from the unassigned commits, or from a branch
//!   where nothing that stays depends on it); restack its branch on that branch.
//! - **A dangling reference** that a trial leaves (a name removed, a use left that a commit outside fixes) extends the
//!   candidate by that commit, so a fix needs no second round for it.
//! - **Verified.** A candidate is taken only when its trial build has more branches that apply cleanly and no branch that
//!   applied cleanly before fails. Among them the best by what the user notices: the failing branches, then the emptied
//!   branches, the commits that change branch or base (a restack changes the base of all its commits), the rewritten
//!   subjects.
//! - **Bounded.** A number of trials per problem and in all, and a time; a search cut short says so (`SearchState`).

use crate::autosquash::plan_autosquash;
use crate::check::{BranchCheck, CheckOutcome, CheckReport, CheckSession};
use crate::dangling_references::uses_added_declarations;
use crate::dependencies::compute_dependencies;
use crate::missing_commits::{changed_files, missing_commits};
use crate::move_plan::plan_move;
use crate::regroup::{RegroupError, RegroupPlan};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, instrument};

/// How much the search may try
#[derive(Debug, Clone, Copy)]
pub struct FixBudget {
  /// Trial builds in all
  pub trials: usize,
  /// Trial builds for one problem, its extensions included
  pub trials_per_problem: usize,
  /// The time of the trials; the first check and the line dependencies before them don't count
  pub time: Duration,
}

impl Default for FixBudget {
  fn default() -> Self {
    Self {
      trials: 24,
      trials_per_problem: 8,
      time: Duration::from_secs(20),
    }
  }
}

/// Whether the search tried every candidate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchState {
  Complete,
  /// It stopped at the trial or time limit: another search may find more
  Budget,
  Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixStepKind {
  /// `commits` leave the conflicting branch for `to`, the branch whose stack has what they need
  Move,
  /// `commits`, what the branch `to` needs, come into it from `from`
  PullIn,
  /// The branch `fixes` is stacked on `to`
  Restack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixStep {
  pub kind: FixStepKind,
  /// Oldest first; empty for a restack
  pub commits: Vec<String>,
  /// The branches the commits leave, an unassigned commit leaves none; the old parent of a restack
  pub from: Vec<String>,
  pub to: String,
  /// The branch whose conflict or dangling reference the step is for
  pub fixes: String,
}

/// Steps that belong together: they share a branch, and their plan alone has the result
#[derive(Debug, Clone)]
pub struct FixComponent {
  pub steps: Vec<FixStep>,
  pub plan: RegroupPlan,
  /// The branches that conflicted (or had a dangling reference) and apply cleanly afterwards
  pub fixed: Vec<String>,
  /// The branches blocked by a parent before that apply cleanly afterwards
  pub unblocked: Vec<String>,
  /// The branches that have no commits afterwards
  pub emptied: Vec<String>,
  /// The branches stacked on an emptied one: built on its nearest ancestor with commits
  pub orphaned_children: Vec<String>,
  /// The subjects the plan rewrites
  pub rewritten: usize,
}

#[derive(Debug, Clone)]
pub struct ConflictFix {
  /// The HEAD the fix is for
  pub head: String,
  pub components: Vec<FixComponent>,
  /// The branches that still conflict or have a dangling reference afterwards
  pub unresolved: Vec<String>,
  pub search: SearchState,
  pub trials: usize,
}

/// A candidate: where the commits go, and the stack parents it sets
#[derive(Debug, Clone, Default)]
struct Change {
  /// Commit id -> the branch it goes to
  moves: BTreeMap<String, String>,
  /// Child -> parent
  restacks: BTreeMap<String, String>,
  steps: Vec<FixStep>,
}

impl Change {
  fn merged(&self, other: &Change) -> Option<Change> {
    let mut merged = self.clone();
    for (id, target) in &other.moves {
      if merged.moves.insert(id.clone(), target.clone()).is_some_and(|previous| previous != *target) {
        return None;
      }
    }
    for (child, parent) in &other.restacks {
      if merged.restacks.insert(child.clone(), parent.clone()).is_some_and(|previous| previous != *parent) {
        return None;
      }
    }
    merged.steps.extend(other.steps.iter().cloned());
    Some(merged)
  }
}

/// The line dependencies of the commits ahead of the baseline, by position (oldest first)
struct Graph {
  ids: Vec<String>,
  position: HashMap<String, usize>,
  /// Position -> the older commits it depends on, and its fixup unit
  needs: Vec<Vec<usize>>,
  /// Position -> the newer commits that depend on it, and its fixup unit
  needed_by: Vec<Vec<usize>>,
  /// Commit id -> the files it changes, old paths of renames included
  files_of: HashMap<String, HashSet<String>>,
}

/// What a report says of the branches
struct Facts<'a> {
  report: &'a CheckReport,
  group_of: HashMap<&'a str, Option<&'a str>>,
}

impl<'a> Facts<'a> {
  fn new(report: &'a CheckReport) -> Self {
    Self {
      report,
      group_of: report.regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.group.as_deref())).collect(),
    }
  }

  fn branch(&self, name: &str) -> Option<&'a BranchCheck> {
    self.report.branches.iter().find(|check| check.name == name)
  }

  fn group(&self, id: &str) -> Option<&'a str> {
    self.group_of.get(id).copied().flatten()
  }

  /// The branch and the branches it is built on, nearest first
  fn chain(&self, name: &str) -> Vec<&'a str> {
    let mut chain: Vec<&'a str> = Vec::new();
    let mut current = self.branch(name).map(|check| check.name.as_str());
    while let Some(branch) = current.filter(|branch| !chain.contains(branch)) {
      chain.push(branch);
      current = self.branch(branch).and_then(|check| check.built_on.as_deref());
    }
    chain
  }

  /// Whether `name` is built on `ancestor`, at any depth
  fn is_built_on(&self, name: &str, ancestor: &str) -> bool {
    self.chain(name).iter().skip(1).any(|branch| *branch == ancestor)
  }

  fn is_clean(&self, check: &BranchCheck) -> bool {
    matches!(check.outcome, CheckOutcome::Ok { .. }) && self.confirmed_dangling(&check.name).is_empty()
  }

  /// `(removed_by, fixed_by)` of the dangling references of `branch` that a commit proves
  fn confirmed_dangling(&self, branch: &str) -> Vec<(&'a str, &'a str)> {
    self
      .report
      .dangling
      .get(branch)
      .into_iter()
      .flatten()
      .filter(|reference| reference.is_confirmed())
      .filter_map(|reference| Some((reference.removed_by.as_str(), reference.fixed_by.as_deref()?)))
      .collect()
  }

  fn bad_count(&self) -> usize {
    self.report.branches.iter().filter(|check| !self.is_clean(check)).count()
  }

  /// The branches with a conflict or a proven dangling reference: the conflicts first (git refuses them, a dangling
  /// reference is a heuristic of a failing build), each in build order
  fn problems(&self) -> Vec<&'a BranchCheck> {
    let conflicts = self.report.branches.iter().filter(|check| matches!(check.outcome, CheckOutcome::Conflict { .. }));
    let dangling = self
      .report
      .branches
      .iter()
      .filter(|check| matches!(check.outcome, CheckOutcome::Ok { .. }) && !self.confirmed_dangling(&check.name).is_empty());
    conflicts.chain(dangling).collect()
  }

  /// The branches whose build a change can alter: the ones it names and every branch built on them
  fn affected_by(&self, change: &Change) -> Vec<String> {
    let named: BTreeSet<&str> = change
      .moves
      .values()
      .map(String::as_str)
      .chain(change.restacks.iter().flat_map(|(child, parent)| [child.as_str(), parent.as_str()]))
      .chain(change.steps.iter().flat_map(|step| step.from.iter().chain([&step.to, &step.fixes]).map(String::as_str)))
      .chain(change.moves.keys().filter_map(|id| self.group(id)))
      .collect();
    self
      .report
      .branches
      .iter()
      .filter(|check| self.chain(&check.name).iter().any(|branch| named.contains(branch)))
      .map(|check| check.name.clone())
      .collect()
  }
}

/// How good a verified candidate is, the least first: the failing branches, the emptied branches, the commits that change
/// branch or base (a restack changes the base of every commit of its branch), the rewritten subjects
type Score = (usize, usize, usize, usize);

impl CheckSession {
  /// The regroup after which the branches that conflict now (or have a proven dangling reference) apply cleanly, as far
  /// as `budget` lets the search go; on top of `base_plan` when given. Trial builds only: writes commit objects and the
  /// object cache
  #[instrument(skip(self, base_plan, cancel))]
  pub fn find_conflict_fix(&self, base_plan: Option<&RegroupPlan>, budget: FixBudget, cancel: &AtomicBool) -> Result<ConflictFix, RegroupError> {
    let base_plan = base_plan.cloned().unwrap_or_default();
    let base_report = self.check_only(Some(&base_plan), None, None)?;
    let mut trials = 0;
    if Facts::new(&base_report).problems().is_empty() {
      return Ok(ConflictFix {
        head: base_report.head.clone(),
        components: Vec::new(),
        unresolved: Vec::new(),
        search: SearchState::Complete,
        trials,
      });
    }
    let graph = self.dependency_graph()?;
    let lines = self.changed_lines()?;
    let started = Instant::now();

    let mut change = Change::default();
    let mut report = base_report.clone();
    // the problems no candidate fixed, by branch and commit: not tried again while the report keeps them
    let mut given_up: HashSet<(String, String)> = HashSet::new();
    let mut search = SearchState::Complete;
    'rounds: loop {
      let facts = Facts::new(&report);
      let problem = facts
        .problems()
        .into_iter()
        .find(|check| !given_up.contains(&(check.name.clone(), problem_commit(&facts, check))));
      let Some(problem) = problem else { break };
      let key = (problem.name.clone(), problem_commit(&facts, problem));
      let mut best: Option<(Score, Change, CheckReport)> = None;
      let mut problem_trials = 0;
      let mut pending: Vec<Change> = candidates(&facts, problem, &graph, lines, &change);
      while let Some(candidate) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
          search = SearchState::Canceled;
          break 'rounds;
        }
        if trials >= budget.trials || started.elapsed() >= budget.time {
          search = SearchState::Budget;
          break 'rounds;
        }
        if problem_trials >= budget.trials_per_problem {
          search = SearchState::Budget;
          break;
        }
        let Some(merged) = change.merged(&candidate) else { continue };
        let Ok(plan) = self.plan_of(&base_plan, &merged) else { continue };
        trials += 1;
        problem_trials += 1;
        // the dangling references of the branches the change can't alter stay as they are
        let affected = facts.affected_by(&merged);
        let Ok(mut trial) = self.check_only(Some(&plan), None, Some(&affected)) else { continue };
        for (branch, references) in &report.dangling {
          if !affected.contains(branch) {
            trial.dangling.insert(branch.clone(), references.clone());
          }
        }
        let trial_facts = Facts::new(&trial);
        // what the branch it fills still needs, a dangling reference's fix or a new conflict's commits: take it along
        if let Some(extension) = extension(&trial_facts, &candidate, &merged, &graph) {
          pending.push(extension);
        }
        let Some(score) = score_of(&facts, &trial_facts, &merged) else { continue };
        debug!(branch = %problem.name, ?score, steps = merged.steps.len(), elapsed = ?started.elapsed(), "verified a candidate");
        if best.as_ref().is_none_or(|(best_score, _, _)| score < *best_score) {
          best = Some((score, merged, trial));
        }
      }
      match best {
        Some((_, merged, trial)) => {
          change = merged;
          report = trial;
        }
        None => {
          given_up.insert(key);
        }
      }
    }

    let components = self.components(&base_plan, &base_report, &report, &change, &mut trials)?;
    let final_facts = Facts::new(&report);
    Ok(ConflictFix {
      head: base_report.head.clone(),
      components,
      unresolved: final_facts.problems().iter().map(|check| check.name.clone()).collect(),
      search,
      trials,
    })
  }

  fn dependency_graph(&self) -> Result<Graph, RegroupError> {
    let commits = &self.snapshot().commits;
    let ids: Vec<String> = commits.iter().map(|commit| commit.id.clone()).collect();
    let position: HashMap<String, usize> = ids.iter().enumerate().map(|(index, id)| (id.clone(), index)).collect();
    let diffs = self.commit_diffs()?;
    let mut needs: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    let mut needed_by: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    for dependency in compute_dependencies(&diffs) {
      needs[dependency.from].push(dependency.to);
      needed_by[dependency.to].push(dependency.from);
    }
    // a fixup unit moves whole: its members need each other
    let snapshot = self.snapshot();
    for group in plan_autosquash(snapshot.head.clone(), commits, &snapshot.grouping).groups {
      let unit: Vec<usize> = std::iter::once(group.target.as_str())
        .chain(group.folded.iter().map(|fixup| fixup.commit_id.as_str()))
        .filter_map(|id| position.get(id).copied())
        .collect();
      for &member in &unit {
        for &other in unit.iter().filter(|other| **other != member) {
          needs[member].push(other);
          needed_by[member].push(other);
        }
      }
    }
    Ok(Graph {
      ids,
      position,
      needs,
      needed_by,
      files_of: changed_files(&diffs),
    })
  }

  /// The regroup of `change` on top of `base_plan`: each target's commits as `plan_move` moves them, and the stack
  /// parents it sets
  fn plan_of(&self, base_plan: &RegroupPlan, change: &Change) -> Result<RegroupPlan, RegroupError> {
    let mut plan = base_plan.clone();
    let mut by_target: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (id, target) in &change.moves {
      by_target.entry(target.as_str()).or_default().push(id.clone());
    }
    for (target, ids) in by_target {
      // a restacked target keeps its new parent in the path
      let path = change.restacks.get(target).map_or_else(|| target.to_string(), |parent| format!("{parent}/{target}"));
      merge_plan(&mut plan, plan_move(self.snapshot(), &ids, Some(&path))?);
    }
    for (child, parent) in &change.restacks {
      plan.branches.entry(format!("{parent}/{child}")).or_default();
    }
    Ok(plan)
  }

  /// The steps grouped into components by the branches they touch; each component's plan alone is checked when there
  /// are several, and the components merge into one when one alone doesn't keep its result
  fn components(&self, base_plan: &RegroupPlan, base: &CheckReport, report: &CheckReport, change: &Change, trials: &mut usize) -> Result<Vec<FixComponent>, RegroupError> {
    if change.steps.is_empty() {
      return Ok(Vec::new());
    }
    let mut groups: Vec<Vec<FixStep>> = Vec::new();
    for step in &change.steps {
      let touches = |other: &FixStep| {
        let names = |step: &FixStep| -> BTreeSet<String> { step.from.iter().chain([&step.to, &step.fixes]).cloned().collect() };
        !names(step).is_disjoint(&names(other)) || step.commits.iter().any(|id| other.commits.contains(id))
      };
      let joined: Vec<usize> = groups.iter().enumerate().filter(|(_, group)| group.iter().any(touches)).map(|(index, _)| index).collect();
      let mut group = vec![step.clone()];
      for index in joined.into_iter().rev() {
        let mut other = groups.remove(index);
        other.append(&mut group);
        group = other;
      }
      groups.push(group);
    }
    let whole = |steps: Vec<FixStep>| -> Result<FixComponent, RegroupError> {
      let plan = self.plan_of(base_plan, &change_of(&steps, change))?;
      Ok(component(base, report, steps, plan))
    };
    if groups.len() == 1 {
      return Ok(vec![whole(groups.remove(0))?]);
    }
    let fixed_by_all = component(base, report, Vec::new(), RegroupPlan::default()).fixed;
    let mut components = Vec::with_capacity(groups.len());
    for steps in groups {
      let plan = self.plan_of(base_plan, &change_of(&steps, change))?;
      *trials += 1;
      let alone = self.check_only(Some(&plan), None, None)?;
      let part = component(base, &alone, steps, plan);
      // a part that alone doesn't fix what it fixes with the others is no part
      let lost = part.steps.iter().any(|step| fixed_by_all.contains(&step.fixes) && !part.fixed.contains(&step.fixes));
      if lost {
        return Ok(vec![whole(change.steps.clone())?]);
      }
      components.push(part);
    }
    Ok(components)
  }
}

/// The commit a problem is about: the conflicting commit, or the first commit a dangling reference names
fn problem_commit(facts: &Facts, check: &BranchCheck) -> String {
  match &check.outcome {
    CheckOutcome::Conflict { commit, .. } => commit.clone(),
    _ => facts
      .confirmed_dangling(&check.name)
      .first()
      .map_or_else(String::new, |(removed_by, _)| (*removed_by).to_string()),
  }
}

/// The commit `commit` of `branch` and the commits of the branch that depend on it, by their lines or by the names it
/// declares, oldest first
fn dependents_in(graph: &Graph, lines: &HashMap<String, Vec<object_cache::commit_lines::FileLines>>, branch: &BranchCheck, commit: &str) -> Vec<String> {
  let own: HashSet<&str> = branch.commits.iter().map(String::as_str).collect();
  let mut moved: BTreeSet<usize> = BTreeSet::new();
  let mut pending: Vec<usize> = graph.position.get(commit).copied().into_iter().collect();
  while let Some(position) = pending.pop() {
    if moved.insert(position) {
      pending.extend(graph.needed_by[position].iter().copied().filter(|other| own.contains(graph.ids[*other].as_str())));
    }
  }
  // a commit that stays and uses a name the moved ones add would lose its declaration
  loop {
    let moved_lines = moved.iter().flat_map(|position| lines.get(&graph.ids[*position]).into_iter().flatten());
    let staying: Vec<usize> = branch
      .commits
      .iter()
      .filter_map(|id| graph.position.get(id).copied())
      .filter(|position| !moved.contains(position))
      .collect();
    let users: Vec<usize> = staying
      .into_iter()
      .filter(|position| uses_added_declarations(moved_lines.clone(), lines.get(&graph.ids[*position]).into_iter().flatten()))
      .collect();
    if users.is_empty() {
      break;
    }
    moved.extend(users);
  }
  moved.into_iter().map(|position| graph.ids[position].clone()).collect()
}

/// Whether `needed` can leave its branches for `target`: no commit left in them, or in a branch built on them other
/// than `target` and the branches built on it, depends on one of them by its lines or by a name it declares
fn movable(facts: &Facts, graph: &Graph, lines: &HashMap<String, Vec<object_cache::commit_lines::FileLines>>, needed: &[String], target: &str) -> bool {
  let needed_positions: HashSet<usize> = needed.iter().filter_map(|id| graph.position.get(id).copied()).collect();
  let owners: BTreeSet<&str> = needed.iter().filter_map(|id| facts.group(id)).collect();
  let holds = |group: &str| group != target && !facts.is_built_on(group, target) && owners.iter().any(|owner| group == *owner || facts.is_built_on(group, owner));
  let staying: Vec<usize> = (0..graph.ids.len())
    .filter(|position| !needed_positions.contains(position))
    .filter(|position| facts.group(&graph.ids[*position]).is_some_and(holds))
    .collect();
  let by_lines = staying.iter().any(|position| graph.needs[*position].iter().any(|to| needed_positions.contains(to)));
  if by_lines {
    return false;
  }
  let moved_lines = needed.iter().flat_map(|id| lines.get(id).into_iter().flatten());
  !uses_added_declarations(moved_lines, staying.iter().flat_map(|position| lines.get(&graph.ids[*position]).into_iter().flatten()))
}

/// The candidates for the problem of `branch`, the one to try first last
fn candidates(facts: &Facts, branch: &BranchCheck, graph: &Graph, lines: &HashMap<String, Vec<object_cache::commit_lines::FileLines>>, done: &Change) -> Vec<Change> {
  let stacks = facts.report.stacks;
  let present: HashSet<&str> = facts.chain(&branch.name).into_iter().collect();
  let mut candidates: Vec<Change> = Vec::new();
  match &branch.outcome {
    CheckOutcome::Conflict { commit, files } => {
      let mut needed = needed_commits(facts, graph, commit, &present);
      if needed.is_empty() {
        needed = missing_commits(facts.report, branch, commit, files, &graph.files_of);
      }
      if needed.is_empty() {
        return candidates;
      }
      let owners: BTreeSet<&str> = needed.iter().filter_map(|id| facts.group(id)).collect();
      let unassigned: Vec<String> = needed.iter().filter(|id| facts.group(id).is_none()).cloned().collect();
      let top = stack_top(facts, &owners, stacks);

      // restack the branch on the branch that has what it needs: no rewrite, but the base of its pull request changes
      if stacks
        && unassigned.is_empty()
        && let Some(top) = top
        && top != branch.name
        && !facts.is_built_on(top, &branch.name)
      {
        let mut change = Change::default();
        change.restacks.insert(branch.name.clone(), top.to_string());
        change.steps.push(FixStep {
          kind: FixStepKind::Restack,
          commits: Vec::new(),
          from: branch.built_on.iter().cloned().collect(),
          to: top.to_string(),
          fixes: branch.name.clone(),
        });
        candidates.push(change);
      }

      // pull what it needs into the branch, when nothing left behind depends on it
      if needed.len() <= 6 && movable(facts, graph, lines, &needed, &branch.name) {
        let mut change = Change::default();
        for id in &needed {
          change.moves.insert(id.clone(), branch.name.clone());
        }
        change.steps.push(FixStep {
          kind: FixStepKind::PullIn,
          commits: needed.clone(),
          from: owners.iter().map(|owner| (*owner).to_string()).collect(),
          to: branch.name.clone(),
          fixes: branch.name.clone(),
        });
        candidates.push(change);
      }

      // move the commit, and what of its branch depends on it, to that branch, the unassigned commits it needs too
      if let Some(top) = top.filter(|top| *top != branch.name) {
        let moved = dependents_in(graph, lines, branch, commit);
        if moved.len() < branch.commits.len() || !stacks {
          let mut change = Change::default();
          for id in moved.iter().chain(&unassigned) {
            change.moves.insert(id.clone(), top.to_string());
          }
          if !unassigned.is_empty() {
            change.steps.push(FixStep {
              kind: FixStepKind::PullIn,
              commits: unassigned.clone(),
              from: Vec::new(),
              to: top.to_string(),
              fixes: branch.name.clone(),
            });
          }
          change.steps.push(FixStep {
            kind: FixStepKind::Move,
            commits: moved,
            from: vec![branch.name.clone()],
            to: top.to_string(),
            fixes: branch.name.clone(),
          });
          candidates.push(change);
        }
      }
    }
    CheckOutcome::Ok { .. } => {
      let references = facts.confirmed_dangling(&branch.name);
      // the commit that fixes the use comes into the branch
      let fixes: Vec<&str> = references.iter().map(|(_, fixed_by)| *fixed_by).collect();
      if let Some(change) = pull_in_change(facts, graph, &fixes, &branch.name, done) {
        candidates.push(change);
      }
      // or the commits that remove the declarations go to the branch whose stack has the fixes
      let owners: BTreeSet<&str> = fixes.iter().filter_map(|id| facts.group(id)).collect();
      if fixes.iter().all(|id| facts.group(id).is_some())
        && let Some(top) = stack_top(facts, &owners, stacks).filter(|top| *top != branch.name)
      {
        let mut moved: BTreeSet<String> = BTreeSet::new();
        for (removed_by, _) in &references {
          moved.extend(dependents_in(graph, lines, branch, removed_by));
        }
        if moved.len() < branch.commits.len() {
          let moved: Vec<String> = graph.ids.iter().filter(|id| moved.contains(*id)).cloned().collect();
          let mut change = Change::default();
          for id in &moved {
            change.moves.insert(id.clone(), top.to_string());
          }
          change.steps.push(FixStep {
            kind: FixStepKind::Move,
            commits: moved,
            from: vec![branch.name.clone()],
            to: top.to_string(),
            fixes: branch.name.clone(),
          });
          candidates.push(change);
        }
      }
    }
    _ => {}
  }
  // tried last to first: the move first, then the pull, then the restack
  candidates
}

/// The commits `commit` needs by its lines that `present` (its branch and the branches it is built on) lacks, and what
/// those need in turn, oldest first
fn needed_commits(facts: &Facts, graph: &Graph, commit: &str, present: &HashSet<&str>) -> Vec<String> {
  let Some(&start) = graph.position.get(commit) else { return Vec::new() };
  let has = |position: usize| facts.group(&graph.ids[position]).is_some_and(|group| present.contains(group));
  let mut needed: BTreeSet<usize> = BTreeSet::new();
  let mut pending = vec![start];
  while let Some(position) = pending.pop() {
    for &to in &graph.needs[position] {
      if to != start && !has(to) && needed.insert(to) {
        pending.push(to);
      }
    }
  }
  needed.into_iter().map(|position| graph.ids[position].clone()).collect()
}

/// The branch whose stack holds every owner: an owner built on all the others, the deepest; with stacked branches off
/// only a single owner
fn stack_top<'a>(facts: &Facts<'a>, owners: &BTreeSet<&'a str>, stacks: bool) -> Option<&'a str> {
  if owners.is_empty() {
    return None;
  }
  if !stacks {
    return (owners.len() == 1).then(|| *owners.iter().next().unwrap_or(&""));
  }
  owners
    .iter()
    .copied()
    .filter(|owner| {
      let chain = facts.chain(owner);
      owners.iter().all(|other| chain.contains(other))
    })
    .max_by_key(|owner| facts.chain(owner).len())
}

/// The fixing commits of the dangling references that are not in `branch` or moving already, pulled into it
fn pull_in_change(facts: &Facts, graph: &Graph, fixes: &[&str], branch: &str, done: &Change) -> Option<Change> {
  let present: HashSet<&str> = facts.chain(branch).into_iter().collect();
  let mut commits: BTreeSet<usize> = BTreeSet::new();
  for id in fixes {
    if done.moves.get(*id).is_some_and(|target| target == branch) || facts.group(id).is_some_and(|group| present.contains(group)) {
      continue;
    }
    if let Some(&position) = graph.position.get(*id) {
      commits.insert(position);
    }
  }
  if commits.is_empty() {
    return None;
  }
  let ids: Vec<String> = commits.into_iter().map(|position| graph.ids[position].clone()).collect();
  let from: BTreeSet<String> = ids.iter().filter_map(|id| facts.group(id)).map(str::to_string).collect();
  let mut change = Change::default();
  for id in &ids {
    change.moves.insert(id.clone(), branch.to_string());
  }
  change.steps.push(FixStep {
    kind: FixStepKind::PullIn,
    commits: ids,
    from: from.into_iter().collect(),
    to: branch.to_string(),
    fixes: branch.to_string(),
  });
  Some(change)
}

/// The candidate extended by what its trial says the branches it moves commits into still need: the commits that fix
/// their dangling references, and the commits a new conflict of theirs needs (at most 6); `None` when there are none
fn extension(trial: &Facts, candidate: &Change, merged: &Change, graph: &Graph) -> Option<Change> {
  let targets: BTreeSet<&str> = candidate
    .moves
    .values()
    .map(String::as_str)
    .chain(candidate.steps.iter().map(|step| step.fixes.as_str()))
    .collect();
  let mut extended = candidate.clone();
  let mut any = false;
  for target in targets {
    let mut wanted: Vec<String> = trial.confirmed_dangling(target).into_iter().map(|(_, fixed_by)| fixed_by.to_string()).collect();
    if let Some(check) = trial.branch(target)
      && let CheckOutcome::Conflict { commit, files } = &check.outcome
    {
      let present: HashSet<&str> = trial.chain(target).into_iter().collect();
      let mut needed = needed_commits(trial, graph, commit, &present);
      if needed.is_empty() {
        needed = missing_commits(trial.report, check, commit, files, &graph.files_of);
      }
      if needed.len() <= 6 {
        wanted.extend(needed);
      }
    }
    let wanted: Vec<&str> = wanted.iter().map(String::as_str).collect();
    if let Some(pull) = pull_in_change(trial, graph, &wanted, target, merged) {
      extended = extended.merged(&pull)?;
      any = true;
    }
  }
  any.then_some(extended)
}

/// The score of a trial against the current report; `None` when it breaks a clean branch or fixes nothing
fn score_of(current: &Facts, trial: &Facts, change: &Change) -> Option<Score> {
  for check in &current.report.branches {
    if current.is_clean(check)
      && let Some(after) = trial.branch(&check.name)
      && !trial.is_clean(after)
    {
      return None;
    }
  }
  let bad = trial.bad_count();
  if bad >= current.bad_count() {
    return None;
  }
  let emptied = current.report.branches.iter().filter(|check| trial.branch(&check.name).is_none()).count();
  let restacked: usize = change.restacks.keys().filter_map(|child| current.branch(child)).map(|check| check.commits.len()).sum();
  Some((bad, emptied, change.moves.len() + restacked, trial.report.regroup.changed_count()))
}

/// The part of `change` that `steps` make
fn change_of(steps: &[FixStep], change: &Change) -> Change {
  let mut part = Change {
    steps: steps.to_vec(),
    ..Change::default()
  };
  for step in steps {
    match step.kind {
      FixStepKind::Restack => {
        if let Some(parent) = change.restacks.get(&step.fixes) {
          part.restacks.insert(step.fixes.clone(), parent.clone());
        }
      }
      FixStepKind::Move | FixStepKind::PullIn => {
        for id in &step.commits {
          if let Some(target) = change.moves.get(id) {
            part.moves.insert(id.clone(), target.clone());
          }
        }
        // a move into a restacked branch keeps its parent
        if let Some(parent) = change.restacks.get(&step.to) {
          part.restacks.insert(step.to.clone(), parent.clone());
        }
      }
    }
  }
  part
}

/// The result of `plan` (checked as `after`) against `base`
fn component(base: &CheckReport, after: &CheckReport, steps: Vec<FixStep>, plan: RegroupPlan) -> FixComponent {
  let (base_facts, after_facts) = (Facts::new(base), Facts::new(after));
  let clean_after = |name: &str| after_facts.branch(name).is_some_and(|check| after_facts.is_clean(check));
  let fixed = base_facts.problems().iter().map(|check| check.name.clone()).filter(|name| clean_after(name)).collect();
  let unblocked = base
    .branches
    .iter()
    .filter(|check| matches!(check.outcome, CheckOutcome::Blocked { .. }))
    .map(|check| check.name.clone())
    .filter(|name| clean_after(name))
    .collect();
  let emptied = base
    .branches
    .iter()
    .map(|check| check.name.clone())
    .filter(|name| after_facts.branch(name).is_none())
    .collect();
  FixComponent {
    steps,
    plan,
    fixed,
    unblocked,
    emptied,
    orphaned_children: after.regroup.orphaned_children.clone(),
    rewritten: after.regroup.changed_count(),
  }
}

/// `other` added to `plan`
fn merge_plan(plan: &mut RegroupPlan, other: RegroupPlan) {
  for (path, ids) in other.branches {
    let entry = plan.branches.entry(path).or_default();
    for id in ids {
      if !entry.contains(&id) {
        entry.push(id);
      }
    }
  }
  for (name, keys) in other.issue_groups {
    let entry = plan.issue_groups.entry(name).or_default();
    for key in keys {
      if !entry.contains(&key) {
        entry.push(key);
      }
    }
  }
  for (list, items) in [
    (&mut plan.unassign, other.unassign),
    (&mut plan.ungroup_issues, other.ungroup_issues),
    (&mut plan.unstack, other.unstack),
  ] {
    for item in items {
      if !list.contains(&item) {
        list.push(item);
      }
    }
  }
}

#[cfg(test)]
#[path = "conflict_fix_test.rs"]
mod conflict_fix_test;
