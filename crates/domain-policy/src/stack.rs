//! The stacks of a table, for the app and `bd` alike (docs/design/rust-domain-authority.md, D8 and the rule index):
//!
//! - `declared_parent`: the branch of the table a branch stacks on, as the user declared it (a `(parent/child)` prefix or
//!   the stored parent of `refs/branch-deck/stack-parents`, `BranchStack::declared_parent`), not the branch a sync built
//!   it on; a parent the table lacks (one without commits) makes a root. The rows of the table, the children, the
//!   descendants and the main line follow it
//! - `build_parent`: the branch a branch is built on, whose commits it carries: its effective base, else the declared
//!   parent of a blocked branch (not built this sync), of the table or not; what parks a branch with its parent
//! - `build_ancestors`: the chain of the effective bases among the branches of the table, root first
//! - `children`, `descendants`: the branches stacked on a branch by declared parent, directly, or further down each
//!   after its parent
//! - `main_line`: below a branch, at each fork the child with the longest chain, the last of equal ones: lane 0 of the stack graph the
//!   page draws (`src/lib/stack-lanes.ts`), the line a Safe Push of its last branch lands in one robot
//! - `root_of`, `whole_stack_of`: the whole stack of a branch, which one Safe Push lands, by declared parent (D8, the
//!   owner's decision: the stack as the user declared it; the parent that blocks a branch is a detail of its build): the
//!   stack branch of its root while the stack forks, else the straight chain from its root, which its last branch that
//!   is not parked lands
//! - `whole_stack_members`, `whole_stack_rest`: the branches of a whole stack as a sentence names them (the main line
//!   first, then the side branches, which only the whole stack lands), and what its Safe Push lands once merged Safe
//!   Pushes landed branches of it whole

use crate::facts::Repository;
use crate::parked::parked_state;
use domain_types::repository_view::{LandedByView, RobotOf, WholeStackRest};
use std::collections::{HashMap, HashSet};

/// The branch of the table `name` stacks on; `None` for a root, a parent the table lacks or the branch itself
#[must_use]
pub fn declared_parent<'a>(repo: &Repository<'a>, name: &str) -> Option<&'a str> {
  let parent = repo.stack(name)?.declared_parent.as_deref()?;
  (parent != name && repo.contains(parent)).then_some(parent)
}

/// The branch `name` is built on, whose commits it carries: its effective base, else the declared parent of a blocked
/// branch; `None` for a branch built on the baseline or one the table lacks
#[must_use]
pub fn build_parent<'a>(repo: &Repository<'a>, name: &str) -> Option<&'a str> {
  let stack = repo.stack(name)?;
  stack.effective_base.as_deref().or(stack.declared_parent.as_deref())
}

/// The branches `name` is built on, root first: the chain of its effective bases, of the branches of the table only (a
/// base the table lacks, a cycle or the branch itself ends it); empty for a root or with stacked branches off
#[must_use]
pub fn build_ancestors<'a>(repo: &Repository<'a>, name: &str) -> Vec<&'a str> {
  let base_of = |name: &str| repo.stack(name).and_then(|stack| stack.effective_base.as_deref());
  let mut ancestors = Vec::new();
  let mut base = base_of(name);
  while let Some(ancestor) = base {
    if ancestor == name || ancestors.contains(&ancestor) || !repo.contains(ancestor) {
      break;
    }
    base = base_of(ancestor);
    ancestors.push(ancestor);
  }
  ancestors.reverse();
  ancestors
}

/// The branches stacked directly on `name` (their declared parent), in the order of the table
#[must_use]
pub fn children<'r, 'a>(repo: &'r Repository<'a>, name: &str) -> &'r [&'a str] {
  repo.children_index(index).get(name).map_or(&[], Vec::as_slice)
}

fn index<'a>(repo: &Repository<'a>) -> HashMap<&'a str, Vec<&'a str>> {
  let mut children: HashMap<&'a str, Vec<&'a str>> = HashMap::new();
  for name in repo.order() {
    if let Some(parent) = declared_parent(repo, name) {
      children.entry(parent).or_default().push(name);
    }
  }
  children
}

/// The branches stacked on `name`, directly or further down, each after its parent: the children of `name`, then those
/// of its last child first, depth-first, every branch once
#[must_use]
pub fn descendants<'a>(repo: &Repository<'a>, name: &str) -> Vec<&'a str> {
  let mut descendants = Vec::new();
  let mut seen = HashSet::from([name]);
  let mut pending = vec![name];
  while let Some(next) = pending.pop() {
    for &child in children(repo, next) {
      if seen.insert(child) {
        descendants.push(child);
        pending.push(child);
      }
    }
  }
  descendants
}

/// The main line of the stack below `name`: at each fork the child with the longest chain below it, the last of
/// children with equal chains; empty when nothing is stacked on `name`
#[must_use]
pub fn main_line<'a>(repo: &Repository<'a>, name: &'a str) -> Vec<&'a str> {
  let mut chains = HashMap::new();
  let mut line = Vec::new();
  let mut current = name;
  loop {
    let mut path = HashSet::from([current]);
    let mut main: Option<(&'a str, usize)> = None;
    for &child in children(repo, current) {
      // a cycle of declared parents comes back to the line
      if child == name || line.contains(&child) {
        continue;
      }
      let length = chain_length(repo, child, &mut path, &mut chains);
      if main.is_none_or(|(_, longest)| length >= longest) {
        main = Some((child, length));
      }
    }
    let Some((child, _)) = main else {
      return line;
    };
    line.push(child);
    current = child;
  }
}

/// The branches of the longest chain down from `name`, `name` included; a branch on `path` above it ends a chain
fn chain_length<'a>(repo: &Repository<'a>, name: &'a str, path: &mut HashSet<&'a str>, chains: &mut HashMap<&'a str, usize>) -> usize {
  if let Some(&known) = chains.get(name) {
    return known;
  }
  path.insert(name);
  let mut longest = 0;
  for &child in children(repo, name) {
    if !path.contains(child) {
      longest = longest.max(chain_length(repo, child, path, chains));
    }
  }
  path.remove(name);
  chains.insert(name, longest + 1);
  longest + 1
}

