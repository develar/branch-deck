use crate::shown::{
  ShowCandidates, close_comment, close_refusal, is_shown, landing_robot, recorded_reviewers, review_state, reviewers_record, show_candidates, shown_news, shown_review,
};
use domain_types::code_owners::{GroupMember, OwnerGroup};
use domain_types::fixtures::{pull_request, robot};
use domain_types::patronus::{PatronusPushMode, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, Reviewer, ReviewerState, ShownState};
use domain_types::reason::Reason;
use domain_types::session::NewsEvent;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// The shown review #12 of auth, ready for review, with the latest reviews `reviewers`
fn shown(reviewers: &[(&str, ReviewerState, bool)]) -> PullRequest {
  PullRequest {
    head_ref_name: "user/review/auth/head".to_owned(),
    base_ref_name: "user/review/auth/base".to_owned(),
    reviewers: reviewers
      .iter()
      .map(|(login, state, requested)| Reviewer {
        login: (*login).to_owned(),
        state: *state,
        requested: *requested,
      })
      .collect(),
    ..pull_request("auth", 12)
  }
}

fn logins(logins: &[&str]) -> Vec<String> {
  logins.iter().map(|login| (*login).to_owned()).collect()
}

#[test]
fn a_shown_review_is_told_by_its_base_of_any_prefix() {
  assert!(is_shown("user/review/auth/base"));
  assert!(is_shown("jane/team/review/feature/sub/base"));
  assert!(!is_shown("master"));
  assert!(!is_shown("user/virtual/auth"));
  assert!(!is_shown("user/review/auth/head"));
  assert!(!is_shown("review/base"));
}

#[test]
fn the_record_of_the_body_names_the_reviewers_it_asks() {
  let body = format!("Adds auth.\n\n{}\n", reviewers_record(&logins(&["alice", "bob"])));
  assert_eq!(recorded_reviewers(&body), logins(&["alice", "bob"]));
  assert_eq!(recorded_reviewers("Adds auth."), Vec::<String>::new());
}

#[test]
fn approved_once_every_reviewer_it_asked_approved_never_by_github_decision_or_others() {
  let asked = logins(&["alice", "bob"]);
  // GitHub's decision is approved after the first approval: bob still waits
  let mut first = shown(&[("alice", ReviewerState::Approved, false), ("bob", ReviewerState::Requested, true)]);
  first.review_decision = Some(domain_types::pull_request::ReviewDecision::Approved);
  assert_eq!(review_state(&first, &asked), ShownState::InReview { waiting: logins(&["bob"]) });
  // a bot that requests changes and someone not asked count for nothing
  let both = shown(&[
    ("alice", ReviewerState::Approved, false),
    ("bob", ReviewerState::Approved, false),
    ("copilot-pull-request-reviewer[bot]", ReviewerState::ChangesRequested, false),
    ("carol", ReviewerState::ChangesRequested, false),
  ]);
  assert_eq!(review_state(&both, &asked), ShownState::Approved { by: asked.clone() });
  // a reviewer asked again after the approval waits again
  let again = shown(&[("alice", ReviewerState::Approved, true), ("bob", ReviewerState::Approved, false)]);
  assert_eq!(review_state(&again, &asked), ShownState::InReview { waiting: logins(&["alice"]) });
}

#[test]
fn changes_requested_by_a_reviewer_it_asked_return_it_to_the_author() {
  let asked = logins(&["alice", "bob"]);
  let changes = shown(&[("alice", ReviewerState::ChangesRequested, false), ("bob", ReviewerState::Approved, false)]);
  assert_eq!(review_state(&changes, &asked), ShownState::ChangesRequested { by: logins(&["alice"]) });
  // a comment is no approval
  let commented = shown(&[("alice", ReviewerState::Commented, false), ("bob", ReviewerState::Approved, false)]);
  assert_eq!(review_state(&commented, &asked), ShownState::InReview { waiting: logins(&["alice"]) });
}

#[test]
fn a_draft_waits_for_the_landing_and_one_without_a_record_never_closes() {
  let draft = PullRequest { is_draft: true, ..shown(&[]) };
  assert_eq!(review_state(&draft, &logins(&["alice"])), ShownState::Draft);
  let nobody = shown(&[("alice", ReviewerState::Approved, false)]);
  assert_eq!(review_state(&nobody, &[]), ShownState::InReview { waiting: Vec::new() });
}

