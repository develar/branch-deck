use git2;

#[tauri::command]
#[specta::specta]
pub async fn get_branch_prefix_from_git_config(repository_path: &str) -> Result<String, String> {
  let config = if repository_path.is_empty() {
    // if no repository path, open default config (global and system)
    git2::Config::open_default().map_err(|e| format!("Failed to open git config: {e}"))?
  } else {
    // If a repository path is provided, open the repository and get its config.
    // This config will automatically include local, global, and system configs in the correct precedence.
    match git2::Repository::open(repository_path) {
      Ok(repo) => repo.config().map_err(|e| format!("Failed to get repository config: {e}"))?,
      Err(_) => {
        // if the repository cannot be opened, fall back to global config only.
        // this is not considered an error for this function, but a missing repo.
        git2::Config::open_default().map_err(|e| format!("Failed to open git config: {e}"))?
      }
    }
  };

  // try to get the branch prefix from the config
  match config.get_string("branchdeck.branchPrefix") {
    Ok(value) => Ok(value),
    Err(e) => {
      // if the error is "not found", return an empty string instead of an error
      if e.code() == git2::ErrorCode::NotFound {
        Ok(String::new())
      } else {
        Err(format!("Failed to get branch prefix from config: {e}"))
      }
    }
  }
}
