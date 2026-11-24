//! The pull requests of others that wait for the user's review ("Your Turn", openspec/specs/repository/review-requests/spec.md)
//! as a fact of the session (`ReviewRequestsFact`, the part `facts.reviewRequests` of the view): each listing replaces
//! the requests, waiting the longest first, a chain of them as one stack (`rows`,
//! `domain_policy::review_request_chains`); a failure of GitHub keeps them (`Stale`), no token, a token GitHub rejects
//! and a repository not on GitHub drop them. A listing tells the requests new since the listing before (`News`,
//! `domain_policy::news::review_requested`): a number and request time it lacked, a re-request too, never a request back
//! to the user after their answer; the first
//! listing after a window watches the session anew (the start of the app, a repository selected again) is silent.
//!
//! While a window shows the Reviews view (`Demand::reviews`) a listing brings the rest of Reviews too
//! (openspec/specs/repository/reviews/spec.md), which the fact keeps through a listing without it; the requests the user answered
//! with a comment, the reviews the user submitted from the app (`submitted`, until a listing says GitHub has them) and
//! the last pull request of every number a listing named, for the review windows that show one no listing has any more.
//!
//! While a window filters Reviews by words (`Demand::reviews_searches`), the session searches the closed pull requests
//! the user reviewed or opened whose titles have them (`closed_answer`): what each search found is kept by its words
//! for two minutes after no window filters by them (`domain_policy::refresh::CLOSED_REVIEWS_KEPT`), and remembered, so
//! the review window of one fetches it.
//!
//! The review fetches of the windows (`kept`, by the pull request a fetch writes, of a stack its top with the layers it
//! brought) count from their start (`fetching`), before the demand of their window may name the pull request, and the
//! prunes of the session keep their review refs (`fetched_numbers`); a fetch goes when the window that showed its pull
//! request closes, also while it runs, so its refs are of no window once it ended, and when it ends without a head while
//! no window shows its pull request (`fetch_ended`). Apart from that, the head each fetch brought (`heads`) stays,
//! whichever window's change came meanwhile, and the layers of a stack (`stacks`, by its top, which Approve N Pull
//! Requests is decided on) stay while a window shows the top, so a stack window opened again decides nothing on the
//! heads of its earlier fetch; a head makes the ask of the readiness of its pull request (`readiness_asks`,
//! openspec/specs/branch/patronus/spec.md, "The readiness of a request"): the head the listing names once it is
//! fetched, with its head branch, its reviewers and its author; only a pull request into the default branch is asked.

use crate::facts::{Observation, seed_head};
use crate::pull_request_facts::availability_error;
use domain_policy::news::{request_key, review_requested, tells_after};
use domain_policy::refresh::CLOSED_REVIEWS_KEPT;
use domain_policy::review_request_chains::{request_rows, stack_layers};
use domain_policy::reviews::closed_search_words;
use domain_types::patronus::{ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::{PullRequestAvailability, PullRequestReviewEvent, ReviewerState};
use domain_types::review_request::{AnsweredRequest, ClosedReviewsResult, LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewRequestsResult, ReviewsAnswer};
use domain_types::session::{Demand, Fact, FactState, NewsEvent, ReviewRequestsView, request_readiness_key};
use std::collections::{BTreeMap, BTreeSet};
use sync_core::github_repository::GitHubRepository;
use sync_utils::timestamp::unix_now;

/// A layer of a stack a review window shows, as its last fetch brought it (`ReviewRequestsFact::fetched_stacks`)
#[derive(Clone, Copy, Debug)]
pub struct FetchedLayer<'a> {
  pub request: &'a ReviewRequest,
  /// The user's own pull request
  pub own: bool,
  /// The head the fetch brought, which the window shows
  pub head: &'a str,
}

