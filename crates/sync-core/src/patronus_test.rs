use crate::patronus::{SpaceRepository, find_target_branch, parse_space_repository};
use test_utils::git_test_utils::TestRepo;

#[test]
fn parses_space_remote_urls() {
  let ultimate = Some(SpaceRepository {
    project_key: "ij".to_string(),
    name: "ultimate".to_string(),
  });
  for url in [
    "ssh://git@git.jetbrains.team/ij/ultimate.git",
    "git@git.jetbrains.team:ij/ultimate.git",
    "https://git.jetbrains.team/ij/ultimate.git",
    "https://user:token@git.jetbrains.team/ij/ultimate",
    "ssh://git@Git.JetBrains.Team:22/ij/ultimate.git\n",
  ] {
    assert_eq!(parse_space_repository(url), ultimate, "{url}");
  }
}

#[test]
fn rejects_remote_urls_outside_space() {
  for url in [
    "git@github.com:JetBrains/intellij-community.git",
    "https://jetbrains.team/p/ij/repositories/ultimate",
    "ssh://git@git.jetbrains.team/ultimate.git",
    "/Users/me/ultimate",
    "",
  ] {
    assert_eq!(parse_space_repository(url), None, "{url}");
  }
}

#[test]
fn the_target_is_the_branch_of_origin_the_baseline_tracks() {
  let upstream = TestRepo::new();
  upstream.create_commit("Initial", "file.txt", "content");
  let clone = TestRepo::new_empty();
  clone.clone_from(upstream.path()).unwrap();
  let path = clone.path().to_str().unwrap();
  let git = clone.git_executor();
  let default_branch = git.execute_command(&["rev-parse", "--abbrev-ref", "HEAD"], path).unwrap();

  assert_eq!(find_target_branch(git, path, &format!("origin/{default_branch}")).unwrap(), Some(default_branch.clone()));
  // a local baseline has no target
  assert_eq!(find_target_branch(git, path, &default_branch).unwrap(), None);

  // a baseline on another remote: Patronus works on the push remote
  clone.add_remote("fork", upstream.path().to_str().unwrap()).unwrap();
  git.execute_command(&["fetch", "--quiet", "fork"], path).unwrap();
  assert_eq!(find_target_branch(git, path, &format!("fork/{default_branch}")).unwrap(), None);
}
