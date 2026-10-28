//! The protocol host-link: one hello and one request per connection, over TLS 1.3 (spec/cli/host.spec.md, "The link").
//!
//! Each connection starts with `Request::Hello` and its `Reply::Hello`, so a client learns at once that the agent speaks
//! another schema. Then the client sends one more request. The replies of each request are listed on it. Any request
//! can be answered by `Reply::Refused` instead.
//!
//! A request that waits, for a slot, for a lock or for git, gets `Reply::Heartbeat` every `HEARTBEAT_SECONDS` before its
//! last reply. A client resets its read deadline at each heartbeat, so a short deadline holds for every request.
//!
//! A run lives on the agent, not on the connection. `Request::Start` answers at once, and `Request::Follow` reads the
//! output from an offset, so a client that lost its connection follows again from the last offset it saw.
//!
//! A change that an older end cannot read raises `SCHEMA_VERSION`. The client and the agent are replaced together, so
//! the protocol keeps no field for an older end, with one exception: the upgrade. The hello, `Request::Upgrade`,
//! `Reply::Upgraded` and `VersionLine` keep their shapes, so the agent of a host takes the next binary over the link

use crate::refusal::{Code, Refusal};
use serde::{Deserialize, Serialize};
use sync_utils::host_ref_name::is_host_ref_name;

/// The schema of the protocol. A change that an older end cannot read raises it. The hello of another schema refuses
/// with `schema`
pub const SCHEMA_VERSION: u32 = 1;

/// The TCP port of `bd-host serve`. yonderd listens on 7741, so both agents serve one host side by side
pub const DEFAULT_PORT: u16 = 7742;

/// The label of the `LaunchAgent` that starts `bd-host serve` on a macOS host
pub const LAUNCHD_LABEL: &str = "com.jetbrains.bd-host";

/// How often the agent sends `Reply::Heartbeat` while a request waits or a followed run is quiet
pub const HEARTBEAT_SECONDS: u64 = 15;

/// How long the supervisor of a run waits for the pack of its commit, see `Request::Start`
pub const PACK_WAIT_SECONDS: u64 = 30 * 60;

/// How long the supervisor of a run waits for the next chunk of the pack of its commit. A client sends a chunk at least
/// every `HEARTBEAT_SECONDS` on the connection of the pack, an empty one while it builds the pack. So a client that is
/// gone ends the wait of its run in this time, and its host slot goes back
pub const PACK_STALL_SECONDS: u64 = 120;

/// How long an agent that answered `Reply::Upgraded` waits for its other requests to end before it exits, so that
/// launchd starts the new binary. A follow does not count: its client connects again
pub const UPGRADE_DRAIN_SECONDS: u64 = 20;

/// How long the watchdog of an upgrade waits for the new binary to listen, after the drain, before it restores the
/// previous binary and restarts the agent
pub const UPGRADE_WATCHDOG_SECONDS: u64 = 60;

/// The largest binary of `Request::Upgrade`
pub const MAX_BINARY: u64 = 256 * 1024 * 1024;

/// What an agent does beyond the base of `SCHEMA_VERSION`, named in its hello, in `bd-host version` and in the line of
/// `bd-host check`. A client checks a feature before it sends its request
pub mod feature {
  /// `Request::Upgrade`, and the commands `check` and `watchdog`: only on macOS, because only launchd starts the new
  /// binary
  pub const UPGRADE: &str = "upgrade";
}

