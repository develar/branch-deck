//! The state of a run that a wait says (openspec/specs/cli/host/spec.md, "wait: wait says each state of the run as it
//! changes"), from what `Reply::Runs` carries of an agent of any build: the steps that have ended so far
//! (`RunInfo::phases`), the end, and the heavy turn of the host. The steps of a run come in the order `heavy` or
//! `exclusive`, `git`, `slot`, `pack_wait`, `checkout`, `prepare`, `command`, `measure`, `stage`, so the last of
//! `slot`, `prepare` and `command` that ended names the step that goes on

use host_link::proto::RunInfo;
use serde_json::Value;

/// What a run does now, as a wait sees it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunState {
  /// A heavy run in the queue of the heavy turn, behind `ahead` runs
  HeavyTurn { ahead: usize },
  /// A run that is not heavy waits for the end of a measurement, an exclusive run that holds the host
  Measurement,
  /// The run waits for a host slot
  Slot,
  /// The run has its slot: the pack, the checkout and the prepare steps
  Prepares,
  /// The command runs
  Command,
  /// The command ended, and the host stages the files of the run
  Stages,
  /// The run has ended
  Ended,
}

impl RunState {
  /// The state of `info`, with `heavy`, the heavy turn of the host as `Reply::Runs` carries it
  pub(crate) fn of(info: &RunInfo, heavy: Option<&Value>) -> Self {
    let ended = |name: &str| info.phases.iter().any(|phase| phase.name == name);
    if info.exit.is_some() {
      return Self::Ended;
    }
    if ended("command") {
      return Self::Stages;
    }
    if ended("prepare") {
      return Self::Command;
    }
    if ended("slot") {
      return Self::Prepares;
    }
    let entries = |key: &str| heavy.and_then(|turn| turn.get(key)).and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let is_run = |entry: &Value| entry.get("run").and_then(Value::as_str) == Some(info.run.as_str());
    if let Some(ahead) = entries("queue").iter().position(is_run) {
      return Self::HeavyTurn { ahead };
    }
    let holds = entries("holders").iter().any(is_run);
    let measured = entries("holders")
      .iter()
      .any(|entry| !is_run(entry) && entry.get("exclusive").and_then(Value::as_bool) == Some(true));
    if !holds && measured && info.phases.is_empty() {
      return Self::Measurement;
    }
    Self::Slot
  }

  /// The line of the state of `run` on `alias` at the local time `time` (`14:02:11`)
  pub(crate) fn line(self, run: &str, alias: &str, time: &str) -> String {
    let what = match self {
      Self::HeavyTurn { ahead: 0 } => format!("waits for the heavy turn of {alias}, first in its queue: `brd-host runs {alias}` names the runs that hold it"),
      Self::HeavyTurn { ahead: 1 } => format!("waits for the heavy turn of {alias}, 1 run ahead: `brd-host runs {alias}` names the runs that hold it"),
      Self::HeavyTurn { ahead } => format!("waits for the heavy turn of {alias}, {ahead} runs ahead: `brd-host runs {alias}` names the runs that hold it"),
      Self::Measurement => format!("waits for the end of a measurement that holds {alias}"),
      Self::Slot => format!("waits for a host slot of {alias}"),
      Self::Prepares => format!("checks out its commit and prepares on {alias}"),
      Self::Command => format!("runs its command on {alias}"),
      Self::Stages => format!("ended its command; {alias} stages its files"),
      Self::Ended => format!("ended on {alias}"),
    };
    format!("brd-host: {time} run {run} {what}")
  }

  /// The state in the words of a sentence that goes on: `it runs its command`
  pub(crate) const fn doing(self) -> &'static str {
    match self {
      Self::HeavyTurn { .. } => "it waits for the heavy turn",
      Self::Measurement => "it waits for the end of a measurement",
      Self::Slot => "it waits for a host slot",
      Self::Prepares => "it checks out its commit and prepares",
      Self::Command => "it runs its command",
      Self::Stages => "its command ended, and the host stages its files",
      Self::Ended => "it ended",
    }
  }
}

#[cfg(test)]
#[path = "state_test.rs"]
mod state_test;
