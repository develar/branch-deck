use crate::kept::{KeptFacts, KeptKind, kept_actions};
use domain_types::actions::{ActionBlocker, ActionId, Availability};
use domain_types::fixtures::{RemoteCounts, remote_status, remote_status_with};
use domain_types::reason::Reason;
use pretty_assertions::assert_eq;

fn unapplied(checked_out: bool, remote: Option<&domain_types::RemoteStatusUpdate>, cached: bool) -> KeptFacts<'_> {
  KeptFacts {
    kind: KeptKind::Unapplied,
    branch: "docs",
    checked_out,
    remote,
    cached,
  }
}

/// Each action by its id, the reason of a disabled one, in the order of the menu
fn summary(facts: &KeptFacts<'_>) -> Vec<(ActionId, Option<ActionBlocker>)> {
  kept_actions(facts)
    .into_iter()
    .map(|action| {
      let blocker = match action.availability {
        Availability::Disabled { reason } => Some(reason),
        _ => None,
      };
      (action.id, blocker)
    })
    .collect()
}

fn rule(reason: Reason) -> ActionBlocker {
  ActionBlocker::Rule { reason }
}

/// An unapplied branch offers its review, its reapply, its push under its own name and its delete; its Safe Push says it
/// lands nothing until it is reapplied
#[test]
fn an_unapplied_branch_offers_its_review_reapply_push_and_delete_and_lands_nothing() {
  let lands_nothing = Some(rule(Reason::KeptLandsNothing { branch: "docs".to_string() }));
  assert_eq!(
    summary(&unapplied(false, None, false)),
    vec![
      (ActionId::ReviewChanges, None),
      (ActionId::Reapply, None),
      (ActionId::Push, None),
      (ActionId::SafePush, lands_nothing),
      (ActionId::DeleteKeptBranch, None),
    ]
  );

  // a remote that has it as it is: nothing to push; one with commits it lacks, a force push of the user's choice
  let pushed = remote_status("docs");
  let behind = remote_status_with(RemoteCounts {
    behind: 1,
    ..RemoteCounts::default()
  });
  let push_of = |remote| summary(&unapplied(false, Some(remote), false)).into_iter().find(|(id, _)| *id == ActionId::Push).unwrap().1;
  assert_eq!(push_of(&pushed), Some(ActionBlocker::UpToDate));
  assert_eq!(push_of(&behind), None);
}

/// A worktree that has the branch checked out keeps its reapply and its delete; the cached frame of an earlier run its
/// push, which waits for the first sync; its review is always there
#[test]
fn a_checked_out_branch_keeps_its_reapply_and_delete_and_a_cached_frame_its_push() {
  let checked_out = Some(rule(Reason::KeptCheckedOut { branch: "docs".to_string() }));
  let blockers: Vec<_> = summary(&unapplied(true, None, true)).into_iter().map(|(id, blocker)| (id, blocker.is_some())).collect();
  assert_eq!(
    blockers,
    vec![
      (ActionId::ReviewChanges, false),
      (ActionId::Reapply, true),
      (ActionId::Push, true),
      (ActionId::SafePush, true),
      (ActionId::DeleteKeptBranch, true)
    ]
  );
  let actions = summary(&unapplied(true, None, true));
  assert_eq!(actions[1].1, checked_out);
  assert_eq!(actions[2].1, Some(ActionBlocker::Cached));
}

/// An archived branch offers its review and its delete
#[test]
fn an_archived_branch_offers_its_review_and_its_delete() {
  let archived = KeptFacts {
    kind: KeptKind::Archived,
    branch: "auth",
    checked_out: false,
    remote: None,
    cached: false,
  };
  assert_eq!(summary(&archived), vec![(ActionId::ReviewChanges, None), (ActionId::DeleteKeptBranch, None)]);
}
