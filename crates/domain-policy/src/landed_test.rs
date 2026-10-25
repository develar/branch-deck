use crate::facts::{RemoteBranch, Robots};
use crate::landed::{LandedFacts, LandedSubject, Of, landed_reason, landed_state, pull_requests_to_close};
use domain_types::fixtures::{pull_request, robot, robot_of, stack_robot, whole_stack_robot_of};
use domain_types::landed::LandedBranch;
use domain_types::landing::{Landed, LandedBy, LandedPullRequest};
use domain_types::patronus::{PatronusPullRequestLink, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::reason::Reason;
use domain_types::repository_view::{BranchPatronus, LandedByView, PatronusView, RobotLandedBranch, RobotOf};
use domain_types::session::{BaselineStatus, LandedSection};
use pretty_assertions::assert_eq;

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
  assert_eq!(pull_requests_to_close(&robot, &landed, subject), vec![closes("auth", 5), closes("login", 6)]);
}

/// A pull request with a commit the branch lacks, one of a branch with work left after what landed, and one of a branch
/// the table lacks stay open
#[test]
fn leaves_a_pull_request_with_a_commit_the_branch_lacks_one_of_a_branch_with_work_left_and_one_the_table_lacks() {
  let robot = merged("r", "oauth");
  let pull_requests = [("auth", open_at("auth", 5, "auth-tip")), ("login", open_at("login", 6, "login-other"))];
  let behind = |name: &str| table_branch(name, &pull_requests, remote("login-other", false, 1));
  assert_eq!(
    pull_requests_to_close(&robot, &[landed_branch("auth", false, &[]), landed_branch("login", false, &[])], behind),
    vec![closes("auth", 5)]
  );
  let at_tip = |name: &str| table_branch(name, &pull_requests, remote("login-other", false, 0));
  assert_eq!(
    pull_requests_to_close(&robot, &[landed_branch("auth", false, &["auth-2"]), landed_branch("gone", false, &[])], at_tip),
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
    pull_requests_to_close(&whole, &[landed_branch("auth", false, &[]), landed_branch("login", false, &[])], subject),
    vec![closes("auth", 5)]
  );
}