/// The features of an agent of this build on this platform: `upgrade` on macOS
pub fn features() -> Vec<&'static str> {
  if cfg!(target_os = "macos") { vec![feature::UPGRADE] } else { Vec::new() }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum Request {
  /// The first message of each connection. Reply: `Reply::Hello`, or `Reply::Refused` with `schema`
  Hello { schema_version: u32, client_version: String },
  /// Which of `commits` the repository has as commit objects. Reply: `Reply::Have`
  Have { repo: String, commits: Vec<String> },
  /// A thin pack follows as a byte stream. The agent writes it into the repository and points the ref of `target` at
  /// `commit`: `refs/branch-deck/host/run/<id>` or `refs/branch-deck/host/base/<name>` (`lease_pool::host_refs`).
  /// Reply: `Reply::Packed`.
  ///
  /// A base is the commit that later packs of the client are thin against, such as its master. The agent keeps it
  /// until the next pack to the same name, or until no pack moved it for 14 days
  Pack { repo: String, commit: String, target: PackTarget },
  /// Starts a run. Reply: `Reply::Started` as soon as the run has its directory.
  ///
  /// A start can come before the pack of its commit. The supervisor then takes the slot while the pack travels, and
  /// waits for the pack before the checkout, up to `PACK_WAIT_SECONDS`, and up to `PACK_STALL_SECONDS` without a chunk
  /// of it. A cancel ends that wait too. A commit that the host has already needs no pack, and the supervisor does not
  /// wait for it
  Start(RunSpec),
  /// The output of a run from the byte offset `offset` of its log. Replies: `Reply::Output` and `Reply::Heartbeat`
  /// while the run goes on, then `Reply::Phases`, then `Reply::Exit`, or `Reply::Refused` when the run failed before or
  /// without an exit code of its command
  Follow { run: String, offset: u64 },
  /// Asks the supervisor of a run to interrupt it: SIGINT to the process group of the command, then SIGKILL after the
  /// grace time, or no command at all when the run has not started it yet. Reply: `Reply::Done` as soon as the
  /// supervisor has the request. The run ends later, and `Request::Follow` shows its end
  Cancel { run: String },
  /// The files that the run staged. Replies: `Reply::File` each followed by its bytes, then `Reply::Fetched`
  Fetch { run: String },
  /// Removes a run and its stage. Reply: `Reply::Done`
  Forget { run: String },
  /// The runs that the agent keeps. Reply: `Reply::Runs`
  Runs,
  /// The lend table of each repository that the agent served. Reply: `Reply::Slots`
  Slots,
  /// Replaces the binary that the `LaunchAgent` names. The new binary follows as a byte stream of `size` bytes whose
  /// SHA-256 is `sha256` (64 lower-case hex digits). Replies: `Reply::Heartbeat` while the agent checks the binary, then
  /// `Reply::Upgraded`. Feature `feature::UPGRADE`.
  ///
  /// After the reply the agent waits up to `UPGRADE_DRAIN_SECONDS` for its other requests, and exits, so that launchd
  /// starts the new binary. A watchdog restores the previous binary when the new one does not listen within
  /// `UPGRADE_WATCHDOG_SECONDS` after that. A run goes on, because its supervisor has a session of its own.
  ///
  /// `force` takes a binary of a lower version too
  Upgrade {
    size: u64,
    sha256: String,
    #[serde(default, skip_serializing_if = "is_false")]
    force: bool,
  },
}

/// The ref of the host that a pack moves: the ref of a run, or a base of a client. Its JSON is `{"run": "<id>"}` or
/// `{"base": "<name>"}`, never a ref name, so a client cannot name another ref of the host
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackTarget {
  /// The id of a run, which keeps the commit until the give-back of the run
  Run(String),
  /// The name of a base: the affinity of the client
  Base(String),
}

impl PackTarget {
  /// The id or the name
  pub fn name(&self) -> &str {
    match self {
      Self::Run(name) | Self::Base(name) => name,
    }
  }

  /// Whether the id or the name has the rules of a host ref name (`is_host_ref_name`)
  pub fn is_valid(&self) -> bool {
    is_host_ref_name(self.name())
  }
}

/// What one run does. The JSON of a field after `fetch` is optional, and its default is left out
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSpec {
  /// The id that the client chose: `<epoch ms>-<pid>`. It names the run directory and the ref of the pack of the run,
  /// so it has the rules of a host ref name (`is_run_id`)
  pub run: String,
  /// The main repository on the host. A leading `~/` is the home directory of the agent
  pub repo: String,
  /// The affinity of the lend: `checkout-<12 hex>` of the client checkout by default. A run prefers the host slot that
  /// its affinity used last
  pub affinity: String,
  /// The commit to check out in the slot
  pub commit: String,
  /// The command and its arguments, run in the root of the slot
  pub command: Vec<String>,
  /// Directories relative to the slot root, such as `out/bt-runs`. Each entry that the command adds to one goes to the
  /// stage of the run, for `Request::Fetch`
  pub fetch: Vec<String>,
  /// The niceness of the prepare steps and the command, 0 to 19. `None` keeps the niceness of the agent
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub nice: Option<u8>,
  /// Stage the outputs that `[host]` of the commit names, for `Request::Fetch`. The agent removes each output before the
  /// command, so only the outputs of this run come back. Without it the agent leaves the outputs alone
  #[serde(default, skip_serializing_if = "is_false")]
  pub outputs: bool,
  /// Stage the changes that the command made to the files of the slot, as a patch for `git apply`, for
  /// `Request::Fetch`
  #[serde(default, skip_serializing_if = "is_false")]
  pub changes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub enum Reply {
  Hello {
    schema_version: u32,
    agent_version: String,
    /// `std::env::consts::OS` and `ARCH` of the agent
    os: String,
    arch: String,
    /// The features of the agent, see `feature`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    features: Vec<String>,
    /// The SHA-256 of the binary of the agent, see `crate::digest`. An agent that cannot read its binary sends none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    binary_sha256: Option<String>,
  },
  Have {
    commits: Vec<String>,
  },
  Packed,
  Started {
    run: String,
  },
  /// One line of the output of a run, without its line end. `end` is the offset after it
  Output {
    stream: Stream,
    text: String,
    end: u64,
  },
  Heartbeat {
    elapsed_ms: u64,
  },
  /// How long each step of the run took on the host, in the order of the steps. It comes before the last reply of a
  /// `Follow` of a run that has ended
  Phases {
    phases: Vec<Phase>,
  },
  /// The exit code of the command. A command that a signal ended answers 128 plus the signal number
  Exit {
    code: i32,
  },
  /// One staged file, relative to the slot root with `/` separators. Its bytes follow as a byte stream
  File {
    path: String,
    size: u64,
    executable: bool,
    /// What the file is. A client that set neither `RunSpec::outputs` nor `RunSpec::changes` gets only
    /// `FileKind::Staged`
    #[serde(default, skip_serializing_if = "FileKind::is_staged")]
    kind: FileKind,
  },
  Fetched {
    files: u64,
    bytes: u64,
  },
  Runs {
    runs: Vec<RunInfo>,
  },
  /// The lend table of each repository that the agent served
  Slots {
    repositories: Vec<RepositorySlots>,
  },
  Done {
    message: String,
  },
  /// The new binary of `Request::Upgrade` is in place. `previous_sha256` names the binary that the watchdog restores,
  /// and `version` is the version that the new binary printed
  Upgraded {
    previous_sha256: String,
    sha256: String,
    version: String,
  },
  /// The agent refuses or the run failed. `code` is a name of `refusal::Code`; a client reads it with
  /// `Refusal::of_agent`, so a code of a newer build is no protocol error
  Refused {
    code: String,
    message: String,
  },
}

/// What a fetched file is
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
  /// A new entry of a `fetch` directory of the run
  #[default]
  Staged,
  /// A file of the output `root`, a path that `[host]` names. The path of the file is `root` or below it. The client
  /// replaces its own copy of `root` with the one of the run
  Output { root: String },
  /// The changes that the command made to the files of the slot, as a patch for `git apply` in the root of a checkout.
  /// The path of the file is a name, not a path of the slot
  Changes,
}

