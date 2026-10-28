//! The refusal codes of bd-host (spec/cli/host.spec.md, "Refusal codes"): one variant per code, its name, its exit
//! code and its words in one table (`Code::row`), so a code cannot have two exits. A caller acts on the code and the
//! exit code, never on the message. Exit 75 means that the host did not run the command, so the caller can run it
//! locally

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The exit code of `bd-host` for a refusal
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
  /// 2: a command line that bd-host does not accept
  Usage,
  /// 75: the host did not run the command
  NotRun,
  /// 130: Ctrl+C
  Interrupted,
  /// 255: the command ran, or whether it ran is unknown, or the refusal is of no run
  Refused,
}

impl Exit {
  pub const fn code(self) -> u8 {
    match self {
      Self::Usage => 2,
      Self::NotRun => 75,
      Self::Interrupted => 130,
      Self::Refused => 255,
    }
  }
}

/// How far a run came when the refusal came: before `Request::Start`, after it, or after the command started on the
/// host (the phases of the agent have `command`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reached {
  Nothing,
  Started,
  CommandStarted,
}

/// The exit of a code by how far the run came
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitRule {
  /// The same exit at every point
  Always(Exit),
  /// 75 before `Request::Start`, 255 after it
  NotRunBeforeStart,
  /// 75 until the command started, 255 after
  NotRunBeforeCommand,
}

impl ExitRule {
  /// The text of the column "Exit" of the spec, which `bd-host schema` prints too
  pub const fn text(self) -> &'static str {
    match self {
      Self::Always(exit) => match exit {
        Exit::Usage => "2",
        Exit::NotRun => "75",
        Exit::Interrupted => "130",
        Exit::Refused => "255",
      },
      Self::NotRunBeforeStart => "75, 255 after the start",
      Self::NotRunBeforeCommand => "75, 255 after the command started",
    }
  }
}

/// One row of the table: the name of a code, its exit rule and when it comes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
  pub name: &'static str,
  pub exit: ExitRule,
  pub words: &'static str,
}

/// A refusal code
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Code {
  Usage,
  Interrupted,
  Config,
  NoHost,
  BadHostFile,
  Unreachable,
  Untrusted,
  Schema,
  TimedOut,
  Link,
  LocalChanges,
  Behind,
  GitFailed,
  BadRequest,
  BadRunId,
  RunExists,
  UnknownRun,
  RepoMissing,
  PackFailed,
  PackMissing,
  SlotTimeout,
  LeaseFailed,
  OldGit,
  CheckoutFailed,
  BadConfig,
  PrepareFailed,
  SpawnFailed,
  AgentError,
  FetchFailed,
  ApplyFailed,
  BadBinary,
  Downgrade,
  PlistChanged,
  UpgradeFailed,
  RolledBack,
  AgentFailed,
  SshFailed,
  LaunchdFailed,
  SkillEdited,
}

impl Code {
  /// Every code, in the order of the table of the spec
  pub const ALL: [Self; 39] = [
    Self::Usage,
    Self::Interrupted,
    Self::Config,
    Self::NoHost,
    Self::BadHostFile,
    Self::Unreachable,
    Self::Untrusted,
    Self::Schema,
    Self::TimedOut,
    Self::Link,
    Self::LocalChanges,
    Self::Behind,
    Self::GitFailed,
    Self::BadRequest,
    Self::BadRunId,
    Self::RunExists,
    Self::UnknownRun,
    Self::RepoMissing,
    Self::PackFailed,
    Self::PackMissing,
    Self::SlotTimeout,
    Self::LeaseFailed,
    Self::OldGit,
    Self::CheckoutFailed,
    Self::BadConfig,
    Self::PrepareFailed,
    Self::SpawnFailed,
    Self::AgentError,
    Self::FetchFailed,
    Self::ApplyFailed,
    Self::BadBinary,
    Self::Downgrade,
    Self::PlistChanged,
    Self::UpgradeFailed,
    Self::RolledBack,
    Self::AgentFailed,
    Self::SshFailed,
    Self::LaunchdFailed,
    Self::SkillEdited,
  ];

