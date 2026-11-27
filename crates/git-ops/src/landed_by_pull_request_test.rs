use crate::landed_by_pull_request::MergedAsk;
use crate::landed_pairing::landed_pairing_test::{at, in_scope, sides, squash};
use crate::landed_pairing::{AfterConflict, CandidateBy, LandedCandidate, pair_after_conflict, pair_before_replay, read_landed};
use crate::update_branch::{CheckPurpose, LandedLookup, UpstreamRef, check_update};
use crate::word_resolve::ResolveMode;
use domain_types::pull_request::{KnownMergedPullRequests, MergedPullRequest, MergedPullRequests};
use domain_types::update_project::UpdateState;
use pretty_assertions::assert_eq;
use std::sync::atomic::{AtomicU32, Ordering};
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;
use test_utils::process_budget::{counting, processes};

/// The address GitHub gives a user who keeps theirs private: the squash of their pull request is of it
const NOREPLY: &str = "Test User <1+test@users.noreply.github.com>";

/// The merged pull requests a test knows, answered as the app's listing answers them, with how often they were asked
struct Known {
  known: KnownMergedPullRequests,
  asked: AtomicU32,
}

impl Known {
  fn of(merged: Vec<MergedPullRequest>) -> Self {
    Self {
      known: KnownMergedPullRequests::new(merged),
      asked: AtomicU32::new(0),
    }
  }

  fn asked(&self) -> u32 {
    self.asked.load(Ordering::Relaxed)
  }
}

impl MergedPullRequests for Known {
  fn merged_since(&self, since: u32) -> Option<KnownMergedPullRequests> {
    self.asked.fetch_add(1, Ordering::Relaxed);
    self.known.merged_since(since)
  }
}

/// Pull request #`number`, whose head was at `head`, merged at 900 as `merge_commit` by `email`
fn merged(number: u32, head: &str, merge_commit: &str, email: &str) -> MergedPullRequest {
  MergedPullRequest {
    number,
    head_ref_name: format!("user/virtual/feature-{number}"),
    head_ref_oid: head.to_string(),
    merge_commit: merge_commit.to_string(),
    merge_author_email: Some(email.to_string()),
    merged_at: 900,
  }
}

/// The pairing after a conflict of `local` with `upstream` with the merged pull requests `merged` answers, and the
/// processes of the pairing after the conflict
fn paired(repo: &TestRepo, merged: &dyn MergedPullRequests) -> (AfterConflict, Vec<(String, u32)>) {
  in_scope(repo, |scope| {
    let reads = read_landed(repo.git_executor(), repo.path_str(), scope).unwrap();
    let before = pair_before_replay(repo.git_executor(), repo.path_str(), &reads).unwrap();
    let git = counting();
    let after = pair_after_conflict(&git, repo.path_str(), scope, &reads, before.pairing, &before.paths, Some(merged)).unwrap();
    (after, processes(&git))
  })
}

/// The candidates of `paired` with the pull requests `merged`
fn candidates(repo: &TestRepo, merged: Vec<MergedPullRequest>) -> Vec<LandedCandidate> {
  paired(repo, &Known::of(merged)).0.pairing.candidates
}

fn by_pull_request(number: u32, unit: &[String], upstream: &str) -> LandedCandidate {
  LandedCandidate {
    commit: unit[0].clone(),
    upstream: upstream.to_string(),
    landed_with: unit[1..].to_vec(),
    by: CandidateBy::PullRequest(number),
  }
}

/// The two commits of a pull request on `local`, the second rewriting the line of the first
fn change_and_rework() -> [HistoryCommit; 2] {
  [at("(feature) Change line 5", &[(5, "B1")], 200), at("(feature) Rework line 5", &[(5, "B2")], 300)]
}

/// `commits` as their virtual branch pushed them: copies on `master` without the `(prefix)`, under their identity; the
/// tip
fn pushed(repo: &TestRepo, branch: &str, commits: &[HistoryCommit]) -> String {
  repo.git(&["branch", branch, "master"]);
  let copies: Vec<HistoryCommit> = commits
    .iter()
    .map(|commit| HistoryCommit {
      message: commit.message.replace("(feature) ", "").replace("(other) ", ""),
      ..commit.clone()
    })
    .collect();
  repo.commit_history(&format!("refs/heads/{branch}"), &copies).pop().unwrap()
}

