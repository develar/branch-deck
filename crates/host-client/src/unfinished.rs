//! `brd-host cancel-unfinished` (openspec/specs/cli/host/spec.md, "follow, cancel, fetch, forget, runs"): the cancel of
//! each run of a checkout whose end the checkout has not finished, on the host that its record names. The end of a
//! lease runs it (`lease_pool::host_cancel`): the runs of a lease that nobody follows after its return would keep their
//! place in the queue of the host until `heavy_timeout`.
//!
//! The hosts go in parallel, each one within `max` from the start; the runs of one host go one after the other, each
//! as `brd-host cancel` sends it. A host that does not answer keeps its runs, which the next end of a lease of the slot
//! tries again. A run that the agent took the cancel of, that ended already, or that the agent no longer knows
//! (`unknown_run`) is cancelled: its record goes, so a later end does not cancel it again. A run that the agent does not
//! know yet, whose record is younger than `START_GRACE`, stays: its start can still come. A record that another
//! process of the checkout finishes (it holds `<run>.lock`) stays too

use crate::Client;
use crate::git::Checkout;
use crate::phases::duration_text;
use crate::record::RunFiles;
use crate::run::{cancel_on, is_link};
use futures_util::future::join_all;
use host_link::refusal::{Code, Refusal};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use tokio::time::{Instant, timeout_at};

/// How long `cancel-unfinished` waits for each host when `--max` names no time
pub const DEFAULT_MAX: Duration = Duration::from_secs(5);

/// How long after `<run>.host` an agent that knows no such run may still get its start: `Client::run` writes the record
/// before it sends `Start`, which waits for a connection up to the read deadline of the client
const START_GRACE: Duration = Duration::from_secs(120);

/// What `cancel-unfinished` did, in the order of the run ids
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Unfinished {
  /// The runs that the host holds no wait for now
  pub cancelled: Vec<CancelledRun>,
  /// The runs whose cancel did not come to the host, or that the host refused
  pub not_cancelled: Vec<UncancelledRun>,
}

/// A run that the host holds no wait for now
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CancelledRun {
  pub run: String,
  /// The alias of its host
  pub host: String,
  /// What the agent said: it has the cancel request, the run ended already, or it knows no such run
  pub answer: String,
}

/// A run whose cancel did not come to the host, or that the host refused
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UncancelledRun {
  pub run: String,
  /// The alias of its host
  pub host: String,
  /// The refusal code, such as `timed_out` or `unreachable`
  pub code: Code,
  pub message: String,
}

impl Client<'_> {
  /// Cancels each run of the checkout of `dir` whose end it has not finished, on the host of its record, each host
  /// within `max`, the hosts in parallel. The record of each cancelled run goes. A checkout without run records answers
  /// an empty `Unfinished` and connects to no host
  pub async fn cancel_unfinished(&self, dir: &Path, max: Duration) -> Result<Unfinished, Refusal> {
    let checkout = Checkout::open(&self.git, dir)?;
    let Some(files) = RunFiles::existing(&checkout.runs_dir()) else {
      return Ok(Unfinished::default());
    };
    let mut by_host: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (run, alias) in files.unfinished()? {
      by_host.entry(alias).or_default().push(run);
    }
    let deadline = Instant::now() + max;
    let hosts = join_all(by_host.into_iter().map(|(alias, runs)| self.cancel_on_host(alias, runs, deadline, max))).await;
    let mut answer = Unfinished::default();
    for (host, run, result) in hosts.into_iter().flatten() {
      match result {
        Ok(answer_text) => answer.cancelled.push(CancelledRun { run, host, answer: answer_text }),
        // a start that the client sends after it wrote `<run>.host` can still come: the agent knows no such run yet
        Err(refusal) if refusal.code == Code::UnknownRun && files.host_age(&run).is_some_and(|age| age >= START_GRACE) => {
          answer.cancelled.push(CancelledRun {
            run,
            host,
            answer: refusal.message,
          });
        }
        Err(refusal) if refusal.code == Code::UnknownRun => answer.not_cancelled.push(UncancelledRun {
          message: format!(
            "{}; its record is younger than {} s, so its start can still come: the next end of a lease of this checkout tries again",
            refusal.message,
            START_GRACE.as_secs()
          ),
          run,
          host,
          code: refusal.code,
        }),
        Err(refusal) => answer.not_cancelled.push(UncancelledRun {
          run,
          host,
          code: refusal.code,
          message: refusal.message,
        }),
      }
    }
    answer.cancelled.sort_by(|a, b| a.run.cmp(&b.run));
    answer.not_cancelled.sort_by(|a, b| a.run.cmp(&b.run));
    for run in &mut answer.cancelled {
      if !files.forget_record(&run.run) {
        run.answer.push_str("; another process of this checkout finishes the run, so its record stays");
      }
    }
    Ok(answer)
  }

  /// The cancel of each of `runs` on the host of `alias` up to `deadline`. After a lost link or the deadline the rest
  /// of the runs get the same refusal without a try: the host does not answer
  async fn cancel_on_host(&self, alias: String, runs: Vec<String>, deadline: Instant, max: Duration) -> Vec<(String, String, Result<String, Refusal>)> {
    let mut host = match self.host(&alias) {
      Ok(host) => host,
      Err(refusal) => return runs.into_iter().map(|run| (alias.clone(), run, Err(refusal.clone()))).collect(),
    };
    let mut answers = Vec::with_capacity(runs.len());
    let mut down: Option<Refusal> = None;
    for run in runs {
      let result = match &down {
        Some(refusal) => Err(refusal.clone()),
        None => timeout_at(deadline, cancel_on(&mut host, &run, self.timing.read)).await.unwrap_or_else(|_| {
          Err(Refusal::new(
            Code::TimedOut,
            format!(
              "{alias} did not answer the cancel within {}: the run can go on there, and the next end of a lease of this checkout tries again",
              duration_text(u64::try_from(max.as_millis()).unwrap_or(u64::MAX))
            ),
          ))
        }),
      };
      if let Err(refusal) = &result
        && (is_link(refusal) || Instant::now() >= deadline)
      {
        down = Some(refusal.clone());
      }
      answers.push((alias.clone(), run, result));
    }
    answers
  }
}

#[cfg(test)]
#[path = "unfinished_test.rs"]
mod unfinished_test;
