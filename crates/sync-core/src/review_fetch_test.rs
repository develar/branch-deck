use crate::review_fetch::{GitHubCredentials, NoCommonHistory, fetch_review_head, fetch_review_head_until, prune_review_refs, review_refs_to_prune};
use git_executor::git_command_executor::{ProgressStop, StoppedCommand};
use pretty_assertions::assert_eq;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use sync_types::review_request::{FetchReviewHeadParams, ReviewHeadState, ReviewRange, ReviewSource};
use test_log::test;
use test_utils::fake_executable::fake_executable;
use test_utils::git_test_utils::TestRepo;

const GITHUB_URL: &str = "https://github.com/acme/monorepo.git";
const NUMBER: u32 = 7;

/// `github` stands for github.com/acme/monorepo: `master` and the branch of pull request #7 (two commits, also as
/// `refs/pull/7/head`). `local` was cloned from it before the pull request, so it has neither commit of the pull
/// request; its URLs of github.com lead to `github`
struct Fixture {
  github: TestRepo,
  local: TestRepo,
  base: String,
  first: String,
  head: String,
}

fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, path(repo)).unwrap()
}

fn fixture() -> Fixture {
  let github = TestRepo::new();
  let base = github.create_commit("Initial", "README.md", "# Test\n");
  let local = TestRepo::new_empty();
  local.clone_from(github.path()).unwrap();
  github.checkout_new_branch("alice/feature").unwrap();
  let first = github.create_commit("Add login\n\nThe form and its body\nover two lines", "login.txt", "login\n");
  let head = github.create_commit("Add logout", "logout.txt", "logout\n");
  git(&github, &["update-ref", &format!("refs/pull/{NUMBER}/head"), &head]);
  github.checkout("master").unwrap();
  local.set_config(&github_rewrite(&github), GITHUB_URL).unwrap();
  local.set_config("branchdeck.githubRepository", "acme/monorepo").unwrap();
  Fixture { github, local, base, first, head }
}

/// The config key that sends the fetches of github.com/acme/monorepo to `github`
fn github_rewrite(github: &TestRepo) -> String {
  format!("url.{}.insteadOf", path(github))
}

impl Fixture {
  fn params(&self) -> FetchReviewHeadParams {
    FetchReviewHeadParams {
      repository_path: path(&self.local).to_string(),
      number: NUMBER,
      head_ref_name: "alice/feature".to_string(),
      head_ref_oid: self.head.clone(),
      base_ref_name: "master".to_string(),
      base_ref_oid: self.base.clone(),
      is_cross_repository: false,
    }
  }

  fn fetch(&self, params: &FetchReviewHeadParams) -> anyhow::Result<ReviewRange> {
    fetch_review_head(self.local.git_executor(), params, &mut |_| {}, &|| false)
  }

  /// `origin` becomes a Space repository that GitHub mirrors: it has the branches, not `refs/pull/*`
  fn mirror(&self) -> TestRepo {
    let space = TestRepo::new_empty();
    git(&space, &["clone", "--bare", "-q", path(&self.github), "."]);
    git(&self.local, &["remote", "set-url", "origin", path(&space)]);
    space
  }

  /// github.com is reached through `url` instead
  fn redirect_github(&self, url: &str) {
    git(&self.local, &["config", "--unset-all", &github_rewrite(&self.github)]);
    self.local.set_config(&format!("url.{url}.insteadOf"), GITHUB_URL).unwrap();
  }

  fn refs(&self, prefix: &str) -> Vec<String> {
    git(&self.local, &["for-each-ref", "--format=%(refname) %(objectname)", prefix])
      .lines()
      .map(str::to_string)
      .collect()
  }
}

fn commit_ids(range: &ReviewRange) -> Vec<&str> {
  range.commits.iter().map(|commit| commit.id.as_str()).collect()
}

