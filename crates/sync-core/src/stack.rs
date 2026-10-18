//! Stacked branches: `(auth/login) X` puts the commit into `login` and declares `auth` as the parent of `login`;
//! `(auth/login/oauth) X` also declares `auth` as the parent of `login`. A branch whose prefixes declare no parent takes
//! its stack parent (`stack_parents.rs`). Opt-in per repository (`git config branchdeck.stacks true`), because without
//! it a `/` in a prefix is part of a flat branch name.

use crate::config_snapshot::ConfigSnapshot;
use crate::stack_parents::STACK_PARENTS_REF;
use anyhow::Result;
use branch_integration::archive::ARCHIVE_RETENTION_DAYS;
use branch_integration::cache::{CachedDetection, NOTES_REF, deserialize_from_cache};
use branch_integration::common::ListedBranch;
use branch_integration::detector::{BaselineAncestry, CachePlan, PendingBranch, archived_within_retention, detect_pending_branches, resolve_baseline};
use branch_integration::strategy::{DetectionStrategy, get_detection_strategy};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::{BranchKind, sanitize_branch_name, strip_branch_namespace};
use git_ops::notes::read_notes;
use git_ops::revs::short_id;
use indexmap::{IndexMap, IndexSet};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use sync_types::branch_integration::BranchIntegrationStatus;
use sync_types::stack::{BlockReason, BranchStack, DeclaredParent, ParentSource, ParentState, StackBlock, StackProblem};

pub const STACKS_CONFIG_KEY: &str = "branchdeck.stacks";

