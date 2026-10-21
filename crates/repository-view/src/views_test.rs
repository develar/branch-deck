use crate::branch_table::BranchTable;
use crate::views::RepositoryViews;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use sync_types::repository_view::{GetRepositoryViewParams, RemoteFacts, RobotMatch};

fn params(revision: u32) -> GetRepositoryViewParams {
  GetRepositoryViewParams {
    repository_path: "/repo".to_string(),
    branch_prefix: "test".to_string(),
    revision,
    robots: Vec::new(),
    matched: RobotMatch::default(),
    remote: RemoteFacts::default(),
  }
}

/// Before the first sync the view is empty, of table 0; each finished sync numbers its table; the asker's revision comes
/// back; another branch prefix has its own table
#[test]
fn a_view_is_of_the_table_the_last_sync_finished() {
  let views = RepositoryViews::default();
  let git = GitCommandExecutor::new();
  let view = views.view(&git, &params(7));
  assert_eq!((view.revision, view.table_generation, view.patronus.branches.len()), (7, 0, 0));

  views.keep("/repo", "test", BranchTable::default());
  assert_eq!(views.view(&git, &params(8)).table_generation, 1);
  views.keep("/repo", "test", BranchTable::default());
  assert_eq!(views.view(&git, &params(9)).table_generation, 2);
  let other = GetRepositoryViewParams {
    branch_prefix: "other".to_string(),
    ..params(9)
  };
  assert_eq!(views.view(&git, &other).table_generation, 0);
}
