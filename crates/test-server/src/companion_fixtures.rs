//! The companion repositories of a test repository (spec/branch/declared-needs.spec.md#companion-repositories): a test
//! asks for one (`POST /repositories/{id}/companion`) and gets the path of a new repository of one commit beside the
//! test repository, `<id>.companions/<name>` under the root of the server, deleted with the test repository. The folder
//! dialog of Add Companion… then picks it (`POST /repositories/{id}/dialog-pick`)

use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use git_ops::ref_transaction::RefTransaction;
use serde::Deserialize;
use std::sync::Arc;
use sync_utils::span::in_current_span;

/// The empty tree of git: the one commit of a companion changes nothing
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionFixture {
  name: String,
}

/// `POST /repositories/{id}/companion`: the path of the new repository
pub async fn add_companion(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<CompanionFixture>) -> Result<Json<String>, StatusCode> {
  if !state.repositories.contains_key(&id) {
    return Err(StatusCode::UNPROCESSABLE_ENTITY);
  }
  let path = state.companions_of(&id).join(&fixture.name);
  let git = state.git_executor.clone();
  let created = tokio::task::spawn_blocking(in_current_span(move || -> anyhow::Result<String> {
    std::fs::create_dir_all(&path)?;
    let path = path.to_string_lossy().into_owned();
    git.execute_command(&["init", "-q", "-b", "master"], &path)?;
    let identity = ["-c", "user.name=Companion", "-c", "user.email=companion@example.com"];
    let commit = git.execute_command(&[&identity[..], &["commit-tree", EMPTY_TREE, "-m", "Initial commit"]].concat(), &path)?;
    let mut transaction = RefTransaction::new();
    transaction.create("refs/heads/master", commit.trim());
    transaction.commit(&git, &path, "companion fixture")?;
    Ok(path)
  }))
  .await
  .map_err(|_joined| StatusCode::INTERNAL_SERVER_ERROR)?;
  created.map(Json).map_err(|error| {
    tracing::warn!(id, error = %format!("{error:#}"), "cannot create the companion");
    StatusCode::INTERNAL_SERVER_ERROR
  })
}
