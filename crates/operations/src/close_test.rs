use crate::close::{close_landed_plan, close_plan, close_request, close_told_plan, landed_comment};
use domain_types::fixtures::{pull_request, robot, stack_robot};
use domain_types::landing::{LandedPullRequest, LandedWith, LandingKey, PlannedEffect};
use domain_types::operation::{Intent, OperationStep, PullRequestField, Resource};
use domain_types::patronus::{PatronusPullRequestLink, PatronusPushMode, PatronusRobotStatus};
use domain_types::pull_request::PullRequestState;
use domain_types::reason::Reason;
use domain_types::session::PullRequestsView;
use pretty_assertions::assert_eq;
use repository_model::landed_closes::ToldLanding;

/// The listing of the session: the pull request #5 of auth, #6 of login and #12 of the whole air stack
fn listed() -> PullRequestsView {
  let mut listed = PullRequestsView::default();
  listed.pull_requests.insert("auth".to_owned(), pull_request("auth", 5));
  listed.pull_requests.insert("login".to_owned(), pull_request("login", 6));
  listed.stack_pull_requests.insert("air".to_owned(), pull_request("air", 12));
  listed
}

fn close(branch: &str, number: u32, comment: &str) -> PlannedEffect {
  PlannedEffect::ClosePullRequest {
    branch: branch.to_owned(),
    number,
    comment: comment.to_owned(),
  }
}

/// The close the user asks closes an open pull request with its reason, of a branch or of a whole stack; a closed one
/// gets no second comment, a merged one is refused, one the session doesn't list has no plan. The plan holds the pull
/// request as listed, and the operation the state of the pull request
#[test]
fn a_close_closes_an_open_pull_request_with_its_comment_and_nothing_else() {
  let mut listed = listed();
  let comment = "Closed in Branch Deck: auth landed.";
  let plan = close_plan(&listed, 12, comment).unwrap();
  assert_eq!(plan.effects, [close("air", 12, comment)]);
  assert_eq!((plan.blockers.len(), plan.confirmations.len()), (0, 0));
  let request = close_request(&plan.intent, Some(&plan));
  assert_eq!(
    (request.resources, request.step),
    (
      vec![Resource::PullRequest {
        number: 12,
        field: PullRequestField::State,
      }],
      OperationStep::Writing
    )
  );
  let mut moved = listed.clone();
  moved.stack_pull_requests.get_mut("air").unwrap().head_ref_oid = "newer".to_owned();
  assert_ne!(close_plan(&moved, 12, comment).unwrap().fingerprint, plan.fingerprint);

  listed.stack_pull_requests.get_mut("air").unwrap().state = PullRequestState::Closed;
  assert_eq!(close_plan(&listed, 12, comment).unwrap().effects, [] as [PlannedEffect; 0], "no second comment");
  listed.stack_pull_requests.get_mut("air").unwrap().state = PullRequestState::Merged;
  assert_eq!(close_plan(&listed, 12, comment).unwrap().blockers, [Reason::PullRequestMerged { number: 12 }]);
  assert_eq!(close_plan(&listed, 13, comment), None);
}

