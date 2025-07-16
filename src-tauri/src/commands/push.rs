use git_ops::git_command::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use tauri::State;

/// Pushes a specific branch to the remote repository
#[tauri::command]
#[specta::specta]
pub async fn push_branch(git_executor: State<'_, GitCommandExecutor>, repository_path: &str, branch_prefix: &str, branch_name: &str) -> Result<String, String> {
  let final_branch_name = to_final_branch_name(branch_prefix, branch_name).map_err(|e| format!("{e:?}"))?;
  git_executor
    .execute_command(
      &[
        "-c",
        "credential.helper=",
        "-c",
        "log.showSignature=false",
        "push",
        "--porcelain",
        "--force",
        "origin",
        &format!("refs/heads/{final_branch_name}:{final_branch_name}"),
      ],
      repository_path,
    )
    .map_err(|e| e.to_string())
}