/// The review requests of a session
#[derive(Clone, Debug, Default)]
pub struct ReviewRequestsFact {
  view: ReviewRequestsView,
  /// The requests of the listing before, by number and request time, which the next listing's news are told against;
  /// `None` while that listing is silent
  known: Option<BTreeSet<(u32, u32)>>,
  /// The requests by name the user answered with a comment, of the last listing
  answered: Vec<AnsweredRequest>,
  /// The rest of Reviews of the last listing that asked it, and how it stands
  reviews: Fact<ReviewsAnswer>,
  reviews_checked_at: u32,
  /// A window shows the Reviews view: the rest is `Loading` until a listing brings it
  reviews_shown: bool,
  /// The reviews the user submitted from the app by number: the verdict and its commit
  submitted: BTreeMap<u32, (PullRequestReviewEvent, String)>,
  /// The last pull request of each number a listing named, and how the user took part in it then (`None`: a request by
  /// name)
  remembered: BTreeMap<u32, (ReviewRequest, Option<ReviewInvolvement>)>,
  /// The pull requests the review windows show (`Demand::review_windows`)
  windows: Vec<u32>,
  /// The head the last review fetch of each pull request brought (of a stack, of each layer), whichever window asked for
  /// it and whatever change of the windows came meanwhile: the readiness of a pull request a window shows is asked at it
  /// (`readiness_asks`), and the layers of a stack show it (`fetched_stacks`)
  heads: BTreeMap<u32, String>,
  /// The layers the last fetch of each stack brought (Review Stack), the bottom first, by its top: shown while a window
  /// shows the top, and gone once no window does, so a stack window opened again shows none until its own fetch ended
  stacks: BTreeMap<u32, Vec<u32>>,
  /// The review fetches whose refs the prunes keep (`fetched_numbers`), by the pull request they write, with the layers
  /// a fetch of a stack brought: here from its start (`fetching`), whether or not the demand of its window named the pull
  /// request yet, until the window that showed it closes, also while the fetch runs, or until it ends without a head
  /// while no window shows it (`fetch_ended`)
  kept: BTreeMap<u32, Vec<u32>>,
  /// The review fetches that started (`fetching`) and did not end, by the pull request they write
  in_flight: BTreeMap<u32, u32>,
  /// The words the windows filter Reviews by, which the session searches the closed pull requests by
  /// (`closed_search_words` of `Demand::reviews_searches`), while a window shows the view
  closed_wanted: Vec<String>,
  /// What each search of the closed pull requests found, by its words (`None`: GitHub could not be asked), and when
  /// (unix seconds); kept for `CLOSED_REVIEWS_KEPT` after no window filters by its words
  closed: BTreeMap<String, ClosedAnswer>,
}

/// What a search of the closed pull requests found, and when
#[derive(Clone, Debug)]
struct ClosedAnswer {
  found: Option<ClosedReviewsResult>,
  observed_at: u32,
  /// Unix seconds when the last window stopped filtering by its words; `None` while one does
  unwanted_at: Option<u32>,
}

impl ReviewRequestsFact {
  /// The part of the view
  #[must_use]
  pub const fn view(&self) -> &ReviewRequestsView {
    &self.view
  }

  /// A window watches the session anew: its next listing tells nothing, as the first one of a start
  pub fn watched_anew(&mut self) {
    self.known = None;
  }

