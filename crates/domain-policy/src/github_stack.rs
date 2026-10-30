//! The stacks of GitHub of a table (docs/design/github-stacks.md; spec/branch/stacked-branches.spec.md, "Stacks of
//! GitHub"): on a route without Patronus a stack that does not fork is a stack of GitHub, the pull request of each branch
//! into its parent's branch, the root's into the target, judged by the rules of the target and merged with every layer
//! below it.
//!
//! - `chain_of`: the branches of the stack of GitHub a branch is a layer of, bottom first: the straight chain of its
//!   stack by declared parent above the branches that landed, while each branch is built on its declared parent and
//!   none is parked; none for a stack that forks and for a chain of one branch, whose pull request goes into the target
//! - `layer_parent`: the branch whose branch the pull request of a layer goes into
//! - `stack_writes`: what brings the stack of GitHub of a chain in line with it, by the open pull requests of its
//!   branches: nothing, an add of the new top layers, or the unstack of what is there, the retargets and a new stack;
//!   in the order GitHub requires, as GitHub refuses the retarget of a member
//! - `layers_below`: the open layers below a pull request in its stack, which its merge merges with it
//! - `rebased_by_github`: the layer GitHub moved onto the target after the layers below it merged, whose push waits for
//!   Update Project

use crate::facts::Repository;
use crate::parked::parked_state;
use crate::stack::{WholeStack, declared_parent, whole_stack_of};
use domain_types::pull_request::{PullRequest, PullRequestState, StackWrite};
use domain_types::reason::Reason;
use std::collections::BTreeSet;

/// The branches of the stack of GitHub `name` is a layer of, bottom first (module docs): `forks(root)` says the stack of
/// a root forks (its stack branch lands it), `landed(branch)` that a branch landed; `None` while `name` is no layer
#[must_use]
pub fn chain_of<'a>(repo: &Repository<'a>, forks: impl Fn(&str) -> bool, landed: impl Fn(&str) -> bool, name: &str) -> Option<Vec<&'a str>> {
  let WholeStack::Chain { root, .. } = whole_stack_of(repo, forks, name)? else {
    return None;
  };
  let mut straight = vec![root];
  while let [only] = crate::stack::children(repo, straight[straight.len() - 1]) {
    if straight.contains(only) {
      break;
    }
    straight.push(only);
  }
  let mut chain: Vec<&'a str> = Vec::new();
  for branch in straight.into_iter().skip_while(|branch| landed(branch)) {
    if parked_state(repo, branch).is_some() {
      break;
    }
    // the bottom is built on the baseline or on the branch that landed below it; a layer on its declared parent, never
    // blocked (its build took no base) or built on the baseline: its pull request would show another diff
    if let Some(&below) = chain.last() {
      let base = repo.stack(branch).and_then(|stack| stack.effective_base.as_deref());
      if base != Some(below) || declared_parent(repo, branch) != Some(below) {
        break;
      }
    }
    chain.push(branch);
  }
  (chain.len() > 1 && chain.contains(&name)).then_some(chain)
}

/// The branch whose branch the pull request of `name` goes into as a layer of `chain`: the one below it; `None` for the
/// bottom, whose pull request goes into the target, and for a branch of no chain
#[must_use]
pub fn layer_parent<'a>(chain: Option<&[&'a str]>, name: &str) -> Option<&'a str> {
  let chain = chain?;
  let at = chain.iter().position(|branch| *branch == name)?;
  at.checked_sub(1).map(|below| chain[below])
}

