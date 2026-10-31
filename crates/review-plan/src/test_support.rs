//! What the tests of the plan share: the plan of a test repository without a cache

use crate::mark_reviewed::{MarkReviewedParams, mark_reviewed_core};
use crate::{PlanSources, ReviewPlan, ReviewPlanParams, review_plan_core};
use file_diffs::accio::SymbolIndex;
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use test_utils::git_test_utils::TestRepo;

/// The branch prefix of the tests
pub(crate) const PREFIX: &str = "jane";

/// The plan of `params` in `repo`, with `index` for the fan-in
pub(crate) fn plan_of(repo: &TestRepo, index: Option<&dyn SymbolIndex>, params: &ReviewPlanParams) -> anyhow::Result<ReviewPlan> {
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::for_scans());
  let sources = PlanSources {
    cache: &cache,
    file_diffs: &file_diffs,
    index,
  };
  review_plan_core(repo.git_executor(), sources, PREFIX, params)
}

/// The params of the plan of `base..head`
pub(crate) fn range(repo: &TestRepo, base: &str, head: &str) -> ReviewPlanParams {
  ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: crate::ReviewSubject::Range {
      base: base.to_string(),
      head: head.to_string(),
    },
    since_reviewed: false,
    concern_edges: crate::concerns::ConcernEdges::default(),
  }
}

/// The params of the plan of the branch `auth` on the baseline `master`, since its review or whole
pub(crate) fn branch(repo: &TestRepo, since_reviewed: bool) -> ReviewPlanParams {
  ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: crate::ReviewSubject::Branch {
      name: "auth".to_string(),
      base: None,
      baseline_branch: Some("master".to_string()),
    },
    since_reviewed,
    concern_edges: crate::concerns::ConcernEdges::default(),
  }
}

/// Mark the branch `auth` of `repo` reviewed, on the baseline `master`
pub(crate) fn mark(repo: &TestRepo) {
  let params = MarkReviewedParams {
    repository_path: repo.path_str().to_string(),
    branch: "auth".to_string(),
    base: None,
    baseline_branch: Some("master".to_string()),
  };
  mark_reviewed_core(repo.git_executor(), PREFIX, &params).unwrap();
}