  /// The answer of a listing of `observation`: whether it changed the part, and the news of the requests it added. An
  /// observation not newer than the one of the head changes nothing
  pub fn answer(&mut self, observation: Observation, result: ReviewRequestsResult) -> (bool, Option<NewsEvent>) {
    let seq = observation.seq.0;
    if seq <= self.view.head.seq {
      return (false, None);
    }
    let before = self.view.clone();
    let reviews_before = self.reviews.clone();
    if self.view.head.state == FactState::Cached && !matches!(result.availability, PullRequestAvailability::Available { .. }) {
      // no listing of this process stands behind the requests of the frame of an earlier run
      self.view.requests = Vec::new();
      self.view.rows = Vec::new();
      self.view.total_count = 0;
      self.view.checked_at = 0;
    }
    let ReviewRequestsResult {
      availability,
      total_count,
      mut requests,
      answered,
      others,
    } = result;
    let mut news = None;
    let state = match &availability {
      PullRequestAvailability::Available { .. } => {
        requests.sort_by_key(|request| (request.waiting_since, request.number));
        // a listing after the one of the cached frame of an earlier run is no news
        news = tells_after(&self.view.head.state).then(|| self.told(&requests)).flatten();
        self.known = Some(requests.iter().map(request_key).collect());
        self.remember(&requests, &answered, others.as_ref());
        self.answered = answered;
        if let Some(others) = others {
          self.reviews.value = Some(others);
          self.reviews.state = FactState::Fresh;
          self.reviews_checked_at = observation.observed_at;
        }
        self.view.total_count = total_count.max(u32::try_from(requests.len()).unwrap_or(u32::MAX));
        self.view.rows = request_rows(&requests);
        self.view.requests = requests;
        self.view.checked_at = observation.observed_at;
        FactState::Fresh
      }
      // the rows go: nothing waits for a user GitHub doesn't know, nor off GitHub
      PullRequestAvailability::NotGitHub | PullRequestAvailability::SignedOut | PullRequestAvailability::NotAuthenticated { .. } => {
        self.view.requests = Vec::new();
        self.view.rows = Vec::new();
        self.view.total_count = 0;
        self.answered = Vec::new();
        self.reviews.value = None;
        if self.known.is_some() {
          self.known = Some(BTreeSet::new());
        }
        if matches!(availability, PullRequestAvailability::NotGitHub) {
          FactState::Fresh
        } else {
          FactState::Unavailable {
            reason: availability_error(&availability),
          }
        }
      }
      // the rows stay with the hint of why GitHub can't be asked
      failed if self.view.checked_at != 0 => FactState::Stale {
        reason: Some(availability_error(failed)),
      },
      failed => FactState::Unavailable {
        reason: availability_error(failed),
      },
    };
    // the rest of Reviews stands as the listing does, but a listing on GitHub that didn't ask it leaves it unread
    let unread = matches!(availability, PullRequestAvailability::Available { .. }) && self.reviews.value.is_none();
    if !unread {
      self.reviews.state = state.clone();
    }
    self.reviews.seq = seq;
    self.reviews.observed_at = observation.observed_at;
    let head = &mut self.view.head;
    head.value = Some(availability);
    head.state = state;
    head.seq = seq;
    head.observed_at = observation.observed_at;
    // GitHub has the reviews of the app once a listing gives the user's latest review
    self
      .submitted
      .retain(|number, (_, commit)| self.remembered.get(number).is_none_or(|(request, _)| request.reviewed_commit.as_ref() != Some(commit)));
    (self.view != before || self.reviews != reviews_before, news)
  }

  /// The windows of the session now: the Reviews view, the words it filters by, and the pull requests their review
  /// windows show
  pub fn demand(&mut self, demand: &Demand) {
    self.demand_at(demand, unix_now());
  }

  /// `demand` at `now` (unix seconds), which starts the time the answers of the words no window filters by any more
  /// are kept
  pub fn demand_at(&mut self, demand: &Demand, now: u32) {
    self.reviews_shown = demand.reviews;
    self.closed_wanted = closed_wanted(demand);
    let kept = u32::try_from(CLOSED_REVIEWS_KEPT.as_secs()).unwrap_or(u32::MAX);
    self.closed.retain(|_, answer| answer.unwanted_at.is_none_or(|at| now.saturating_sub(at) < kept));
    for (words, answer) in &mut self.closed {
      if self.closed_wanted.contains(words) {
        answer.unwanted_at = None;
      } else {
        answer.unwanted_at.get_or_insert(now);
      }
    }
    // the fetch of a window that closed goes, also one that runs, with the layers of its stack: its refs are of no
    // window, nor is the stack it showed. One that started before the demand of its window named the pull request stays
    for number in &self.windows {
      if !demand.review_windows.contains(number) {
        self.kept.remove(number);
        self.stacks.remove(number);
      }
    }
    self.windows.clone_from(&demand.review_windows);
  }

  /// A review fetch of the pull request `number` (of a stack its top) starts (`operations::review_fetch`), before it
  /// writes a review ref: the window asks for it apart from its demand, which may reach the session after the fetch, so
  /// the refs of the fetch are kept from its start (`fetched_numbers`) until the window that showed the pull request
  /// closes, or until the fetch ends without a head while no window shows it (`fetch_ended`)
  pub fn fetching(&mut self, number: u32) {
    *self.in_flight.entry(number).or_default() += 1;
    self.kept.entry(number).or_default();
  }

  /// The review fetch of the pull request `number` ended with `head` (`operations::review_fetch`); whether it changed
  /// what the session knows. The head is its window's whatever change of the windows came meanwhile; its refs stay kept
  /// only while the fetch is (one whose window closed while it ran went with it)
  pub fn fetched(&mut self, number: u32, head: String) -> bool {
    self.ended(number);
    self.heads.insert(number, head.clone()) != Some(head)
  }