  /// The row of the code: its name, its exit rule and its words
  pub const fn row(self) -> Row {
    let (name, exit, words) = match self {
      Self::Usage => (
        "usage",
        ExitRule::Always(Exit::Usage),
        "An alias that is not `[0-9A-Za-z._-]+`, a run without a command, a working directory in no git checkout, a descriptor of `--report-fd` that is not open, a fetch without a record and without `--fetch`. A command line that clap refuses exits 2 too.",
      ),
      Self::Interrupted => (
        "interrupted",
        ExitRule::Always(Exit::Interrupted),
        "Ctrl+C before the start, or a second Ctrl+C during the follow.",
      ),
      Self::Config => (
        "config",
        ExitRule::Always(Exit::NotRun),
        "No home directory, or an identity, a record or a file of bd-host that cannot be read or written.",
      ),
      Self::NoHost => (
        "no_host",
        ExitRule::Always(Exit::NotRun),
        "No host file for the alias. The message names `bd-host setup <alias>`.",
      ),
      Self::BadHostFile => (
        "bad_host_file",
        ExitRule::Always(Exit::NotRun),
        "A host file that is not JSON of its shape, or that names another alias.",
      ),
      Self::Unreachable => (
        "unreachable",
        ExitRule::Always(Exit::NotRun),
        "No address answers. The message names each address and the lookup, and Local Network privacy when the relay cannot pass it.",
      ),
      Self::Untrusted => (
        "untrusted",
        ExitRule::Always(Exit::NotRun),
        "The agent shows another certificate than the pin, or it refuses the certificate of the client.",
      ),
      Self::Schema => ("schema", ExitRule::Always(Exit::NotRun), "The agent speaks another schema of host-link."),
      Self::TimedOut => (
        "timed_out",
        ExitRule::NotRunBeforeStart,
        "No reply within the deadline of the request; for a follow, 10 min over all reconnects.",
      ),
      Self::Link => (
        "link",
        ExitRule::NotRunBeforeStart,
        "The connection closed, or a reply came that the protocol does not allow there.",
      ),
      Self::LocalChanges => ("local_changes", ExitRule::Always(Exit::NotRun), "`--head` with tracked or untracked changes."),
      Self::Behind => ("behind", ExitRule::Always(Exit::NotRun), "The host has none of the candidate commits."),
      Self::GitFailed => (
        "git_failed",
        ExitRule::Always(Exit::NotRun),
        "A git call of the client failed: the snapshot, the candidates or the pack. The message has the words of git.",
      ),
      Self::BadRequest => (
        "bad_request",
        ExitRule::NotRunBeforeStart,
        "A request that the agent does not accept, such as a fetch of a run that goes on.",
      ),
      Self::BadRunId => ("bad_run_id", ExitRule::Always(Exit::NotRun), "A run id that cannot name a directory."),
      Self::RunExists => ("run_exists", ExitRule::Always(Exit::NotRun), "A start with the id of a kept run."),
      Self::UnknownRun => (
        "unknown_run",
        ExitRule::Always(Exit::Refused),
        "No kept run has the id. After a start: the run is gone, and whether its command ran is unknown.",
      ),
      Self::RepoMissing => ("repo_missing", ExitRule::Always(Exit::NotRun), "The repository path is no git repository on the host."),
      Self::PackFailed => ("pack_failed", ExitRule::Always(Exit::NotRun), "The agent cannot land the pack: no pack, or git refused it."),
      Self::PackMissing => (
        "pack_missing",
        ExitRule::Always(Exit::NotRun),
        "The pack of the commit did not come within 30 min, or sent no chunk for 2 min.",
      ),
      Self::SlotTimeout => ("slot_timeout", ExitRule::Always(Exit::NotRun), "The run got no host slot within an hour."),
      Self::LeaseFailed => ("lease_failed", ExitRule::Always(Exit::NotRun), "The lend failed. The message has the error of the pool."),
      Self::OldGit => (
        "old_git",
        ExitRule::Always(Exit::NotRun),
        "The git of the host is below the floor of Branch Deck, and Homebrew cannot upgrade it. The message is the hint of Branch Deck.",
      ),
      Self::CheckoutFailed => ("checkout_failed", ExitRule::Always(Exit::NotRun), "The commit cannot be checked out in the slot."),
      Self::BadConfig => (
        "bad_config",
        ExitRule::Always(Exit::NotRun),
        "`[host]` of `.branchdeck.toml` of the commit does not parse, has an unknown key or a value of the wrong shape.",
      ),
      Self::PrepareFailed => ("prepare_failed", ExitRule::Always(Exit::NotRun), "A prepare step exited with another code than 0."),
      Self::SpawnFailed => ("spawn_failed", ExitRule::Always(Exit::NotRun), "The command cannot start."),
      Self::AgentError => (
        "agent_error",
        ExitRule::NotRunBeforeCommand,
        "The agent cannot read or write its own files, or a supervisor got lost.",
      ),
      Self::FetchFailed => (
        "fetch_failed",
        ExitRule::Always(Exit::Refused),
        "The fetch did not copy every file. The run stays on the host for `bd-host fetch` and `bd-host forget`.",
      ),
      Self::ApplyFailed => (
        "apply_failed",
        ExitRule::Always(Exit::Refused),
        "The changes do not apply to the working tree, or they did not come because the fetch failed.",
      ),
      Self::BadBinary => (
        "bad_binary",
        ExitRule::Always(Exit::Refused),
        "The binary of an upgrade has another size or SHA-256, or `version --json` does not answer as an agent of this schema with `upgrade`.",
      ),
      Self::Downgrade => (
        "downgrade",
        ExitRule::Always(Exit::Refused),
        "The binary of an upgrade has a lower version, and the request has no `force`.",
      ),
      Self::PlistChanged => (
        "plist_changed",
        ExitRule::Always(Exit::Refused),
        "The new binary would write another LaunchAgent than the one on the host: `bd-host setup` installs it.",
      ),
      Self::UpgradeFailed => (
        "upgrade_failed",
        ExitRule::Always(Exit::Refused),
        "The agent cannot stage, keep or swap its binary, or start the watchdog, or another upgrade runs.",
      ),
      Self::RolledBack => (
        "rolled_back",
        ExitRule::Always(Exit::Refused),
        "The new agent did not listen in time, and the previous binary answers.",
      ),
      Self::AgentFailed => (
        "agent_failed",
        ExitRule::Always(Exit::Refused),
        "The binary for a host cannot be read or is of another platform, the host is not macOS for `upgrade`, or `setup --agent` on the host failed.",
      ),
      Self::SshFailed => (
        "ssh_failed",
        ExitRule::Always(Exit::Refused),
        "An ssh call of `setup <alias>` failed or ran past its deadline.",
      ),
      Self::LaunchdFailed => ("launchd_failed", ExitRule::Always(Exit::Refused), "A `launchctl` call of `setup` failed."),
      Self::SkillEdited => (
        "skill_edited",
        ExitRule::Always(Exit::Refused),
        "`skill install` found a copy with local edits, without `--force`.",
      ),
    };
    Row { name, exit, words }
  }

