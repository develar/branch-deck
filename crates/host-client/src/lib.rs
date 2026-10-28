//! The client of the remote runs (spec/cli/host.spec.md, "run" to "upgrade"; docs/design/host-runs.md): a run of a
//! checkout on a host, its commit of the working tree, its thin pack, its follow, its fetch and the sync of its base,
//! the host files, `setup <alias>` over ssh and the upgrade over the link. Its git runs in process through
//! `git_ops::host_client`, `git_ops::host_objects` and `git_ops::worktree_snapshot`: no `bd` child.
//!
//! - `config`: the identity of the client, one host file per alias, and the last base of each affinity, under
//!   `~/.config/bd-host/`
//! - `host`: the address choice and the hello of each connection to an agent
//! - `git`: the git of the checkout of a run: the snapshot, the thin pack, `[host]` of the commit and the apply
//! - `run`: a run, its follow, the requests of one reply, and the end of a run as `RunOutcome` or `RunFailure`
//! - `pack`, `base`: the thin pack of a run, and the base of the later packs
//! - `fetch`, `record`: the files that come back, and the record of a run that says which ones may come
//! - `phases`: the steps of a run for a person
//! - `report`: the report of a run and the hello as JSON, for `--report-fd` and `hello --json`
//! - `setup`: makes a host ready over ssh; `process` is the seam of its children
//! - `upgrade`: sends this binary to the agent of a host over the link
//! - `interrupt`: Ctrl+C of a run, counted
//!
//! Each failure is a `host_link::refusal::Refusal` with a code of the table of the spec. The exit of a run follows how
//! far it came (`RunFailure`); the exit of another request is `request_exit`.

mod base;
pub mod config;
mod fetch;
mod git;
pub mod host;
pub mod interrupt;
mod pack;
pub mod phases;
pub mod process;
mod record;
pub mod report;
pub mod run;
pub mod setup;
pub mod upgrade;

#[cfg(test)]
mod testkit;

#[cfg(test)]
mod lib_test;

use config::Config;
use git_executor::git_command_executor::GitCommandExecutor;
use host::Host;
use host_link::proto::{HEARTBEAT_SECONDS, UPGRADE_DRAIN_SECONDS, UPGRADE_WATCHDOG_SECONDS};
use host_link::refusal::{Code, Exit, Refusal};
use process::Processes;
use std::path::Path;
use std::time::Duration;

/// The exit of `bd-host` for a refusal of a request that is no run: `follow`, `cancel`, `fetch`, `forget`, `runs`,
/// `hello`, `slots`, `setup` and `upgrade` exit 2 for `usage`, 130 for `interrupted`, else 255
pub const fn request_exit(refusal: &Refusal) -> Exit {
  match refusal.code {
    Code::Usage => Exit::Usage,
    Code::Interrupted => Exit::Interrupted,
    _ => Exit::Refused,
  }
}

/// The words that name how a host gets its agent and its host file
pub(crate) fn setup_hint(alias: &str) -> String {
  format!("`bd-host setup {alias}` installs the agent and writes the host file")
}

/// The build of this client: what its hello sends, and what `hello --json` and `upgrade` compare
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
  /// The version of Branch Deck
  pub version: String,
  /// The SHA-256 of the binary, its build id; `None` when the binary cannot be read
  pub sha256: Option<String>,
}

/// The deadlines of the client
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
  /// The TCP connect and the handshake of one address
  pub(crate) connect: Duration,
  /// The lookup of the host name
  pub(crate) resolve: Duration,
  /// The wait for each frame of a reply, and the deadline of each write. A request that waits gets a heartbeat every
  /// 15 s. A host that clones a new slot can hold back its heartbeats for a minute, so only two quiet minutes are a
  /// lost link
  pub(crate) read: Duration,
  /// How long `Follow` connects again after the last reply of the agent
  pub(crate) reconnect_for: Duration,
  /// The first wait before a new connection of `Follow`. It doubles up to `max_backoff`
  pub(crate) first_backoff: Duration,
  pub(crate) max_backoff: Duration,
  /// How long `setup` waits for the agent that `bd-host setup --agent` started
  pub(crate) agent_start: Duration,
  /// How often the connection of a pack gets an empty chunk while git builds the pack
  pub(crate) keepalive: Duration,
  /// How long `upgrade` waits for the hello of the new agent: the drain of the old agent, the wait of the watchdog,
  /// and 30 s for launchd
  pub(crate) upgrade: Duration,
}

