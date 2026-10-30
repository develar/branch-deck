use crate::facts::{RemoteBranch, Robots};
use crate::landed::{LandedFacts, LandedSubject, Landing, Of, landed_reason, landed_state, merged_pull_request, pull_requests_to_close};
use domain_types::fixtures::{pull_request, robot, robot_of, stack_robot, whole_stack_robot_of};
use domain_types::landed::LandedBranch;
use domain_types::landing::{BaselinePush, Landed, LandedBy, LandedPullRequest};
use domain_types::patronus::{PatronusPullRequestLink, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::{BranchPatronus, LandedByView, PatronusView, RobotLandedBranch, RobotOf};
use domain_types::session::{BaselineStatus, LandedSection};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// The fetched baseline `origin/master` has `remaining` commits of `name` left out
fn landing(name: &str, remaining: u32) -> LandedBranch {
  LandedBranch {
    name: Some(name.to_owned()),
    commits: Vec::new(),
    remaining,
    landed_at: 1,
  }
}

fn section(landing: Vec<LandedBranch>) -> LandedSection {
  LandedSection {
    baseline: Some(BaselineStatus {
      baseline_branch: "origin/master".to_owned(),
      update: None,
      check_error: None,
    }),
    landing,
    landed: Vec::new(),
  }
}

/// What `by` landed of each of `branches`: `(branch, own, after)`
fn view(by: &RobotOf, branches: &[(&str, bool, &[&str])]) -> PatronusView {
  let mut view = PatronusView::default();
  for (name, own, after) in branches {
    view.branches.insert(
      (*name).to_owned(),
      BranchPatronus {
        landed_by: Some(LandedByView {
          by: by.clone(),
          own: *own,
          landed: vec![format!("{name}-1")],
          after: after.iter().map(|id| (*id).to_owned()).collect(),
        }),
        ..BranchPatronus::default()
      },
    );
  }
  view
}

fn merged(id: &str, branch: &str) -> PatronusRobot {
  robot(id, Some(branch), PatronusRobotStatus::Success, 1)
}

fn with_state(state: PullRequestState, pull_request: PullRequest) -> PullRequest {
  PullRequest { state, ..pull_request }
}

/// `name` at `<name>-tip`, its remote unknown, with `pull_request`
fn branch<'a>(name: &'a str, pull_request: Option<&'a PullRequest>) -> LandedSubject<'a> {
  LandedSubject {
    of: Of::Branch(name),
    tip: Some(match name {
      "auth" => "auth-tip",
      "login" => "login-tip",
      _ => "oauth-tip",
    }),
    remote: None,
    pull_request,
  }
}

fn remote(tip: &str, up_to_date: bool, drops: u32) -> RemoteBranch<'_> {
  RemoteBranch {
    tip: Some(tip),
    up_to_date,
    drops,
    others: 0,
  }
}

/// Merged Safe Pushes into master landed it: `by` each robot, `own` the subject's own
fn robots_into_master(by: Vec<RobotOf>, own: bool) -> LandedBy {
  LandedBy::Robots {
    by,
    own,
    target: "master".to_owned(),
  }
}

/// The merged Safe Push `r` of oauth landed auth and a part of login (`login-2` after it): its robot, the robots and the
/// Patronus view
fn landed_by_oauth() -> (RobotOf, Vec<PatronusRobot>, PatronusView) {
  let oauth = robot_of("oauth", "r");
  let view = view(&oauth, &[("auth", false, &[]), ("login", false, &["login-2"])]);
  (oauth, vec![merged("r", "oauth")], view)
}