/// Whether stacked branches are on for the repository (`branchdeck.stacks`, a git boolean; off when unset): one
/// `ConfigSnapshot`; an operation that reads more of the config takes `ConfigSnapshot::stacks` of its snapshot
pub fn stacks_enabled(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<bool> {
  ConfigSnapshot::read(git_executor, repo_path)?.stacks()
}

/// A prefix with an empty stack level, as in `(a//b)`, or no branch name at all, as in `(/)`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("({prefix}) has an empty stack level")]
pub struct StackSyntaxError {
  pub prefix: String,
  /// The last non-empty level, sanitized: the branch the commit is grouped into anyway; `None` if there is none
  pub leaf: Option<String>,
}

/// The levels of a stack prefix, root first and the leaf last, each sanitized like a flat branch name. Empty levels at
/// the ends are dropped (`(auth/)` is `auth`, `(/login)` is `login`); one in the middle (`(a//b)`) is an error.
pub fn stack_path(prefix: &str) -> Result<Vec<String>, StackSyntaxError> {
  let levels: Vec<String> = prefix.split('/').map(|level| sanitize_branch_name(level.trim())).collect();
  let error = |leaf: Option<&String>| StackSyntaxError {
    prefix: prefix.to_string(),
    leaf: leaf.cloned(),
  };
  let (Some(first), Some(last)) = (levels.iter().position(|level| !level.is_empty()), levels.iter().rposition(|level| !level.is_empty())) else {
    return Err(error(None));
  };
  let path = &levels[first..=last];
  if path.iter().any(String::is_empty) {
    return Err(error(path.last()));
  }
  Ok(path.to_vec())
}

/// The branch a stack prefix names (its last level) and the parent it declares (the level before it)
pub fn stack_of(prefix: &str) -> Result<(String, Option<String>), StackSyntaxError> {
  let mut path = stack_path(prefix)?;
  let leaf = path.pop().expect("a stack path is not empty");
  Ok((leaf, path.pop()))
}

/// The leaf of a prefix, for comparing prefixes that name the same branch (`(login)` and `(auth/login)`)
pub fn leaf_of(prefix: &str) -> Option<String> {
  stack_of(prefix).ok().map(|(leaf, _)| leaf)
}

/// A branch of the stack forest: one with commits, or an ancestor that only prefixes or stack parents declare
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StackNode {
  /// The parent all its declaring commits agree on, else its stack parent (`stack_parents.rs`); `None` for a root and
  /// for conflicting parents
  pub declared_parent: Option<String>,
  /// Where `declared_parent` comes from
  pub parent_source: Option<ParentSource>,
  pub problems: Vec<StackProblem>,
  /// A commit of the branch comes before every commit of its parent on the current branch
  pub child_precedes_parent: bool,
  pub has_commits: bool,
}

/// How the grouped branches stack
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StackForest {
  /// Every branch with commits and every declared ancestor, in first-seen order
  pub nodes: IndexMap<String, StackNode>,
  /// The branches with commits, parents before their children, otherwise in first-seen order
  pub order: Vec<String>,
}

impl StackForest {
  pub fn declared_parent(&self, branch: &str) -> Option<&str> {
    self.nodes.get(branch)?.declared_parent.as_deref()
  }

  /// The state of the declared parent as far as the grouping knows it (`Active` or `Missing`); `None` for a branch
  /// without one
  pub fn parent_state(&self, branch: &str) -> Option<ParentState> {
    let parent = self.declared_parent(branch)?;
    Some(if self.nodes.get(parent).is_some_and(|node| node.has_commits) {
      ParentState::Active
    } else {
      ParentState::Missing
    })
  }

  pub fn problems(&self, branch: &str) -> &[StackProblem] {
    self.nodes.get(branch).map_or(&[], |node| node.problems.as_slice())
  }
}

/// What the commits of one grouping declare, collected while grouping
#[derive(Debug, Default)]
pub(crate) struct StackDeclarations {
  /// Every named branch, in first-seen order (a prefix names its root first)
  names: IndexSet<String>,
  /// Child -> parent -> the commits that declare the edge
  edges: HashMap<String, IndexMap<String, Vec<String>>>,
  self_parents: HashMap<String, Vec<String>>,
  syntax_errors: HashMap<String, Vec<String>>,
  /// Child -> parent: the stack parents, which place a branch whose prefixes declare no parent
  stored: BTreeMap<String, String>,
}

impl StackDeclarations {
  /// Declarations that place a branch without a declared parent by `stored` (child -> parent)
  pub(crate) fn with_stored(stored: BTreeMap<String, String>) -> Self {
    Self { stored, ..Self::default() }
  }

  /// Record the edges of a stack path that `commit_id` declares
  pub(crate) fn declare(&mut self, path: &[String], commit_id: &str) {
    for name in path {
      self.names.insert(name.clone());
    }
    for pair in path.windows(2) {
      let (parent, child) = (&pair[0], &pair[1]);
      if parent == child {
        self.self_parents.entry(child.clone()).or_default().push(commit_id.to_string());
      } else {
        self.edges.entry(child.clone()).or_default().entry(parent.clone()).or_default().push(commit_id.to_string());
      }
    }
  }

  /// A branch named without a parent: a bare `(login)` or an issue key
  pub(crate) fn note_branch(&mut self, name: &str) {
    if !self.names.contains(name) {
      self.names.insert(name.to_string());
    }
  }

  pub(crate) fn syntax_error(&mut self, leaf: &str, commit_id: &str) {
    self.note_branch(leaf);
    self.syntax_errors.entry(leaf.to_string()).or_default().push(commit_id.to_string());
  }

  /// The forest for the branches with commits, given the position of each one's oldest commit, and the children of the
  /// stack parents that no node reaches, sorted: idle here
  pub(crate) fn into_forest(self, first_positions: &HashMap<&str, usize>) -> (StackForest, Vec<String>) {
    let node_of = |name: &str| StackNode {
      has_commits: first_positions.contains_key(name),
      ..StackNode::default()
    };
    let mut nodes: IndexMap<String, StackNode> = self.names.iter().map(|name| (name.clone(), node_of(name))).collect();
    // the stack parent of a node without a declared parent is a node too, and so on up its chain: a parent without
    // commits passes its children on to its own parent
    let mut reached = BTreeSet::new();
    let mut index = 0;
    while let Some((name, _)) = nodes.get_index(index) {
      index += 1;
      let Some(parent) = self.stored.get(name) else { continue };
      reached.insert(name.clone());
      if !self.edges.contains_key(name) && !nodes.contains_key(parent) {
        nodes.insert(parent.clone(), node_of(parent));
      }
    }
    let idle = self.stored.keys().filter(|child| !reached.contains(*child)).cloned().collect();

    for (name, node) in &mut nodes {
      let stored = self.stored.get(name);
      if let Some(parents) = self.edges.get(name) {
        if parents.len() == 1 && stored.is_none_or(|stored| parents.contains_key(stored)) {
          node.declared_parent = parents.keys().next().cloned();
          node.parent_source = Some(ParentSource::Prefix);
        } else {
          let mut declared: Vec<DeclaredParent> = parents
            .iter()
            .map(|(parent, commits)| DeclaredParent {
              parent: parent.clone(),
              commits: commits.clone(),
            })
            .collect();
          // the stack parent, which no commit declares
          if let Some(stored) = stored.filter(|stored| !parents.contains_key(*stored)) {
            declared.push(DeclaredParent {
              parent: stored.clone(),
              commits: Vec::new(),
            });
          }
          node.problems.push(StackProblem::ConflictingParents(declared));
        }
      } else if let Some(stored) = stored {
        node.declared_parent = Some(stored.clone());
        node.parent_source = Some(ParentSource::Store);
      }
      if let Some(commits) = self.self_parents.get(name) {
        node.problems.push(StackProblem::SelfParent(commits.clone()));
      }
      if let Some(commits) = self.syntax_errors.get(name) {
        node.problems.push(StackProblem::Syntax(commits.clone()));
      }
    }

    add_leaf_collisions(&mut nodes);
    let cycle_members = add_cycles(&mut nodes);

    let parents: Vec<(String, String)> = nodes.iter().filter_map(|(name, node)| Some((name.clone(), node.declared_parent.clone()?))).collect();
    for (child, parent) in parents {
      if let (Some(child_position), Some(parent_position)) = (first_positions.get(child.as_str()), first_positions.get(parent.as_str())) {
        nodes[&child].child_precedes_parent = child_position < parent_position;
      }
    }

    let order = topological_order(&nodes, &cycle_members);
    (StackForest { nodes, order }, idle)
  }
}

/// Branch names that differ only in case: each one names the others
fn add_leaf_collisions(nodes: &mut IndexMap<String, StackNode>) {
  let mut by_lowercase: IndexMap<String, Vec<String>> = IndexMap::new();
  for name in nodes.keys() {
    by_lowercase.entry(name.to_lowercase()).or_default().push(name.clone());
  }
  for names in by_lowercase.values().filter(|names| names.len() > 1) {
    for name in names {
      for other in names.iter().filter(|other| *other != name) {
        nodes[name].problems.push(StackProblem::LeafCollision(other.clone()));
      }
    }
  }
}

/// Marks the branches whose declared parents lead back to them and returns them
fn add_cycles(nodes: &mut IndexMap<String, StackNode>) -> HashSet<String> {
  let mut members = HashSet::new();
  let mut done: HashSet<String> = HashSet::new();
  let mut cycles = Vec::new();
  for start in nodes.keys() {
    // each branch has at most one parent, so a walk either ends or enters a cycle
    let mut path: Vec<&String> = Vec::new();
    let mut current = Some(start);
    while let Some(name) = current {
      if done.contains(name) {
        break;
      }
      if let Some(index) = path.iter().position(|visited| *visited == name) {
        cycles.push(path[index..].iter().map(|name| (*name).clone()).collect::<Vec<String>>());
        break;
      }
      path.push(name);
      current = nodes
        .get(name)
        .and_then(|node| node.declared_parent.as_ref())
        .and_then(|parent| nodes.get_key_value(parent).map(|(key, _)| key));
    }
    done.extend(path.into_iter().cloned());
  }

  for cycle in cycles {
    for (index, name) in cycle.iter().enumerate() {
      let mut rotated = cycle[index..].to_vec();
      rotated.extend_from_slice(&cycle[..index]);
      nodes[name].problems.push(StackProblem::Cycle(rotated));
      members.insert(name.clone());
    }
  }
  members
}

/// Branches with commits, each after its ancestors; a cycle is cut where it is entered
fn topological_order(nodes: &IndexMap<String, StackNode>, cycle_members: &HashSet<String>) -> Vec<String> {
  fn visit<'a>(name: &'a str, nodes: &'a IndexMap<String, StackNode>, cycle_members: &HashSet<String>, visited: &mut HashSet<&'a str>, order: &mut Vec<String>) {
    if !visited.insert(name) {
      return;
    }
    let Some(node) = nodes.get(name) else { return };
    if !cycle_members.contains(name)
      && let Some(parent) = &node.declared_parent
    {
      visit(parent, nodes, cycle_members, visited, order);
    }
    if node.has_commits {
      order.push(name.to_string());
    }
  }

  let mut visited = HashSet::new();
  let mut order = Vec::new();
  for name in nodes.keys() {
    visit(name, nodes, cycle_members, &mut visited, &mut order);
  }
  order
}

