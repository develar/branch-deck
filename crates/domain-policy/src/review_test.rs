use crate::review::{ApprovalsSkip, Candidates, CountsIn, ReviewState, candidates, review_state, skips_approvals};
use domain_types::fixtures::pull_request;
use domain_types::patronus::{
  MissingApprovalsReason, MissingReviewGroup, PatronusMergeRequest, PatronusProfile, PatronusPushMode, PatronusReviewRequirement, Person, ReviewReadiness,
  UnavailableReadinessReason, UnknownReadinessReason,
};
use domain_types::pull_request::{PullRequest, ReviewCandidate, Reviewer, ReviewerState};
use pretty_assertions::assert_eq;

fn person(full_name: &str, login: Option<&str>) -> Person {
  Person {
    login: login.map(str::to_owned),
    full_name: Some(full_name.to_owned()),
  }
}

fn group(name: &str, suggested: Vec<Person>) -> MissingReviewGroup {
  MissingReviewGroup {
    name: name.to_owned(),
    needed: Some(1),
    approved_by: Vec::new(),
    requested: Vec::new(),
    suggested,
    changes_requested_by: Vec::new(),
  }
}

fn missing(groups: Vec<MissingReviewGroup>) -> ReviewReadiness {
  ReviewReadiness::Missing {
    groups,
    not_counted: Vec::new(),
    reason: MissingApprovalsReason::ApprovalsMissing,
  }
}

fn candidate(login: &str, full_name: &str, groups: &[&str], preselected: bool) -> ReviewCandidate {
  ReviewCandidate {
    login: login.to_owned(),
    full_name: Some(full_name.to_owned()),
    groups: groups.iter().map(|group| (*group).to_owned()).collect(),
    preselected,
  }
}

/// The pull request of the author `me`
fn of_me(reviewers: Vec<Reviewer>) -> PullRequest {
  PullRequest {
    reviewers,
    author: Some("me".to_owned()),
    ..pull_request("wsm-skip-wa-2", 1078)
  }
}

#[test]
fn are_the_suggested_ones_of_the_missing_groups_one_per_group_nobody_is_asked_for_chosen_each_once() {
  let ilya = person("Ilya Korennoy", Some("ikorennoy"));
  let alice = person("Alice", Some("alice-gh"));
  let readiness = missing(vec![
    group("IJPL Code Platform", vec![ilya, alice]),
    group("UI", vec![person("Alice", Some("ALICE-gh")), person("Bob", Some("bob"))]),
    group("Build", vec![person("Erin", None)]),
  ]);
  assert_eq!(
    candidates(Some(&readiness), &of_me(Vec::new())),
    Candidates {
      candidates: vec![
        candidate("ikorennoy", "Ilya Korennoy", &["IJPL Code Platform"], true),
        candidate("alice-gh", "Alice", &["IJPL Code Platform", "UI"], true),
        candidate("bob", "Bob", &["UI"], false),
      ],
      // Erin has no GitHub login: `bd request-review` names the group for --reviewer
      without_reviewer: vec!["Build".to_owned()],
    }
  );
}

#[test]
fn leave_out_the_author_whoever_is_asked_already_and_an_approval_that_counts_a_group_someone_is_asked_for_chooses_nobody() {
  let ilya = person("Ilya Korennoy", Some("ikorennoy"));
  let bob = person("Bob", Some("bob"));
  let readiness = missing(vec![
    MissingReviewGroup {
      requested: vec![ilya.clone()],
      ..group("IJPL Code Platform", vec![ilya, person("Alice", Some("alice-gh"))])
    },
    MissingReviewGroup {
      approved_by: vec![bob.clone()],
      ..group("UI", vec![bob, person("Me", Some("me")), person("Carol", Some("carol"))])
    },
  ]);
  let asked = Reviewer {
    login: "carol".to_owned(),
    state: ReviewerState::Commented,
    requested: true,
  };
  assert_eq!(
    candidates(Some(&readiness), &of_me(vec![asked])),
    Candidates {
      candidates: vec![candidate("alice-gh", "Alice", &["IJPL Code Platform"], false)],
      without_reviewer: vec!["UI".to_owned()],
    }
  );
}

#[test]
fn are_none_when_no_approval_is_missing_or_the_readiness_is_unknown() {
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  for readiness in [Some(&ReviewReadiness::Approved { groups: Vec::new() }), Some(&unknown), None] {
    assert_eq!(candidates(readiness, &of_me(Vec::new())), Candidates::default(), "{readiness:?}");
  }
}

