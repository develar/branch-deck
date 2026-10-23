use crate::github_repository::{GitHubRepository, parse_github_repository, parse_space_mirror, resolve_github_repository};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

pub(crate) fn repository(owner: &str, name: &str) -> GitHubRepository {
  GitHubRepository {
    owner: owner.to_string(),
    name: name.to_string(),
  }
}

#[test]
fn parses_github_remote_urls() {
  for url in [
    "git@github.com:JetBrains/ultimate.git",
    "git@github.com:JetBrains/ultimate",
    "https://github.com/JetBrains/ultimate",
    "https://github.com/JetBrains/ultimate.git",
    "https://github.com/JetBrains/ultimate/",
    "https://user:token@github.com/JetBrains/ultimate.git",
    "ssh://git@github.com/JetBrains/ultimate.git",
    "ssh://git@ssh.github.com:443/JetBrains/ultimate.git",
    "git://github.com/JetBrains/ultimate.git",
    "  https://GitHub.com/JetBrains/ultimate.git\n",
  ] {
    assert_eq!(parse_github_repository(url), Some(repository("JetBrains", "ultimate")), "{url}");
  }
}

#[test]
fn rejects_remote_urls_outside_github() {
  for url in [
    "git@gitlab.com:JetBrains/ultimate.git",
    "https://git.jetbrains.team/ij/ultimate.git",
    "ssh://git@github.example.com/JetBrains/ultimate.git",
    "/Users/me/repos/ultimate.git",
    "../upstream.git",
    ".",
    "https://github.com/JetBrains",
    "https://github.com/JetBrains/ultimate/tree/master",
    "C:\\repos\\ultimate",
  ] {
    assert_eq!(parse_github_repository(url), None, "{url}");
  }
}

pub(crate) fn github_repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.add_remote("origin", "git@github.com:JetBrains/ultimate.git").unwrap();
  repo
}

#[expect(clippy::unwrap_in_result, reason = "a test helper: a git error fails the test, `None` is the answer it checks")]
pub(crate) fn resolve(repo: &TestRepo) -> Option<GitHubRepository> {
  resolve_github_repository(repo.git_executor(), repo.path().to_str().unwrap()).unwrap()
}

#[test]
fn a_remote_outside_github_has_no_github_repository() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  // no origin at all
  assert_eq!(resolve(&repo), None);
  repo.add_remote("origin", "git@gitlab.com:JetBrains/ultimate.git").unwrap();
  assert_eq!(resolve(&repo), None);
}

/// A checkout whose `origin` is a Space repository: Space mirrors the pushed branches to GitHub
fn mirrored_repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.add_remote("origin", "ssh://git@git.example.com/team/monorepo.git").unwrap();
  repo
}

#[test]
fn the_configured_github_repository_wins_over_origin() {
  let repo = mirrored_repo();
  repo.create_commit("Initial commit", "README.md", "# Test");
  assert_eq!(resolve(&repo), None);

  repo.set_config("branchdeck.githubRepository", "acme/monorepo").unwrap();
  assert_eq!(resolve(&repo), Some(repository("acme", "monorepo")));
}

#[test]
fn reads_the_mirror_from_the_committed_space_settings() {
  let repo = mirrored_repo();
  repo.create_commit(
    "Space settings",
    ".space/repository-settings.json",
    r#"{"version": "1.1", "mirror": {"url": "git@github.com:acme/monorepo.git", "useKeyAuth": true}}"#,
  );
  assert_eq!(resolve(&repo), Some(repository("acme", "monorepo")));

  // a file of the working tree is not read: a checkout without the commit has no mirror
  repo.git_executor().execute_command(&["update-ref", "-d", "HEAD"], repo.path().to_str().unwrap()).unwrap();
  assert_eq!(resolve(&repo), None);
}

#[test]
fn a_space_mirror_must_be_on_github() {
  assert_eq!(
    parse_space_mirror(r#"{"mirror": {"url": "https://github.com/acme/monorepo.git"}}"#),
    Some(repository("acme", "monorepo"))
  );
  assert_eq!(parse_space_mirror(r#"{"mirror": {"url": "git@gitlab.com:acme/monorepo.git"}}"#), None);
  assert_eq!(parse_space_mirror(r#"{"mirror": null}"#), None);
  assert_eq!(parse_space_mirror("not json"), None);
}
