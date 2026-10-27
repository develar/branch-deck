use crate::file::REPOSITORY_STATE_FILE_NAME;
use crate::linked_pull_requests::{MAX_LINKS, forget, link, linked};
use crate::service::SettingsService;
use domain_types::review_request::LinkedPullRequest;
use pretty_assertions::assert_eq;

fn at(number: u32, linked_at: u32) -> LinkedPullRequest {
  LinkedPullRequest { number, linked_at }
}

#[test]
fn keeps_the_links_of_each_repository_newest_first_in_the_repository_state() {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open(directory.path().to_path_buf()).unwrap();
  link(&service, "acme/monorepo", at(101, 1)).unwrap();
  link(&service, "acme/monorepo", at(102, 2)).unwrap();
  link(&service, "acme/other", at(7, 3)).unwrap();
  // a link again moves to the front with its new time
  link(&service, "acme/monorepo", at(101, 4)).unwrap();
  assert_eq!(linked(&service, "acme/monorepo"), vec![at(101, 4), at(102, 2)]);
  assert_eq!(linked(&service, "acme/other"), vec![at(7, 3)]);

  assert!(forget(&service, "acme/monorepo", &[999]).unwrap().is_none(), "nothing to forget writes nothing");
  forget(&service, "acme/monorepo", &[101]).unwrap();
  assert_eq!(linked(&service, "acme/monorepo"), vec![at(102, 2)]);

  service.flush().unwrap();
  let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(directory.path().join(REPOSITORY_STATE_FILE_NAME)).unwrap()).unwrap();
  assert_eq!(saved["linkedPullRequests"]["acme/monorepo"], serde_json::json!([{"number": 102, "linkedAt": 2}]));
  // a load keeps them
  drop(service);
  let service = SettingsService::open(directory.path().to_path_buf()).unwrap();
  assert_eq!(linked(&service, "acme/monorepo"), vec![at(102, 2)]);
}

#[test]
fn a_repository_keeps_the_newest_links() {
  let directory = tempfile::tempdir().unwrap();
  let service = SettingsService::open(directory.path().to_path_buf()).unwrap();
  for number in 1..=u32::try_from(MAX_LINKS).unwrap() + 1 {
    link(&service, "acme/monorepo", at(number, number)).unwrap();
  }
  let links = linked(&service, "acme/monorepo");
  assert_eq!(links.len(), MAX_LINKS);
  assert_eq!(links.first(), Some(&at(51, 51)));
  assert!(!links.contains(&at(1, 1)));
}
