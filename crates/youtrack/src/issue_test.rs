use crate::client::{YouTrackClient, YouTrackError};
use crate::issue::{apply_command, fetch_issue, fetch_me, set_field_command};
use domain_types::youtrack::{YouTrackField, YouTrackIssue, YouTrackState, YouTrackUser};
use github::fake_github::{FakeAnswer, start_fake_server};
use pretty_assertions::assert_eq;
use serde_json::json;

fn client() -> YouTrackClient {
  YouTrackClient::new("Branch Deck test")
}

/// An issue as YouTrack answers it (the shape of a real answer of 2026-10, cut down)
pub(crate) fn open_issue() -> serde_json::Value {
  json!({
    "idReadable": "ABC-1",
    "summary": "The file chooser cannot load a remote file",
    "resolved": null,
    "customFields": [
      {"$type": "SingleEnumIssueCustomField", "id": "92-1", "name": "Priority", "value": {"$type": "EnumBundleElement", "name": "Major"}},
      {"$type": "StateIssueCustomField", "id": "92-2", "name": "State", "value": {"$type": "StateBundleElement", "name": "Open", "isResolved": false}},
      {"$type": "MultiOwnedIssueCustomField", "id": "92-3", "name": "Subsystem", "value": [{"$type": "OwnedBundleElement", "name": "Core"}]},
      {"$type": "SingleUserIssueCustomField", "id": "92-4", "name": "Assignee", "value": {"$type": "User", "login": "jane", "fullName": "Jane Doe", "name": "Jane Doe"}},
      {"$type": "SingleVersionIssueCustomField", "id": "92-5", "name": "Latest Affected", "value": null},
      {"$type": "DateIssueCustomField", "id": "92-6", "name": "Due Date", "value": 1_792_409_230_772_u64}
    ]
  })
}

/// The values of the state field: `Fixed` among them
pub(crate) fn state_values() -> serde_json::Value {
  json!({"projectCustomField": {"bundle": {"values": [{"name": "Open"}, {"name": "Fixed"}, {"name": "Won't fix"}]}}})
}

/// A read that timed out is not asked again: it already waited all the time a read has (the clock of the test runs ahead
/// while the answer is held)
#[tokio::test(start_paused = true)]
async fn an_issue_read_that_timed_out_is_not_asked_again() {
  let (fake, url) = start_fake_server().await;
  fake.answer("GET", "/api/issues/ABC-1", FakeAnswer::ok(open_issue()));
  let held = fake.hold("GET", "/api/issues/ABC-1");
  let error = fetch_issue(&client(), &url, "perm-secret", "ABC-1").await.unwrap_err();
  assert!(matches!(error, YouTrackError::Timeout), "{error:?}");
  assert_eq!(held.arrivals(), 1);
}

#[tokio::test]
async fn an_issue_shows_its_state_and_its_single_value_fields_by_type() {
  let (fake, url) = start_fake_server().await;
  fake.answer("GET", "/api/issues/ABC-1", FakeAnswer::ok(open_issue()));
  fake.answer("GET", "/api/issues/ABC-1/customFields/92-2", FakeAnswer::ok(state_values()));
  let issue = fetch_issue(&client(), &url, "perm-secret", "ABC-1").await.unwrap();
  assert_eq!(
    issue,
    YouTrackIssue {
      key: "ABC-1".to_string(),
      summary: "The file chooser cannot load a remote file".to_string(),
      url: format!("{url}/issue/ABC-1"),
      resolved: None,
      state: Some(YouTrackState {
        field: "State".to_string(),
        value: Some("Open".to_string()),
        resolved: false,
        has_fixed: true,
      }),
      fields: vec![
        YouTrackField {
          name: "Priority".to_string(),
          value: "Major".to_string()
        },
        YouTrackField {
          name: "Assignee".to_string(),
          value: "Jane Doe".to_string()
        },
      ],
    }
  );
  let request = &fake.requests_to("GET", "/api/issues/ABC-1")[0];
  assert_eq!(request.authorization.as_deref(), Some("Bearer perm-secret"));
  assert!(request.query.as_deref().unwrap().starts_with("fields=idReadable"));
}

#[tokio::test]
async fn a_resolved_issue_does_not_ask_for_the_values_of_its_state() {
  let (fake, url) = start_fake_server().await;
  let mut answer = open_issue();
  answer["resolved"] = json!(1_792_409_230_772_u64);
  answer["customFields"][1]["value"] = json!({"name": "Fixed", "isResolved": true});
  fake.answer("GET", "/api/issues/ABC-1", FakeAnswer::ok(answer));
  let issue = fetch_issue(&client(), &url, "perm-secret", "ABC-1").await.unwrap();
  assert_eq!(issue.resolved, Some(1_792_409_230));
  assert_eq!(issue.state.map(|state| (state.resolved, state.has_fixed)), Some((true, false)));
  assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_command_names_the_field_and_the_value_without_braces() {
  assert_eq!(set_field_command("State", "In Progress"), "State In Progress");
  let (fake, url) = start_fake_server().await;
  fake.answer("POST", "/api/commands", FakeAnswer::ok(json!({"id": "1"})));
  apply_command(&client(), &url, "perm-secret", "ABC-1", &set_field_command("Stage", "Won't fix"))
    .await
    .unwrap();
  let request = &fake.requests_to("POST", "/api/commands")[0];
  assert_eq!(request.json(), json!({"query": "Stage Won't fix", "issues": [{"idReadable": "ABC-1"}]}));

  fake.answer(
    "POST",
    "/api/commands",
    FakeAnswer::status(400, json!({"error": "bad_request", "error_description": "Unknown command: Fixed"})),
  );
  let error = apply_command(&client(), &url, "perm-secret", "ABC-1", "State Fixed").await.unwrap_err();
  assert_eq!(error.message(), "Unknown command: Fixed");
}

#[tokio::test]
async fn a_redirect_is_not_followed() {
  let (fake, url) = start_fake_server().await;
  fake.answer("GET", "/api/users/me", FakeAnswer::status(302, serde_json::Value::Null).header("location", "/elsewhere"));
  let error = fetch_me(&client(), &url, "perm-secret").await.unwrap_err();
  assert!(matches!(error, YouTrackError::Http { status: 302, .. }), "{error:?}");
  assert_eq!(fake.requests().len(), 1);

  fake.answer("GET", "/api/users/me", FakeAnswer::ok(json!({"login": "jane", "fullName": ""})));
  assert_eq!(
    fetch_me(&client(), &url, "perm-secret").await.unwrap(),
    YouTrackUser {
      login: "jane".to_string(),
      full_name: "jane".to_string()
    }
  );
}