/// The closes of a merged robot close the pull requests the news of its merge names, each with the comment that names
/// the robot (the pull request it started from, its branch or the stack branch of the whole stack); a robot that did
/// not merge, a Dry Run among them, closes nothing
#[test]
fn the_closes_of_a_merged_robot_close_what_its_landing_closes_with_a_comment_of_the_robot() {
  let listed = listed();
  let mut oauth = robot("r1", Some("oauth"), PatronusRobotStatus::Success, 1);
  oauth.pull_request = Some(PatronusPullRequestLink {
    number: 7,
    url: "https://github.com/acme/monorepo/pull/7".to_owned(),
  });
  let closes = [
    LandedPullRequest {
      branch: "auth".to_owned(),
      number: 5,
    },
    LandedPullRequest {
      branch: "login".to_owned(),
      number: 6,
    },
  ];
  let comment = "Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch.";
  assert_eq!(landed_comment(&oauth, "user"), comment);
  let plan = close_landed_plan(&oauth, &closes, &listed, "user").unwrap();
  assert_eq!(
    plan.intent,
    Intent::CloseLandedPullRequests {
      landing: LandingKey::Robot { robot_id: "r1".to_owned() }
    }
  );
  assert_eq!(plan.effects, [close("auth", 5, comment), close("login", 6, comment)]);
  let numbers: Vec<Resource> = close_request(&plan.intent, Some(&plan)).resources;
  assert_eq!(numbers.len(), 2);

  let whole = stack_robot("r2", "air", PatronusRobotStatus::Success, "tip");
  assert_eq!(
    landed_comment(&whole, "user"),
    "Landed in master with the Safe Push of the whole air stack (user/stack/air), which holds this branch."
  );
  assert_eq!(close_landed_plan(&whole, &[], &listed, "user").unwrap().effects, [] as [PlannedEffect; 0]);

  let running = robot("r3", Some("oauth"), PatronusRobotStatus::Running, 1);
  assert_eq!(close_landed_plan(&running, &closes, &listed, "user"), None);
  let dry_run = domain_types::patronus::PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..oauth
  };
  assert_eq!(close_landed_plan(&dry_run, &closes, &listed, "user"), None);
}

fn landed(branch: &str, number: u32) -> LandedPullRequest {
  LandedPullRequest {
    branch: branch.to_owned(),
    number,
  }
}

/// The closes of a landing without a robot close what the session told of it, each with the comment that names what
/// landed it: of a push the commit it pushed and the branch or the whole stack, the branch's own pull request apart; of
/// the fetched baseline its name; of a merged pull request its number
#[test]
fn the_closes_of_a_landing_without_a_robot_name_what_landed_each_branch() {
  let listed = listed();
  let pushed = ToldLanding {
    with: LandedWith::Push {
      subject: "login".to_owned(),
      tip: "0123456789abcdef".to_owned(),
      upstream: "origin/master".to_owned(),
    },
    closes: vec![landed("auth", 5), landed("login", 6)],
  };
  let plan = close_told_plan("landing-1", &pushed, &listed, "user");
  assert_eq!(
    plan.intent,
    Intent::CloseLandedPullRequests {
      landing: LandingKey::Told { id: "landing-1".to_owned() }
    }
  );
  assert_eq!(
    plan.effects,
    [
      close("auth", 5, "Landed in master as 0123456 with the push of user/virtual/login, which is built on this branch."),
      close("login", 6, "Landed in master as 0123456 with the push of user/virtual/login."),
    ]
  );
  assert_eq!(close_request(&plan.intent, Some(&plan)).resources.len(), 2);

  let stack = ToldLanding {
    with: LandedWith::Push {
      subject: "stack:air".to_owned(),
      tip: "fedcba9876543210".to_owned(),
      upstream: "origin/master".to_owned(),
    },
    closes: vec![landed("auth", 5)],
  };
  assert_eq!(
    close_told_plan("landing-2", &stack, &listed, "user").effects,
    [close(
      "auth",
      5,
      "Landed in master as fedcba9 with the push of the whole air stack (user/stack/air), which holds this branch."
    )]
  );
  let fetched = ToldLanding {
    with: LandedWith::Baseline {
      upstream: "origin/master".to_owned(),
    },
    closes: vec![landed("auth", 5)],
  };
  assert_eq!(
    close_told_plan("landing-3", &fetched, &listed, "user").effects,
    [close("auth", 5, "Landed in master: origin/master has every commit of this branch.")]
  );
  let merged = ToldLanding {
    with: LandedWith::PullRequest {
      number: 7,
      subject: "login".to_owned(),
      target: "master".to_owned(),
    },
    closes: vec![landed("auth", 5)],
  };
  assert_eq!(
    close_told_plan("landing-4", &merged, &listed, "user").effects,
    [close(
      "auth",
      5,
      "Landed in master with #7, the pull request of user/virtual/login, which is built on this branch."
    )]
  );
}
