use crate::journals::{activity_projects, remember_journal};
use crate::service::SettingsService;
use domain_types::settings::SettingsPatch;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

fn service() -> (tempfile::TempDir, SettingsService) {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), Duration::from_hours(1)).unwrap();
  (directory, service)
}

#[test]
fn the_recent_projects_have_the_journals_remembered_for_them_and_one_opened_before_has_none() {
  let (_directory, service) = service();
  let projects = json!([{ "path": "/work/a" }, { "path": "/work/b" }, { "path": "/work/a-feature" }]);
  service
    .set(
      SettingsPatch::Value {
        key: "recentProjects".to_owned(),
        value: projects,
      },
      "main",
    )
    .unwrap();
  assert_eq!(
    activity_projects(&service),
    [("/work/a".to_owned(), None), ("/work/b".to_owned(), None), ("/work/a-feature".to_owned(), None)]
  );

  let journal = "/work/a/.git/branch-deck/journal.jsonl";
  let change = remember_journal(&service, "/work/a", journal).unwrap().expect("a change for the windows");
  assert_eq!((change.key.as_str(), change.value.clone()), ("journalPaths", json!({ "/work/a": journal })));
  // a worktree of the same repository shares its journal, and keeps the path of the other
  remember_journal(&service, "/work/a-feature", journal).unwrap().unwrap();
  // remembered already: no change
  assert_eq!(remember_journal(&service, "/work/a", journal).unwrap(), None);
  assert_eq!(
    activity_projects(&service),
    [
      ("/work/a".to_owned(), Some(journal.to_owned())),
      ("/work/b".to_owned(), None),
      ("/work/a-feature".to_owned(), Some(journal.to_owned())),
    ]
  );
}

#[test]
fn no_recent_projects_have_no_journals() {
  let (_directory, service) = service();
  remember_journal(&service, "/work/a", "/work/a/.git/branch-deck/journal.jsonl").unwrap();
  assert_eq!(activity_projects(&service), []);
}
