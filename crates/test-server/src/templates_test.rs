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
