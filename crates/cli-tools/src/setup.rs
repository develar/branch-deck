//! The rule of the Setup dot (spec/repository/setup-attention.spec.md): what in the setup of the machine waits for the
//! user, from facts the caller brings. Only what has a direct action and the app relies on: an update that is ready, a
//! failed app update check when none is downloaded, a newer CLI or skill, a sign-in of a CLI that expires within a week
//! or expired, the app's YouTrack sign-in that stopped. A tool that is not installed is never an item: the user may not want it.

use crate::parse::compare_versions;
use domain_types::cli_tools::{CliToolSignInState, CliToolStatus};
use domain_types::setup::{SetupEntry, SetupItem, SetupSkip};
use std::cmp::Ordering;

/// A sign-in that expires within this many seconds asks to be renewed
pub const EXPIRING_WITHIN: u32 = 7 * 24 * 60 * 60;

pub struct SetupFacts<'a> {
  /// The version of a downloaded update of the app
  pub app_update: Option<&'a str>,
  /// The error of the last app update check, when no downloaded update waits
  pub app_update_check_failure: Option<&'a str>,
  pub tools: &'a [CliToolStatus],
  /// The app's YouTrack sign-in was refused or expired
  pub youtrack_broken: bool,
  pub skips: &'a [SetupSkip],
  /// Unix seconds
  pub now: u32,
}

/// The items in the order of the menu: the app first, then each tool, the YouTrack sign-in last
pub fn setup_entries(facts: &SetupFacts<'_>) -> Vec<SetupEntry> {
  let mut items = Vec::new();
  if let Some(version) = facts.app_update {
    items.push(SetupItem::AppUpdate { version: version.to_string() });
  } else if let Some(message) = facts.app_update_check_failure {
    items.push(SetupItem::AppUpdateCheckFailed { message: message.to_string() });
  }
  for status in facts.tools.iter().filter(|status| status.binary.is_some()) {
    tool_items(status, facts.now, &mut items);
  }
  if facts.youtrack_broken {
    items.push(SetupItem::YouTrackSignIn);
  }
  items
    .into_iter()
    .filter(|item| !skipped(item, facts.skips, facts.now))
    .map(|item| SetupEntry {
      key: item.key(),
      skippable: item.skippable(),
      item,
    })
    .collect()
}

fn tool_items(status: &CliToolStatus, now: u32, items: &mut Vec<SetupItem>) {
  let tool = status.tool;
  let newer = status
    .latest
    .as_deref()
    .filter(|latest| status.version.as_deref().is_some_and(|version| compare_versions(latest, version) == Ordering::Greater));
  if let Some(latest) = newer {
    items.push(SetupItem::ToolUpdate {
      tool,
      version: latest.to_string(),
    });
  } else if let Some(skill) = &status.skill
    && let (Some(installed), Some(bundled)) = (&skill.installed, &skill.bundled)
    && compare_versions(bundled, installed) == Ordering::Greater
  {
    // a skill never installed is the user's choice; an old one misleads the agents
    items.push(SetupItem::SkillUpdate { tool, version: bundled.clone() });
  }
  for server in &status.servers {
    let Some(expires_at) = server.expires_at else { continue };
    if expires_at <= now {
      items.push(SetupItem::SignInExpired { tool, server: server.url.clone() });
    } else if server.state == CliToolSignInState::Authenticated && expires_at - now <= EXPIRING_WITHIN {
      items.push(SetupItem::SignInExpiring {
        tool,
        server: server.url.clone(),
        expires_at,
      });
    }
  }
}

fn skipped(item: &SetupItem, skips: &[SetupSkip], now: u32) -> bool {
  item.skippable() && {
    let key = item.key();
    skips.iter().any(|skip| skip.key == key && skip.until.is_none_or(|until| now < until))
  }
}
