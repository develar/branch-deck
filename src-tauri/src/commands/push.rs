use git_ops::git_command::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PushBranchParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub branch_name: String,
}

/// Pushes a specific branch to the remote repository
#[tauri::command]
#[specta::specta]
pub async fn push_branch(git_executor: State<'_, GitCommandExecutor>, params: PushBranchParams) -> Result<String, String> {
  let repository_path = &params.repository_path;
  let branch_prefix = &params.branch_prefix;
  let branch_name = &params.branch_name;
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