/// What brings the stack of GitHub of `chain` in line with it (module docs), by `open(branch)`, the open pull request of
/// a branch, and `target`. The layers are the pull requests of the chain from the bottom up while each goes where it
/// should: the bottom's into the target, each other's into the branch of the one below or into another virtual branch,
/// which a retarget moves; a branch without a pull request, or with one into another base above the bottom, ends them.
/// One into the target above the bottom (of before the stacks of GitHub) ends them too, unless the user takes it into
/// the stack (`join`: Link GitHub Stack), which retargets it into the branch below it. Fewer than two layers make no
/// stack, and one already there stays as it is
#[must_use]
pub fn stack_writes<'p>(chain: &[&str], open: impl Fn(&str) -> Option<&'p PullRequest>, target: &str, prefix: &str, join: bool) -> Vec<StackWrite> {
  let mut layers: Vec<(&str, &PullRequest)> = Vec::new();
  for branch in chain {
    let Some(pull_request) = open(branch) else {
      break;
    };
    let base = pull_request.base_ref_name.as_str();
    let goes = match layers.last() {
      None => base == target,
      Some((_, below)) => base == below.head_ref_name || is_virtual(base, prefix) || (join && base == target),
    };
    if !goes {
      break;
    }
    layers.push((branch, pull_request));
  }
  if layers.len() < 2 {
    return Vec::new();
  }
  // the retarget of each layer whose pull request goes into another virtual branch, by its place among the layers
  let retargets: Vec<(usize, StackWrite)> = layers
    .windows(2)
    .enumerate()
    .filter(|(_, pair)| pair[1].1.base_ref_name != pair[0].1.head_ref_name)
    .map(|(at, pair)| {
      let write = StackWrite::Retarget {
        branch: pair[1].0.to_string(),
        number: pair[1].1.number,
        from: pair[1].1.base_ref_name.clone(),
        to: pair[0].1.head_ref_name.clone(),
      };
      (at + 1, write)
    })
    .collect();
  let numbers = |layers: &[(&str, &PullRequest)]| layers.iter().map(|(_, pull_request)| pull_request.number).collect::<Vec<_>>();
  // the lowest layers, members of one stack into the target in order and its top, the rest outside every stack: the
  // rest goes on top of it, retargeted first (only a member's retarget is refused)
  let members = layers.iter().take_while(|(_, pull_request)| pull_request.stack.is_some()).count();
  let rest_outside = layers[members..].iter().all(|(_, pull_request)| pull_request.stack.is_none());
  let members_retargeted = retargets.iter().any(|(at, _)| *at < members);
  if rest_outside && !members_retargeted {
    let retargets = retargets.iter().map(|(_, write)| write.clone());
    let Some(stack) = layers[0].1.stack.as_ref() else {
      return retargets.chain([StackWrite::Create { pull_requests: numbers(&layers) }]).collect();
    };
    let in_order = layers[..members].iter().zip(stack.position..).all(|((_, pull_request), position)| {
      pull_request
        .stack
        .as_ref()
        .is_some_and(|entry| entry.number == stack.number && entry.base_ref_name == target && entry.position == position)
    });
    let top = u32::try_from(members).ok().and_then(|members| stack.position.checked_add(members - 1));
    if in_order && top == Some(stack.size) {
      return match &layers[members..] {
        [] => Vec::new(),
        above => retargets
          .chain([StackWrite::Add {
            stack: stack.number,
            pull_requests: numbers(above),
          }])
          .collect(),
      };
    }
  }
  let stacks: BTreeSet<u32> = layers
    .iter()
    .filter_map(|(_, pull_request)| pull_request.stack.as_ref().map(|entry| entry.number))
    .collect();
  let mut writes: Vec<StackWrite> = stacks.into_iter().map(|stack| StackWrite::Unstack { stack }).collect();
  writes.extend(retargets.into_iter().map(|(_, write)| write));
  writes.push(StackWrite::Create { pull_requests: numbers(&layers) });
  writes
}

/// The open pull requests of the layers below `pull_request` in its stack of GitHub among `listed`, bottom first: what
/// a merge of it merges with it; none outside a stack
pub fn layers_below<'a>(listed: impl IntoIterator<Item = &'a PullRequest>, pull_request: &PullRequest) -> Vec<&'a PullRequest> {
  let Some(stack) = &pull_request.stack else {
    return Vec::new();
  };
  let mut below: Vec<&PullRequest> = listed
    .into_iter()
    .filter(|layer| layer.state == PullRequestState::Open)
    .filter(|layer| layer.stack.as_ref().is_some_and(|entry| entry.number == stack.number && entry.position < stack.position))
    .collect();
  below.sort_by_key(|layer| layer.stack.as_ref().map_or(0, |entry| entry.position));
  below
}

/// Why a push of `branch` waits for Update Project: its open pull request is a layer of a stack of GitHub that GitHub
/// moved onto `target` after the layers below it merged (it goes into the target now, a member above the bottom), while
/// the branch is still built on another branch (`effective_base`): its push would put the merged layers back
#[must_use]
pub fn rebased_by_github(branch: &str, pull_request: &PullRequest, effective_base: Option<&str>, target: &str) -> Option<Reason> {
  let moved = pull_request.state == PullRequestState::Open
    && pull_request.base_ref_name == target
    && pull_request.stack.as_ref().is_some_and(|stack| stack.position > 1 && stack.base_ref_name == target);
  (moved && effective_base.is_some()).then(|| Reason::LayerRebased {
    branch: branch.to_owned(),
    number: pull_request.number,
    target: target.to_owned(),
  })
}

fn is_virtual(base: &str, prefix: &str) -> bool {
  domain_types::branch_names::strip_branch_namespace(base, prefix, domain_types::branch_names::BranchKind::Virtual).is_some_and(|name| !name.is_empty())
}

#[cfg(test)]
#[path = "github_stack_test.rs"]
mod github_stack_test;