/// Where each branch with commits goes: on its parent, on the nearest active ancestor of a parent without commits, on
/// the baseline, or nowhere (blocked by a parent that can't be built, or with a problem of its own). A parent without
/// commits never blocks: the local history is linear on the baseline, so the child's commits can be picked onto the
/// ancestor below it like those of any flat branch. `empty_parent_state` tells how a declared ancestor without commits
/// stands (`Integrated`, `Unapplied` or `Missing`), a label for the row and `bd status`.
pub fn place_branches(forest: &StackForest, empty_parent_state: &HashMap<String, ParentState>) -> IndexMap<String, BranchStack> {
  let state_of = |name: &str| {
    if forest.nodes.get(name).is_some_and(|node| node.has_commits) {
      ParentState::Active
    } else {
      empty_parent_state.get(name).copied().unwrap_or(ParentState::Missing)
    }
  };

  let mut placed: IndexMap<String, BranchStack> = IndexMap::with_capacity(forest.order.len());
  // parents first, so a parent with commits is placed before its children
  for name in &forest.order {
    let node = &forest.nodes[name];
    let mut stack = BranchStack {
      declared_parent: node.declared_parent.clone(),
      parent_source: node.parent_source,
      parent_state: node.declared_parent.as_deref().map(state_of),
      problems: node.problems.clone(),
      child_precedes_parent: node.child_precedes_parent,
      ..BranchStack::default()
    };
    if stack.problems.is_empty() {
      // the walk passes ancestors without commits; `visited` guards against a cycle among ancestors without commits
      let mut visited = HashSet::new();
      let mut current = node.declared_parent.as_deref();
      while let Some(ancestor) = current {
        if !visited.insert(ancestor) {
          break;
        }
        let block = |reason| {
          Some(StackBlock {
            parent: ancestor.to_string(),
            reason,
          })
        };
        match state_of(ancestor) {
          ParentState::Active => {
            match placed.get(ancestor) {
              Some(parent) if !parent.problems.is_empty() => stack.blocked_by = block(BlockReason::Error),
              Some(parent) if parent.blocked_by.is_some() => stack.blocked_by = block(BlockReason::Blocked),
              _ => stack.effective_base = Some(ancestor.to_string()),
            }
            break;
          }
          ParentState::Integrated | ParentState::Unapplied | ParentState::Missing => {
            stack.pr_base_changed = true;
            current = forest.declared_parent(ancestor);
          }
        }
      }
    }
    placed.insert(name.clone(), stack);
  }
  placed
}

