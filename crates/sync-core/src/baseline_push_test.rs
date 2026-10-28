use crate::baseline_push::{BaselinePush, BaselinePushed, push_baseline};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, pre_receive_hook, stack_ahead_of_bare_origin};

/// The bare `origin` of `stack_ahead_of_bare_origin`
const ORIGIN: &str = ".git/origin.git";

fn push<'a>(repo: &'a TestRepo, tip: &'a str, size_limit: Option<u64>) -> BaselinePushed {
  let push = BaselinePush {
    repository_path: repo.path_str(),
    baseline: "origin/master",
    remote: "origin",
    target: "master",
    tip,
    size_limit,
  };
  let report = push_baseline(repo.git_executor(), &push, &mut |_| {}).unwrap();
  assert_eq!((report.upstream.remote.as_str(), report.upstream.remote_ref.as_str()), ("origin", "refs/heads/master"));
  report.pushed
}

fn remote_master(repo: &TestRepo) -> String {
  repo.git(&["--git-dir", ORIGIN, "rev-parse", "refs/heads/master"])
}

/// A copy built on the target lands as a fast-forward of it, by the push URL: the remote-tracking ref of the baseline
/// stays where the last fetch left it, only `fetch_baseline` moves it; the same commit again is up to date
#[test]
fn a_copy_built_on_the_target_lands_as_a_fast_forward_and_the_tracking_ref_stays() {
  let repo = stack_ahead_of_bare_origin();
  let fetched = repo.rev_parse("refs/remotes/origin/master").unwrap();
  let alpha = repo.rev_parse("test/virtual/alpha").unwrap();
  assert!(matches!(push(&repo, &alpha, None), BaselinePushed::Pushed { up_to_date: false }));
  assert_eq!(remote_master(&repo), alpha);
  assert_eq!(repo.rev_parse("refs/remotes/origin/master").unwrap(), fetched);
  assert!(matches!(push(&repo, &alpha, None), BaselinePushed::Pushed { up_to_date: true }));
}

/// A target that moved since the copy was built on it takes no push: no fast-forward, nothing moves
#[test]
fn a_target_that_moved_refuses_the_copy_as_no_fast_forward() {
  let repo = stack_ahead_of_bare_origin();
  let base = repo.rev_parse("refs/remotes/origin/master").unwrap();
  let tree = repo.git(&["rev-parse", &format!("{base}^{{tree}}")]);
  let other = repo.git(&["commit-tree", &tree, "-p", &base, "-m", "Someone else"]);
  repo.git(&["push", "-q", ORIGIN, &format!("{other}:refs/heads/master")]);
  let alpha = repo.rev_parse("test/virtual/alpha").unwrap();
  let pushed = push(&repo, &alpha, None);
  assert!(matches!(&pushed, BaselinePushed::Moved { summary } if summary.starts_with("[rejected]")), "{pushed:?}");
  assert_eq!(remote_master(&repo), other);
}

/// The server refusing the push (a hook, a protected branch) is said in its own words; the size check refuses a push
/// over the limit before it talks to the remote
#[test]
fn the_server_refusing_the_push_is_said_in_its_words_and_a_large_push_never_goes() {
  let repo = stack_ahead_of_bare_origin();
  let base = remote_master(&repo);
  let alpha = repo.rev_parse("test/virtual/alpha").unwrap();
  assert!(matches!(push(&repo, &alpha, Some(1)), BaselinePushed::TooLarge(_)));
  pre_receive_hook(&repo.path().join(ORIGIN), "echo 'master is protected' >&2\nexit 1");
  let pushed = push(&repo, &alpha, None);
  assert!(
    matches!(&pushed, BaselinePushed::Refused { message } if message.contains("[remote rejected]") && message.contains("master is protected")),
    "{pushed:?}"
  );
  assert_eq!(remote_master(&repo), base);
}

/// The push goes to the remote and the branch its plan named: a baseline that tracks another one now sends nothing
#[test]
fn a_baseline_that_tracks_another_remote_than_the_plan_named_pushes_nothing() {
  let repo = stack_ahead_of_bare_origin();
  let base = remote_master(&repo);
  let alpha = repo.rev_parse("test/virtual/alpha").unwrap();
  let push = BaselinePush {
    repository_path: repo.path_str(),
    baseline: "origin/master",
    remote: "upstream",
    target: "master",
    tip: &alpha,
    size_limit: None,
  };
  let error = push_baseline(repo.git_executor(), &push, &mut |_| {}).unwrap_err();
  assert!(error.to_string().contains("not master of upstream"), "{error:#}");
  assert_eq!(remote_master(&repo), base);
}