fn requirement(merge_request: Option<PatronusMergeRequest>) -> PatronusReviewRequirement {
  PatronusReviewRequirement {
    required: true,
    merge_request,
    groups: Vec::new(),
    skipped: false,
  }
}

/// The approvals a start misses count in the pull request it starts from, else in the one the branch doesn't have yet;
/// a repository only on Space (no GitHub repository of the branches) is read once: they count in the merge request
/// Patronus found open, else nowhere known, whatever reason the answer gives. No answer is pending, never missing
#[test]
fn the_review_says_where_the_missing_approvals_count_and_reads_a_repository_only_on_space_once() {
  let groups = vec![group("UI", Vec::new())];
  let without_pull_request = ReviewReadiness::Missing {
    groups: groups.clone(),
    not_counted: Vec::new(),
    reason: MissingApprovalsReason::NoPullRequestContext,
  };
  let open = pull_request("auth", 7);
  let merge_request = PatronusMergeRequest {
    project_key: "IJ".to_owned(),
    review_key: "IJ-CR-7".to_owned(),
    url: "https://jetbrains.team/p/ij/reviews/7".to_owned(),
  };
  let with_merge_request = requirement(Some(merge_request.clone()));
  let without_merge_request = requirement(None);
  let cases = [
    (true, Some(&open), &without_merge_request, CountsIn::PullRequest { number: 7, url: &open.url }),
    (true, None, &with_merge_request, CountsIn::NewPullRequest),
    (false, None, &with_merge_request, CountsIn::MergeRequest(&merge_request)),
    (false, None, &without_merge_request, CountsIn::UnknownRepository),
  ];
  for (github, start, requirement, counts_in) in cases {
    assert_eq!(
      review_state(Some(&without_pull_request), Some(requirement), github, start),
      ReviewState::Missing { groups: &groups, counts_in },
      "{github} {start:?}"
    );
  }

  let unavailable = ReviewReadiness::Unavailable {
    reason: UnavailableReadinessReason::Error,
    message: "The review check of Patronus is unavailable right now".to_owned(),
  };
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  assert_eq!(review_state(None, None, true, None), ReviewState::Pending);
  assert_eq!(review_state(Some(&ReviewReadiness::NotRequired), None, false, None), ReviewState::NotRequired);
  assert_eq!(
    review_state(Some(&ReviewReadiness::Approved { groups: Vec::new() }), None, true, Some(&open)),
    ReviewState::Approved
  );
  assert_eq!(review_state(Some(&unknown), None, true, Some(&open)), ReviewState::Unknown);
  assert_eq!(
    review_state(Some(&unavailable), None, true, None),
    ReviewState::Unavailable {
      message: "The review check of Patronus is unavailable right now"
    }
  );
}

fn profile(skips_review: Option<bool>) -> PatronusProfile {
  PatronusProfile {
    id: "lsc".to_owned(),
    name: "LSC".to_owned(),
    description: None,
    allowed_push_modes: Vec::new(),
    skips_review,
  }
}

/// D2: a Safe Push skips the approvals when the user says so, or through the API with a profile known to skip the review;
/// a `/patronus` comment takes no profile, a Dry Run has no review, a pull request it opens starts nothing
#[test]
fn a_safe_push_skips_the_approvals_when_the_user_says_so_or_by_a_profile_known_to_skip_the_review_through_the_api() {
  let skips = profile(Some(true));
  let (safe_push, dry_run) = (PatronusPushMode::RebaseAutosquash, PatronusPushMode::DryRun);
  assert_eq!(skips_approvals(safe_push, false, true, true, None), Some(ApprovalsSkip::WithoutApprovals));
  assert_eq!(skips_approvals(safe_push, false, false, false, Some(&skips)), Some(ApprovalsSkip::Profile));
  assert_eq!(skips_approvals(safe_push, false, true, false, Some(&skips)), Some(ApprovalsSkip::WithoutApprovals));
  assert_eq!(skips_approvals(safe_push, false, false, true, Some(&skips)), None, "a comment takes no profile");
  assert_eq!(skips_approvals(safe_push, false, false, false, Some(&profile(None))), None, "Patronus doesn't tell");
  assert_eq!(skips_approvals(safe_push, false, false, false, None), None);
  assert_eq!(skips_approvals(dry_run, false, true, false, Some(&skips)), None, "a dry run has no review");
  assert_eq!(skips_approvals(safe_push, true, true, false, Some(&skips)), None, "a pull request it opens starts nothing");
}