#[test]
fn fetches_the_pull_ref_from_a_github_origin() {
  let fixture = fixture();
  git(&fixture.local, &["remote", "set-url", "origin", GITHUB_URL]);
  let mut progress = 0;
  let range = fetch_review_head(fixture.local.git_executor(), &fixture.params(), &mut |_| progress += 1, &|| false).unwrap();

  assert_eq!((range.base.as_str(), range.head.as_str()), (fixture.base.as_str(), fixture.head.as_str()));
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Same, ReviewSource::Origin));
  // oldest first, with what the preview shows
  assert_eq!(commit_ids(&range), [fixture.first.as_str(), fixture.head.as_str()]);
  assert_eq!(range.commits[1].subject, "Add logout");
  assert_eq!(range.commits[1].message, "Add logout");
  // the body too, line breaks kept
  assert_eq!(range.commits[0].subject, "Add login");
  assert_eq!(range.commits[0].message, "Add login\n\nThe form and its body\nover two lines");
  assert_eq!(range.commits[1].author, "Test User");
  assert!(range.commits[1].author_time > 0);
  assert!(range.commits[1].committer_time > 0);
  // the base was local: only the head ref is written
  assert_eq!(fixture.refs("refs/branch-deck/review/"), [format!("refs/branch-deck/review/7/head {}", fixture.head)]);
  assert!(progress > 0);
}

#[test]
fn commits_that_are_local_need_no_fetch_and_no_ref() {
  let fixture = fixture();
  git(&fixture.local, &["fetch", "-q", "origin", "alice/feature"]);
  // a fetch would fail
  git(&fixture.local, &["remote", "set-url", "origin", "/nonexistent/monorepo.git"]);
  let range = fixture.fetch(&fixture.params()).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Same, ReviewSource::Local));
  assert_eq!(commit_ids(&range), [fixture.first.as_str(), fixture.head.as_str()]);
  assert_eq!(fixture.refs("refs/branch-deck/review/"), Vec::<String>::new());
}

#[test]
fn fetches_the_branch_from_the_mirror_without_touching_its_remote_tracking_refs() {
  let fixture = fixture();
  let _space = fixture.mirror();
  let origin_refs = fixture.refs("refs/remotes/");
  let range = fixture.fetch(&fixture.params()).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Same, ReviewSource::Origin));
  assert_eq!(range.head, fixture.head);
  // `--refmap=`: no `refs/remotes/origin/alice/feature`; `--no-prune`: nothing pruned
  assert_eq!(fixture.refs("refs/remotes/"), origin_refs);
  // no FETCH_HEAD, the working tree and the index untouched
  assert!(!fixture.local.path().join(".git/FETCH_HEAD").exists());
  assert_eq!(git(&fixture.local, &["status", "--porcelain"]), "");
}

#[test]
fn a_mirror_ahead_of_the_pull_request_shows_the_pull_request() {
  let fixture = fixture();
  let space = fixture.mirror();
  // pushed to Space, not mirrored to GitHub yet
  let work = TestRepo::new_empty();
  work.clone_from(space.path()).unwrap();
  work.checkout("alice/feature").unwrap();
  let newer = work.create_commit("Add more", "more.txt", "more\n");
  git(&work, &["push", "-q", "origin", "alice/feature"]);

  let range = fixture.fetch(&fixture.params()).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::SourceAhead, ReviewSource::Origin));
  assert_eq!(range.head, fixture.head);
  assert_eq!(fixture.local.rev_parse("refs/branch-deck/review/7/head").unwrap(), newer);
}

#[test]
fn a_mirror_behind_the_pull_request_falls_back_to_github() {
  let fixture = fixture();
  let space = fixture.mirror();
  // Space has only the first commit of the branch
  git(&space, &["update-ref", "refs/heads/alice/feature", &fixture.first]);
  let range = fixture.fetch(&fixture.params()).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Same, ReviewSource::GitHub));
  assert_eq!(range.head, fixture.head);
  assert_eq!(fixture.local.rev_parse("refs/branch-deck/review/7/head").unwrap(), fixture.head);
}

#[test]
fn a_force_pushed_source_branch_diverged() {
  let fixture = fixture();
  let space = fixture.mirror();
  // the pull request's commits are local from an earlier review, the base is not
  git(&fixture.local, &["fetch", "-q", path(&fixture.github), &fixture.head]);
  let other = TestRepo::new_empty();
  other.clone_from(space.path()).unwrap();
  let rewritten = other.create_commit("Rewritten", "other.txt", "other\n");
  git(&other, &["push", "-q", "-f", "origin", "HEAD:alice/feature"]);
  let advanced = fixture.github.create_commit("Advance master", "master.txt", "next\n");
  let params = FetchReviewHeadParams {
    base_ref_oid: advanced,
    ..fixture.params()
  };

  let range = fixture.fetch(&params).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Diverged, ReviewSource::Origin));
  assert_eq!(range.head, fixture.head);
  assert_eq!(fixture.local.rev_parse("refs/branch-deck/review/7/head").unwrap(), rewritten);
}

