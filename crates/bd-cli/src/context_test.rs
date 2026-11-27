use crate::fixture_test::{PREFIX, context_with_the_config_prefix, environment, repo_with_baseline, repository};
use bd_core::bd_command::{CLI_EXE, CLI_NAME, bd_command, shell_word};
use bd_core::context::{RepoContext, RepoOptions};
use pretty_assertions::assert_eq;

/// The brd of a context (`RepoContext::bd`) names its repository with one `-C`, whether the binary of brd is known (the
/// app) or not (a test): a `next` command of the repository appends only its arguments to it, as `brd update --continue`
/// in a lease does with its `brd -C <lease> lease pick`. It carries the `--prefix` of the command line, and so does the
/// brd of another worktree (`bd_in`); a prefix of the config is not named
#[test_log::test]
fn the_brd_of_a_context_names_its_repository_once() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let bin = tempfile::tempdir().unwrap();
  let binary = bin.path().join(if cfg!(windows) { CLI_EXE } else { CLI_NAME });
  std::fs::write(&binary, "brd").unwrap();
  let options = RepoOptions {
    repo: None,
    prefix: Some(PREFIX),
    full_ids: false,
    no_cache: false,
  };
  let mut known = environment(repo.path());
  known.executable = Some(binary);
  let program = bd_command(&known).unwrap().for_shell();
  let in_repository = format!(" -C {} --prefix {PREFIX}", shell_word(&repository(&repo)));

  let unknown = RepoContext::resolve(&environment(repo.path()), &options).unwrap();
  let known = RepoContext::resolve(&known, &options).unwrap();
  assert_eq!(known.bd_in("/tmp/lease"), format!("{CLI_NAME} -C /tmp/lease --prefix {PREFIX}"));
  assert_eq!((unknown.bd, known.bd), (format!("{CLI_NAME}{in_repository}"), format!("{program}{in_repository}")));
  let configured = context_with_the_config_prefix(&repo);
  assert_eq!(configured.bd, format!("{CLI_NAME} -C {}", shell_word(&repository(&repo))));
}

/// The config the context read for the branch prefix is the one of the whole command (the sync takes it); after a wait
/// for the repository lock it is read again: another process may have changed it meanwhile
#[test_log::test]
fn the_config_is_read_once_and_again_after_a_wait_for_the_lock() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let context = context_with_the_config_prefix(&repo);
  let reference = format!("refs/heads/{PREFIX}/virtual/auth");
  repo.set_config(&format!("branch.{PREFIX}/virtual/auth.description"), "Add login\n").unwrap();
  assert_eq!(context.config().unwrap().descriptions().get(&reference), None, "read with the prefix");

  context.forget_config();
  assert_eq!(context.config().unwrap().descriptions()[&reference].title, "Add login");
}

/// HEAD's branch, the remotes with their push URLs and the baseline are read once for the whole command (its sync and
/// the push URL of `origin` take them); after a wait for the repository lock they are read again, as the config
#[test_log::test]
fn the_facts_are_read_once_and_again_after_a_wait_for_the_lock() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let context = context_with_the_config_prefix(&repo);
  let branch = context.head_branch().unwrap();
  let url = context.origin_url().unwrap();
  let baseline = context.baseline().unwrap();
  let processes = context.git.process_stats().unwrap().total();

  repo.git(&["remote", "set-url", "--push", "origin", "ssh://git@git.jetbrains.team/ij/app.git"]);
  repo.git(&["switch", "-q", "--detach"]);
  assert_eq!(context.head_branch().unwrap(), branch);
  assert_eq!(context.origin_url().unwrap(), url);
  assert_eq!(context.baseline().unwrap(), baseline);
  assert_eq!(context.git.process_stats().unwrap().total(), processes, "no process for a fact read already");

  context.forget_facts();
  assert_eq!(context.head_branch().unwrap(), None);
  assert_eq!(context.origin_url().unwrap().as_deref(), Some("ssh://git@git.jetbrains.team/ij/app.git"));
  assert_eq!(context.baseline().unwrap().branch, None);
}

/// HEAD's branch and its baseline do not outlive a call over the network before the lock: the user may switch or detach
/// HEAD while a fetch runs; the remotes stay
#[test_log::test]
fn the_head_is_read_again_after_a_call_over_the_network() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let context = context_with_the_config_prefix(&repo);
  let url = context.origin_url().unwrap();
  let baseline = context.over_network(|| {
    let baseline = context.baseline().unwrap();
    repo.git(&["switch", "-q", "--detach"]);
    baseline
  });
  assert!(baseline.branch.is_some(), "HEAD was on a branch before the call");
  let processes = context.git.process_stats().unwrap().total();

  assert_eq!(context.origin_url().unwrap(), url);
  assert_eq!(context.git.process_stats().unwrap().total(), processes, "the remotes are not read again");
  assert_eq!(context.head_branch().unwrap(), None);
  assert_eq!(context.baseline().unwrap().branch, None);
}
