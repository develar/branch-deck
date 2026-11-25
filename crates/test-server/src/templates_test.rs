use crate::templates::Templates;
use pretty_assertions::assert_eq;

/// A server of the same build copies the templates the server before it built: the second one builds nothing
#[tokio::test]
async fn a_template_built_by_one_server_is_taken_as_it_is_by_the_next() {
  let cache = tempfile::tempdir().unwrap();
  let first = Templates::on_demand(cache.path().to_path_buf()).path("simple").await.unwrap();
  assert!(first.join(".git").is_dir(), "{first:?}");
  // what only the first build has: a rebuild would lose it
  std::fs::write(first.join("built-by-the-first-server"), "").unwrap();

  let second = Templates::on_demand(cache.path().to_path_buf()).path("simple").await.unwrap();
  assert_eq!(second, first);
  assert!(second.join("built-by-the-first-server").exists());
  // no half-built copy is left beside the template
  let mut entries: Vec<String> = std::fs::read_dir(cache.path())
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  entries.sort();
  assert_eq!(entries, ["simple"]);
}

/// The repositories that ask for a template at once wait for one build of it
#[tokio::test]
async fn the_repositories_that_ask_at_once_wait_for_one_build() {
  let cache = tempfile::tempdir().unwrap();
  let templates = Templates::on_demand(cache.path().to_path_buf());
  let (a, b) = tokio::join!(templates.path("unassigned"), templates.path("unassigned"));
  assert_eq!(a.unwrap(), b.unwrap());
  templates.path("no-such-template").await.unwrap_err();
}

/// Every template builds and has its directory, and so does its copy synced once, which only the templates without a
/// prefix of their own or without git lack: a route test builds only the ones it asks for, so a change of a template
/// or of the fixture code under it would otherwise break only the E2E tests that copy it
#[tokio::test(flavor = "multi_thread")]
async fn every_template_builds() {
  let cache = tempfile::tempdir().unwrap();
  let templates = std::sync::Arc::new(Templates::presyncing(cache.path().to_path_buf()));
  let mut builds = tokio::task::JoinSet::new();
  for name in templates.names() {
    let templates = std::sync::Arc::clone(&templates);
    builds.spawn(async move {
      let built = async { Ok::<_, anyhow::Error>((templates.path(name).await?, templates.synced(name).await?)) };
      (name, built.await.map_err(|e| format!("{e:#}")))
    });
  }
  let (mut failed, mut unsynced) = (Vec::new(), Vec::new());
  while let Some(built) = builds.join_next().await {
    match built.unwrap() {
      (name, Ok((path, synced))) => {
        assert!(path.is_dir() && synced.is_dir(), "{path:?} {synced:?}");
        if synced == path {
          unsynced.push(name);
        }
      }
      (name, Err(e)) => failed.push(format!("{name}: {e}")),
    }
  }
  failed.sort();
  unsynced.sort_unstable();
  assert_eq!(failed, Vec::<String>::new());
  assert_eq!(unsynced, ["bare", "empty-non-git", "empty_git", "simple_no_prefix"]);
}

/// The names of the refs of the repository at `path` under `prefix`
fn refs(path: &std::path::Path, prefix: &str) -> Vec<String> {
  let output = test_utils::git_test_utils::git_command()
    .args(["for-each-ref", "--format=%(refname)", prefix])
    .current_dir(path)
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
  String::from_utf8(output.stdout).unwrap().lines().map(str::to_owned).collect()
}

/// The copy of a template synced once has the virtual branches and the mapping notes its first sync builds, and none of
/// the files that sync leaves in `$GIT_DIR/branch-deck/` (its mark, which the app reads as a `brd sync` of a terminal);
/// the template stays as it is built, and a template without a prefix of its own, or without git, is its own copy
#[tokio::test(flavor = "multi_thread")]
async fn a_template_synced_once_has_its_virtual_branches_and_notes_and_no_mark_of_the_sync() {
  let cache = tempfile::tempdir().unwrap();
  let templates = Templates::presyncing(cache.path().to_path_buf());
  let synced = templates.synced("issue_links").await.unwrap();
  let raw = templates.path("issue_links").await.unwrap();
  assert_eq!(synced, cache.path().join("issue_links.synced"));
  assert_eq!(refs(&synced, "refs/heads/user-name/virtual/").len(), 7);
  assert_eq!(refs(&synced, "refs/notes/"), ["refs/notes/commits"]);
  assert!(!synced.join(".git/branch-deck").exists());
  assert_eq!(refs(&raw, "refs/heads/user-name/"), Vec::<String>::new(), "the template is as it is built");
  for name in ["simple_no_prefix", "bare", "empty-non-git"] {
    assert_eq!(templates.synced(name).await.unwrap(), templates.path(name).await.unwrap(), "{name}");
  }
  assert_eq!(
    Templates::on_demand(cache.path().to_path_buf()).synced("issue_links").await.unwrap(),
    raw,
    "a route test copies the template as it is built"
  );
}