fn member(login: Option<&str>, available: bool, rotated: bool) -> GroupMember {
  GroupMember {
    username: login.unwrap_or("nologin").to_owned(),
    github_login: login.map(str::to_owned),
    available,
    rotated,
  }
}

#[test]
fn one_available_reviewer_of_each_owner_group_never_the_author() {
  let air = OwnerGroup {
    name: "Air Plugin".to_owned(),
    // the reviewers of the group come before its members
    reviewers: vec![
      member(Some("me"), true, false),
      member(Some("away"), false, false),
      member(Some("passed"), true, true),
      member(Some("rita"), true, false),
    ],
    members: vec![member(Some("mike"), true, false)],
  };
  let platform = OwnerGroup {
    name: "IJPL Code Platform".to_owned(),
    reviewers: Vec::new(),
    members: vec![member(None, true, false), member(Some("paul"), true, false)],
  };
  // rita reviews both: one is enough
  let shared = OwnerGroup {
    name: "Shared".to_owned(),
    reviewers: Vec::new(),
    members: vec![member(Some("sam"), true, false), member(Some("Rita"), true, false)],
  };
  let nobody = OwnerGroup {
    name: "Nobody".to_owned(),
    reviewers: Vec::new(),
    members: vec![member(Some("me"), true, false), member(Some("off"), false, false)],
  };
  assert_eq!(
    show_candidates(&[air, platform, shared, nobody], "Me"),
    ShowCandidates {
      reviewers: logins(&["rita", "paul"]),
      without_reviewer: logins(&["Nobody"]),
    }
  );
}

#[test]
fn a_close_waits_for_every_approval_unless_the_user_says_so() {
  let asked = format!("Adds auth.\n{}", reviewers_record(&logins(&["alice", "bob"])));
  let waiting = shown_review(shown(&[("alice", ReviewerState::Approved, false), ("bob", ReviewerState::Requested, true)]), &asked);
  assert_eq!(
    close_refusal(&waiting, false),
    Some(Reason::ReviewNotApproved {
      number: 12,
      waiting: logins(&["bob"])
    })
  );
  assert_eq!(close_refusal(&waiting, true), None);
  assert_eq!(close_comment(&waiting, "master"), "Closed without the approval of bob. The change is in master.");
  let approved = shown_review(shown(&[("alice", ReviewerState::Approved, false), ("bob", ReviewerState::Approved, false)]), &asked);
  assert_eq!(close_refusal(&approved, false), None);
  assert_eq!(
    close_comment(&approved, "master"),
    "Reviewed after landing: approved by alice, bob. The change is in master."
  );
}

#[test]
fn a_draft_opens_once_a_safe_push_of_its_branch_merged_and_an_approved_one_closes() {
  let record = reviewers_record(&logins(&["alice"]));
  let draft = shown_review(PullRequest { is_draft: true, ..shown(&[]) }, &record);
  let approved = shown_review(
    PullRequest {
      number: 13,
      ..shown(&[("alice", ReviewerState::Approved, false)])
    },
    &record,
  );
  let reviews = BTreeMap::from([("auth".to_owned(), draft), ("login".to_owned(), approved)]);
  let merged = |id: &str, mode: PatronusPushMode, start: u32| domain_types::patronus::PatronusRobot {
    source_revision: Some(format!("{id}-source")),
    push_mode: mode,
    ..robot(id, Some("auth"), PatronusRobotStatus::Success, start)
  };
  let running = robot("running", Some("auth"), PatronusRobotStatus::Running, 3);
  // a robot still running opens nothing, nor does a merged Dry Run
  let dry = merged("dry", PatronusPushMode::DryRun, 4);
  assert_eq!(
    shown_news(&reviews, &[&running, &dry]),
    [NewsEvent::ShownReviewApproved {
      branch: "login".to_owned(),
      number: 13
    }]
  );
  let (older, newer) = (
    merged("older", PatronusPushMode::RebaseAutosquash, 1),
    merged("newer", PatronusPushMode::RebaseAutosquash, 2),
  );
  assert_eq!(
    shown_news(&reviews, &[&older, &newer, &running]),
    [
      NewsEvent::ShownReviewLanded {
        branch: "auth".to_owned(),
        number: 12
      },
      NewsEvent::ShownReviewApproved {
        branch: "login".to_owned(),
        number: 13
      }
    ]
  );
  // the newest merged one says what landed
  assert_eq!(landing_robot([&older, &newer, &running], "auth").map(|robot| robot.id.as_str()), Some("newer"));
}
