use crate::parse::{compare_versions, parse_skill_list, parse_skill_version, parse_teamcity_auth, parse_version, parse_youtrack_auth};
use domain_types::cli_tools::{CliToolServer, CliToolSignInState};
use pretty_assertions::assert_eq;
use std::cmp::Ordering;

#[test]
fn reads_the_version_of_each_cli() {
  assert_eq!(parse_version("teamcity version 1.6.0\n").as_deref(), Some("1.6.0"));
  assert_eq!(parse_version("youtrack version v0.7.0 (abc123)").as_deref(), Some("0.7.0"));
  assert_eq!(parse_version("0.7.0").as_deref(), Some("0.7.0"));
  assert_eq!(parse_version("youtrack dev"), None);
}

#[test]
fn orders_versions_by_their_numbers() {
  assert_eq!(compare_versions("1.10.0", "1.9.2"), Ordering::Greater);
  assert_eq!(compare_versions("1.6", "1.6.0"), Ordering::Equal);
  assert_eq!(compare_versions("0.6.0", "0.7.0"), Ordering::Less);
  assert_eq!(compare_versions("1.7.0-rc1", "1.7.0"), Ordering::Equal);
}

#[test]
fn reads_the_servers_of_teamcity_with_their_user_and_expiry() {
  let stdout = r#"[
    {"server":"https://buildserver.labs.intellij.net/","auth_method":"token","token_source":"keyring",
     "user":{"id":0,"username":"vladimir.krivosheev","name":"Vladimir Krivosheev"},
     "token_expiry":"2026-11-21T16:58:19.041Z","status":"authenticated"},
    {"server":"https://ci.example","status":"error","error":"token missing or could not be retrieved"},
    {"server":"https://guest.example","status":"guest"}
  ]"#;
  assert_eq!(
    parse_teamcity_auth(stdout).unwrap(),
    vec![
      CliToolServer {
        url: "https://buildserver.labs.intellij.net".to_string(),
        user: Some("vladimir.krivosheev".to_string()),
        state: CliToolSignInState::Authenticated,
        error: None,
        expires_at: Some(1_795_280_299),
      },
      CliToolServer {
        url: "https://ci.example".to_string(),
        user: None,
        state: CliToolSignInState::Failed,
        error: Some("token missing or could not be retrieved".to_string()),
        expires_at: None,
      },
      CliToolServer {
        url: "https://guest.example".to_string(),
        user: None,
        state: CliToolSignInState::Guest,
        error: None,
        expires_at: None,
      },
    ]
  );
  // signed in to no server: one entry without a server
  assert_eq!(
    parse_teamcity_auth(r#"[{"server":"","status":"error","error":"not logged in to any TeamCity server"}]"#).unwrap(),
    vec![]
  );
  assert!(parse_teamcity_auth("Error: unknown flag").unwrap_err().contains("teamcity auth status"));
}

#[test]
fn reads_the_instances_of_youtrack_as_one_object_or_an_array() {
  let one = r#"{"url":"https://youtrack.jetbrains.com","login":"develar","fullName":"Vladimir","status":"authenticated"}"#;
  assert_eq!(
    parse_youtrack_auth(one).unwrap(),
    vec![CliToolServer {
      url: "https://youtrack.jetbrains.com".to_string(),
      user: Some("develar".to_string()),
      state: CliToolSignInState::Authenticated,
      error: None,
      expires_at: None,
    }]
  );
  let many = r#"[{"alias":"work","url":"https://a.example/","status":"invalid token","active":true},{"alias":"x","url":"https://b.example","status":"not configured"}]"#;
  assert_eq!(
    parse_youtrack_auth(many).unwrap(),
    vec![CliToolServer {
      url: "https://a.example".to_string(),
      user: None,
      state: CliToolSignInState::Failed,
      error: Some("invalid token".to_string()),
      expires_at: None,
    }]
  );
  assert_eq!(parse_youtrack_auth(r#"{"url":"","status":"not configured"}"#).unwrap(), vec![]);
}

#[test]
fn reads_the_version_of_a_skill_and_of_the_skill_list() {
  let skill = "---\nname: teamcity-cli\nversion: 1.0.0\ndescription: Use when\n---\n\n# TeamCity CLI\nversion: 9\n";
  assert_eq!(parse_skill_version(skill).as_deref(), Some("1.0.0"));
  assert_eq!(parse_skill_version("# no front matter\nversion: 1"), None);
  let list =
    "Name                 Version  Description\nteamcity-cli         1.4.0    Use when working with TeamCity (default)\nmigrate-to-teamcity  0.3.1    Use when migrating\n";
  assert_eq!(parse_skill_list(list, "teamcity-cli").as_deref(), Some("1.4.0"));
  assert_eq!(parse_skill_list(list, "other"), None);
}
