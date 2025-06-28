use crate::git::git_command::GitCommandExecutor;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_branch_prefix_from_git_config(git_executor: State<'_, GitCommandExecutor>, repository_path: &str) -> Result<String, String> {
  git_executor.execute_command(&["config", "get", "branchdeck.branchPrefix"], repository_path)
}
