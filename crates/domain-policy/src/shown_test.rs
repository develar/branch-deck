use crate::shown::{
  CLOSED_ELSEWHERE_DELAY, ContentLanding, LandedBy, LandedHead, ShowCandidates, close_comment, close_refusal, closed_elsewhere, closed_remains, fix_conflict_comment,
  fix_taken_comment, is_shown, landed_by_content, landed_head, landed_record, landing_robot, mark_land, mark_refusals, pending_fixes, recorded_landed, recorded_reviewers,
  review_state, reviewed_before_line, reviewers_record, show_candidates, shown_news, shown_review, state_now, with_record,
};
use domain_types::code_owners::{GroupMember, OwnerGroup};
use domain_types::fixtures::{pull_request, robot};
use domain_types::landed::{LandedBranch, LandedCommit};
use domain_types::landing::{FactWait, LandFact};
use domain_types::patronus::PatronusRobot;
use domain_types::patronus::{PatronusPushMode, PatronusRobotStatus};
use domain_types::pull_request::PullRequestState;
use domain_types::pull_request::{ClosedShownReview, PullRequest, Reviewer, ReviewerState, ShownReview, ShownState};
use domain_types::reason::Reason;
use domain_types::session::LandedSection;
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

/// No pull request of any branch, into master
const NO_PULLS: (&BTreeMap<String, PullRequest>, &str) = (&BTreeMap::new(), "master");

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
  assert_eq!(review_state(&draft, &logins(&["alice"])), ShownState::Draft { landing: false });
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
  let merged = |id: &str, mode: PatronusPushMode, start: u32| PatronusRobot {
    source_revision: Some(format!("{id}-source")),
    push_mode: mode,
    ..robot(id, Some("auth"), PatronusRobotStatus::Success, start)
  };
  let running = robot("running", Some("auth"), PatronusRobotStatus::Running, 3);
  // a robot still running opens nothing, nor does a merged Dry Run
  let dry = merged("dry", PatronusPushMode::DryRun, 4);
  assert_eq!(
    shown_news(&reviews, &[&running, &dry], NO_PULLS, &BTreeMap::new(), &[], 0),
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
    shown_news(&reviews, &[&older, &newer, &running], NO_PULLS, &BTreeMap::new(), &[], 0),
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

#[test]
fn the_record_of_the_fixes_it_took_goes_beside_the_reviewers_and_keeps_the_text() {
  let body = format!("Adds auth.\n\n{}", reviewers_record(&logins(&["alice"])));
  assert_eq!(recorded_landed(&body), Vec::<String>::new());
  let recorded = with_record(&body, &logins(&["alice", "bob"]), &logins(&["s2", "s3"]), None);
  assert_eq!(
    recorded,
    format!("Adds auth.\n\n{}\n{}", reviewers_record(&logins(&["alice", "bob"])), landed_record(&logins(&["s2", "s3"])))
  );
  assert_eq!(
    (recorded_reviewers(&recorded), recorded_landed(&recorded)),
    (logins(&["alice", "bob"]), logins(&["s2", "s3"]))
  );
  // a second record replaces the first
  assert_eq!(with_record(&recorded, &logins(&["alice", "bob"]), &logins(&["s2", "s3"]), None), recorded);
  assert_eq!(with_record("", &logins(&["alice"]), &[], None), reviewers_record(&logins(&["alice"])));
  assert_eq!(
    fix_conflict_comment("0123456789abcdef", "https://patronus/robot/r2"),
    "Landed a follow-up as 0123456789; it does not apply on this review's commits: https://patronus/robot/r2"
  );
}

#[test]
fn the_comment_of_a_fix_that_took_files_names_them_as_a_list_of_names_does() {
  let url = "https://patronus/robot/r2";
  assert_eq!(
    fix_taken_comment("0123456789abcdef", &logins(&["auth.rs"]), url, "master"),
    "Landed a follow-up as 0123456789; `auth.rs` also shows what master changed in it after this review: https://patronus/robot/r2"
  );
  assert_eq!(
    fix_taken_comment("0123456789abcdef", &logins(&["a", "b", "c", "d"]), url, "main"),
    "Landed a follow-up as 0123456789; `a`, `b`, `c` and `d` also show what main changed in them after this review: https://patronus/robot/r2"
  );
  // from five on the first three and the count
  assert_eq!(
    fix_taken_comment("0123456789abcdef", &logins(&["a", "b", "c", "d", "e"]), url, "master"),
    "Landed a follow-up as 0123456789; `a`, `b`, `c` and 2 more also show what master changed in them after this review: https://patronus/robot/r2"
  );
}

/// A merged Safe Push `id` of auth that started at `start` on `source`
fn merged(id: &str, source: &str, start: u32) -> PatronusRobot {
  PatronusRobot {
    source_revision: Some(source.to_owned()),
    ..robot(id, Some("auth"), PatronusRobotStatus::Success, start)
  }
}

#[test]
fn a_fix_that_merged_after_the_review_opened_waits_until_the_review_took_it() {
  let record = reviewers_record(&logins(&["alice"]));
  // opened on what r1 landed, alice requested changes
  let open = shown_review(
    PullRequest {
      head_ref_oid: "s1".to_owned(),
      ..shown(&[("alice", ReviewerState::ChangesRequested, false)])
    },
    &record,
  );
  let (before, first) = (merged("r0", "s0", 1), merged("r1", "s1", 2));
  // an older Safe Push of the branch and the one it shows: nothing to take, the author's move
  assert_eq!(pending_fixes(&open, "auth", [&before, &first]), Vec::<&PatronusRobot>::new());
  assert_eq!(state_now(&open, "auth", [&before, &first]), ShownState::ChangesRequested { by: logins(&["alice"]) });
  // the fix runs: the review waits for it, no one's move
  let fix = robot("r2", Some("auth"), PatronusRobotStatus::Running, 3);
  let dry = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..robot("dry", Some("auth"), PatronusRobotStatus::Running, 4)
  };
  assert_eq!(state_now(&open, "auth", [&first, &dry]), ShownState::ChangesRequested { by: logins(&["alice"]) });
  assert_eq!(state_now(&open, "auth", [&first, &fix]), ShownState::Extending { robot: "r2".to_owned() });
  // it merged: due until the record names it, the newest told
  let (fix, next) = (merged("r2", "s2", 3), merged("r3", "s3", 4));
  assert_eq!(
    pending_fixes(&open, "auth", [&next, &first, &fix])
      .iter()
      .map(|robot| robot.id.as_str())
      .collect::<Vec<_>>(),
    ["r2", "r3"]
  );
  assert_eq!(state_now(&open, "auth", [&first, &fix, &next]), ShownState::Extending { robot: "r3".to_owned() });
  let reviews = BTreeMap::from([("auth".to_owned(), open)]);
  assert_eq!(
    shown_news(&reviews, &[&first, &fix, &next], NO_PULLS, &BTreeMap::new(), &[], 0),
    [NewsEvent::ShownReviewFixLanded {
      branch: "auth".to_owned(),
      number: 12,
      robot_id: "r3".to_owned()
    }]
  );
  // taken (its head moved past s1): the record names them, and the reviews say what it waits for
  let extended = shown_review(
    PullRequest {
      head_ref_oid: "h3".to_owned(),
      ..shown(&[("alice", ReviewerState::ChangesRequested, true)])
    },
    &with_record("", &logins(&["alice"]), &logins(&["s2", "s3"]), None),
  );
  assert_eq!(pending_fixes(&extended, "auth", [&first, &fix, &next]), Vec::<&PatronusRobot>::new());
  assert_eq!(state_now(&extended, "auth", [&first, &fix, &next]), ShownState::InReview { waiting: logins(&["alice"]) });
  // an approval waits for the fix that lands: nothing closes while it runs
  let approved = shown_review(
    PullRequest {
      head_ref_oid: "s1".to_owned(),
      ..shown(&[("alice", ReviewerState::Approved, false)])
    },
    &record,
  );
  let running = robot("r2", Some("auth"), PatronusRobotStatus::Running, 3);
  assert_eq!(
    shown_news(
      &BTreeMap::from([("auth".to_owned(), approved.clone())]),
      &[&first, &running],
      NO_PULLS,
      &BTreeMap::new(),
      &[],
      0
    ),
    []
  );
  let extending = ShownReview {
    state: ShownState::Extending { robot: "r2".to_owned() },
    ..approved
  };
  assert_eq!(
    close_refusal(&extending, false),
    Some(Reason::ReviewNotApproved {
      number: 12,
      waiting: logins(&["alice"])
    })
  );
  // a draft takes no fix: its open lands the branch
  let draft = shown_review(PullRequest { is_draft: true, ..shown(&[]) }, &record);
  assert_eq!(pending_fixes(&draft, "auth", [&first, &fix]), Vec::<&PatronusRobot>::new());
}

