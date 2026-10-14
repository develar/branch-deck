//! Stacked branches: `(auth/login) X` puts the commit into `login` and declares `auth` as the parent of `login`;
//! `(auth/login/oauth) X` also declares `auth` as the parent of `login`. Opt-in per repository
//! (`git config branchdeck.stacks true`), because without it a `/` in a prefix is part of a flat branch name.

use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::sanitize_branch_name;
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

pub const STACKS_CONFIG_KEY: &str = "branchdeck.stacks";

/// Whether stacked branches are on for the repository (`branchdeck.stacks`, a git boolean; off when unset)
pub fn stacks_enabled(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<bool> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["config", "--type=bool", "--get", STACKS_CONFIG_KEY], repo_path)?;
  match exit_code {
    0 => Ok(output == "true"),
    // not set
    1 => Ok(false),
    _ => Err(anyhow!("{STACKS_CONFIG_KEY} is not a boolean (true or false)")),
  }
}

/// A prefix with an empty stack level, as in `(a//b)`, or no branch name at all, as in `(/)`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSyntaxError {
  pub prefix: String,
  /// The last non-empty level, sanitized: the branch the commit is grouped into anyway; `None` if there is none
  pub leaf: Option<String>,
}

impl std::fmt::Display for StackSyntaxError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "({}) has an empty stack level", self.prefix)
  }
}

impl std::error::Error for StackSyntaxError {}

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

/// Why a branch of a stack can't be placed; it never fails the other branches
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackProblem {
  /// Commits declare different parents, in first-declared order
  ConflictingParents(Vec<DeclaredParent>),
  /// Following the declared parents leads back to the branch: the branches of the cycle, starting with this one
  Cycle(Vec<String>),
  /// `(a/a)`: the commits that declare the branch as its own parent
  SelfParent(Vec<String>),
  /// Another branch has the same name ignoring case (`Login` and `login`)
  LeafCollision(String),
  /// `(a//b)`: the commits whose prefix has an empty level
  Syntax(Vec<String>),
}

/// A parent and the commits that declare it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredParent {
  pub parent: String,
  pub commits: Vec<String>,
}

/// The state of a declared parent. Integrated and unapplied parents need the integration data of a sync, so the
/// grouping knows only these two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentState {
  /// The parent has commits
  Active,
  /// No commit is grouped into the parent
  Missing,
}

/// A branch of the stack forest: one with commits, or an ancestor that only prefixes declare
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StackNode {
  /// The parent all its declaring commits agree on; `None` for a root and for conflicting parents
  pub declared_parent: Option<String>,
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

  /// The state of the declared parent; `None` for a branch without one
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
}

impl StackDeclarations {
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

  /// The forest for the branches with commits, given the position of each one's oldest commit
  pub(crate) fn into_forest(self, first_positions: &HashMap<&str, usize>) -> StackForest {
    let mut nodes: IndexMap<String, StackNode> = self
      .names
      .iter()
      .map(|name| {
        let node = StackNode {
          has_commits: first_positions.contains_key(name.as_str()),
          ..StackNode::default()
        };
        (name.clone(), node)
      })
      .collect();

    for (name, node) in &mut nodes {
      if let Some(parents) = self.edges.get(name) {
        if parents.len() == 1 {
          node.declared_parent = parents.keys().next().cloned();
        } else {
          let declared = parents.iter().map(|(parent, commits)| DeclaredParent {
            parent: parent.clone(),
            commits: commits.clone(),
          });
          node.problems.push(StackProblem::ConflictingParents(declared.collect()));
        }
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
    StackForest { nodes, order }
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
