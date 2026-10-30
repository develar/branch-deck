use crate::pull_request::{RulesAnswer, base_of, dismisses_approvals, start_base, start_pull_request};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::pull_request;
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestBase, PullRequestStackEntry, PullRequestState, Reviewer, ReviewerState};
use domain_types::session::{Fact, FactState};
use pretty_assertions::assert_eq;

fn into(base: &str) -> PullRequest {
  PullRequest {
    base_ref_name: base.to_owned(),
    ..pull_request("auth", 7)
  }
}

fn approved() -> PullRequest {
  PullRequest {
    reviewers: vec![Reviewer {
      login: "alice".to_owned(),
      state: ReviewerState::Approved,
      requested: false,
    }],
    ..pull_request("auth", 7)
  }
}

fn rules(state: FactState, dismiss: Option<bool>) -> Fact<BranchRules> {
  Fact {
    value: dismiss.map(|dismiss_stale_reviews_on_push| BranchRules {
      dismiss_stale_reviews_on_push,
      ..BranchRules::default()
    }),
    state,
    seq: 1,
    observed_at: 0,
  }
}

/// Patronus merges a pull request into its base: into the target (or a base GitHub didn't say) it counts; into a virtual
/// branch of the user, whatever branch it names (a parent's, another of the table, one that landed and is gone), it
/// is retargeted to the target; into any other base (a release branch), which may be meant, it stays as it is
#[test]
fn a_pull_request_goes_into_the_target_into_a_virtual_branch_it_leaves_or_into_another_base() {
  assert_eq!(base_of(&into("master"), "master", "user"), PullRequestBase::Target);
  assert_eq!(base_of(&into(""), "master", "user"), PullRequestBase::Target);
  for parent in ["auth", "gone", "auth/login"] {
    assert_eq!(
      base_of(&into(&format!("user/virtual/{parent}")), "master", "user"),
      PullRequestBase::Virtual {
        from: parent.to_owned(),
        retarget: "master".to_owned(),
      },
      "{parent}"
    );
  }
  // another prefix, the namespace itself, a stack branch, a release branch: another base
  for base in ["jane/virtual/auth", "user/virtual/", "user/stack/auth", "release-252"] {
    assert_eq!(base_of(&into(base), "master", "user"), PullRequestBase::Other { base: base.to_owned() }, "{base}");
  }
}

/// A pull request into a virtual branch that GitHub lists in a stack into the target is a layer of it, never retargeted;
/// one in a stack into another base is retargeted as any other, and no robot starts from a layer
/// (docs/design/github-stacks.md)
#[test]
fn a_member_of_a_stack_of_github_into_the_target_is_a_layer() {
  let layer = |stack_base: &str| PullRequest {
    stack: Some(PullRequestStackEntry {
      number: 4,
      base_ref_name: stack_base.to_owned(),
      size: 2,
      position: 2,
    }),
    ..into("user/virtual/auth")
  };
  assert_eq!(base_of(&layer("master"), "master", "user"), PullRequestBase::Layer { parent: "auth".to_owned() });
  assert_eq!(
    base_of(&layer("user/virtual/zc"), "master", "user"),
    PullRequestBase::Virtual {
      from: "auth".to_owned(),
      retarget: "master".to_owned(),
    }
  );
  // the bottom layer goes into the target as any pull request
  let bottom = PullRequest {
    base_ref_name: "master".to_owned(),
    ..layer("master")
  };
  assert_eq!(base_of(&bottom, "master", "user"), PullRequestBase::Target);
  assert_eq!(start_pull_request(Some(&layer("master")), "master", "user", Some("acme/monorepo"), false), None);
}

#[test]
fn a_safe_push_starts_from_the_open_pull_request_into_the_target_or_a_virtual_branch_while_patronus_names_the_github_repository() {
  let github = Some("acme/monorepo");
  let open = pull_request("auth", 7);
  assert_eq!(start_pull_request(Some(&open), "master", "user", github, false).map(|it| it.number), Some(7));
  // a base not read yet counts as the target
  assert_eq!(start_pull_request(Some(&into("")), "master", "user", github, false).map(|it| it.number), Some(7));
  // a branch starts from one into a virtual branch, which its start retargets first (D11); a whole stack doesn't: its
  // stack branch is no virtual branch
  let virtual_base = into("user/virtual/login");
  assert_eq!(start_pull_request(Some(&virtual_base), "master", "user", github, false).map(|it| it.number), Some(7));
  assert_eq!(start_pull_request(Some(&virtual_base), "master", "user", github, true), None);
  assert_eq!(
    start_base(&virtual_base, "master", "user", true),
    PullRequestBase::Other {
      base: "user/virtual/login".to_owned()
    }
  );
  // through the API: another base, a closed one, none, or no GitHub repository Patronus knows
  let closed = PullRequest {
    state: PullRequestState::Closed,
    ..open.clone()
  };
  assert_eq!(start_pull_request(Some(&into("release-252")), "master", "user", github, false), None);
  assert_eq!(start_pull_request(Some(&closed), "master", "user", github, false), None);
  assert_eq!(start_pull_request(None, "master", "user", github, false), None);
  assert_eq!(start_pull_request(Some(&open), "master", "user", None, false), None);
}

/// A push of new commits into an approved pull request keeps or dismisses its approvals by the rules of its base, as the
/// session last read them (a stale answer too); a read that failed is unknown with its reason and one not answered yet
/// loads, never "keeps them"; a pull request nobody approved has nothing to lose
#[test]
fn a_push_keeps_or_dismisses_the_approvals_by_the_rules_and_never_reads_a_missing_answer_as_keeping_them() {
  let failure = ApiError::new(ErrorCode::NetworkError, "GitHub answered 502");
  let stale = FactState::Stale { reason: None };
  assert_eq!(dismisses_approvals(Some(&rules(FactState::Fresh, Some(true))), &approved()), RulesAnswer::Dismiss);
  assert_eq!(dismisses_approvals(Some(&rules(FactState::Fresh, Some(false))), &approved()), RulesAnswer::Keep);
  assert_eq!(dismisses_approvals(Some(&rules(stale, Some(true))), &approved()), RulesAnswer::Dismiss);
  let unavailable = rules(FactState::Unavailable { reason: failure.clone() }, None);
  assert_eq!(dismisses_approvals(Some(&unavailable), &approved()), RulesAnswer::Unknown { reason: &failure });
  assert_eq!(dismisses_approvals(Some(&rules(FactState::Loading, None)), &approved()), RulesAnswer::Loading);
  assert_eq!(dismisses_approvals(Some(&rules(FactState::Unknown, None)), &approved()), RulesAnswer::Loading);
  assert_eq!(dismisses_approvals(None, &approved()), RulesAnswer::Loading);
  // nobody approved: nothing to dismiss, whatever the rules
  assert_eq!(dismisses_approvals(Some(&unavailable), &pull_request("auth", 7)), RulesAnswer::Keep);
  assert_eq!(dismisses_approvals(None, &pull_request("auth", 7)), RulesAnswer::Keep);
}
