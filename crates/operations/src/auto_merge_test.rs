use crate::auto_merge::{disable_plan, disable_request};
use domain_types::fixtures::pull_request;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{Intent, PullRequestField, Resource};
use domain_types::pull_request::{MergeMethod, PullRequestAvailability, PullRequestState};
use domain_types::reason::Reason;
use domain_types::session::{Fact, FactState, PullRequestsView};
use pretty_assertions::assert_eq;

/// The listing of the session with the pull request #7 of auth changed by `change`
fn listed(change: impl FnOnce(&mut domain_types::pull_request::PullRequest)) -> PullRequestsView {
  let mut auth = pull_request("auth", 7);
  change(&mut auth);
  PullRequestsView {
    head: Fact {
      value: Some(PullRequestAvailability::Available {
        owner: "acme".to_string(),
        name: "monorepo".to_string(),
      }),
      state: FactState::Fresh,
      seq: 1,
      observed_at: 1,
    },
    pull_requests: [("auth".to_string(), auth)].into(),
    ..PullRequestsView::default()
  }
}

/// Cancel Auto-Merge disables the auto-merge of an open pull request that has one, does nothing of one without it,
/// refuses a merged one and has no plan of a closed one or one the listing lacks; it holds the auto-merge of the pull
/// request, which its enable holds too
#[test]
fn cancel_auto_merge_disables_the_auto_merge_of_an_open_pull_request_that_has_one() {
  let auto = listed(|it| it.auto_merge = Some(MergeMethod::Rebase));
  let plan = disable_plan(&auto, 7).unwrap();
  assert_eq!((plan.effects, plan.blockers), (vec![PlannedEffect::DisableAutoMerge { number: 7 }], Vec::new()));
  assert_eq!(disable_plan(&listed(|_| {}), 7).unwrap().effects, []);
  let merged = listed(|it| it.state = PullRequestState::Merged);
  assert_eq!(disable_plan(&merged, 7).unwrap().blockers, [Reason::PullRequestMerged { number: 7 }]);
  assert!(disable_plan(&listed(|it| it.state = PullRequestState::Closed), 7).is_none());
  assert!(disable_plan(&auto, 8).is_none());
  let request = disable_request(7);
  assert_eq!(request.intent, Intent::DisableAutoMerge { number: 7 });
  assert_eq!(
    request.resources,
    [Resource::PullRequest {
      number: 7,
      field: PullRequestField::AutoMerge
    }]
  );
}