/// Row 1 of the next step: the fetched baseline has all of it first, then merged Safe Pushes (also a part of it, the
/// commits after it the work left), then its pull request merged with the commits it has (D13)
#[test]
fn what_landed_a_branch_the_baseline_first_then_merged_safe_pushes_then_its_pull_request() {
  let (oauth, robots, view) = landed_by_oauth();
  let robots = Robots::new(&robots);
  let merged_pull_request = with_state(PullRequestState::Merged, pull_request("auth", 5));
  let baseline = section(vec![landing("auth", 0), landing("login", 1)]);
  let facts = |baseline| LandedFacts {
    pushed: None,
    baseline,
    view: Some(&view),
    robots: &robots,
  };
  let in_baseline = Landed {
    by: LandedBy::Baseline {
      upstream: "origin/master".to_owned(),
    },
    after: Vec::new(),
  };
  assert_eq!(landed_state(&facts(&baseline), &branch("auth", Some(&merged_pull_request))), Some(in_baseline));
  let no_landing = section(Vec::new());
  assert_eq!(
    landed_state(&facts(&no_landing), &branch("auth", Some(&merged_pull_request))),
    Some(Landed {
      by: robots_into_master(vec![oauth.clone()], false),
      after: Vec::new(),
    })
  );
  // a part of it landed: one in the fetched baseline with commits left is no landing
  assert_eq!(
    landed_state(&facts(&baseline), &branch("login", None)),
    Some(Landed {
      by: robots_into_master(vec![oauth], false),
      after: vec!["login-2".to_owned()],
    })
  );
  let without_robots = LandedFacts {
    pushed: None,
    baseline: &no_landing,
    view: None,
    robots: &robots,
  };
  assert_eq!(
    landed_state(&without_robots, &branch("auth", Some(&merged_pull_request))),
    Some(Landed {
      by: LandedBy::PullRequest { number: 5 },
      after: Vec::new(),
    })
  );
  assert_eq!(landed_state(&without_robots, &branch("oauth", None)), None);
}

/// A merged pull request lands the branch only with the commits it has: its head is the tip, or the remote branch that
/// has what the branch has (rebuilt on a newer baseline since); one of other commits is new work under the same name,
/// and an open or closed one lands nothing
#[test]
fn a_pull_request_lands_the_branch_only_merged_with_the_commits_it_has() {
  let robots = Robots::new(&[]);
  let no_landing = section(Vec::new());
  let facts = LandedFacts {
    pushed: None,
    baseline: &no_landing,
    view: None,
    robots: &robots,
  };
  let pushed_before = PullRequest {
    head_ref_oid: "auth-pushed".to_owned(),
    ..with_state(PullRequestState::Merged, pull_request("auth", 5))
  };
  let landed = |subject: LandedSubject<'_>| landed_state(&facts, &subject).map(|landed| landed.by);
  let at = |remote| LandedSubject {
    remote: Some(remote),
    ..branch("auth", Some(&pushed_before))
  };
  assert_eq!(landed(at(remote("auth-pushed", true, 0))), Some(LandedBy::PullRequest { number: 5 }));
  assert_eq!(landed(at(remote("auth-pushed", false, 0))), None, "the branch has work its pull request lacks");
  assert_eq!(landed(at(remote("auth-other", true, 0))), None, "the remote moved on since");
  assert_eq!(landed(branch("auth", Some(&pushed_before))), None, "no remote status: only the tip tells");
  let open = pull_request("auth", 5);
  let closed = with_state(PullRequestState::Closed, pull_request("auth", 5));
  assert_eq!(landed(branch("auth", Some(&open))), None);
  assert_eq!(landed(branch("auth", Some(&closed))), None);
}

/// A whole stack landed when every branch of it did: in the fetched baseline, or by merged Safe Pushes whole (`own` when
/// its own robot landed them all, each robot once in the order they merged), or its pull request merged with the
/// commits of its stack branch
#[test]
fn a_whole_stack_landed_when_every_branch_of_it_did() {
  let members = ["auth".to_owned(), "login".to_owned()];
  let whole = Of::WholeStack { root: "auth", members: &members };
  let stack = |pull_request| LandedSubject {
    of: whole,
    tip: Some("stack-tip"),
    remote: None,
    pull_request,
  };
  let robots = [stack_robot("s", "auth", PatronusRobotStatus::Success, "stack-tip"), merged("r", "login")];
  let robots = Robots::new(&robots);
  let by_stack = view(&whole_stack_robot_of("auth", "s"), &[("auth", false, &[]), ("login", false, &[])]);
  let no_landing = section(Vec::new());
  let facts = |view| LandedFacts {
    pushed: None,
    baseline: &no_landing,
    view,
    robots: &robots,
  };
  let by = |facts: &LandedFacts<'_>, subject| landed_state(facts, &subject).map(|landed| landed.by);
  assert_eq!(
    by(&facts(Some(&by_stack)), stack(None)),
    Some(robots_into_master(vec![whole_stack_robot_of("auth", "s")], true))
  );
  let left = view(&robot_of("login", "r"), &[("login", true, &[])]);
  assert_eq!(by(&facts(Some(&left)), stack(None)), None, "auth is left");
  let all_in_baseline = section(vec![landing("auth", 0), landing("login", 0)]);
  let baseline = LandedFacts {
    pushed: None,
    baseline: &all_in_baseline,
    ..facts(None)
  };
  assert_eq!(
    by(&baseline, stack(None)),
    Some(LandedBy::Baseline {
      upstream: "origin/master".to_owned()
    })
  );
  let merged_pull_request = PullRequest {
    head_ref_oid: "stack-tip".to_owned(),
    ..with_state(PullRequestState::Merged, pull_request("auth", 12))
  };
  assert_eq!(by(&facts(None), stack(Some(&merged_pull_request))), Some(LandedBy::PullRequest { number: 12 }));
}

