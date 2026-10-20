//! Issues as the popover shows them, the user of a token, and the command that changes an issue's state. A field is
//! told by its type (`$type`), never by its name: the names are the project's.

use crate::client::{YouTrackClient, YouTrackError};
use crate::url::{api_url, issue_page};
use serde::Deserialize;
use serde_json::{Value, json};
use sync_types::youtrack::{YouTrackField, YouTrackIssue, YouTrackState, YouTrackUser};
use tracing::instrument;

/// The value Mark Fixed sets: the resolved state of YouTrack's default workflow
pub const FIXED_STATE: &str = "Fixed";

const ISSUE_FIELDS: &str = "idReadable,summary,resolved,customFields($type,id,name,value($type,name,login,fullName,isResolved,presentation))";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserAnswer {
  login: String,
  full_name: Option<String>,
}

/// The user of the token (`GET /api/users/me`)
#[instrument(skip(client, token))]
pub async fn fetch_me(client: &YouTrackClient, base: &str, token: &str) -> Result<YouTrackUser, YouTrackError> {
  let url = api_url(base, &["users", "me"], "login,fullName").map_err(YouTrackError::InvalidResponse)?;
  let user: UserAnswer = client.get(&url, Some(token)).await?;
  Ok(YouTrackUser {
    full_name: user.full_name.filter(|name| !name.is_empty()).unwrap_or_else(|| user.login.clone()),
    login: user.login,
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueAnswer {
  id_readable: String,
  #[serde(default)]
  summary: Option<String>,
  /// Unix milliseconds
  resolved: Option<u64>,
  #[serde(default)]
  custom_fields: Vec<FieldAnswer>,
}

#[derive(Deserialize)]
struct FieldAnswer {
  #[serde(rename = "$type")]
  kind: String,
  id: Option<String>,
  name: String,
  #[serde(default)]
  value: Value,
}

/// The issue: its summary, its state field and its other single-value fields. For an open issue with a state field,
/// one more request tells whether that field has the value `Fixed` (the bundle of a project is too large to ask for
/// with every issue: versions, users).
#[instrument(skip(client, token))]
pub async fn fetch_issue(client: &YouTrackClient, base: &str, token: &str, key: &str) -> Result<YouTrackIssue, YouTrackError> {
  let url = api_url(base, &["issues", key], ISSUE_FIELDS).map_err(YouTrackError::InvalidResponse)?;
  let answer: IssueAnswer = client.get(&url, Some(token)).await?;
  let (mut issue, state_field_id) = issue_of(base, answer);
  if let (Some(state), Some(field_id)) = (issue.state.as_mut(), state_field_id)
    && !state.resolved
  {
    state.has_fixed = has_value(client, base, token, key, &field_id, FIXED_STATE).await?;
  }
  Ok(issue)
}

/// The issue of an answer, and the id of its state field
fn issue_of(base: &str, answer: IssueAnswer) -> (YouTrackIssue, Option<String>) {
  let mut state = None;
  let mut state_field_id = None;
  let mut fields = Vec::new();
  for field in answer.custom_fields {
    match field.kind.as_str() {
      "StateIssueCustomField" | "StateMachineIssueCustomField" if state.is_none() => {
        state_field_id = field.id;
        state = Some(YouTrackState {
          value: name_of(&field.value),
          resolved: field.value.get("isResolved").and_then(Value::as_bool).unwrap_or(false),
          field: field.name,
          has_fixed: false,
        });
      }
      "SingleEnumIssueCustomField" | "SingleOwnedIssueCustomField" | "SingleVersionIssueCustomField" | "SingleBuildIssueCustomField" | "SingleUserIssueCustomField" => {
        if let Some(value) = name_of(&field.value) {
          fields.push(YouTrackField { name: field.name, value });
        }
      }
      // several values, dates, texts, numbers: not in the popover
      _ => {}
    }
  }
  let issue = YouTrackIssue {
    url: issue_page(base, &answer.id_readable),
    key: answer.id_readable,
    summary: answer.summary.unwrap_or_default(),
    resolved: answer.resolved.map(|millis| u32::try_from(millis / 1000).unwrap_or(u32::MAX)),
    state,
    fields,
  };
  (issue, state_field_id)
}

/// The shown name of a value: a user by full name, a bundle element by name
fn name_of(value: &Value) -> Option<String> {
  ["fullName", "name", "presentation", "login"]
    .iter()
    .find_map(|key| value.get(key).and_then(Value::as_str).filter(|name| !name.is_empty()))
    .map(str::to_string)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FieldBundleAnswer {
  project_custom_field: Option<ProjectFieldAnswer>,
}

#[derive(Deserialize)]
struct ProjectFieldAnswer {
  bundle: Option<BundleAnswer>,
}

#[derive(Deserialize)]
struct BundleAnswer {
  #[serde(default)]
  values: Vec<BundleValue>,
}

#[derive(Deserialize)]
struct BundleValue {
  name: String,
}

/// The field of the issue has a value named `value` in its project
async fn has_value(client: &YouTrackClient, base: &str, token: &str, key: &str, field_id: &str, value: &str) -> Result<bool, YouTrackError> {
  let url = api_url(base, &["issues", key, "customFields", field_id], "projectCustomField(bundle(values(name)))").map_err(YouTrackError::InvalidResponse)?;
  let answer: FieldBundleAnswer = client.get(&url, Some(token)).await?;
  Ok(
    answer
      .project_custom_field
      .and_then(|field| field.bundle)
      .is_some_and(|bundle| bundle.values.iter().any(|it| it.name == value)),
  )
}

/// `State In progress`: a command names the field, then the value, spaces and all. YouTrack takes no braces around
/// either (`State {Fixed}` is "State expected"; checked with `/api/commands/assist`, 2026-10-20).
pub fn set_field_command(field: &str, value: &str) -> String {
  format!("{field} {value}")
}

/// Applies a YouTrack command to the issue (`POST /api/commands`), as the user of the token
#[instrument(skip(client, token))]
pub async fn apply_command(client: &YouTrackClient, base: &str, token: &str, key: &str, query: &str) -> Result<(), YouTrackError> {
  let url = api_url(base, &["commands"], "id").map_err(YouTrackError::InvalidResponse)?;
  let _: Value = client.post(&url, token, &json!({ "query": query, "issues": [{ "idReadable": key }] })).await?;
  Ok(())
}