  /// The review fetch of the pull request `number` (of a stack its top) ended without a head: it failed or was stopped
  /// (`operations::review_fetch`). It goes with the layers of its stack when no window shows the pull request and no
  /// other fetch of it runs, the pull request kept still while a stack that is kept has it; a window that shows it keeps
  /// it, with the head of an earlier fetch
  pub fn fetch_ended(&mut self, number: u32) {
    self.ended(number);
    if !self.windows.contains(&number) && !self.in_flight.contains_key(&number) {
      self.kept.remove(&number);
    }
  }

  /// The pull requests whose review fetches are kept, of a stack its top and the layers it brought (some more than
  /// once): the prunes keep their review refs, also before the demand of their window names them
  pub fn fetched_numbers(&self) -> impl Iterator<Item = u32> + '_ {
    self.kept.iter().flat_map(|(number, layers)| std::iter::once(*number).chain(layers.iter().copied()))
  }

  /// The fetch of the stack of `top` ended with `layers`, the bottom first, each with its head; whether it changed what
  /// the session knows. The heads and the stack are its window's as `fetched` says; the layers are kept with the top
  /// only while its fetch is
  pub fn stack_fetched(&mut self, top: u32, layers: Vec<(u32, String)>) -> bool {
    self.ended(top);
    let numbers: Vec<u32> = layers.iter().map(|(number, _)| *number).collect();
    if let Some(kept) = self.kept.get_mut(&top) {
      kept.clone_from(&numbers);
    }
    let mut changed = self.stacks.insert(top, numbers.clone()) != Some(numbers);
    for (number, head) in layers {
      changed |= self.heads.insert(number, head.clone()) != Some(head);
    }
    changed
  }

  /// A review fetch of `number` ended, one of those that started
  fn ended(&mut self, number: u32) {
    if let Some(running) = self.in_flight.get_mut(&number) {
      *running -= 1;
      if *running == 0 {
        self.in_flight.remove(&number);
      }
    }
  }

  /// The stacks the review windows show as their last fetch brought them, by top: each layer's pull request as a listing
  /// named it last, whether it is the user's own, and the head the fetch brought; a layer no listing named is left out
  pub fn fetched_stacks(&self) -> impl Iterator<Item = (u32, Vec<FetchedLayer<'_>>)> {
    self.stacks.iter().filter(|(top, _)| self.windows.contains(top)).map(|(top, numbers)| {
      let layers = numbers
        .iter()
        .filter_map(|number| {
          Some(FetchedLayer {
            request: self.pull_request(*number)?,
            own: self.is_own(*number),
            head: self.heads.get(number)?,
          })
        })
        .collect();
      (*top, layers)
    })
  }

  /// The pull requests of `windows` (the review windows now) into the default branch, by number, each with the ask of
  /// its readiness (`request:<number>`) once the head the listing names is the one its review fetch brought: the head
  /// branch, the pull request with its reviewers and its author. A pull request into another branch is not asked
  #[must_use]
  pub fn readiness_asks(&self, windows: &[u32]) -> BTreeMap<u32, Option<ReviewReadinessBranch>> {
    windows
      .iter()
      .filter_map(|number| {
        // nothing is decided of a merged or closed one: its readiness is never asked
        let request = self.pull_request(*number).filter(|request| request.base_is_default && !request.is_closed())?;
        let fetched = self.heads.get(number).filter(|head| **head == request.head_ref_oid);
        let ask = fetched.map(|head| ReviewReadinessBranch {
          branch_name: request_readiness_key(*number),
          source: None,
          pull_request_head: Some(request.head_ref_name.clone()),
          tip: head.clone(),
          start: ReviewStart::PullRequest { number: *number },
          reviewers: request.reviewers.clone(),
          author: request.author.clone(),
        });
        Some((*number, ask))
      })
      .collect()
  }

  /// The pull request of `entry` the user just linked joins the links of Reviews at once, newest first, before the
  /// listing the link asks for brings it; remembered, so its review window fetches it. A merged or closed one is only
  /// remembered: Reviews keeps no link of it. Nothing joins while Reviews was never read. Whether it changed what the
  /// session knows
  pub fn linked(&mut self, entry: ReviewEntry) -> bool {
    self.remembered.insert(entry.request.number, (entry.request.clone(), Some(entry.involvement.clone())));
    if entry.request.is_closed() {
      return false;
    }
    let Some(reviews) = self.reviews.value.as_mut() else {
      return false;
    };
    let number = entry.request.number;
    reviews.linked.retain(|linked| linked.number() != number);
    reviews.linked.insert(0, LinkedAnswer::Open { entry: Box::new(entry) });
    true
  }

  /// The pull request `number` the user removed leaves the links of Reviews at once, before the listing the removal
  /// asks for; whether it changed what the session knows
  pub fn unlinked(&mut self, number: u32) -> bool {
    let Some(reviews) = self.reviews.value.as_mut() else {
      return false;
    };
    let before = reviews.linked.len();
    reviews.linked.retain(|linked| linked.number() != number);
    reviews.linked.len() != before
  }

  /// What the search of the closed pull requests by `words` found at `observed_at` (unix seconds): kept by its words
  /// while a window filters by them, its pull requests remembered, so the review window of one fetches it; GitHub
  /// unavailable keeps that it failed. An answer of words no window filters by any more is dropped. Whether it changed
  /// what the session knows
  pub fn closed_answer(&mut self, words: String, result: ClosedReviewsResult, observed_at: u32) -> bool {
    if !self.closed_wanted.contains(&words) {
      return false;
    }
    let found = matches!(result.availability, PullRequestAvailability::Available { .. }).then_some(result);
    for entry in found.iter().flat_map(|found| found.reviewed.entries.iter().chain(&found.authored.entries)) {
      self.remembered.insert(entry.request.number, (entry.request.clone(), Some(entry.involvement.clone())));
    }
    let before = self.closed.get(&words).map(|answer| answer.found.clone());
    let changed = before.as_ref() != Some(&found);
    self.closed.insert(
      words,
      ClosedAnswer {
        found,
        observed_at,
        unwanted_at: None,
      },
    );
    changed
  }

  /// The words the windows filter Reviews by now, each with what its search found: `None` while no answer came,
  /// `Some(None)` when GitHub could not be asked
  pub fn closed_searches(&self) -> impl Iterator<Item = (&str, Option<Option<&ClosedReviewsResult>>)> {
    self
      .closed_wanted
      .iter()
      .map(|words| (words.as_str(), self.closed.get(words).map(|answer| answer.found.as_ref())))
  }

  /// When the search by `words` was answered last (unix seconds); `None` for words never answered or no longer kept
  #[must_use]
  pub fn closed_observed_at(&self, words: &str) -> Option<u32> {
    self.closed.get(words).map(|answer| answer.observed_at)
  }

  /// The review the user submitted from the app on `commit`; whether it changed what the session knows
  pub fn submitted(&mut self, number: u32, verdict: PullRequestReviewEvent, commit: String) -> bool {
    self.submitted.insert(number, (verdict, commit.clone())) != Some((verdict, commit))
  }

  /// The commit the user approved last of `request`: the review submitted from the app, else GitHub's latest review
  #[must_use]
  pub fn approved<'a>(&'a self, request: &'a ReviewRequest) -> Option<&'a str> {
    match self.submitted.get(&request.number) {
      Some((PullRequestReviewEvent::Approve, commit)) => Some(commit),
      Some(_) => None,
      None => request.reviewed_commit.as_deref().filter(|_| request.reviewed_state == Some(ReviewerState::Approved)),
    }
  }

  /// The GitHub repository the last listing read the requests of
  #[must_use]
  pub fn github_repository(&self) -> Option<GitHubRepository> {
    match &self.view.head.value {
      Some(PullRequestAvailability::Available { owner, name }) => Some(GitHubRepository {
        owner: owner.clone(),
        name: name.clone(),
      }),
      _ => None,
    }
  }

  /// The requests by name the user answered with a comment
  #[must_use]
  pub fn answered(&self) -> &[AnsweredRequest] {
    &self.answered
  }

  /// The rest of Reviews as it stands: `Loading` while a window shows the view and no listing brought it yet, `Unknown`
  /// while none shows it and none did
  #[must_use]
  pub fn reviews(&self) -> (Fact<PullRequestAvailability>, Option<&ReviewsAnswer>, u32) {
    let mut head = Fact {
      value: self.view.head.value.clone(),
      ..Fact::default()
    };
    head.state = match (&self.reviews.value, &self.reviews.state) {
      // never read: loading while the view shows, unknown while nothing asks it
      (None, FactState::Loading) if !self.reviews_shown => FactState::Unknown,
      (_, state) => state.clone(),
    };
    head.seq = self.reviews.seq;
    head.observed_at = self.reviews.observed_at;
    (head, self.reviews.value.as_ref(), self.reviews_checked_at)
  }

  /// Whether the pull request `number` is the user's own, as the last listing that named it said
  #[must_use]
  pub fn is_own(&self, number: u32) -> bool {
    matches!(self.remembered.get(&number), Some((_, Some(ReviewInvolvement::Authored { .. }))))
  }

  /// The pull request `number` as the last listing that named it had it: what a review of it fetches
  /// (`operations::review_fetch`), also once no listing has it any more
  #[must_use]
  pub fn pull_request(&self, number: u32) -> Option<&ReviewRequest> {
    self.remembered.get(&number).map(|(request, _)| request)
  }

  /// The layers of the stack of `top` down to `bottom`, the bottom first, as the pull requests a listing named last link
  /// them now (`domain_policy::review_request_chains::stack_layers`): what a review of the stack fetches
  /// (`operations::review_fetch`); empty when no listing named `top`
  #[must_use]
  pub fn stack(&self, top: u32, bottom: u32) -> Vec<ReviewRequest> {
    let known: Vec<&ReviewRequest> = self.remembered.values().map(|(request, _)| request).collect();
    let listed: Vec<u32> = self.view.requests.iter().map(|request| request.number).collect();
    stack_layers(top, bottom, &known, &listed)
      .into_iter()
      .filter_map(|number| self.pull_request(number).cloned())
      .collect()
  }

  /// The pull requests the review windows show that no listing has now, as a listing named them last
  pub fn windows(&self) -> impl Iterator<Item = &(ReviewRequest, Option<ReviewInvolvement>)> {
    self.windows.iter().filter_map(|number| self.remembered.get(number))
  }

  /// Every pull request of a listing by number, so a review window keeps it once the listing has it no longer
  fn remember(&mut self, requests: &[ReviewRequest], answered: &[AnsweredRequest], others: Option<&ReviewsAnswer>) {
    let mut remember = |request: &ReviewRequest, involvement: Option<ReviewInvolvement>| {
      self.remembered.insert(request.number, (request.clone(), involvement));
    };
    for request in requests {
      remember(request, None);
    }
    for answered in answered {
      remember(&answered.request, None);
    }
    let Some(others) = others else {
      return;
    };
    let linked = others.linked.iter().filter_map(|linked| match linked {
      LinkedAnswer::Open { entry } => Some(entry.as_ref()),
      LinkedAnswer::Closed { .. } | LinkedAnswer::Missing { .. } => None,
    });
    let entries = others.team.entries.iter().chain(&others.reviewed.entries).chain(&others.authored.entries).chain(linked);
    for ReviewEntry { request, involvement } in entries {
      remember(request, Some(involvement.clone()));
    }
  }

  /// The news of `requests` against the listing before (`domain_policy::news::review_requested`): none while it is
  /// silent
  fn told(&self, requests: &[ReviewRequest]) -> Option<NewsEvent> {
    review_requested(self.known.as_ref()?, requests)
  }

  /// The review requests of the frame of an earlier run (`crate::warm_start`): `Cached` until GitHub lists them, the
  /// first listing silent; a failure of that listing leaves none of them; none when the frame had no listing
  pub(crate) fn cached(mut view: ReviewRequestsView) -> Self {
    if seed_head(&mut view.head) { Self { view, ..Self::default() } } else { Self::default() }
  }
}

/// The words the windows filter Reviews by that the session searches the closed pull requests by, once each, while a
/// window shows the view
fn closed_wanted(demand: &Demand) -> Vec<String> {
  if !demand.reviews {
    return Vec::new();
  }
  let words: BTreeSet<String> = demand.reviews_searches.iter().filter_map(|text| closed_search_words(text)).collect();
  words.into_iter().collect()
}

#[cfg(test)]
#[path = "review_request_facts_test.rs"]
pub(crate) mod review_request_facts_test;