/// How integration detection runs for a declared ancestor whose virtual branch still exists but whose commits left
/// HEAD: the sync archives that branch, so it is detected before the branches are placed
#[derive(Debug, Clone, Copy)]
pub struct PendingDetection<'a> {
  pub baseline_branch: &'a str,
  /// The commit of the baseline when the caller read it (a sync, at its start); `None` resolves it
  pub baseline_commit: Option<&'a str>,
  pub strategy: DetectionStrategy,
  /// Write the results as detection notes (a sync); a read-only command only detects
  pub cache: bool,
  /// The archive retention period: an archived parent without a usable note is detected in full only within it
  pub retention_days: u64,
}

/// The detection of a read-only command (`bd status`, `bd check`): the strategy of a sync, without writing notes
pub fn read_only_detection(baseline_branch: &str) -> PendingDetection<'_> {
  PendingDetection {
    baseline_branch,
    baseline_commit: None,
    strategy: get_detection_strategy(),
    cache: false,
    retention_days: ARCHIVE_RETENTION_DAYS,
  }
}

/// How the declared ancestors without commits stand: `Unapplied` with an unapplied branch of that name, `Integrated`
/// when its branch is integrated, else `Missing`. The branch is the virtual branch of that name, which this sync
/// archives, else the newest archived one; it is integrated by its detection note when that holds for the current
/// baseline, else it is detected now (`detection`) by the rule of the archive step (`BaselineAncestry::plan`: a
/// `NotIntegrated` note the baseline moved on from incrementally, a note without a usable baseline in full, the latter
/// for an archived branch only within the retention period). `branch_refs` maps the full names of the branches under
/// the prefix to their tips. At most one git process for the notes and the subjects of the tips, only for ancestors
/// with such a branch, plus the detection.
pub fn empty_parent_states(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  branch_refs: &HashMap<String, String>,
  forest: &StackForest,
  detection: PendingDetection<'_>,
) -> Result<HashMap<String, ParentState>> {
  let refs = branch_refs.iter().map(|(name, tip)| (name.as_str(), tip.as_str()));
  parent_states(git_executor, repo_path, branch_prefix, refs, forest, detection, |tips| {
    let mut input = String::with_capacity(tips.len() * 41);
    for tip in tips {
      input.push_str(tip);
      input.push('\n');
    }
    let notes_arg = format!("--notes={NOTES_REF}");
    let output = git_executor
      .cmd(&["log", "--no-walk", "--stdin", "--format=%H%x1f%s%x1f%N%x1e", &notes_arg], repo_path)
      .stdin(input.as_bytes())
      .text()?;
    let mut noted = HashMap::new();
    for record in output.split('\u{1e}') {
      let mut fields = record.trim_start().splitn(3, '\u{1f}');
      if let (Some(tip), Some(subject), Some(note)) = (fields.next(), fields.next(), fields.next()) {
        noted.insert(tip.to_string(), (subject.to_string(), deserialize_from_cache(note.trim()).ok()));
      }
    }
    Ok(noted)
  })
}