/// What a start says of what landed all of the subject: the code of each form; nothing while work is left
#[test]
fn the_reason_a_start_gives_of_what_landed_all_of_the_subject() {
  let (oauth, robots, view) = landed_by_oauth();
  let robots = Robots::new(&robots);
  let baseline = section(vec![landing("oauth", 0)]);
  let facts = LandedFacts {
    pushed: None,
    baseline: &baseline,
    view: Some(&view),
    robots: &robots,
  };
  assert_eq!(
    landed_reason(&facts, &branch("auth", None)),
    Some(Reason::Landed {
      branch: "auth".to_owned(),
      by: oauth,
      own: false,
      target: "master".to_owned(),
    })
  );
  assert_eq!(landed_reason(&facts, &branch("login", None)), None, "a commit after it is left");
  assert_eq!(
    landed_reason(&facts, &branch("oauth", None)),
    Some(Reason::LandedIn {
      upstream: "origin/master".to_owned()
    })
  );
  let merged_pull_request = with_state(PullRequestState::Merged, pull_request("login", 6));
  let no_robots = LandedFacts { view: None, ..facts };
  assert_eq!(
    landed_reason(&no_robots, &branch("login", Some(&merged_pull_request))),
    Some(Reason::PullRequestMerged { number: 6 })
  );
}

/// What the merged robot landed, as the Patronus view lists it (`landed_by_robot`)
fn landed_branch(name: &str, own: bool, after: &[&str]) -> RobotLandedBranch {
  RobotLandedBranch {
    branch: name.to_owned(),
    own,
    after: after.iter().map(|id| (*id).to_owned()).collect(),
  }
}

/// The branches of the table: auth at its tip, login pushed before the last sync rebuilt it on a newer baseline (its
/// remote has nothing it lacks), oauth at its tip; each with its pull request of `pull_requests`
fn table_branch<'a>(name: &str, pull_requests: &'a [(&'a str, PullRequest)], remote_of_login: RemoteBranch<'a>) -> Option<LandedSubject<'a>> {
  let (name, pull_request) = pull_requests
    .iter()
    .find(|(branch, _)| *branch == name)
    .map_or((None, None), |(branch, pull_request)| (Some(*branch), Some(pull_request)));
  let name = name?;
  Some(LandedSubject {
    remote: (name == "login").then_some(remote_of_login),
    ..branch(name, pull_request)
  })
}

fn open_at(branch: &str, number: u32, head: &str) -> PullRequest {
  PullRequest {
    head_ref_oid: head.to_owned(),
    ..pull_request(branch, number)
  }
}

fn closes(name: &str, number: u32) -> LandedPullRequest {
  LandedPullRequest { branch: name.to_owned(), number }
}

/// D7: the open pull requests of the branches a merged robot landed whole besides its own, holding nothing the branch
/// lacks (its head the tip, or the remote tip with no commit the branch lacks); the robot's own is Patronus' to close
#[test]
fn closes_the_pull_requests_of_the_branches_a_merged_robot_landed_whole_not_of_its_own_branch() {
  let robot = merged("r", "oauth");
  let pull_requests = [
    ("auth", open_at("auth", 5, "auth-tip")),
    ("login", open_at("login", 6, "login-old")),
    ("oauth", open_at("oauth", 7, "oauth-tip")),
  ];
  let landed = [landed_branch("auth", false, &[]), landed_branch("login", false, &[]), landed_branch("oauth", true, &[])];
  let subject = |name: &str| table_branch(name, &pull_requests, remote("login-old", false, 0));
  assert_eq!(
    pull_requests_to_close(&Landing::Robot { robot: &robot, landed: &landed }, subject),
    vec![closes("auth", 5), closes("login", 6)]
  );
}