#[test]
fn a_draft_whose_branch_the_baseline_landed_opens_while_no_robot_of_it_is_listed() {
  let draft = shown_review(PullRequest { is_draft: true, ..shown(&[]) }, &reviewers_record(&logins(&["alice"])));
  let reviews = BTreeMap::from([("auth".to_owned(), draft)]);
  let commit = LandedCommit {
    id: "c1".to_owned(),
    subject: "Add auth".to_owned(),
    upstream: None,
  };
  let section = LandedSection {
    baseline: None,
    landing: vec![
      LandedBranch {
        name: Some("auth".to_owned()),
        commits: vec![commit.clone()],
        remaining: 0,
        landed_at: 1,
      },
      LandedBranch {
        name: Some("login".to_owned()),
        commits: vec![commit],
        remaining: 1,
        landed_at: 1,
      },
    ],
    landed: Vec::new(),
  };
  // only the branch all of whose commits landed
  let landed = landed_by_content(&section);
  assert_eq!(landed, BTreeMap::from([("auth".to_owned(), 1)]));
  let none: [&PatronusRobot; 0] = [];
  assert_eq!(
    shown_news(&reviews, &none, NO_PULLS, &landed, &[], 0),
    [NewsEvent::ShownReviewLanded {
      branch: "auth".to_owned(),
      number: 12
    }]
  );
  // its robot still runs: it opens once that one merged
  let running = robot("r1", Some("auth"), PatronusRobotStatus::Running, 1);
  assert_eq!(shown_news(&reviews, &[&running], NO_PULLS, &landed, &[], 0), []);
}

