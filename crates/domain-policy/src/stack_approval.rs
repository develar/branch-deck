//! Approve N Pull Requests of a stack a review window shows (openspec/specs/repository/review-requests/spec.md, "Review
//! window: Approve N Pull Requests approves every layer the window shows"): one action approves each layer on the head
//! the window showed it at (`stack_approval`), but a layer the user approved on that head already, which is skipped, and
//! an own one, which GitHub takes no review of. It is the one action of the stack itself (`stack_review_actions`), what
//! All Changes offers: Approve, Request Changes and Safe Push are decisions of one pull request, of the layer shown.
//!
//! Before the write GitHub's head of every layer is read again (`head_checks`): a stack lands bottom up, each layer with
//! what it is built on (docs/design/landing.md), so the layers below the lowest layer whose head moved are approved as
//! the user saw them and can land, while the moved layer is refused by its number, and so is every layer above it: its
//! changes are those from the head of the layer below, which is not the one the user read any more, and what it is
//! built on is not what will land.

use domain_types::actions::{ActionBlocker, ActionId, ActionState, Availability};
use domain_types::operation::ReviewedLayer;
use domain_types::session::ReviewStackApproval;

/// A layer of the stack as the session knows it
#[derive(Clone, Copy, Debug)]
pub struct LayerFacts<'a> {
  pub number: u32,
  /// The head the window shows: the one its fetch brought
  pub head: &'a str,
  /// The commit the user approved last: the session's review, else GitHub's latest review of the user
  pub approved: Option<&'a str>,
  /// The user's own: GitHub takes no review of its author
  pub own: bool,
}

/// Approve N Pull Requests of the stack of `layers`, the bottom first: the layers it approves on the head the window
/// shows, and those the user approved on it already
#[must_use]
pub fn stack_approval(layers: &[LayerFacts<'_>]) -> ReviewStackApproval {
  let approved_already = |layer: &LayerFacts<'_>| layer.approved == Some(layer.head);
  let approves: Vec<ReviewedLayer> = layers
    .iter()
    .filter(|layer| !layer.own && !approved_already(layer))
    .map(|layer| ReviewedLayer {
      number: layer.number,
      commit: layer.head.to_owned(),
    })
    .collect();
  let approved: Vec<u32> = layers.iter().filter(|layer| !layer.own && approved_already(layer)).map(|layer| layer.number).collect();
  ReviewStackApproval { approves, approved }
}

/// The actions of a stack itself in its review window, what All Changes offers: Approve N Pull Requests alone, disabled
/// once nothing is left to approve. Approve, Request Changes and Safe Push are not among them: each is of one pull
/// request, offered on the layer shown (`actions::review_actions`). It waits while the actions read a frame nobody
/// checked yet (`cached`), as Approve does
#[must_use]
pub fn stack_review_actions(approval: &ReviewStackApproval, cached: bool) -> Vec<ActionState> {
  let blocker = if cached {
    Some(ActionBlocker::Cached)
  } else if !approval.approves.is_empty() {
    None
  } else if approval.approved.is_empty() {
    Some(ActionBlocker::OwnPullRequest)
  } else {
    Some(ActionBlocker::StackApproved)
  };
  vec![ActionState {
    id: ActionId::ApproveStack,
    availability: Availability::of(blocker),
  }]
}

/// What GitHub's head of a layer says of its approval, read just before the write
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerCheck {
  /// Its head and the heads of the layers below it are the ones the window showed
  Approve,
  /// Its head moved since the window showed it: GitHub shows `head` now
  Moved { head: String },
  /// The head of the layer `below`, under it, moved: its changes are not those the user read
  BelowMoved { below: u32 },
}

/// The check of each of `layers` (the bottom first, as the window showed them) against `heads`, GitHub's head of each
/// now, in the same order: the layers below the lowest one that moved are approved, that one and every one above it
/// refused
#[must_use]
pub fn head_checks(layers: &[ReviewedLayer], heads: &[String]) -> Vec<LayerCheck> {
  let mut moved: Option<u32> = None;
  layers
    .iter()
    .zip(heads)
    .map(|(layer, head)| {
      if *head != layer.commit {
        moved.get_or_insert(layer.number);
        LayerCheck::Moved { head: head.clone() }
      } else if let Some(below) = moved {
        LayerCheck::BelowMoved { below }
      } else {
        LayerCheck::Approve
      }
    })
    .collect()
}