/// A pull request with a commit the branch lacks, one of a branch with work left after what landed, and one of a branch
/// the table lacks stay open
#[test]
fn leaves_a_pull_request_with_a_commit_the_branch_lacks_one_of_a_branch_with_work_left_and_one_the_table_lacks() {
  let robot = merged("r", "oauth");
  let pull_requests = [("auth", open_at("auth", 5, "auth-tip")), ("login", open_at("login", 6, "login-other"))];
  let behind = |name: &str| table_branch(name, &pull_requests, remote("login-other", false, 1));
  assert_eq!(
    pull_requests_to_close(
      &Landing::Robot {
        robot: &robot,
        landed: &[landed_branch("auth", false, &[]), landed_branch("login", false, &[])],
      },
      behind
    ),
    vec![closes("auth", 5)]
  );
  let at_tip = |name: &str| table_branch(name, &pull_requests, remote("login-other", false, 0));
  assert_eq!(
    pull_requests_to_close(
      &Landing::Robot {
        robot: &robot,
        landed: &[landed_branch("auth", false, &["auth-2"]), landed_branch("gone", false, &[])],
      },
      at_tip
    ),
    Vec::new()
  );
}

/// The pull request the robot started from is Patronus' to close, of the whole stack too
#[test]
fn leaves_the_pull_request_the_robot_started_from_to_patronus() {
  let whole = PatronusRobot {
    pull_request: Some(PatronusPullRequestLink { number: 7, url: "u7".to_owned() }),
    ..stack_robot("r", "auth", PatronusRobotStatus::Success, "stack-tip")
  };
  let pull_requests = [("auth", open_at("auth", 5, "auth-tip")), ("login", open_at("login", 7, "login-tip"))];
  let subject = |name: &str| table_branch(name, &pull_requests, remote("login-tip", true, 0));
  assert_eq!(
    pull_requests_to_close(
      &Landing::Robot {
        robot: &whole,
        landed: &[landed_branch("auth", false, &[]), landed_branch("login", false, &[])],
      },
      subject
    ),
    vec![closes("auth", 5)]
  );
}

fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(|name| (*name).to_owned()).collect()
}

/// A push of the copy of a piece closes the pull requests of what it landed that show an older version of a branch (a
/// head the push left out: login pushed before an amend), its own branch's too; one whose head is its branch's tip is in
/// the target now, in the history of the copy, which GitHub merges by itself; a pull request with a commit the branch
/// lacks stays open
#[test]
fn a_push_closes_the_pull_requests_whose_head_it_left_out_and_leaves_those_in_the_history_of_its_copy_to_github() {
  let pull_requests = [
    ("auth", open_at("auth", 5, "auth-tip")),
    ("login", open_at("login", 6, "login-old")),
    ("oauth", open_at("oauth", 7, "oauth-old")),
  ];
  let subject = |name: &str| {
    let found = table_branch(name, &pull_requests, remote("login-old", false, 0))?;
    Some(match name {
      "oauth" => LandedSubject {
        remote: Some(remote("oauth-old", false, 0)),
        ..found
      },
      _ => found,
    })
  };
  let lands = names(&["auth", "login", "oauth"]);
  assert_eq!(
    pull_requests_to_close(
      &Landing::Push {
        branches: &lands,
        chain: true,
        target: "master"
      },
      subject
    ),
    vec![closes("login", 6), closes("oauth", 7)]
  );
  // a stack branch copies every branch anew: no head of a branch is in the target
  assert_eq!(
    pull_requests_to_close(
      &Landing::Push {
        branches: &lands,
        chain: false,
        target: "master"
      },
      subject
    ),
    vec![closes("auth", 5), closes("login", 6), closes("oauth", 7)]
  );
  let behind = |name: &str| table_branch(name, &pull_requests, remote("login-old", false, 1));
  assert_eq!(
    pull_requests_to_close(
      &Landing::Push {
        branches: &lands,
        chain: true,
        target: "master"
      },
      behind
    ),
    Vec::new()
  );
}