/// `empty_parent_states` of the branches under the prefix as `list_branches` lists them, with the subjects of their
/// tips: only the notes are read (`read_notes`)
pub fn empty_parent_states_listed(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  branches: &[ListedBranch],
  forest: &StackForest,
  detection: PendingDetection<'_>,
) -> Result<HashMap<String, ParentState>> {
  let refs = branches.iter().map(|branch| (branch.name.as_str(), branch.tip.as_str()));
  parent_states(git_executor, repo_path, branch_prefix, refs, forest, detection, |tips| {
    let mut notes = read_notes(git_executor, repo_path, NOTES_REF, tips)?;
    let subjects: HashMap<&str, &str> = branches.iter().map(|branch| (branch.tip.as_str(), branch.subject.as_str())).collect();
    Ok(
      tips
        .iter()
        .map(|tip| {
          let cached = notes.remove(*tip).and_then(|note| deserialize_from_cache(note.trim()).ok());
          ((*tip).to_string(), (subjects.get(tip).copied().unwrap_or_default().to_string(), cached))
        })
        .collect(),
    )
  })
}

/// Tip -> (subject, parsed detection note) of the tips given
type NotedTips = HashMap<String, (String, Option<CachedDetection>)>;

/// `empty_parent_states` of `refs` (full name, tip); `read_noted` reads the subjects and the detection notes of the
/// candidate tips
fn parent_states<'a>(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  refs: impl Iterator<Item = (&'a str, &'a str)>,
  forest: &StackForest,
  detection: PendingDetection<'_>,
  read_noted: impl FnOnce(&[&str]) -> Result<NotedTips>,
) -> Result<HashMap<String, ParentState>> {
  let empty: Vec<&str> = forest.nodes.iter().filter(|(_, node)| !node.has_commits).map(|(name, _)| name.as_str()).collect();
  let mut states = HashMap::with_capacity(empty.len());
  if empty.is_empty() {
    return Ok(states);
  }

  // ancestor -> (date, name suffix, full name, tip) of its newest archived branch
  let mut newest_archived: HashMap<&str, (&str, u32, &str, &str)> = HashMap::new();
  // ancestor -> (full name, tip) of its virtual branch
  let mut pending: HashMap<&str, (&str, &str)> = HashMap::new();
  for (full_name, tip) in refs {
    if let Some(name) = strip_branch_namespace(full_name, branch_prefix, BranchKind::Unapplied) {
      if let Some(parent) = empty.iter().find(|parent| name_suffix(name, parent).is_some()) {
        states.insert((*parent).to_string(), ParentState::Unapplied);
      }
    } else if let Some(name) = strip_branch_namespace(full_name, branch_prefix, BranchKind::Virtual) {
      if let Some(parent) = empty.iter().find(|parent| **parent == name) {
        pending.insert(parent, (full_name, tip));
      }
    } else if let Some((date, name)) = strip_branch_namespace(full_name, branch_prefix, BranchKind::Archived).and_then(|rest| rest.split_once('/'))
      && let Some((parent, suffix)) = empty.iter().find_map(|parent| name_suffix(name, parent).map(|suffix| (parent, suffix)))
      // of one day the archive with the highest suffix is the newest (`auth`, `auth-1`, …, `auth-10`)
      && newest_archived.get(parent).is_none_or(|(newest_date, newest_suffix, _, _)| (*newest_date, *newest_suffix) < (date, suffix))
    {
      newest_archived.insert(parent, (date, suffix, full_name, tip));
    }
  }
  // the virtual branch is newer than any archived one of the name. `(parent, full name, tip, within the retention
  // period)`: a virtual branch always is, an archived one by its date
  let candidates: Vec<(&str, &str, &str, bool)> = newest_archived
    .into_iter()
    .filter(|(parent, _)| !pending.contains_key(parent))
    .map(|(parent, (date, _, full_name, tip))| (parent, full_name, tip, archived_within_retention(date, detection.retention_days)))
    .chain(pending.iter().map(|(parent, (full_name, tip))| (*parent, *full_name, *tip, true)))
    .filter(|(parent, ..)| !states.contains_key(*parent))
    .collect();
  if candidates.is_empty() {
    return Ok(states);
  }

  let tips: Vec<&str> = candidates.iter().map(|(_, _, tip, _)| *tip).collect();
  let noted = read_noted(&tips)?;

  // a note holds, or the branch is detected now, by the rule of the archive step (`BaselineAncestry::plan`): a
  // `NotIntegrated` note holds only for the baseline it was checked against
  let baseline = match detection.baseline_commit {
    Some(commit) => commit.to_string(),
    None => resolve_baseline(git_executor, repo_path, detection.baseline_branch)?,
  };
  let mut ancestry = BaselineAncestry::new(git_executor, repo_path, &baseline);
  let mut held: HashMap<&str, &BranchIntegrationStatus> = HashMap::new();
  let mut undetected: Vec<PendingBranch<'_>> = Vec::new();
  for (_, full_name, tip, within_retention) in &candidates {
    let (subject, cached) = noted.get(*tip).map_or(("", None), |(subject, cached)| (subject.as_str(), cached.as_ref()));
    match ancestry.plan(cached, *within_retention) {
      CachePlan::Holds(cached) => {
        held.insert(tip, &cached.info.status);
      }
      CachePlan::Detect(previous) => undetected.push(PendingBranch {
        branch: full_name,
        tip,
        subject,
        previous,
      }),
    }
  }
  let detected = detect_pending_branches(git_executor, repo_path, &baseline, detection.strategy, &undetected, detection.cache)?;

  for (parent, _, tip, _) in candidates {
    let status = detected.get(tip).or_else(|| held.get(tip).copied());
    if matches!(status, Some(BranchIntegrationStatus::Integrated { .. })) {
      states.insert(parent.to_string(), ParentState::Integrated);
    }
  }
  Ok(states)
}