#[test]
fn a_head_that_cannot_be_got_shows_the_source_branch() {
  let fixture = fixture();
  let space = fixture.mirror();
  git(&space, &["update-ref", "refs/heads/alice/feature", &fixture.first]);
  // GitHub is not reachable
  fixture.redirect_github("/nonexistent/monorepo.git");

  let range = fixture.fetch(&fixture.params()).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Missing, ReviewSource::Origin));
  assert_eq!(range.head, fixture.first);
  assert_eq!(commit_ids(&range), [fixture.first.as_str()]);
}

#[test]
fn a_fork_is_fetched_from_github() {
  let fixture = fixture();
  // origin is Space: the fork's branch is not there
  let _space = fixture.mirror();
  let params = FetchReviewHeadParams {
    head_ref_name: "someone/feature".to_string(),
    is_cross_repository: true,
    ..fixture.params()
  };
  let range = fixture.fetch(&params).unwrap();
  assert_eq!((range.head_state, range.source), (ReviewHeadState::Same, ReviewSource::GitHub));
  assert_eq!(range.head, fixture.head);
}

#[test]
fn a_missing_base_is_fetched_in_the_same_fetch() {
  let fixture = fixture();
  git(&fixture.local, &["remote", "set-url", "origin", GITHUB_URL]);
  // master moved on GitHub after the branch was cut: a stale origin/master would put its changes into the range
  let advanced = fixture.github.create_commit("Advance master", "master.txt", "next\n");
  let params = FetchReviewHeadParams {
    base_ref_oid: advanced.clone(),
    ..fixture.params()
  };
  let range = fixture.fetch(&params).unwrap();
  assert_eq!(range.base, fixture.base);
  assert_eq!(commit_ids(&range), [fixture.first.as_str(), fixture.head.as_str()]);
  assert_eq!(
    fixture.refs("refs/branch-deck/review/"),
    [
      format!("refs/branch-deck/review/7/base {advanced}"),
      format!("refs/branch-deck/review/7/head {}", fixture.head)
    ]
  );
}

#[test]
fn a_pull_request_without_common_history_is_an_error() {
  let fixture = fixture();
  git(&fixture.local, &["remote", "set-url", "origin", GITHUB_URL]);
  let tree = git(&fixture.github, &["rev-parse", "HEAD^{tree}"]);
  let orphan = git(&fixture.github, &["commit-tree", "-m", "Unrelated", &tree]);
  git(&fixture.github, &["update-ref", &format!("refs/pull/{NUMBER}/head"), &orphan]);
  let params = FetchReviewHeadParams {
    head_ref_oid: orphan,
    ..fixture.params()
  };
  let error = fixture.fetch(&params).unwrap_err();
  assert!(error.is::<NoCommonHistory>(), "{error:#}");
  assert_eq!(error.to_string(), "The pull request has no common history with master");
}

#[test]
fn refused_github_credentials_are_told_how_to_fix() {
  let fixture = fixture();
  let _space = fixture.mirror();
  let dir = tempfile::tempdir().unwrap();
  // the words of git-remote-https when GIT_TERMINAL_PROMPT=0 keeps it from asking
  let ssh = fake_executable(
    dir.path(),
    "ssh",
    "echo \"fatal: could not read Username for 'https://github.com': terminal prompts disabled\" >&2\nexit 128",
  );
  fixture.local.set_config("core.sshCommand", ssh.to_str().unwrap()).unwrap();
  fixture.redirect_github("ssh://git@github.example/acme/monorepo.git");
  let params = FetchReviewHeadParams {
    is_cross_repository: true,
    ..fixture.params()
  };
  let error = fixture.fetch(&params).unwrap_err();
  assert!(error.is::<GitHubCredentials>(), "{error:#}");
  assert_eq!(
    error.to_string(),
    "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)"
  );
}

/// A fixture whose origin is an ssh remote that never answers
fn hanging_fixture() -> (Fixture, tempfile::TempDir) {
  let fixture = fixture();
  let dir = tempfile::tempdir().unwrap();
  let ssh = fake_executable(dir.path(), "ssh", "exec sleep 30");
  fixture.local.set_config("core.sshCommand", ssh.to_str().unwrap()).unwrap();
  git(&fixture.local, &["remote", "set-url", "origin", "ssh://git@git.example.com/team/monorepo.git"]);
  (fixture, dir)
}

