//! Whether a subject waits for the approvals of the review readiness (spec/branch/patronus.spec.md, "Code-owner review";
//! `awaits_approval`), the news they make when they arrive being `news::approval_news`. What it is asked with is
//! `pull_request::start_pull_request`.

use domain_types::patronus::{ReviewReadiness, ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::{Reviewer, ReviewerState};

/// An answer of the readiness with what it was asked for
#[derive(Clone, Copy, Debug)]
pub struct Answered<'a> {
  pub ask: &'a ReviewReadinessBranch,
  pub readiness: &'a ReviewReadiness,
}

/// Whether the approvals of a subject asked as `ask` can be news (`news::approval_news`): its last answer, of the same tip and
/// the pull request it starts from now, misses approvals, or Patronus can't judge them while GitHub waits for
/// reviewers. Such a subject is asked again when its reviews change, also while nothing shows it, and each poll of the
/// review requests lists the pull requests for its new reviews
#[must_use]
pub fn awaits_approval(last: Answered<'_>, ask: &ReviewReadinessBranch) -> bool {
  matches!(last.ask.start, ReviewStart::PullRequest { .. })
    && last.ask.tip == ask.tip
    && last.ask.start == ask.start
    && match last.readiness {
      ReviewReadiness::Missing { .. } => true,
      readiness => is_unjudged(readiness) && !waiting_reviewers(&last.ask.reviewers).is_empty(),
    }
}

pub(crate) const fn is_unjudged(readiness: &ReviewReadiness) -> bool {
  matches!(readiness, ReviewReadiness::Unknown { .. } | ReviewReadiness::Unavailable { .. })
}

/// The reviewers an open pull request waits for, as GitHub tells it: asked and not approving since; bots left out
/// (`waitingReviewers` of the page)
pub(crate) fn waiting_reviewers(reviewers: &[Reviewer]) -> Vec<&String> {
  reviewers
    .iter()
    .filter(|reviewer| reviewer.requested && reviewer.state != ReviewerState::Approved && !is_bot(&reviewer.login))
    .map(|reviewer| &reviewer.login)
    .collect()
}

fn is_bot(login: &str) -> bool {
  login.ends_with("[bot]") || login.get(..7).is_some_and(|start| start.eq_ignore_ascii_case("copilot"))
}
