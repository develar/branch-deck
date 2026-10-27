//! Progress of `git fetch --progress` and `git push --progress`, read from their stderr lines in the C locale
//! (`execute_remote_command_with_progress`). Git has no machine-readable progress (trace2 reports a stage only when it
//! ends), so this is a port of IDEA's `GitStandardProgressAnalyzer`: every stage has a share of the whole, earlier
//! stages count as done. Best effort: a line it doesn't know changes nothing, and nothing depends on it but the labels
//! of the Update and Push buttons.

use serde::Serialize;
use std::time::{Duration, Instant};

/// How far a fetch or a push is, for the button that runs it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
  /// The whole transfer, 0 to 100, never decreasing
  pub percent: u8,
  /// Received (fetch) or sent (push) so far, as git prints it, e.g. "5.79 MiB"
  pub transferred: Option<String>,
  /// The transfer rate, as git prints it, e.g. "32.57 MiB/s"
  pub speed: Option<String>,
  /// The last progress line of git, without the `remote: ` prefix, e.g. "Receiving objects:  45% (136/302)"
  pub line: String,
}

/// The stages of a transfer in their order, with the share of the whole (IDEA's numbers: not measured, just plausible).
/// A fetch counts and compresses on the server and resolves deltas locally, a push the other way round; the `remote: `
/// prefix makes no difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
  Counting,
  Compressing,
  Transfer,
  Resolving,
}

const STAGES: [Stage; 4] = [Stage::Counting, Stage::Compressing, Stage::Transfer, Stage::Resolving];

impl Stage {
  fn from_title(title: &str) -> Option<Self> {
    match title {
      "Counting objects" => Some(Self::Counting),
      "Compressing objects" => Some(Self::Compressing),
      // a fetch under `fetch.unpackLimit` objects unpacks them instead of indexing a pack; a push writes them
      "Receiving objects" | "Unpacking objects" | "Writing objects" => Some(Self::Transfer),
      "Resolving deltas" => Some(Self::Resolving),
      _ => None,
    }
  }

  const fn share(self) -> f64 {
    match self {
      Self::Counting | Self::Resolving => 0.05,
      Self::Compressing => 0.1,
      Self::Transfer => 0.8,
    }
  }
}

/// Without a total, counting reports only the number of objects: IDEA counts 5000 as done
const COUNTED_AS_DONE: f64 = 5000.0;

/// Turns the stderr lines of a fetch or a push into `TransferProgress`
#[derive(Debug, Default)]
pub struct TransferProgressAnalyzer {
  /// Done fraction (0 to 1) of every stage seen, by stage order
  done: [f64; STAGES.len()],
  percent: u8,
  transferred: Option<String>,
  speed: Option<String>,
  /// The last analyzed line reported its stage complete
  stage_ended: bool,
}

impl TransferProgressAnalyzer {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// The progress after `line`; `None` for a line that is not a progress line of a known stage
  pub fn analyze(&mut self, line: &str) -> Option<TransferProgress> {
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
    // a fetch ends a stage with `, done.`, the server of a push its deltas with `, completed with 2 local objects.`
    self.stage_ended = self.done[index] >= 1.0;
    let total: f64 = STAGES.iter().zip(self.done).map(|(stage, done)| stage.share() * done).sum();
    // whole percents of a sum of shares; rounded so that all stages done is 100
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a whole percent, clamped to 0..=100")]
    let percent = (total * 100.0).round().clamp(0.0, 100.0) as u8;
    self.percent = self.percent.max(percent);

    if stage == Stage::Transfer
      && let Some((transferred, speed)) = throughput(rest)
    {
      self.transferred = Some(transferred.to_string());
      self.speed = Some(speed.to_string());
    }
    Some(TransferProgress {
      percent: self.percent,
      transferred: self.transferred.clone(),
      speed: self.speed.clone(),
      line: line.to_string(),
    })
  }
}

/// `5.79 MiB` and `32.57 MiB/s` from `100% (302/302), 5.79 MiB | 32.57 MiB/s, done.`
fn throughput(rest: &str) -> Option<(&str, &str)> {
  let (_, after_count) = rest.split_once("), ")?;
  let after_count = after_count.strip_suffix(", done.").unwrap_or(after_count);
  let (transferred, speed) = after_count.split_once(" | ")?;
  Some((transferred.trim(), speed.trim()))
}

/// At most this often a progress goes to the window; the end of a stage always goes out, so the last amount is shown
/// even when a stage ends within the interval
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Feeds the stderr lines of a `--progress` command through `TransferProgressAnalyzer` to `on_progress`, throttled:
/// the first progress, then a changed percent or amount once `PROGRESS_INTERVAL` passed or a stage ended
pub struct TransferProgressReporter<'a> {
  analyzer: TransferProgressAnalyzer,
  last_sent: Option<(Instant, TransferProgress)>,
  on_progress: &'a mut dyn FnMut(TransferProgress),
}

impl<'a> TransferProgressReporter<'a> {
  pub fn new(on_progress: &'a mut dyn FnMut(TransferProgress)) -> Self {
    Self {
      analyzer: TransferProgressAnalyzer::new(),
      last_sent: None,
      on_progress,
    }
  }

  pub fn on_line(&mut self, line: &str) {
    let Some(progress) = self.analyzer.analyze(line) else {
      return;
    };
    let due = match &self.last_sent {
      None => true,
      Some((at, sent)) => {
        let changed = sent.percent != progress.percent || sent.transferred != progress.transferred;
        changed && (at.elapsed() >= PROGRESS_INTERVAL || self.analyzer.stage_ended)
      }
    };
    if due {
      (self.on_progress)(progress.clone());
      self.last_sent = Some((Instant::now(), progress));
    }
  }
}

#[cfg(test)]
#[path = "transfer_progress_test.rs"]
mod transfer_progress_test;