/// The top-level root of the stack of `name` by declared parent: `name` itself for a root; a cycle of declared parents
/// ends at the branch before it comes back
#[must_use]
pub fn root_of<'a>(repo: &Repository<'a>, name: &'a str) -> &'a str {
  let mut root = name;
  let mut seen = HashSet::from([name]);
  while let Some(parent) = declared_parent(repo, root) {
    if !seen.insert(parent) {
      break;
    }
    root = parent;
  }
  root
}

/// The whole stack of a branch, which one Safe Push lands
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WholeStack<'a> {
  /// The stack of `root` forks: its stack branch `{prefix}/stack/<root>` carries every commit of it
  Forking { root: &'a str },
  /// A straight chain from `root`, a branch alone included: `leaf`, its last branch that is not parked, carries every
  /// commit of it that lands (a parked one at its end stays local)
  Chain { root: &'a str, leaf: &'a str },
  /// The stack of `root` forks at `fork`, and no stack branch of it is known: a sync builds it
  Unbuilt { root: &'a str, fork: &'a str },
}

impl<'a> WholeStack<'a> {
  /// The top-level root of the stack
  #[must_use]
  pub const fn root(&self) -> &'a str {
    match self {
      Self::Forking { root } | Self::Chain { root, .. } | Self::Unbuilt { root, .. } => root,
    }
  }
}

/// The whole stack of `name` (module docs): the stack of its root by declared parent, forking while `forks(root)`, the
/// stack branch of the root has members; `None` for a branch the table lacks
#[must_use]
pub fn whole_stack_of<'a>(repo: &Repository<'a>, forks: impl Fn(&str) -> bool, name: &str) -> Option<WholeStack<'a>> {
  let root = root_of(repo, repo.branch(name)?.name);
  if forks(root) {
    return Some(WholeStack::Forking { root });
  }
  let mut chain = vec![root];
  loop {
    let current = chain[chain.len() - 1];
    match children(repo, current) {
      [] => break,
      // a cycle of declared parents comes back to the chain
      [only] if chain.contains(only) => break,
      [only] => chain.push(only),
      _ => return Some(WholeStack::Unbuilt { root, fork: current }),
    }
  }
  let leaf = chain.iter().rev().find(|branch| parked_state(repo, branch).is_none()).copied().unwrap_or(root);
  Some(WholeStack::Chain { root, leaf })
}

/// The branches of the whole stack of `root` as a sentence names them: of `members` (the stack branch's, any order) the
/// main line of `root` first, root first, then the side branches, which only the whole stack lands, in their order
#[must_use]
pub fn whole_stack_members<'m>(repo: &Repository<'_>, root: &str, members: &'m [String]) -> Vec<&'m str> {
  let line: Vec<&str> = match repo.branch(root) {
    Some(branch) => std::iter::once(branch.name).chain(main_line(repo, branch.name)).collect(),
    None => vec![root],
  };
  let mut ordered: Vec<&'m str> = line.iter().filter_map(|name| members.iter().find(|member| member == name).map(String::as_str)).collect();
  for member in members {
    if !ordered.contains(&member.as_str()) {
      ordered.push(member);
    }
  }
  ordered
}

/// What the Safe Push of the whole stack of `root` lands: of `members` (as a sentence names them, `whole_stack_members`)
/// those merged Safe Pushes did not land whole, their commits (`of` a member: what landed it, and its commit count; one
/// landed with commits after it lands those only; while none landed, `commit_count`, the stack branch's), what landed
/// the others, each robot once, and whether the robot of the whole stack itself landed all of it
#[must_use]
pub fn whole_stack_rest<'v>(root: &str, members: &[&str], commit_count: u32, of: impl Fn(&str) -> (Option<&'v LandedByView>, u32)) -> WholeStackRest {
  let mut rest = WholeStackRest {
    by_own_robot: true,
    ..WholeStackRest::default()
  };
  for &name in members {
    let (landed, commit_count) = of(name);
    match landed {
      Some(landed) if landed.after.is_empty() => {
        rest.landed.push(name.to_string());
        if !rest.by.iter().any(|by| same_subject(by, &landed.by)) {
          rest.by.push(landed.by.clone());
        }
        rest.by_own_robot &= landed.by.whole_stack && landed.by.branch == root;
      }
      _ => {
        rest.members.push(name.to_string());
        rest.commit_count += landed.map_or(commit_count, |landed| u32::try_from(landed.after.len()).unwrap_or(u32::MAX));
        rest.by_own_robot = false;
      }
    }
  }
  if rest.landed.is_empty() {
    // the stack branch has the commits of every branch, counted by the sync
    rest.commit_count = commit_count;
    rest.by_own_robot = false;
  }
  rest
}

/// Two robots of the same subject: a sentence names them once
fn same_subject(a: &RobotOf, b: &RobotOf) -> bool {
  a.branch == b.branch && a.whole_stack == b.whole_stack
}
