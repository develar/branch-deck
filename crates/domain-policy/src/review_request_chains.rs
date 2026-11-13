//! The stacks of the requests of Your Turn (openspec/specs/repository/review-requests/spec.md, "Card: A chain of requests
//! is one stack"): a request whose base is the head branch of another request of the repository is the layer above it.
//! A chain that does not fork is one row, its bottom first, at the place of its layer that waits the longest; a branch
//! two requests go into links neither of them, a request from a fork is never a layer below another, since its head
//! branch is not one of the repository, and a request into the default branch is the bottom of its chain. The names
//! alone decide: GitHub's own stacks are such chains too.

use domain_types::review_request::{RequestRow, ReviewRequest};
use std::collections::HashMap;

/// The rows of `requests`, in their order (waiting the longest first): each request once, a chain of two or more as one
/// `Stack` at the place of its first request in the order
#[must_use]
pub fn request_rows(requests: &[ReviewRequest]) -> Vec<RequestRow> {
  // the head branch of each request of the repository, by its name; a name two requests share links neither
  let mut by_head: HashMap<&str, Option<usize>> = HashMap::new();
  for (index, request) in requests.iter().enumerate() {
    if !request.is_cross_repository && !request.head_ref_name.is_empty() {
      by_head.entry(request.head_ref_name.as_str()).and_modify(|slot| *slot = None).or_insert(Some(index));
    }
  }
  // the request each one is built on, then those built on each
  let below: Vec<Option<usize>> = requests
    .iter()
    .enumerate()
    .map(|(index, request)| {
      if request.base_is_default {
        return None;
      }
      by_head.get(request.base_ref_name.as_str()).copied().flatten().filter(|&lower| lower != index)
    })
    .collect();
  let mut uppers: Vec<Vec<usize>> = vec![Vec::new(); requests.len()];
  for (index, lower) in below.iter().enumerate() {
    if let Some(lower) = lower {
      uppers[*lower].push(index);
    }
  }
  // a link stands only where the lower request has the upper one alone above it
  let above: Vec<Option<usize>> = uppers.iter().map(|it| if let [upper] = it.as_slice() { Some(*upper) } else { None }).collect();
  let linked_below = |index: usize| below[index].filter(|&lower| above[lower] == Some(index));

  let mut placed = vec![false; requests.len()];
  let mut rows = Vec::new();
  for index in 0..requests.len() {
    if placed[index] {
      continue;
    }
    // down to the bottom of its chain, then up; a cycle of names stops where it began
    let mut bottom = index;
    let mut steps = 0;
    while let Some(lower) = linked_below(bottom) {
      steps += 1;
      if lower == index || steps > requests.len() {
        break;
      }
      bottom = lower;
    }
    let mut layers = Vec::new();
    let mut current = Some(bottom);
    while let Some(layer) = current {
      if placed[layer] {
        break;
      }
      placed[layer] = true;
      layers.push(requests[layer].number);
      current = above[layer].filter(|&upper| linked_below(upper) == Some(layer));
    }
    rows.push(match layers.as_slice() {
      [number] => RequestRow::Single { number: *number },
      _ => RequestRow::Stack { layers },
    });
  }
  rows
}

/// The layers of the stack of `top` down to `bottom`, the bottom first: from `top` the pull request whose head branch is
/// the base of the layer above, of `known` (every pull request a listing named) and not from a fork, preferring one of
/// `listed` (the requests of the last listing) and then the newest, until `bottom`, a layer into the default branch, or a
/// base no pull request has. Empty when `known` lacks `top`
#[must_use]
pub fn stack_layers(top: u32, bottom: u32, known: &[&ReviewRequest], listed: &[u32]) -> Vec<u32> {
  let Some(mut layer) = known.iter().find(|request| request.number == top) else {
    return Vec::new();
  };
  let mut layers = vec![top];
  while layer.number != bottom && !layer.base_is_default {
    let below = known
      .iter()
      .filter(|request| !request.is_cross_repository && request.head_ref_name == layer.base_ref_name && !layers.contains(&request.number))
      .max_by_key(|request| (listed.contains(&request.number), request.number));
    let Some(below) = below else { break };
    layers.push(below.number);
    layer = below;
  }
  layers.reverse();
  layers
}
