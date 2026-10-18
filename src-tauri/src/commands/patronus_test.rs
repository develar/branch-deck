use crate::commands::patronus::PatronusState;
use git_executor::git_command_executor::GitCommandExecutor;

/// The Space repository costs one push URL of `origin`, and nothing while it is kept
#[tokio::test]
async fn the_space_repository_is_one_push_url_and_then_kept() {
  let git = GitCommandExecutor::new().with_process_stats("test");
  let repository = tempfile::tempdir().unwrap();
  let path = repository.path().to_str().unwrap();
  git.execute_command(&["init", "-q"], path).unwrap();
  git
    .execute_command(&["remote", "add", "origin", "ssh://git@git.jetbrains.team/ij/ultimate.git"], path)
    .unwrap();
  let processes = || git.process_stats().unwrap().total();
  let before = processes();

  let patronus = PatronusState::new("test");
  let space = patronus.space_repository(&git, path).await.unwrap().expect("on Space");
  assert_eq!((space.project_key.as_str(), space.name.as_str()), ("ij", "ultimate"));
  assert_eq!(processes(), before + 1, "one remote get-url");
  patronus.space_repository(&git, path).await.unwrap();
  assert_eq!(processes(), before + 1, "kept");
}
