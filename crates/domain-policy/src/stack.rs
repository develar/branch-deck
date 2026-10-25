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

use crate::facts::Repository;
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
