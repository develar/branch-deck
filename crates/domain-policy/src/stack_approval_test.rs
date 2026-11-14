use crate::stack_approval::{LayerCheck, LayerFacts, head_checks, stack_approval};
use domain_types::actions::{ActionBlocker, Availability};
use domain_types::operation::ReviewedLayer;
use pretty_assertions::assert_eq;

const fn layer(number: u32, head: &'static str, approved: Option<&'static str>) -> LayerFacts<'static> {
  LayerFacts {
    number,
    head,
    approved,
    own: false,
  }
}

fn reviewed(number: u32, commit: &str) -> ReviewedLayer {
  ReviewedLayer {
    number,
    commit: commit.to_owned(),
  }
}

/// openspec/specs/repository/review-requests/spec.md, "Review window: Approve N Pull Requests approves every layer the
/// window shows": every layer on the head the window shows, a layer approved on that head skipped (an approval of an
/// older head is approved again), an own one left out; nothing left to approve is "approved", a stack of the user's
/// own is theirs, and a cached frame waits
#[test]
fn a_stack_approval_approves_each_layer_on_its_shown_head_and_skips_those_approved_on_it() {
  let approval = stack_approval(&[layer(301, "a", Some("a")), layer(302, "b", Some("old")), layer(303, "c", None)], false);
  assert_eq!(approval.approves, [reviewed(302, "b"), reviewed(303, "c")]);
  assert_eq!(approval.approved, [301]);
  assert_eq!(approval.action.availability, Availability::Enabled);

  let own = LayerFacts {
    own: true,
    ..layer(304, "c", None)
  };
  let done = stack_approval(&[layer(301, "a", Some("a")), own], false);
  assert_eq!((done.approves, done.approved), (Vec::new(), vec![301]));
  assert_eq!(
    done.action.availability,
    Availability::Disabled {
      reason: ActionBlocker::StackApproved
    }
  );
  assert_eq!(
    stack_approval(&[own], false).action.availability,
    Availability::Disabled {
      reason: ActionBlocker::OwnPullRequest
    }
  );
  assert_eq!(
    stack_approval(&[layer(303, "c", None)], true).action.availability,
    Availability::Disabled { reason: ActionBlocker::Cached }
  );
}

/// openspec/specs/repository/review-requests/spec.md, "Review window: A layer whose head moved is refused, and the
/// layers above it": the layers below the lowest moved one are approved, it is refused with GitHub's head, every layer
/// above it by the one below that moved, also one whose own head is unchanged
#[test]
fn the_layers_below_the_lowest_moved_head_are_approved_and_it_and_those_above_are_refused() {
  let layers = [reviewed(301, "a"), reviewed(302, "b"), reviewed(303, "c"), reviewed(304, "d")];
  let same = ["a", "b", "c", "d"].map(String::from);
  assert_eq!(
    head_checks(&layers, &same),
    [LayerCheck::Approve, LayerCheck::Approve, LayerCheck::Approve, LayerCheck::Approve]
  );
  let moved = ["a", "b2", "c", "d2"].map(String::from);
  assert_eq!(
    head_checks(&layers, &moved),
    [
      LayerCheck::Approve,
      LayerCheck::Moved { head: "b2".to_owned() },
      LayerCheck::BelowMoved { below: 302 },
      LayerCheck::Moved { head: "d2".to_owned() },
    ]
  );
  let top = ["a", "b", "c", "d2"].map(String::from);
  assert_eq!(head_checks(&layers, &top)[3], LayerCheck::Moved { head: "d2".to_owned() });
  assert_eq!(head_checks(&layers, &top)[..3], [LayerCheck::Approve, LayerCheck::Approve, LayerCheck::Approve]);
}
