//! The sign-in of the `youtrack` CLI with a permanent token the app made from its own YouTrack sign-in
//! (spec/repository/cli-tools.spec.md#the-youtrack-cli): `auth login --no-browser` reads the token from its stdin,
//! checks it and keeps it in the CLI's own keychain item. The token never is an argument or a line of a log; the
//! credential variables of the app's environment are not passed on; the instance the CLI had active stays active.

use crate::environment::ToolEnvironment;
use crate::parse::{YoutrackInstance, parse_youtrack_instances};
use domain_types::cli_tools::CliTool;
use tracing::{info, instrument};

/// Signs the `youtrack` CLI in to `base_url` with `token`; the login it is signed in as
#[instrument(skip(environment, token))]
pub fn sign_in_youtrack_with_token(environment: &ToolEnvironment, base_url: &str, token: &str) -> Result<String, String> {
  let binary = environment.find_tool(CliTool::Youtrack).ok_or_else(|| "youtrack is not installed".to_string())?;
  let base_url = base_url.trim_end_matches('/');
  let before = instances(environment, &binary)?;
  let login = environment.run(
    &binary,
    &["auth", "login", "--base-url", base_url, "--no-browser", "--json", "--quiet"],
    Some(format!("{token}\n").into_bytes()),
  )?;
  if !login.status.success() {
    return Err(login.failure_message("youtrack"));
  }
  let after = instances(environment, &binary)?;
  // the sign-in makes its instance the active one: the one the user had stays it
  let was_active = before
    .iter()
    .find(|instance| instance.active && instance.url != base_url)
    .and_then(|instance| instance.alias.clone());
  let now_active = after.iter().find(|instance| instance.active).and_then(|instance| instance.alias.clone());
  if let Some(alias) = was_active
    && now_active.as_deref() != Some(alias.as_str())
  {
    environment.run_ok(&binary, &["instance", "use", &alias, "--quiet"])?;
  }
  let signed_in = after.iter().find(|instance| instance.url == base_url && instance.authenticated);
  let login = signed_in
    .and_then(|instance| instance.login.clone())
    .ok_or_else(|| format!("youtrack took the token, but is not signed in to {base_url}"))?;
  info!(%login, "the youtrack CLI is signed in");
  Ok(login)
}

fn instances(environment: &ToolEnvironment, binary: &std::path::Path) -> Result<Vec<YoutrackInstance>, String> {
  let output = environment.run_ok(binary, &["auth", "status", "--json", "--quiet"])?;
  parse_youtrack_instances(&String::from_utf8_lossy(&output.stdout))
}