/// The shown review `number` of `branch` closed at 1000, its head and base at `head` and `base` as it ended, the remote's
/// branches of it at `head_tip` and `base_tip`
fn closed(branch: &str, number: u32, head_tip: Option<&str>, base_tip: Option<&str>) -> ClosedShownReview {
  ClosedShownReview {
    branch: branch.to_owned(),
    number,
    url: format!("https://github.com/o/r/pull/{number}"),
    head_ref_oid: "head".to_owned(),
    base_ref_oid: "base".to_owned(),
    closed_at: 1000,
    head_tip: head_tip.map(str::to_owned),
    base_tip: base_tip.map(str::to_owned),
  }
}

#[test]
fn a_review_closed_elsewhere_an_hour_ago_loses_the_branches_still_at_its_tips_unless_the_branch_has_an_open_one() {
  let after = 1000 + CLOSED_ELSEWHERE_DELAY;
  let auth = closed("auth", 12, Some("head"), Some("base"));
  // its head moved (a newer review took the name), its base still at its tip: only the base goes
  let login = closed("login", 13, Some("newer"), Some("base"));
  // both gone or moved: nothing to delete
  let gone = closed("gone", 14, None, Some("other"));
  let all = [auth.clone(), login.clone(), gone.clone()];
  assert_eq!(closed_remains(&auth), Some((Some("head".to_owned()), Some("base".to_owned()))));
  assert_eq!(closed_remains(&login), Some((None, Some("base".to_owned()))));
  assert_eq!(closed_remains(&gone), None);
  assert_eq!(
    shown_news(&BTreeMap::new(), &[], NO_PULLS, &BTreeMap::new(), &all, after),
    [
      NewsEvent::ShownReviewClosedElsewhere {
        branch: "auth".to_owned(),
        number: 12
      },
      NewsEvent::ShownReviewClosedElsewhere {
        branch: "login".to_owned(),
        number: 13
      }
    ]
  );
  // within the hour after its close: nothing yet
  assert_eq!(closed_elsewhere(&all, &BTreeMap::new(), after - 1), Vec::<&ClosedShownReview>::new());
  // an open review of auth is listed: its branches stay
  let open = BTreeMap::from([("auth".to_owned(), shown_review(shown(&[]), &reviewers_record(&logins(&["alice"]))))]);
  assert_eq!(closed_elsewhere(&all, &open, after), [&login]);
  // two closed of auth, on the same branches: the newest close deletes them, once
  let newer = ClosedShownReview {
    number: 15,
    closed_at: 1010,
    ..auth.clone()
  };
  assert_eq!(closed_elsewhere(&[newer.clone(), auth], &BTreeMap::new(), after + 10), [&newer]);
}

