use crate::push::{action, drops_remote_commits, needs_push, over_moved_remote, replaces_older_versions};
use domain_types::{DroppedCommit, PushAction, RemoteStatusUpdate};
use pretty_assertions::assert_eq;

/// The counts of a remote status a case sets; the rest is a pushed branch the remote has as it is
#[derive(Clone, Copy, Default)]
struct Counts {
  absent: bool,
  ahead: usize,
  behind: u32,
  amended: u32,
  restacked: u32,
  landed: u32,
  reworded: u32,
  ancestors_ahead: u32,
  rebased: bool,
}

fn status(counts: Counts) -> RemoteStatusUpdate {
  let mut status = RemoteStatusUpdate {
    branch_name: "auth".to_string(),
    remote_exists: !counts.absent,
    remote_tip: (!counts.absent).then(|| "remote-tip".to_string()),
    unpushed_commits: (0..counts.ahead).map(|index| format!("commit-{index}")).collect(),
    commits_behind: counts.behind,
    dropped_commits: (0..counts.behind)
      .map(|index| DroppedCommit {
        id: format!("dropped-{index}"),
        subject: "Fix".to_string(),
        author: "Ann".to_string(),
        author_time: 0,
      })
      .collect(),
    commits_amended: counts.amended,
    commits_restacked: counts.restacked,
    commits_landed: counts.landed,
    landed_commits: Vec::new(),
    commits_reworded: counts.reworded,
    ancestor_commits_ahead: counts.ancestors_ahead,
    my_unpushed_count: 0,
    fast_forward: !counts.rebased && counts.behind == 0,
    last_push_time: 0,
    action: PushAction::UpToDate,
  };
  status.action = action(&status);
  status
}

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
      Counts {
        absent: true,
        ahead: 2,
        ..Counts::default()
      },
      PushAction::Create,
    ),
    (
      "no remote branch, no commit",
      Counts {
        absent: true,
        ..Counts::default()
      },
      PushAction::Create,
    ),
    ("the remote has the branch", Counts::default(), PushAction::UpToDate),
    ("new commits on the remote's line", Counts { ahead: 2, ..Counts::default() }, PushAction::Push),
    // a rebased branch with new commits: not a fast-forward of its remote
    (
      "rebased with new commits",
      Counts {
        ahead: 2,
        rebased: true,
        ..Counts::default()
      },
      force(false, false),
    ),
    ("remote commits the branch lacks", Counts { behind: 1, ..Counts::default() }, force(true, false)),
    (
      "amended here",
      Counts {
        ahead: 1,
        amended: 1,
        rebased: true,
        ..Counts::default()
      },
      force(false, true),
    ),
    // nothing of its own to push: the remote has the older versions of its parent's commits
    (
      "restacked",
      Counts {
        restacked: 1,
        rebased: true,
        ..Counts::default()
      },
      force(false, true),
    ),
    // the parent got a new commit since: the remote branch is an older piece, its Safe Push would land it
    (
      "the parent's new commit",
      Counts {
        ancestors_ahead: 1,
        rebased: true,
        ..Counts::default()
      },
      force(false, true),
    ),
    // reworded: the remote has the changes with the old message, a robot would land it
    (
      "reworded",
      Counts {
        reworded: 1,
        rebased: true,
        ..Counts::default()
      },
      force(false, true),
    ),
    // landed: the remote has older versions of commits the baseline has, nothing of them is lost
    (
      "landed",
      Counts {
        ahead: 1,
        landed: 7,
        rebased: true,
        ..Counts::default()
      },
      force(false, true),
    ),
    (
      "amended and someone's commit",
      Counts {
        ahead: 1,
        amended: 1,
        behind: 1,
        ..Counts::default()
      },
      force(true, true),
    ),
    // by content: the remote has the work of a branch only rebased onto a newer baseline
    (
      "only rebased",
      Counts {
        rebased: true,
        ..Counts::default()
      },
      PushAction::UpToDate,
    ),
  ];
  let actions: Vec<(&str, PushAction)> = cases.iter().map(|(case, counts, _)| (*case, action(&status(*counts)))).collect();
  let expected: Vec<(&str, PushAction)> = cases.iter().map(|(case, _, expected)| (*case, *expected)).collect();
  assert_eq!(actions, expected);
}

/// Up to date is the only action without work for the remote, and the parts of a force push are the rules of their own
#[test]
fn a_push_has_work_unless_the_remote_has_the_branch_as_it_is() {
  for counts in [
    Counts {
      absent: true,
      ..Counts::default()
    },
    Counts { ahead: 1, ..Counts::default() },
    Counts { behind: 1, ..Counts::default() },
    Counts { amended: 1, ..Counts::default() },
    Counts {
      restacked: 1,
      ..Counts::default()
    },
    Counts { landed: 1, ..Counts::default() },
    Counts { reworded: 1, ..Counts::default() },
    Counts {
      ancestors_ahead: 1,
      ..Counts::default()
    },
  ] {
    let remote = status(counts);
    assert!(needs_push(&remote), "{:?} has work", remote.action);
    assert_ne!(remote.action, PushAction::UpToDate);
  }
  let rebased = status(Counts {
    rebased: true,
    ..Counts::default()
  });
  assert!(!needs_push(&rebased));
  assert!(!replaces_older_versions(&rebased));
  assert!(!drops_remote_commits(&rebased));
  assert!(replaces_older_versions(&status(Counts {
    restacked: 1,
    ..Counts::default()
  })));
  assert!(drops_remote_commits(&status(Counts { behind: 2, ..Counts::default() })));
}

/// pushes again over the new remote tip when the remote has no commits the branch lacks: nothing is lost. A remote with
/// someone's commits, or one that has the branch as it is now, is told instead
#[test]
fn pushes_again_over_a_moved_remote_only_when_nothing_of_it_is_lost() {
  let cases = [
    ("moved to an older commit of the branch", Counts { ahead: 2, ..Counts::default() }, true),
    (
      "rebuilt older versions",
      Counts {
        amended: 1,
        rebased: true,
        ..Counts::default()
      },
      true,
    ),
    (
      "deleted",
      Counts {
        absent: true,
        ahead: 1,
        ..Counts::default()
      },
      true,
    ),
    ("someone's commit", Counts { behind: 1, ..Counts::default() }, false),
    (
      "someone's commit beside an amended one",
      Counts {
        ahead: 1,
        amended: 1,
        behind: 1,
        ..Counts::default()
      },
      false,
    ),
    ("has the branch as it is", Counts::default(), false),
  ];
  let answers: Vec<(&str, bool)> = cases.iter().map(|(case, counts, _)| (*case, over_moved_remote(&status(*counts)))).collect();
  let expected: Vec<(&str, bool)> = cases.iter().map(|(case, _, expected)| (*case, *expected)).collect();
  assert_eq!(answers, expected);
}