/// The issue of #284: GitHub squashed the pull request with an edit of the review, so its lines are none of the sums of
/// the local commits; the pull request pushed both, so both are its candidate. The squash is of the local author: no
/// `log`, one `rev-list` of the head (after the `rev-list --cherry-mark` and the `diff-tree` of the changed lines)
#[test]
fn a_pull_request_squashed_with_a_review_edit_is_a_candidate_of_the_commits_it_pushed() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &change_and_rework());
  let head = pushed(&repo, "pushed", &change_and_rework());
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2 reviewed")], 900));
  let known = Known::of(vec![merged(1, &head, &upstream, "test@example.com")]);
  let (after, processes) = paired(&repo, &known);
  assert_eq!(after.pairing.candidates, [by_pull_request(1, &local, &upstream)]);
  assert_eq!(
    after.asked,
    Some(MergedAsk {
      since: 200,
      revision: Some(known.known.revision)
    })
  );
  assert_eq!(processes, [("diff-tree".to_string(), 1), ("rev-list".to_string(), 2)]);
}

/// The squash is of a noreply address, never read with the local authors: one `log` of the upstream by that address
/// finds it
#[test]
fn a_pull_request_squashed_under_a_noreply_address_is_a_candidate_of_the_commits_it_pushed() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &change_and_rework());
  let head = pushed(&repo, "pushed", &change_and_rework());
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2")], 900).by(NOREPLY));
  let (after, processes) = paired(&repo, &Known::of(vec![merged(1, &head, &upstream, "1+test@users.noreply.github.com")]));
  assert_eq!(after.pairing.candidates, [by_pull_request(1, &local, &upstream)]);
  assert_eq!(processes, [("log".to_string(), 1), ("rev-list".to_string(), 2)]);
}

/// A local commit made after the push is no commit of the pull request, whatever its lines
#[test]
fn a_commit_never_pushed_stays_out_of_its_pull_request() {
  let repo = sides();
  let mut commits = change_and_rework().to_vec();
  let head = pushed(&repo, "pushed", &commits);
  commits.push(at("(feature) Rework line 5 again", &[(5, "B3")], 950));
  let local = repo.commit_history("refs/heads/local", &commits);
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2 reviewed")], 900).by(NOREPLY));
  assert_eq!(
    candidates(&repo, vec![merged(1, &head, &upstream, "1+test@users.noreply.github.com")]),
    [by_pull_request(1, &local[..2], &upstream)]
  );
}

/// The pull request of a later layer of a stack pushed the earlier layer too: squashed, it lands both, whatever their
/// groups. The earlier layer's own pull request, merged into the later one's head, has no merge commit in the upstream
#[test]
fn a_stacked_layer_squashed_with_its_base_brings_both() {
  let repo = sides();
  let commits = [at("(feature) Change line 5", &[(5, "B1")], 200), at("(other) Change line 8", &[(5, "B1"), (8, "C1")], 300)];
  let local = repo.commit_history("refs/heads/local", &commits);
  let base_layer = pushed(&repo, "pushed-base", &commits[..1]);
  let head = pushed(&repo, "pushed", &commits);
  let upstream = squash(&repo, at("Feature and other (#2)", &[(5, "B1 reviewed"), (8, "C1")], 900));
  let elsewhere = repo.rev_parse("pushed").unwrap();
  assert_eq!(
    candidates(
      &repo,
      vec![merged(1, &base_layer, &elsewhere, "test@example.com"), merged(2, &head, &upstream, "test@example.com")]
    ),
    [by_pull_request(2, &local, &upstream)]
  );
}

/// The repository lacks the head of the pull request (never fetched): it pushed nothing known
#[test]
fn a_pull_request_whose_head_is_missing_pairs_nothing() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &change_and_rework());
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2 reviewed")], 900));
  let missing = "1".repeat(40);
  assert_eq!(candidates(&repo, vec![merged(1, &missing, &upstream, "test@example.com")]), []);
}

/// A merge commit that is no commit of the upstream since the merge base (another branch, an older squash) is no
/// landing of the commits; the search of it reads only that range
#[test]
fn a_merge_commit_outside_the_upstream_since_the_merge_base_pairs_nothing() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &change_and_rework());
  let head = pushed(&repo, "pushed", &change_and_rework());
  repo.git(&["branch", "elsewhere", "master"]);
  let elsewhere = repo.commit_history("refs/heads/elsewhere", &[at("Feature (#1)", &[(5, "B2")], 900).by(NOREPLY)]).remove(0);
  squash(&repo, at("Unrelated", &[(2, "two")], 800));
  let (after, processes) = paired(&repo, &Known::of(vec![merged(1, &head, &elsewhere, "1+test@users.noreply.github.com")]));
  assert_eq!(after.pairing.candidates, []);
  // no walk of the head: no merge commit is in the upstream
  assert_eq!(processes, [("diff-tree".to_string(), 1), ("log".to_string(), 1), ("rev-list".to_string(), 1)]);
}