/// The draft #12 of auth marked at 100 (`created_at`), recording alice
fn mark() -> ShownReview {
  ShownReview {
    created_at: 100,
    ..shown_review(PullRequest { is_draft: true, ..shown(&[]) }, &reviewers_record(&logins(&["alice"])))
  }
}

/// A merged Safe Push `id` of auth on `source` that started at `start` and finished a minute later
fn finished(id: &str, source: &str, start: u32) -> PatronusRobot {
  PatronusRobot {
    finish_time: Some(start + 60),
    ..merged(id, source, start)
  }
}

#[test]
fn a_mark_opens_by_what_landed_its_branch_since_never_by_an_older_robot_or_pull_request_of_the_name() {
  let draft = mark();
  // an old robot of an earlier auth merged before the mark, a pull request of it merged then: nothing landed it since
  let old = finished("r0", "s0", 10);
  let old_pull = PullRequest {
    state: PullRequestState::Merged,
    closed_at: Some(70),
    ..pull_request("auth", 7)
  };
  assert_eq!(landed_head(Some(&draft), "auth", [&old], Some(&old_pull), "master", None), None);
  let reviews = BTreeMap::from([("auth".to_owned(), draft.clone())]);
  let pulls = BTreeMap::from([("auth".to_owned(), old_pull)]);
  assert_eq!(shown_news(&reviews, &[&old], (&pulls, "master"), &BTreeMap::new(), &[], 0), []);
  // its own Safe Push since, or a safe-merge someone started from its pull request: the commit the robot took
  let landing = finished("r1", "s1", 200);
  let by_robot = landed_head(Some(&draft), "auth", [&old, &landing], None, "master", None);
  assert_eq!(
    by_robot,
    Some(LandedHead {
      by: LandedBy::Robot { robot: "r1".to_owned() },
      head: Some("s1".to_owned()),
      reviewed_in: None,
    })
  );
  // a robot still running: the draft lands, it opens nothing yet
  let running = robot("r2", Some("auth"), PatronusRobotStatus::Running, 300);
  assert_eq!(state_now(&draft, "auth", [&running]), ShownState::Draft { landing: true });
  assert_eq!(state_now(&draft, "auth", [&old]), ShownState::Draft { landing: false });
  // the pull request merged into master since, its robot not listed: once the baseline has the branch whole, its head
  let reviewed = PullRequest {
    state: PullRequestState::Merged,
    closed_at: Some(400),
    head_ref_oid: "p1".to_owned(),
    ..pull_request("auth", 8)
  };
  let content = ContentLanding { landed_at: 410, tip: Some("tip") };
  assert_eq!(landed_head(Some(&draft), "auth", [&old], Some(&reviewed), "master", None), None);
  assert_eq!(
    landed_head(Some(&draft), "auth", [&old], Some(&reviewed), "master", Some(content)),
    Some(LandedHead {
      by: LandedBy::PullRequest { number: 8 },
      head: Some("p1".to_owned()),
      reviewed_in: Some(8),
    })
  );
  // one into another branch lands nothing: by content only, on the tip of the branch
  let elsewhere = PullRequest {
    base_ref_name: "user/virtual/util".to_owned(),
    ..reviewed.clone()
  };
  assert_eq!(
    landed_head(Some(&draft), "auth", [&old], Some(&elsewhere), "master", Some(content)),
    Some(LandedHead {
      by: LandedBy::Content,
      head: Some("tip".to_owned()),
      reviewed_in: None,
    })
  );
  // a landing by content found before the mark, or while a robot of the branch runs, opens nothing
  let before = ContentLanding { landed_at: 90, tip: Some("tip") };
  assert_eq!(landed_head(Some(&draft), "auth", [&old], None, "master", Some(before)), None);
  assert_eq!(landed_head(Some(&draft), "auth", [&running], None, "master", Some(content)), None);
  // the robot says what landed, the merged pull request where it was reviewed before
  assert_eq!(
    landed_head(Some(&draft), "auth", [&landing], Some(&reviewed), "master", None).map(|landed| (landed.head, landed.reviewed_in)),
    Some((Some("s1".to_owned()), Some(8)))
  );
  // no review: any landing, the review is created after it
  assert_eq!(
    landed_head(None, "auth", [&old], None, "master", None).and_then(|landed| landed.head),
    Some("s0".to_owned())
  );
  // the news of the session: the draft opens once something landed it since
  assert_eq!(
    shown_news(&reviews, &[&old, &landing], NO_PULLS, &BTreeMap::new(), &[], 0),
    [NewsEvent::ShownReviewLanded {
      branch: "auth".to_owned(),
      number: 12
    }]
  );
  // a robot older than the review is no fix of it
  let opened = ShownReview {
    pull_request: PullRequest {
      is_draft: false,
      head_ref_oid: "tip".to_owned(),
      ..draft.pull_request.clone()
    },
    ..draft
  };
  assert_eq!(
    pending_fixes(&opened, "auth", [&old, &landing]).iter().map(|robot| robot.id.as_str()).collect::<Vec<_>>(),
    ["r1"]
  );
}

