//! The newest release of each CLI of Settings → Tools, from GitHub (`releases/latest` of its repository), asked with the
//! user's token (the token chain) and conditional on the last answer, at most once a day: an office behind one address
//! shares the 60 anonymous requests an hour, so the CLIs' own update checks are not asked. Kept in a file of the app
//! data, so a restart asks nothing; offline or signed out, the last answer stays.

use domain_types::cli_tools::CliTool;
use github::service::{GitHubService, RequestKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::{info, instrument, warn};

/// How long an answer holds
pub const CHECK_INTERVAL: u32 = 24 * 60 * 60;

/// The GitHub repository of a tool's releases
pub const fn release_repository(tool: CliTool) -> (&'static str, &'static str) {
  match tool {
    CliTool::Teamcity => ("JetBrains", "teamcity-cli"),
    CliTool::Youtrack => ("JetBrains", "youtrack-cli"),
  }
}

/// What the file keeps
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownReleases {
  /// Unix seconds of the last check that answered
  pub checked_at: u32,
  /// By the tool's command: `teamcity` → `1.7.0`
  pub latest: BTreeMap<String, String>,
}

impl KnownReleases {
  pub fn latest(&self, tool: CliTool) -> Option<&str> {
    self.latest.get(tool.command()).map(String::as_str)
  }
}

#[derive(Deserialize)]
struct ReleaseDto {
  tag_name: String,
}

pub fn releases_file(app_data_dir: &Path) -> PathBuf {
  app_data_dir.join("cli-tool-releases.json")
}

/// The releases the file keeps; none without one
pub fn read_releases(file: &Path) -> KnownReleases {
  std::fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

/// The releases, asked again when the last check is older than a day, every tool at once: every tool's answer, or what
/// the file keeps for a tool GitHub didn't answer
#[instrument(skip(service, file))]
pub async fn refresh_releases(service: &GitHubService, file: Option<&Path>, now: u32) -> KnownReleases {
  let known = file.map_or_else(KnownReleases::default, read_releases);
  if now.saturating_sub(known.checked_at) < CHECK_INTERVAL {
    return known;
  }
  let release_of = |tool: CliTool| async move {
    let (owner, name) = release_repository(tool);
    let repository = format!("{owner}/{name}");
    let release = service
      .ask("", &repository, RequestKind::Single, |client, token| async move {
        client.get::<ReleaseDto>(&token, &["repos", owner, name, "releases", "latest"], &[]).await
      })
      .await;
    (tool, release)
  };
  // every tool at once
  let [teamcity, youtrack] = CliTool::ALL.map(release_of);
  let answers: [_; 2] = tokio::join!(teamcity, youtrack).into();
  let mut latest = known.latest.clone();
  let mut answered = false;
  for (tool, release) in answers {
    match release {
      Ok(release) => {
        let version = release.tag_name.trim_start_matches('v').to_string();
        info!(tool = tool.command(), %version, "the newest release");
        latest.insert(tool.command().to_string(), version);
        answered = true;
      }
      Err(failure) => warn!(tool = tool.command(), ?failure, "can't read the newest release"),
    }
  }
  if !answered {
    return known;
  }
  let releases = KnownReleases { checked_at: now, latest };
  if let Some(file) = file {
    let written = serde_json::to_vec(&releases).map_err(std::io::Error::other).and_then(|bytes| std::fs::write(file, bytes));
    if let Err(e) = written {
      warn!(error = %e, "can't keep the releases");
    }
  }
  releases
}
