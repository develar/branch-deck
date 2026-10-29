//! The predicates of a remote status by its counts (`remote_status`): what a push does, whether it has work, whether a
//! push refused over a moved remote goes again

use crate::PushAction;
use crate::fixtures::{RemoteCounts, remote_status_with};
use pretty_assertions::assert_eq;

const fn force(drops: bool, replaces: bool) -> PushAction {
  PushAction::Force { drops, replaces }
}

/// The action of a push by the counts of its remote status, which compare content: the push button, the outgoing bars,
/// the toasts and `bd push` read it
#[test]
fn a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status() {
  let cases = [
    (
      "no remote branch",
      RemoteCounts {
        absent: true,
        ahead: 2,
        ..RemoteCounts::default()
      },
      PushAction::Create,
    ),
    (
      "no remote branch, no commit",
      RemoteCounts {
        absent: true,
        ..RemoteCounts::default()
      },
      PushAction::Create,
    ),
    ("the remote has the branch", RemoteCounts::default(), PushAction::UpToDate),
    (
      "new commits on the remote's line",
      RemoteCounts {
        ahead: 2,
        ..RemoteCounts::default()
      },
      PushAction::Push,
    ),
    // a rebased branch with new commits: not a fast-forward of its remote
    (
      "rebased with new commits",
      RemoteCounts {
        ahead: 2,
        rebased: true,
        ..RemoteCounts::default()
      },
      force(false, false),
    ),
    (
      "remote commits the branch lacks",
      RemoteCounts {
        behind: 1,
        ..RemoteCounts::default()
      },
      force(true, false),
    ),
    (
      "amended here",
      RemoteCounts {
        ahead: 1,
        amended: 1,
        rebased: true,
        ..RemoteCounts::default()
      },
      force(false, true),
    ),
    // nothing of its own to push: the remote has the older versions of its parent's commits
    (
      "restacked",
      RemoteCounts {
        restacked: 1,
        rebased: true,
        ..RemoteCounts::default()
      },
      force(false, true),
    ),
    // the parent got a new commit since: the remote branch is an older piece, its Safe Push would land it
    (
      "the parent's new commit",
      RemoteCounts {
        ancestors_ahead: 1,
        rebased: true,
        ..RemoteCounts::default()
      },
      force(false, true),
    ),
    // reworded: the remote has the changes with the old message, a robot would land it
    (
      "reworded",
      RemoteCounts {
        reworded: 1,
        rebased: true,
        ..RemoteCounts::default()
      },
      force(false, true),
    ),
    // landed: the remote has older versions of commits the baseline has, nothing of them is lost
    (
      "landed",
      RemoteCounts {
        ahead: 1,
        landed: 7,
        rebased: true,
        ..RemoteCounts::default()
      },
      force(false, true),
    ),
    (
      "amended and someone's commit",
      RemoteCounts {
        ahead: 1,
        amended: 1,
        behind: 1,
        ..RemoteCounts::default()
      },
      force(true, true),
    ),
    // by content: the remote has the work of a branch only rebased onto a newer baseline
    (
      "only rebased",
      RemoteCounts {
        rebased: true,
        ..RemoteCounts::default()
      },
      PushAction::UpToDate,
    ),
  ];
  let actions: Vec<(&str, PushAction)> = cases.iter().map(|(case, counts, _)| (*case, remote_status_with(*counts).push_action())).collect();
  let expected: Vec<(&str, PushAction)> = cases.iter().map(|(case, _, expected)| (*case, *expected)).collect();
  assert_eq!(actions, expected);
}

/// Up to date is the only action without work for the remote, and the parts of a force push are predicates of their own
#[test]
fn a_push_has_work_unless_the_remote_has_the_branch_as_it_is() {
  for counts in [
    RemoteCounts {
      absent: true,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      ahead: 1,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      behind: 1,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      amended: 1,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      restacked: 1,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      landed: 1,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      reworded: 1,
      ..RemoteCounts::default()
    },
    RemoteCounts {
      ancestors_ahead: 1,
      ..RemoteCounts::default()
    },
  ] {
    let remote = remote_status_with(counts);
    assert!(remote.needs_push(), "{:?} has work", remote.action);
    assert_ne!(remote.action, PushAction::UpToDate);
  }
  let rebased = remote_status_with(RemoteCounts {
    rebased: true,
    ..RemoteCounts::default()
  });
  assert!(!rebased.needs_push());
  assert!(!rebased.replaces_older_versions());
  assert!(!rebased.drops_remote_commits());
  assert!(
    remote_status_with(RemoteCounts {
      restacked: 1,
      ..RemoteCounts::default()
    })
    .replaces_older_versions()
  );
  assert!(
    remote_status_with(RemoteCounts {
      behind: 2,
      ..RemoteCounts::default()
    })
    .drops_remote_commits()
  );
}

/// pushes again over the new remote tip when the remote has no commits the branch lacks: nothing is lost. A remote with
/// someone's commits, or one that has the branch as it is now, is told instead
#[test]
fn pushes_again_over_a_moved_remote_only_when_nothing_of_it_is_lost() {
  let cases = [
    (
      "moved to an older commit of the branch",
      RemoteCounts {
        ahead: 2,
        ..RemoteCounts::default()
      },
      true,
    ),
    (
      "rebuilt older versions",
      RemoteCounts {
        amended: 1,
        rebased: true,
        ..RemoteCounts::default()
      },
      true,
    ),
    (
      "deleted",
      RemoteCounts {
        absent: true,
        ahead: 1,
        ..RemoteCounts::default()
      },
      true,
    ),
    (
      "someone's commit",
      RemoteCounts {
        behind: 1,
        ..RemoteCounts::default()
      },
      false,
    ),
    (
      "someone's commit beside an amended one",
      RemoteCounts {
        ahead: 1,
        amended: 1,
        behind: 1,
        ..RemoteCounts::default()
      },
      false,
    ),
    ("has the branch as it is", RemoteCounts::default(), false),
  ];
  let answers: Vec<(&str, bool)> = cases.iter().map(|(case, counts, _)| (*case, remote_status_with(*counts).pushes_again())).collect();
  let expected: Vec<(&str, bool)> = cases.iter().map(|(case, _, expected)| (*case, *expected)).collect();
  assert_eq!(answers, expected);
}