impl Timing {
  pub(crate) const DEFAULT: Self = Self {
    connect: Duration::from_secs(5),
    resolve: Duration::from_secs(5),
    read: Duration::from_secs(120),
    reconnect_for: Duration::from_secs(600),
    first_backoff: Duration::from_secs(1),
    max_backoff: Duration::from_secs(30),
    agent_start: Duration::from_secs(60),
    keepalive: Duration::from_secs(HEARTBEAT_SECONDS),
    upgrade: Duration::from_secs(UPGRADE_DRAIN_SECONDS + UPGRADE_WATCHDOG_SECONDS + 30),
  };
}

/// The client of one process: its configuration, its build, its git, its seam for ssh, the sink of its stderr lines,
/// and the count of the Ctrl+C presses
pub struct Client<'a> {
  config: Config,
  build: Build,
  git: GitCommandExecutor,
  processes: &'a dyn Processes,
  say: &'a (dyn Fn(&str) + Sync),
  /// The sink of the stdout lines of a run, or `None` to collect them into `RunOutcome::stdout`
  stdout: Option<&'a (dyn Fn(&str) + Sync)>,
  presses: &'a (dyn Fn() -> u32 + Sync),
  timing: Timing,
  /// How long the build of a pack waits before git starts, so a test can press Ctrl+C or count keepalives meanwhile;
  /// zero outside the tests
  hold_pack: Duration,
}

impl<'a> Client<'a> {
  /// `say` takes each stderr line of a run as it comes, and each progress line of the client. `presses` answers how
  /// many times the person pressed Ctrl+C; `interrupt::presses` is the count of the terminal. The git of the client is
  /// the one that `GitCommandExecutor::new` finds, and `with_git` names another one
  pub fn new(config: Config, build: Build, processes: &'a dyn Processes, say: &'a (dyn Fn(&str) + Sync), presses: &'a (dyn Fn() -> u32 + Sync)) -> Self {
    Self {
      config,
      build,
      git: GitCommandExecutor::new(),
      processes,
      say,
      stdout: None,
      presses,
      timing: Timing::DEFAULT,
      hold_pack: Duration::ZERO,
    }
  }

  /// Runs git through `git`, such as the one of `GitInfo::discover_for_cli`
  #[must_use]
  pub fn with_git(mut self, git: GitCommandExecutor) -> Self {
    self.git = git;
    self
  }

  /// Sends each stdout line of a run to `stdout` as it comes, instead of `RunOutcome::stdout` at the end
  #[must_use]
  pub const fn with_stdout(mut self, stdout: &'a (dyn Fn(&str) + Sync)) -> Self {
    self.stdout = Some(stdout);
    self
  }

  pub const fn config(&self) -> &Config {
    &self.config
  }

  pub const fn build(&self) -> &Build {
    &self.build
  }

  /// Whether the person pressed Ctrl+C
  fn interrupted(&self) -> bool {
    (self.presses)() > 0
  }

  /// The host of an alias, from its host file
  pub fn host(&self, alias: &str) -> Result<Host, Refusal> {
    Host::open(&self.config, alias, &self.build.version, self.timing)
  }

  #[cfg(test)]
  pub(crate) const fn with_timing(mut self, timing: Timing) -> Self {
    self.timing = timing;
    self
  }

  #[cfg(test)]
  pub(crate) const fn with_hold_pack(mut self, hold: Duration) -> Self {
    self.hold_pack = hold;
    self
  }
}

/// The affinity of a local checkout on a host: `checkout-<12 hex digits>`, one per checkout path, so a run prefers the
/// slot that its checkout used last. The hash is FNV-1a, stable across builds and platforms, unlike the hasher of the
/// standard library. bt and `bd-host run` name the same affinity for one checkout
pub fn affinity(checkout: &Path) -> String {
  let hash = checkout
    .to_string_lossy()
    .bytes()
    .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
  format!("checkout-{:012x}", hash & 0xffff_ffff_ffff)
}

/// A value quoted for a POSIX shell
pub(crate) fn shell_quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', r"'\''"))
}

/// The first 12 digits of a commit or a digest
pub(crate) fn short(id: &str) -> &str {
  id.get(..12).unwrap_or(id)
}
