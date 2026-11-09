//! The command-line tools of the team that agents and the app use, `teamcity` and `youtrack`
//! (openspec/specs/repository/cli-tools/spec.md): where they are, their version, the servers they are signed in to, the agent
//! skill `teamcity` bundles, and the one action a row of Settings → Tools offers. No git and no remote service of the
//! app's own: only the CLIs, run in a `ToolEnvironment`.
//!
//! - `environment`: where the tools are looked for and how they run (the process's, or a directory of fakes)
//! - `install`: the install and update through Homebrew or the CLI's own `update`, the sign-in, the skills
//! - `kept_statuses`: the statuses of the read that started last, which the Setup dot takes
//! - `package_manager`: Homebrew, winget and scoop: which installs a tool here, which installed it, their commands
//! - `parse`: their answers (`--version`, `auth status --json`, the skill, the JSON events and errors)
//! - `setup`: the rule of the Setup dot: what of the app and the tools waits for the user
//! - `youtrack_login`: the sign-in of `youtrack` with a permanent token of the app's YouTrack sign-in
//! - `status`: a tool as Settings → Tools shows it, and its action
//! - `teamcity_login`: the browser sign-in of `teamcity` without a terminal (`auth login --web --json`)

pub mod environment;
pub mod install;
pub mod kept_statuses;
pub mod package_manager;
pub mod parse;
pub mod setup;
pub mod status;
pub mod teamcity_login;
pub mod youtrack_login;

#[cfg(test)]
mod install_test;
#[cfg(test)]
mod kept_statuses_test;
#[cfg(test)]
mod package_manager_test;
#[cfg(test)]
mod parse_test;
#[cfg(test)]
mod setup_test;
#[cfg(test)]
mod status_test;
#[cfg(test)]
mod teamcity_login_test;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod youtrack_login_test;