#[test]
fn a_cancel_kills_the_fetch() {
  let (fixture, _dir) = hanging_fixture();
  let cancelled = AtomicBool::new(false);
  let started = Instant::now();
  let result = std::thread::scope(|scope| {
    scope.spawn(|| {
      std::thread::sleep(Duration::from_millis(300));
      cancelled.store(true, Ordering::SeqCst);
    });
    fetch_review_head(fixture.local.git_executor(), &fixture.params(), &mut |_| {}, &|| cancelled.load(Ordering::SeqCst))
  });
  let error = result.unwrap_err();
  assert_eq!(error.downcast_ref::<StoppedCommand>(), Some(&StoppedCommand::Cancelled), "{error:#}");
  assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

#[test]
fn a_stalled_fetch_is_killed() {
  let (fixture, _dir) = hanging_fixture();
  let stop = ProgressStop {
    idle_timeout: Some(Duration::from_millis(300)),
    is_cancelled: &|| false,
  };
  let error = fetch_review_head_until(fixture.local.git_executor(), &fixture.params(), &mut |_| {}, &stop).unwrap_err();
  assert_eq!(
    error.downcast_ref::<StoppedCommand>(),
    Some(&StoppedCommand::Stalled(Duration::from_millis(300))),
    "{error:#}"
  );
}

#[test]
fn refuses_ids_and_names_that_could_be_options() {
  let fixture = fixture();
  for params in [
    FetchReviewHeadParams {
      head_ref_oid: "--upload-pack=touch".to_string(),
      ..fixture.params()
    },
    FetchReviewHeadParams {
      base_ref_oid: "HEAD~1".to_string(),
      ..fixture.params()
    },
    FetchReviewHeadParams {
      head_ref_name: "a:refs/heads/master".to_string(),
      ..fixture.params()
    },
    FetchReviewHeadParams {
      base_ref_name: "-x".to_string(),
      ..fixture.params()
    },
    FetchReviewHeadParams {
      head_ref_name: "a/../b".to_string(),
      ..fixture.params()
    },
  ] {
    let error = fixture.fetch(&params).unwrap_err().to_string();
    assert!(error.starts_with("Not a "), "{error}");
  }
}

fn review_refs(repo: &TestRepo) -> Vec<String> {
  git(repo, &["for-each-ref", "--format=%(refname)", "refs/branch-deck/review/"])
    .lines()
    .map(str::to_string)
    .collect()
}

#[test]
fn prune_deletes_the_review_refs_not_kept() {
  let repo = TestRepo::new();
  let commit = repo.create_commit("Initial", "README.md", "# Test\n");
  for name in ["7/head", "7/base", "8/head", "not-a-number/head"] {
    git(&repo, &["update-ref", &format!("refs/branch-deck/review/{name}"), &commit]);
  }
  prune_review_refs(repo.git_executor(), path(&repo), &[8, 9]).unwrap();
  assert_eq!(review_refs(&repo), ["refs/branch-deck/review/8/head"]);

  prune_review_refs(repo.git_executor(), path(&repo), &[]).unwrap();
  assert_eq!(review_refs(&repo), Vec::<String>::new());
  // nothing to prune is no error
  prune_review_refs(repo.git_executor(), path(&repo), &[]).unwrap();
}

#[test]
fn prune_deletes_nothing_when_a_ref_moved_meanwhile() {
  let repo = TestRepo::new();
  let first = repo.create_commit("Initial", "README.md", "# Test\n");
  let second = repo.create_commit("Second", "README.md", "# Test 2\n");
  for name in ["7/head", "8/head"] {
    git(&repo, &["update-ref", &format!("refs/branch-deck/review/{name}"), &first]);
  }
  let transaction = review_refs_to_prune(repo.git_executor(), path(&repo), &[]).unwrap();
  // a review fetch of another window
  git(&repo, &["update-ref", "refs/branch-deck/review/8/head", &second]);
  assert!(transaction.commit(repo.git_executor(), path(&repo), "prune review refs").is_err());
  assert_eq!(review_refs(&repo), ["refs/branch-deck/review/7/head", "refs/branch-deck/review/8/head"]);
  // the next prune deletes them
  prune_review_refs(repo.git_executor(), path(&repo), &[]).unwrap();
  assert_eq!(review_refs(&repo), Vec::<String>::new());
}
