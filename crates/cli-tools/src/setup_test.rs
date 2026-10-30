use crate::setup::{EXPIRING_WITHIN, SetupFacts, setup_entries};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolServer, CliToolSignInState, CliToolSkill, CliToolStatus};
use domain_types::setup::{SetupEntry, SetupItem, SetupSkip};
use pretty_assertions::assert_eq;

const NOW: u32 = 1_800_000_000;
const DAY: u32 = 24 * 60 * 60;

fn tool(tool: CliTool, version: Option<&str>, latest: Option<&str>) -> CliToolStatus {
  CliToolStatus {
    tool,
    binary: version.map(|_| format!("/opt/homebrew/bin/{}", tool.command())),
    version: version.map(str::to_string),
    servers: Vec::new(),
    skill: None,
    latest: latest.map(str::to_string),
    manager: None,
    action: None,
    command: None,
    problem: None,
    sign_in_with: None,
  }
}

fn server(url: &str, expires_at: Option<u32>) -> CliToolServer {
  CliToolServer {
    url: url.to_string(),
    user: Some("jane".to_string()),
    state: CliToolSignInState::Authenticated,
    error: None,
    expires_at,
  }
}

fn items(app_update: Option<&str>, tools: &[CliToolStatus], youtrack_broken: bool, skips: &[SetupSkip]) -> Vec<SetupItem> {
  setup_entries(&SetupFacts {
    app_update,
    app_update_check_failure: None,
    tools,
    youtrack_broken,
    skips,
    now: NOW,
  })
  .into_iter()
  .map(|entry| entry.item)
  .collect()
}

fn app_update_entries(app_update: Option<&str>, check_failure: Option<&str>, tools: &[CliToolStatus], skips: &[SetupSkip], now: u32) -> Vec<SetupEntry> {
  setup_entries(&SetupFacts {
    app_update,
    app_update_check_failure: check_failure,
    tools,
    youtrack_broken: false,
    skips,
    now,
  })
}

#[test]
fn the_app_update_then_each_tool_then_the_youtrack_sign_in() {
  let mut teamcity = tool(CliTool::Teamcity, Some("1.6.0"), Some("1.7.0"));
  teamcity.servers = vec![
    server("https://ci.example", Some(NOW + 3 * DAY)),
    server("https://old.example", Some(NOW - DAY)),
    server("https://far.example", Some(NOW + 30 * DAY)),
  ];
  let youtrack = tool(CliTool::Youtrack, Some("0.7.0"), Some("0.7.0"));
  assert_eq!(
    items(Some("1.2.0"), &[teamcity, youtrack], true, &[]),
    vec![
      SetupItem::AppUpdate { version: "1.2.0".to_string() },
      SetupItem::ToolUpdate {
        tool: CliTool::Teamcity,
        version: "1.7.0".to_string()
      },
      SetupItem::SignInExpiring {
        tool: CliTool::Teamcity,
        server: "https://ci.example".to_string(),
        expires_at: NOW + 3 * DAY
      },
      SetupItem::SignInExpired {
        tool: CliTool::Teamcity,
        server: "https://old.example".to_string()
      },
      SetupItem::YouTrackSignIn,
    ]
  );
  assert_eq!(EXPIRING_WITHIN, 7 * DAY);
}

#[test]
fn a_failed_app_update_check_is_the_app_item_unless_an_update_waits() {
  let teamcity = tool(CliTool::Teamcity, Some("1.6.0"), Some("1.7.0"));
  let tools = [teamcity];
  let failed = app_update_entries(None, Some("Network unavailable"), &tools, &[], NOW);
  assert_eq!(
    failed.iter().map(|entry| entry.item.clone()).collect::<Vec<_>>(),
    vec![
      SetupItem::AppUpdateCheckFailed {
        message: "Network unavailable".to_string()
      },
      SetupItem::ToolUpdate {
        tool: CliTool::Teamcity,
        version: "1.7.0".to_string(),
      },
    ]
  );
  assert_eq!(failed[0].key, "app-update-check-failed");
  assert!(failed[0].skippable);

  let with_update = app_update_entries(Some("1.2.0"), Some("Network unavailable"), &tools, &[], NOW);
  assert_eq!(with_update[0].item, SetupItem::AppUpdate { version: "1.2.0".to_string() });
  assert_eq!(with_update[1].item, failed[1].item);
}

#[test]
fn a_failed_app_update_check_can_be_reminded_tomorrow() {
  let skips = [SetupSkip {
    key: "app-update-check-failed".to_string(),
    until: Some(NOW + DAY),
  }];
  let hidden = app_update_entries(None, Some("Network unavailable"), &[], &skips, NOW);
  assert_eq!(hidden, [] as [SetupEntry; 0]);
  assert_eq!(app_update_entries(None, Some("Network unavailable"), &[], &skips, NOW + DAY).len(), 1);
}

#[test]
fn a_tool_not_installed_and_a_skill_never_installed_are_no_items() {
  let missing = tool(CliTool::Youtrack, None, Some("0.7.0"));
  let mut teamcity = tool(CliTool::Teamcity, Some("1.6.0"), Some("1.6.0"));
  teamcity.skill = Some(CliToolSkill {
    name: "teamcity-cli".to_string(),
    installed: None,
    bundled: Some("1.4.0".to_string()),
    action: Some(CliToolAction::InstallSkill),
  });
  assert_eq!(items(None, &[missing, teamcity.clone()], false, &[]), vec![]);

  teamcity.skill = Some(CliToolSkill {
    name: "teamcity-cli".to_string(),
    installed: Some("1.0.0".to_string()),
    bundled: Some("1.4.0".to_string()),
    action: Some(CliToolAction::UpdateSkill),
  });
  assert_eq!(
    items(None, &[teamcity], false, &[]),
    vec![SetupItem::SkillUpdate {
      tool: CliTool::Teamcity,
      version: "1.4.0".to_string()
    }]
  );
}

#[test]
fn a_skip_hides_an_update_until_another_version_and_a_reminder_until_its_time() {
  let mut teamcity = tool(CliTool::Teamcity, Some("1.6.0"), Some("1.7.0"));
  teamcity.servers = vec![server("https://ci.example", Some(NOW + DAY)), server("https://old.example", Some(NOW - DAY))];
  let skips = [
    SetupSkip {
      key: "tool-update:teamcity:1.7.0".to_string(),
      until: None,
    },
    SetupSkip {
      key: "sign-in-expiring:teamcity:https://ci.example".to_string(),
      until: Some(NOW + 60),
    },
    // an expired sign-in can't be skipped
    SetupSkip {
      key: "sign-in-expired:teamcity:https://old.example".to_string(),
      until: None,
    },
  ];
  assert_eq!(
    items(None, std::slice::from_ref(&teamcity), false, &skips),
    vec![SetupItem::SignInExpired {
      tool: CliTool::Teamcity,
      server: "https://old.example".to_string()
    }]
  );
  // a newer release than the one skipped, and the reminder's time is over
  teamcity.latest = Some("1.8.0".to_string());
  let later = setup_entries(&SetupFacts {
    app_update: None,
    app_update_check_failure: None,
    tools: std::slice::from_ref(&teamcity),
    youtrack_broken: false,
    skips: &skips,
    now: NOW + 120,
  });
  let keys: Vec<_> = later.iter().map(|entry| (entry.key.as_str(), entry.skippable)).collect();
  assert_eq!(
    keys,
    vec![
      ("tool-update:teamcity:1.8.0", true),
      ("sign-in-expiring:teamcity:https://ci.example", true),
      ("sign-in-expired:teamcity:https://old.example", false)
    ]
  );
}
