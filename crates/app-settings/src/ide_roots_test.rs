use crate::file::REPOSITORY_STATE_FILE_NAME;
use crate::ide_roots::{picked, remember};
use crate::service::SettingsService;
use pretty_assertions::assert_eq;

fn roots(paths: &[&str]) -> Vec<String> {
  paths.iter().map(|path| (*path).to_owned()).collect()
}

/// The pick of a set of roots is its own, whatever the order the IDE names them in, and stays in the repository state
#[test]
fn the_root_picked_last_is_kept_by_the_set_of_the_roots_in_the_repository_state() {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open(directory.path().to_path_buf()).unwrap();
  let project = roots(&["/work/a", "/work/b"]);
  let other = roots(&["/work/b", "/work/c"]);
  assert_eq!(picked(&service, &project), None, "nothing picked yet");

  remember(&service, &project, "/work/b").unwrap();
  remember(&service, &other, "/work/c").unwrap();
  assert_eq!(
    picked(&service, &roots(&["/work/b", "/work/a"])).as_deref(),
    Some("/work/b"),
    "the same set in another order"
  );
  assert_eq!(picked(&service, &other).as_deref(), Some("/work/c"), "another project keeps its own");
  assert_eq!(picked(&service, &roots(&["/work/a"])), None, "another set of roots");

  service.flush().unwrap();
  let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(directory.path().join(REPOSITORY_STATE_FILE_NAME)).unwrap()).unwrap();
  assert_eq!(saved["ideRootPicks"]["/work/a\n/work/b"], "/work/b");
  // a load keeps them
  drop(service);
  let service = SettingsService::open(directory.path().to_path_buf()).unwrap();
  assert_eq!(picked(&service, &project).as_deref(), Some("/work/b"));
}