impl FileKind {
  pub const fn is_staged(&self) -> bool {
    matches!(self, Self::Staged)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
  Stdout,
  Stderr,
}

/// A run that the agent keeps
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInfo {
  pub run: String,
  pub affinity: String,
  pub commit: String,
  pub command: Vec<String>,
  pub started_ms: u64,
  /// `None` while the run goes on
  pub exit: Option<i32>,
  /// The steps that have ended so far
  #[serde(default)]
  pub phases: Vec<Phase>,
  /// The `fetch` directories of `RunSpec`
  #[serde(default)]
  pub fetch: Vec<String>,
}

/// The lend table of one repository of the host
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositorySlots {
  /// The main repository on the host
  pub repo: String,
  /// The table as `host-slots.json` of the repository holds it (`lease_pool::host_slots::HostSlots`): this crate
  /// reaches no pool, so the table stays JSON here, and `bd-host slots` reads it with the type of the pool
  pub table: serde_json::Value,
}

/// One step of a run and its duration: `git`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`, `stage`. A client
/// adds its own steps, such as `pack`, `upload` and `fetch`, when it reports a run
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase {
  pub name: String,
  pub ms: u64,
}

/// The JSON line of `bd-host version --json` and of `bd-host check --plist <plist>`. An upgrade runs `check` on the new
/// binary before the swap, and reads this line
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionLine {
  /// The version of Branch Deck
  pub version: String,
  /// The schema of the protocol, `SCHEMA_VERSION`
  pub schema: u32,
  #[serde(default)]
  pub features: Vec<String>,
  /// The SHA-256 of the binary, its build id; `None` when the binary cannot be read
  pub sha256: Option<String>,
  /// The state of the `LaunchAgent` for this build, from `check` only: `same`, `changed` or `unknown`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub plist: Option<String>,
}

impl VersionLine {
  /// The line of this binary of `version`, with the features of this build
  pub fn of_this_build(version: &str, sha256: Option<String>) -> Self {
    Self {
      version: version.to_owned(),
      schema: SCHEMA_VERSION,
      features: features().into_iter().map(str::to_owned).collect(),
      sha256,
      plist: None,
    }
  }
}

impl Request {
  /// The hello of this build of the client
  pub fn hello(client_version: &str) -> Self {
    Self::Hello {
      schema_version: SCHEMA_VERSION,
      client_version: client_version.to_owned(),
    }
  }
}

impl Reply {
  /// The hello of an agent of `agent_version` and the binary `binary_sha256` on the platform of this build, with the
  /// features of this build
  pub fn hello(agent_version: &str, binary_sha256: Option<String>) -> Self {
    Self::Hello {
      schema_version: SCHEMA_VERSION,
      agent_version: agent_version.to_owned(),
      os: std::env::consts::OS.to_owned(),
      arch: std::env::consts::ARCH.to_owned(),
      features: features().into_iter().map(str::to_owned).collect(),
      binary_sha256,
    }
  }

  pub fn refused(code: Code, message: impl Into<String>) -> Self {
    Self::Refused {
      code: code.name().to_owned(),
      message: message.into(),
    }
  }

  /// The reply of a refusal
  pub fn of_refusal(refusal: &Refusal) -> Self {
    Self::refused(refusal.code, refusal.message.clone())
  }
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "serde calls `skip_serializing_if` with a reference")]
const fn is_false(value: &bool) -> bool {
  !*value
}

/// Whether `run` can name a run directory and the ref of the pack of the run: the rules of a host ref name
pub fn is_run_id(run: &str) -> bool {
  is_host_ref_name(run)
}
