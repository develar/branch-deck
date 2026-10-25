//! Whether a subject waits for the approvals of the review readiness (spec/branch/patronus.spec.md, "Code-owner review";
//! `awaits_approval`), the news they make when they arrive being `news::approval_news`, and which of the subjects a
//! window shows the session asks it for (`wanted`: those whose next step waits for it, spec/branch/next-step.spec.md,
//! "The readiness store"). What it is asked with is `pull_request::start_pull_request`.

use domain_types::next_step::NextStep;
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

/// Of the subjects a window shows (`shown`, a branch or `stack:<root>`), those whose next step depends on the readiness
/// (`NextStep::needs_readiness`: on GitHub with the pull requests listed, past rows 1 to 6, pushed or not): the session
/// asks Patronus for them; a subject without a step (not in the table, a whole stack that doesn't fork) is not wanted
pub fn wanted<'s, 'v>(shown: impl IntoIterator<Item = &'s str>, step: impl Fn(&str) -> Option<&'v NextStep>) -> Vec<&'s str> {
  shown.into_iter().filter(|key| step(key).is_some_and(|step| step.needs_readiness)).collect()
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

pub(crate) fn is_bot(login: &str) -> bool {
  login.ends_with("[bot]") || login.get(..7).is_some_and(|start| start.eq_ignore_ascii_case("copilot"))
}