/// A rebase merge keeps the identities of the commits: its merge commit, the last rebased commit, is a twin, which the
/// keys pair; the pull request adds nothing
#[test]
fn a_rebase_merged_pull_request_is_left_to_the_keys() {
  let repo = sides();
  let commits = change_and_rework();
  repo.commit_history("refs/heads/local", &commits);
  let head = pushed(&repo, "pushed", &commits);
  let rebased = repo.commit_history(
    "refs/heads/upstream",
    &[
      at("Other change", &[(4, "four")], 100),
      at("Change line 5", &[(4, "four"), (5, "B1")], 200),
      at("Rework line 5", &[(4, "four"), (5, "B2")], 300),
    ],
  );
  let without = paired(&repo, &Known::of(Vec::new())).0.pairing;
  let with = paired(&repo, &Known::of(vec![merged(1, &head, &rebased[2], "test@example.com")])).0.pairing;
  assert_eq!(with, without);
  assert!(with.candidates.iter().all(|candidate| candidate.by == CandidateBy::Key));
}

/// A squash whose lines the local commits sum to is their candidate by changed lines, which takes it before the pull
/// request
#[test]
fn the_changed_lines_take_a_squash_before_its_pull_request() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &change_and_rework());
  let head = pushed(&repo, "pushed", &change_and_rework());
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2")], 900));
  let found = candidates(&repo, vec![merged(1, &head, &upstream, "test@example.com")]);
  assert_eq!(found.iter().map(|candidate| candidate.by).collect::<Vec<_>>(), [CandidateBy::ChangedLines]);
}

/// The update onto `upstream`, the local branch the tests keep the baseline on
fn onto_upstream() -> UpstreamRef<'static> {
  UpstreamRef {
    tracking_ref: "refs/heads/upstream",
    name: "upstream",
    previous: None,
    local: true,
  }
}

/// The state of the check of `purpose` of `local` onto `upstream` with the merged pull requests `merged`, and the
/// processes of it
fn check(repo: &TestRepo, purpose: CheckPurpose, merged: &dyn MergedPullRequests) -> (UpdateState, Vec<(String, u32)>) {
  let git = counting();
  let plan = check_update(&git, repo.path_str(), onto_upstream(), purpose, LandedLookup::by_key_with(Some(merged)), ResolveMode::Off).unwrap();
  (plan.check.state, processes(&git))
}

/// A clean replay asks nothing of GitHub; one that conflicts asks once, and leaves out what the pull request pushed when
/// the upstream has its change
#[test]
fn the_merged_pull_requests_are_asked_only_after_a_conflict() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &[at("(feature) Change line 8", &[(8, "eight")], 200)]);
  squash(&repo, at("Change line 2", &[(2, "two")], 900));
  let known = Known::of(Vec::new());
  let (state, _) = check(&repo, CheckPurpose::Apply, &known);
  assert!(matches!(state, UpdateState::Clean { .. }), "{state:?}");
  assert_eq!(known.asked(), 0);

  let repo = sides();
  repo.commit_history("refs/heads/local", &change_and_rework());
  let head = pushed(&repo, "pushed", &change_and_rework());
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2"), (7, "suggested")], 900).by(NOREPLY));
  let known = Known::of(vec![merged(1, &head, &upstream, "1+test@users.noreply.github.com")]);
  let (state, _) = check(&repo, CheckPurpose::Apply, &known);
  assert!(matches!(state, UpdateState::Clean { .. }), "{state:?}");
  assert_eq!(known.asked(), 1);
}

/// A preview that asked the merged pull requests is cached with their answer: the same answer takes the cached replay
/// (no `replay`), another one checks again
#[test]
fn a_cached_preview_that_asked_is_checked_again_with_other_facts() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &change_and_rework());
  let head = pushed(&repo, "pushed", &change_and_rework());
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2"), (7, "suggested")], 900).by(NOREPLY));
  let none = Known::of(Vec::new());
  let squashed = Known::of(vec![merged(1, &head, &upstream, "1+test@users.noreply.github.com")]);
  let replays = |processes: &[(String, u32)]| processes.iter().filter(|(name, _)| name == "replay").map(|(_, count)| *count).sum::<u32>();

  let (state, first) = check(&repo, CheckPurpose::Preview, &none);
  assert!(matches!(state, UpdateState::Conflict { .. }), "{state:?}");
  assert!(replays(&first) > 0);
  let (state, again) = check(&repo, CheckPurpose::Preview, &none);
  assert!(matches!(state, UpdateState::Conflict { .. }), "{state:?}");
  assert_eq!(replays(&again), 0);

  let (state, other) = check(&repo, CheckPurpose::Preview, &squashed);
  assert!(matches!(state, UpdateState::Clean { .. }), "{state:?}");
  assert!(replays(&other) > 0);
  let (state, cached) = check(&repo, CheckPurpose::Preview, &squashed);
  assert!(matches!(state, UpdateState::Clean { .. }), "{state:?}");
  assert_eq!(replays(&cached), 0);
}
