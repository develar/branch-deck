//! The command-line tools of the team that agents and the app use, `teamcity` and `youtrack`
//! (spec/repository/cli-tools.spec.md): where they are, their version, the servers they are signed in to, the agent
//! skill `teamcity` bundles, and the one action a row of Settings → Tools offers. No git and no remote service of the
//! app's own: only the CLIs, run in a `ToolEnvironment`.
//!
//! - `environment`: where the tools are looked for and how they run (the process's, or a directory of fakes)
//! - `install`: the install and update through Homebrew, the `teamcity` skill
//! - `parse`: their answers (`--version`, `auth status --json`, the skill)
//! - `setup`: the rule of the Setup dot: what of the app and the tools waits for the user
//! - `status`: a tool as Settings → Tools shows it, and its action

pub mod environment;
pub mod install;
pub mod parse;
pub mod setup;
pub mod status;

#[cfg(test)]
mod install_test;
#[cfg(test)]
mod parse_test;
#[cfg(test)]
mod setup_test;
#[cfg(test)]
mod status_test;
#[cfg(test)]
mod test_support;
