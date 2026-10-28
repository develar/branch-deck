//! The machine answers of the client (spec/cli/host.spec.md, "The report of a run", "hello"): the report of a run that
//! `bd-host run --report-fd <n>` writes to its descriptor, and the hello of `bd-host hello --json`. A caller such as bt
//! acts on the codes and the exit codes of these objects, never on the words of a message. The golden JSON of each one
//! is a fenced block of the spec, which the tests read

use crate::Build;
use crate::host::AgentInfo;
use crate::run::{RunFacts, RunFailure, RunOutcome};
use host_link::proto::{Phase, SCHEMA_VERSION};
use host_link::refusal::{Exit, Refusal};
use serde::{Deserialize, Serialize};

/// The schema of the report and of the hello. A change that a caller cannot read raises it
pub const REPORT_SCHEMA: u32 = 1;

/// The report of one run: one JSON object and a line end on the descriptor of `--report-fd`, on every way out after the
/// parse of the command line
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunReport {
  pub schema: u32,
  pub alias: String,
  /// The id of the run; `null` before the client chose it
  pub run: Option<String>,
  /// The shipped commit; `null` before the snapshot read it
  pub commit: Option<String>,
  pub head: Option<String>,
  /// Whether the run shipped the working tree, not the committed HEAD
  pub working_tree: bool,
  /// The repository of the run on the host; `null` before the host answered
  pub repository: Option<String>,
  /// The exit code of `bd-host`
  pub exit: u8,
  /// The exit code of the command; `null` when it did not run or its end is unknown. A command that a signal ended
  /// answers 128 plus the signal number
  pub command_exit: Option<i32>,
  pub refusal: Option<Refusal>,
  /// The steps of the run in their order, each with its milliseconds
  pub phases: Vec<Phase>,
  /// The files that came back, relative to the checkout root with `/`
  pub fetched: Vec<String>,
  /// The count of the files that `--apply` changed; `null` without `--apply`
  pub applied_files: Option<u64>,
}

impl RunReport {
  /// The report of a run whose command exited: `bd-host` exits with the code of the command
  pub fn of_outcome(alias: &str, outcome: &RunOutcome) -> Self {
    Self::of_facts(alias, &outcome.facts, command_exit_code(outcome.code), Some(outcome.code), None)
  }

  /// The report of a run that refused
  pub fn of_failure(alias: &str, failure: &RunFailure) -> Self {
    Self::of_facts(alias, &failure.facts, failure.exit.code(), failure.command_exit, Some(failure.refusal.clone()))
  }

  /// The report of a refusal before the client knew anything of the run, such as a descriptor that is not open
  pub fn of_refusal(alias: &str, working_tree: bool, refusal: &Refusal, exit: Exit) -> Self {
    let facts = RunFacts {
      working_tree,
      ..RunFacts::default()
    };
    Self::of_facts(alias, &facts, exit.code(), None, Some(refusal.clone()))
  }

  fn of_facts(alias: &str, facts: &RunFacts, exit: u8, command_exit: Option<i32>, refusal: Option<Refusal>) -> Self {
    Self {
      schema: REPORT_SCHEMA,
      alias: alias.to_owned(),
      run: facts.run.clone(),
      commit: facts.commit.clone(),
      head: facts.head.clone(),
      working_tree: facts.working_tree,
      repository: facts.repository.clone(),
      exit,
      command_exit,
      refusal,
      phases: facts.phases.clone(),
      fetched: facts.fetched.clone(),
      applied_files: facts.applied_files,
    }
  }

  /// The object as one line of JSON with its line end
  pub fn line(&self) -> String {
    json_line(self)
  }
}

/// The exit of `bd-host` for a command that exited with `code`: the code itself, and 255 for a code outside 0 to 255
pub fn command_exit_code(code: i32) -> u8 {
  u8::try_from(code).unwrap_or(u8::MAX)
}

/// The hello of `bd-host hello --json`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HelloReport {
  pub schema: u32,
  pub alias: String,
  /// The address that answered
  pub address: String,
  pub agent: AgentHello,
  pub client: ClientHello,
  /// Whether both ends run the same build: the same SHA-256 of the binary
  pub same_build: bool,
}

/// What the agent named in its hello
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentHello {
  /// The version of Branch Deck
  pub version: String,
  /// The SHA-256 of its binary, which names a build; `null` when the agent cannot read its binary
  pub build: Option<String>,
  /// The schema of host-link
  pub schema: u32,
  pub os: String,
  pub arch: String,
  pub features: Vec<String>,
}

/// What this client is
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientHello {
  pub version: String,
  pub build: Option<String>,
  pub schema: u32,
}

impl HelloReport {
  pub fn new(alias: &str, agent: &AgentInfo, client: &Build) -> Self {
    Self {
      schema: REPORT_SCHEMA,
      alias: alias.to_owned(),
      address: agent.address.to_string(),
      agent: AgentHello {
        version: agent.version.clone(),
        build: agent.build.clone(),
        schema: agent.schema,
        os: agent.os.clone(),
        arch: agent.arch.clone(),
        features: agent.features.clone(),
      },
      client: ClientHello {
        version: client.version.clone(),
        build: client.sha256.clone(),
        schema: SCHEMA_VERSION,
      },
      same_build: agent.build.is_some() && agent.build == client.sha256,
    }
  }

  pub fn line(&self) -> String {
    json_line(self)
  }
}

/// The answer of `bd-host hello --json` for a refusal: `{"schema": 1, "alias", "refusal": {"code", "message"}}`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HelloRefusal {
  pub schema: u32,
  pub alias: String,
  pub refusal: Refusal,
}

impl HelloRefusal {
  pub fn new(alias: &str, refusal: &Refusal) -> Self {
    Self {
      schema: REPORT_SCHEMA,
      alias: alias.to_owned(),
      refusal: refusal.clone(),
    }
  }

  pub fn line(&self) -> String {
    json_line(self)
  }
}

/// One object as JSON with its line end. The types of this module always serialize
fn json_line<T: Serialize>(value: &T) -> String {
  let mut line = serde_json::to_string(value).unwrap_or_default();
  line.push('\n');
  line
}

#[cfg(test)]
#[path = "report_test.rs"]
mod report_test;
