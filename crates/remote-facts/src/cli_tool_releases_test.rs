use crate::cli_tool_releases::{CHECK_INTERVAL, KnownReleases, read_releases, refresh_releases};
use domain_types::cli_tools::CliTool;
use github::fake_github::{FakeAnswer, start_fake_service};
use pretty_assertions::assert_eq;
use serde_json::json;
use test_log::test;

const TEAMCITY: &str = "/repos/JetBrains/teamcity-cli/releases/latest";
const YOUTRACK: &str = "/repos/JetBrains/youtrack-cli/releases/latest";
const NOW: u32 = 1_800_000_000;

#[test(tokio::test)]
async fn asks_github_once_a_day_and_keeps_the_answer_in_its_file() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("GET", TEAMCITY, FakeAnswer::ok(json!({"tag_name": "v1.7.0"})));
  fake.answer("GET", YOUTRACK, FakeAnswer::ok(json!({"tag_name": "v0.7.0"})));
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("cli-tool-releases.json");

  let releases = refresh_releases(&github, Some(&file), NOW).await;
  assert_eq!((releases.latest(CliTool::Teamcity), releases.latest(CliTool::Youtrack)), (Some("1.7.0"), Some("0.7.0")));
  assert_eq!(read_releases(&file), releases);

  // within the day: the file, no request
  fake.answer("GET", TEAMCITY, FakeAnswer::ok(json!({"tag_name": "v1.8.0"})));
  assert_eq!(refresh_releases(&github, Some(&file), NOW + 60).await.latest(CliTool::Teamcity), Some("1.7.0"));
  assert_eq!(fake.requests_to("GET", TEAMCITY).len(), 1);
  // a day later: asked again
  assert_eq!(refresh_releases(&github, Some(&file), NOW + CHECK_INTERVAL).await.latest(CliTool::Teamcity), Some("1.8.0"));
}

#[test(tokio::test)]
async fn a_failed_answer_keeps_the_last_release_and_asks_again_next_time() {
  let (fake, github) = start_fake_service("t").await;
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("cli-tool-releases.json");
  let kept = KnownReleases {
    checked_at: NOW - CHECK_INTERVAL,
    latest: [("teamcity".to_string(), "1.6.0".to_string())].into(),
  };
  std::fs::write(&file, serde_json::to_vec(&kept).unwrap()).unwrap();
  fake.answer("GET", TEAMCITY, FakeAnswer::status(500, json!({"message": "down"})));
  fake.answer("GET", YOUTRACK, FakeAnswer::status(500, json!({"message": "down"})));
  assert_eq!(refresh_releases(&github, Some(&file), NOW).await, kept);
  // nothing answered: the time of the check stays, so the next read asks again
  assert_eq!(read_releases(&file).checked_at, NOW - CHECK_INTERVAL);
}