#[test]
fn the_body_says_once_where_the_branch_was_reviewed_before_it_landed() {
  let body = format!("Adds auth.\n\n{}", reviewers_record(&logins(&["alice"])));
  let recorded = with_record(&body, &logins(&["alice"]), &[], Some(7));
  assert_eq!(recorded, format!("Adds auth.\n\n{}\n\n{}", reviewed_before_line(7), reviewers_record(&logins(&["alice"]))));
  assert_eq!(reviewed_before_line(7), "Reviewed before landing in #7.");
  assert_eq!(with_record(&recorded, &logins(&["alice"]), &[], Some(7)), recorded);
}

#[test]
fn a_mark_is_refused_as_the_safe_push_with_its_review_without_what_refuses_the_robot() {
  let land = mark_land("auth", &logins(&["alice"]));
  assert_eq!(land.request_review.map(|show| show.reviewers), Some(logins(&["alice"])));
  let blockers = [
    Reason::Draft { number: 7 },
    Reason::ShowLandsStack { lands: logins(&["util", "auth"]) },
    Reason::ShowNeedsApprovals { groups: logins(&["Auth Team"]) },
    Reason::NoReviewer { groups: Vec::new() },
  ];
  let waits = [
    FactWait {
      fact: LandFact::Readiness,
      failed: None,
    },
    FactWait {
      fact: LandFact::Profiles,
      failed: None,
    },
  ];
  // nobody to ask is of the reviewers the mark is given in the end
  let (refused, waited) = mark_refusals(&blockers, &waits, false);
  assert_eq!(refused, blockers[1..3].to_vec());
  assert_eq!(waited, waits[..1].to_vec());
  // the approvals come in its open pull request, before the landing the mark follows
  let (refused, _) = mark_refusals(&blockers, &waits, true);
  assert_eq!(refused, [blockers[1].clone()]);
  // its cancel says the review won't open, not that the change is in master
  assert_eq!(close_comment(&mark(), "master"), "Canceled: this review after landing won't open.");
}
