//! `bd-host schema` (spec/cli/host.spec.md, "version, schema, skill"): one JSON object with the protocol schema and
//! port, every refusal code with its exit code and its words, the exit codes, and the JSON schemas of the report of a
//! run and of the hello. A caller such as bt reads it to act on the codes, never on the words of a message

use host_client::report::{HelloRefusal, HelloReport, REPORT_SCHEMA, RunReport};
use host_link::proto::{DEFAULT_PORT, LAUNCHD_LABEL, SCHEMA_VERSION};
use host_link::refusal::Code;
use schemars::{Schema, schema_for};
use serde::Serialize;

/// The object of `bd-host schema`
#[derive(Debug, Serialize)]
pub(crate) struct SchemaOutput {
  /// The schema of the report and of the hello
  pub(crate) schema: u32,
  pub(crate) protocol: Protocol,
  /// The exit codes of `bd-host` beside the exit code of a command
  pub(crate) exits: Vec<ExitText>,
  /// Every refusal code, in the order of the table of the spec
  pub(crate) codes: Vec<CodeText>,
  /// The JSON schema of the report of `run --report-fd`
  pub(crate) report: Schema,
  /// The JSON schema of `hello --json`
  pub(crate) hello: Schema,
  /// The JSON schema of `hello --json` for a refusal
  pub(crate) hello_refusal: Schema,
}

#[derive(Debug, Serialize)]
pub(crate) struct Protocol {
  /// The schema of host-link
  pub(crate) schema: u32,
  pub(crate) port: u16,
  pub(crate) launch_agent: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct ExitText {
  pub(crate) exit: u8,
  pub(crate) meaning: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct CodeText {
  pub(crate) code: &'static str,
  /// The column "Exit" of the spec: `75`, `255`, or `75, 255 after the start`
  pub(crate) exit: &'static str,
  pub(crate) words: &'static str,
}

/// The exit codes of `bd-host`. A run that exited has the exit code of its command instead
const EXITS: [(u8, &str); 4] = [
  (2, "a usage error: a command line that bd-host does not accept"),
  (75, "the host did not run the command, so the caller can run it locally"),
  (130, "Ctrl+C before the start, or a second Ctrl+C during the follow"),
  (255, "any other refusal: the command ran, or whether it ran is unknown, or the request is no run"),
];

pub(crate) fn schema() -> SchemaOutput {
  SchemaOutput {
    schema: REPORT_SCHEMA,
    protocol: Protocol {
      schema: SCHEMA_VERSION,
      port: DEFAULT_PORT,
      launch_agent: LAUNCHD_LABEL,
    },
    exits: EXITS.iter().map(|&(exit, meaning)| ExitText { exit, meaning }).collect(),
    codes: Code::ALL
      .iter()
      .map(|code| {
        let row = code.row();
        CodeText {
          code: row.name,
          exit: row.exit.text(),
          words: row.words,
        }
      })
      .collect(),
    report: schema_for!(RunReport),
    hello: schema_for!(HelloReport),
    hello_refusal: schema_for!(HelloRefusal),
  }
}

#[cfg(test)]
#[path = "schema_test.rs"]
mod schema_test;