/// A push of the copy of a piece into master leaves to GitHub only a pull request into master whose head it put there: a
/// layer of a stack of GitHub above the bottom goes into the branch below it, which GitHub never marks merged, so the
/// push closes it (docs/design/github-stacks.md)
#[test]
fn a_push_closes_the_layers_of_a_stack_of_github_above_the_bottom() {
  let into = |branch: &str, number: u32, base: &str| PullRequest {
    base_ref_name: base.to_owned(),
    ..open_at(branch, number, &format!("{branch}-tip"))
  };
  let pull_requests = [("auth", into("auth", 5, "master")), ("login", into("login", 6, "user/virtual/auth"))];
  let subject = |name: &str| table_branch(name, &pull_requests, remote("login-tip", true, 0));
  let lands = names(&["auth", "login"]);
  assert_eq!(
    pull_requests_to_close(
      &Landing::Push {
        branches: &lands,
        chain: true,
        target: "master"
      },
      subject
    ),
    vec![closes("login", 6)]
  );
}

/// What the fetched baseline or a merged pull request has closes every open pull request of it that holds nothing more,
/// its head the branch's tip too: a merged or a closed one is GitHub's
#[test]
fn the_fetched_baseline_closes_every_open_pull_request_of_what_it_has_that_holds_nothing_more() {
  let pull_requests = [
    ("auth", open_at("auth", 5, "auth-tip")),
    ("login", with_state(PullRequestState::Merged, open_at("login", 6, "login-tip"))),
    ("oauth", open_at("oauth", 7, "oauth-other")),
  ];
  let subject = |name: &str| table_branch(name, &pull_requests, remote("login-tip", true, 0));
  let landed = names(&["auth", "login", "oauth", "gone"]);
  assert_eq!(pull_requests_to_close(&Landing::Content { branches: &landed }, subject), vec![closes("auth", 5)]);
}

/// The pull request GitHub merged with the commits of the branch, not one with other commits, not an open one
#[test]
fn names_the_pull_request_github_merged_with_the_commits_of_the_branch() {
  let merged = with_state(PullRequestState::Merged, open_at("auth", 5, "auth-tip"));
  assert_eq!(merged_pull_request(&branch("auth", Some(&merged))), Some(5));
  let other = with_state(PullRequestState::Merged, open_at("auth", 5, "auth-other"));
  assert_eq!(merged_pull_request(&branch("auth", Some(&other))), None);
  let open = open_at("auth", 5, "auth-tip");
  assert_eq!(merged_pull_request(&branch("auth", Some(&open))), None);
}

/// A push of this session into the target landed the copy of the subject at the tip it is at, before any fetch shows
/// it: in the baseline it went into, first of all; a rebuild or a new commit since is new work, and a whole stack is
/// told by its key
#[test]
fn a_push_of_this_session_landed_the_subject_at_the_tip_it_pushed() {
  let robots = Robots::new(&[]);
  let no_landing = section(Vec::new());
  let pushed = BTreeMap::from([
    (
      "auth".to_owned(),
      BaselinePush {
        tip: "auth-tip".to_owned(),
        upstream: "origin/master".to_owned(),
      },
    ),
    (
      "stack:auth".to_owned(),
      BaselinePush {
        tip: "stack-tip".to_owned(),
        upstream: "origin/master".to_owned(),
      },
    ),
  ]);
  let facts = LandedFacts {
    pushed: Some(&pushed),
    baseline: &no_landing,
    view: None,
    robots: &robots,
  };
  let in_master = Some(Landed {
    by: LandedBy::Baseline {
      upstream: "origin/master".to_owned(),
    },
    after: Vec::new(),
  });
  assert_eq!(landed_state(&facts, &branch("auth", None)), in_master);
  let rebuilt = LandedSubject {
    tip: Some("rebuilt"),
    ..branch("auth", None)
  };
  assert_eq!(landed_state(&facts, &rebuilt), None);
  let members = ["auth".to_owned(), "login".to_owned()];
  let stack = LandedSubject {
    of: Of::WholeStack { root: "auth", members: &members },
    tip: Some("stack-tip"),
    remote: None,
    pull_request: None,
  };
  assert_eq!(landed_state(&facts, &stack), in_master);
}
