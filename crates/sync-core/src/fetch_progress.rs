//! Progress of `git fetch --progress`, read from its stderr lines in the C locale (`execute_remote_command_with_progress`).
//! Git has no machine-readable progress (trace2 reports a stage only when it ends), so this is a port of IDEA's
//! `GitStandardProgressAnalyzer`: every stage has a share of the whole, earlier stages count as done. Best effort: a
//! line it doesn't know changes nothing, and nothing depends on it but the label of the Update button.

use serde::Serialize;

/// How far a fetch is, for the Update button
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FetchProgress {
  /// The whole fetch, 0 to 100, never decreasing
  pub percent: u8,
  /// Received so far, as git prints it, e.g. "5.79 MiB"
  pub received: Option<String>,
  /// The transfer rate, as git prints it, e.g. "32.57 MiB/s"
  pub speed: Option<String>,
  /// The last progress line of git, without the `remote: ` prefix, e.g. "Receiving objects:  45% (136/302)"
  pub line: String,
}

/// The stages of a fetch in their order, with the share of the whole (IDEA's numbers: not measured, just plausible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
  Counting,
  Compressing,
  Receiving,
  Resolving,
}

const STAGES: [Stage; 4] = [Stage::Counting, Stage::Compressing, Stage::Receiving, Stage::Resolving];

impl Stage {
  fn from_title(title: &str) -> Option<Self> {
    match title {
      "Counting objects" => Some(Self::Counting),
      "Compressing objects" => Some(Self::Compressing),
      // a fetch under `fetch.unpackLimit` objects unpacks them instead of indexing a pack
      "Receiving objects" | "Unpacking objects" => Some(Self::Receiving),
      "Resolving deltas" => Some(Self::Resolving),
      _ => None,
    }
  }

  fn share(self) -> f64 {
    match self {
      Self::Counting | Self::Resolving => 0.05,
      Self::Compressing => 0.1,
      Self::Receiving => 0.8,
    }
  }
}

/// Without a total, counting reports only the number of objects: IDEA counts 5000 as done
const COUNTED_AS_DONE: f64 = 5000.0;

/// Turns the stderr lines of a fetch into `FetchProgress`
#[derive(Debug, Default)]
pub struct FetchProgressAnalyzer {
  /// Done fraction (0 to 1) of every stage seen, by stage order
  done: [f64; STAGES.len()],
  percent: u8,
  received: Option<String>,
  speed: Option<String>,
}

impl FetchProgressAnalyzer {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// The progress after `line`; `None` for a line that is not a progress line of a known stage
  pub fn analyze(&mut self, line: &str) -> Option<FetchProgress> {
    let line = line.strip_prefix("remote: ").unwrap_or(line).trim();
    let (title, rest) = line.split_once(": ")?;
    let stage = Stage::from_title(title)?;
    let rest = rest.trim_start();
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let number: f64 = rest[..digits].parse().ok()?;
    let fraction = if rest[digits..].starts_with('%') {
      number / 100.0
    } else if stage == Stage::Counting {
      number / COUNTED_AS_DONE
    } else {
      return None;
    };

    let index = STAGES.iter().position(|it| *it == stage)?;
    for done in &mut self.done[..index] {
      *done = 1.0;
    }
    self.done[index] = fraction.clamp(0.0, 1.0);
    let total: f64 = STAGES.iter().zip(self.done).map(|(stage, done)| stage.share() * done).sum();
    // whole percents of a sum of shares; rounded so that all stages done is 100
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (total * 100.0).round().clamp(0.0, 100.0) as u8;
    self.percent = self.percent.max(percent);

    if stage == Stage::Receiving
      && let Some((received, speed)) = throughput(rest)
    {
      self.received = Some(received.to_string());
      self.speed = Some(speed.to_string());
    }
    Some(FetchProgress {
      percent: self.percent,
      received: self.received.clone(),
      speed: self.speed.clone(),
      line: line.to_string(),
    })
  }
}

/// `5.79 MiB` and `32.57 MiB/s` from `100% (302/302), 5.79 MiB | 32.57 MiB/s, done.`
fn throughput(rest: &str) -> Option<(&str, &str)> {
  let (_, after_count) = rest.split_once("), ")?;
  let after_count = after_count.strip_suffix(", done.").unwrap_or(after_count);
  let (received, speed) = after_count.split_once(" | ")?;
  Some((received.trim(), speed.trim()))
}

#[cfg(test)]
#[path = "fetch_progress_test.rs"]
mod fetch_progress_test;