/// The `-N` suffix of a name conflict with which an archived or unapplied branch name is `branch`, 0 for the plain
/// name; `None` for another name
pub(crate) fn name_suffix(name: &str, branch: &str) -> Option<u32> {
  let rest = name.strip_prefix(branch)?;
  if rest.is_empty() {
    return Some(0);
  }
  let digits = rest.strip_prefix('-')?;
  if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  // a suffix too long for `u32` still names the branch, after every shorter one
  Some(digits.parse().unwrap_or(u32::MAX))
}

/// A sentence on a stack problem, naming its commits by short id
pub fn describe_problem(problem: &StackProblem) -> String {
  let commits = |ids: &[String]| ids.iter().map(|id| short_id(id)).collect::<Vec<_>>().join(", ");
  match problem {
    StackProblem::ConflictingParents(parents) => {
      let parents: Vec<String> = parents
        .iter()
        .map(|declared| {
          if declared.commits.is_empty() {
            format!("{} (the stack parent in {STACK_PARENTS_REF})", declared.parent)
          } else {
            format!("{} ({})", declared.parent, commits(&declared.commits))
          }
        })
        .collect();
      format!("Commits declare different parents: {}", parents.join(", "))
    }
    StackProblem::Cycle(branches) => format!("The parents form a cycle: {} -> {}", branches.join(" -> "), branches[0]),
    StackProblem::SelfParent(ids) => format!("A prefix makes the branch its own parent: {}", commits(ids)),
    StackProblem::LeafCollision(other) => format!("Branch {other} has the same name ignoring case"),
    StackProblem::Syntax(ids) => format!("A prefix has an empty level: {}", commits(ids)),
  }
}

/// A sentence on why a branch is blocked
pub fn describe_block(block: &StackBlock) -> String {
  let parent = &block.parent;
  match block.reason {
    BlockReason::Error => format!("Blocked: parent {parent} failed"),
    BlockReason::Conflict => format!("Blocked: parent {parent} has a merge conflict"),
    BlockReason::Blocked => format!("Blocked: parent {parent} is blocked"),
  }
}

#[cfg(test)]
#[path = "stack_test.rs"]
mod stack_test;