  /// The name of the code, such as `slot_timeout`
  pub const fn name(self) -> &'static str {
    self.row().name
  }

  /// The code of `name`, or `None` for a name that this build does not know
  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|code| code.name() == name)
  }

  /// The exit of bd-host for this code when the run came as far as `reached`
  pub const fn exit(self, reached: Reached) -> Exit {
    match self.row().exit {
      ExitRule::Always(exit) => exit,
      ExitRule::NotRunBeforeStart if matches!(reached, Reached::Nothing) => Exit::NotRun,
      ExitRule::NotRunBeforeCommand if !matches!(reached, Reached::CommandStarted) => Exit::NotRun,
      ExitRule::NotRunBeforeStart | ExitRule::NotRunBeforeCommand => Exit::Refused,
    }
  }
}

impl fmt::Display for Code {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.name())
  }
}

impl Serialize for Code {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.name())
  }
}

impl<'de> Deserialize<'de> for Code {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let name = String::deserialize(deserializer)?;
    Self::from_name(&name).ok_or_else(|| serde::de::Error::custom(format!("{name:?} is no refusal code of bd-host")))
  }
}

/// The JSON schema of a code: one of the names of the table
impl schemars::JsonSchema for Code {
  fn schema_name() -> std::borrow::Cow<'static, str> {
    "RefusalCode".into()
  }

  fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let names: Vec<&str> = Self::ALL.iter().map(|code| code.name()).collect();
    schemars::json_schema!({ "type": "string", "enum": names })
  }
}

/// A refusal: its code and a message for a person. It prints as `<code>: <message>`, and bd-host puts `bd-host: ` before
/// it on the stderr
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, thiserror::Error)]
#[error("{code}: {message}")]
pub struct Refusal {
  pub code: Code,
  pub message: String,
}

impl Refusal {
  pub fn new(code: Code, message: impl Into<String>) -> Self {
    Self { code, message: message.into() }
  }

  /// The refusal of `Reply::Refused` of an agent. A code that this build does not know becomes `agent_error`, and the
  /// message keeps the code
  pub fn of_agent(code: &str, message: &str) -> Self {
    match Code::from_name(code) {
      Some(code) => Self::new(code, message),
      None => Self::new(
        Code::AgentError,
        format!("the agent refused with the code {code}, which this build does not know: {message}"),
      ),
    }
  }
}

#[cfg(test)]
#[path = "refusal_test.rs"]
mod refusal_test;
