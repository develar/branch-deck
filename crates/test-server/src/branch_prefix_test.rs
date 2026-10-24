//! The prefix of the sync family, resolved by the server as the app resolves it: the page names the repository only

use crate::test_support::{Named, command, create_repository_from, post, start_test_server};
use serde_json::{Value, json};
use std::net::SocketAddr;
use test_utils::git_test_utils::git_command;

/// The virtual branches of the repository at `path`, full names
fn virtual_branches(path: &str) -> Vec<String> {
  let listed = git_command()
    .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
    .current_dir(path)
    .output()
    .unwrap();
  String::from_utf8(listed.stdout)
    .unwrap()
    .lines()
    .filter(|name| name.contains("/virtual/"))
    .map(str::to_owned)
    .collect()
}

/// Sets the app's prefix of the test's settings, as General → Branch prefix does
async fn set_app_prefix(client: &reqwest::Client, addr: SocketAddr, id: &str, prefix: &str) {
  let saved = client
    .post(format!("http://{addr}/store/{id}/appSettings"))
    .json(&json!({ "globalUserBranchPrefix": prefix }))
    .send()
    .await
    .unwrap();
  assert_eq!(saved.status(), 200);
}

/// Syncs the repository at `path` naming nothing but it: the status and the body
async fn sync(client: &reqwest::Client, addr: SocketAddr, path: &str) -> (u16, String) {
  let response = command(client, addr, Named::ByPath, "sync_branches", &json!({ "repositoryPath": path }))
    .send()
    .await
    .unwrap();
  (response.status().as_u16(), response.text().await.unwrap())
}

#[tokio::test]
async fn the_sync_names_the_branches_by_the_app_prefix_where_the_repository_sets_none() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  let (id, path) = create_repository_from(&client, addr, "simple_no_prefix").await;
  set_app_prefix(&client, addr, &id, "jane/").await;

  assert_eq!(
    post(&client, addr, "/invoke/resolve_branch_prefix", json!({ "repositoryPath": path })).await,
    json!({ "prefix": "jane", "source": "app_setting" })
  );
  assert_eq!(sync(&client, addr, &path).await.0, 200);
  let branches = virtual_branches(&path);
  assert!(!branches.is_empty() && branches.iter().all(|name| name.starts_with("jane/virtual/")), "{branches:?}");
}

#[tokio::test]
async fn the_repository_s_own_prefix_wins_and_a_repository_without_any_is_not_synced() {
  let addr = start_test_server().await;
  let client = reqwest::Client::new();
  // the template sets branchdeck.branchPrefix user-name in .git/config
  let (id, path) = create_repository_from(&client, addr, "simple").await;
  set_app_prefix(&client, addr, &id, "jane").await;
  let resolved: Value = post(&client, addr, "/invoke/resolve_branch_prefix", json!({ "repositoryPath": path })).await;
  assert_eq!(resolved, json!({ "prefix": "user-name", "source": "repository_config" }));
  assert_eq!(sync(&client, addr, &path).await.0, 200);
  let branches = virtual_branches(&path);
  assert!(!branches.is_empty() && branches.iter().all(|name| name.starts_with("user-name/virtual/")), "{branches:?}");

  let (_, bare) = create_repository_from(&client, addr, "simple_no_prefix").await;
  // the command's error, as the app's command rejects
  let (_, body) = sync(&client, addr, &bare).await;
  let failure: Value = serde_json::from_str(&body).unwrap();
  assert_eq!(failure["error"]["message"], "Cannot sync without a branch prefix: set one in Settings → General", "{body}");
  assert_eq!(virtual_branches(&bare), [] as [String; 0]);
}
