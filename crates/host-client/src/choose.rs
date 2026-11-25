//! A run to a set of hosts (openspec/specs/cli/host/spec.md, "run: A run to a set goes to the member that ranks first";
//! docs/design/host-runs.md, "Sets of hosts"): the client asks every member at once what the run would meet there
//! (`Request::Estimate`, 3 s each, the feature read from the hello of the same connection), ranks the members that may
//! take it, says its plan in one line, and runs it on the first of them.
//!
//! - **Eligible**: the first member, always; for a heavy run, only a member with its own measure of the command, so a
//!   new command, such as a script of a measurement, stays on the first member, and what a laptop may take is what ran
//!   there once; for a run that is not heavy, every member that answered. Never a member that keeps no slot for the
//!   runs of the repository (`Estimate::slots` 0), the first one neither: its agent would refuse the run with
//!   `no_slots`.
//! - **Rank**: `start_in_ms + unknown_ahead × 10 min + expected_wall_ms`, the least first, a tie by the order of the set.
//!   A wall that a member does not know is the longest that another member knows. A member without an estimate ranks
//!   last.
//! - **A member that takes runs only at once** (`idle_only`) gets the start with `RunSpec::admit_now`: its agent admits
//!   the run at once or refuses it with `busy` before anything ran, and the run goes to the next member. The admission
//!   decides, not the estimate, so two clients never meet on one free member.
//!
//! The estimate is a hint: what the host does with the run is the rule of its heavy turn

use crate::Client;
use crate::config::{HostFile, HostSet};
use crate::git::Checkout;
use crate::host::Host;
use crate::record::RunFiles;
use crate::run::{RunFailure, RunOptions, RunOutcome, RunRequest, sibling};
use git_ops::host_client::Scan;
use host_link::heavy::script_in;
use host_link::proto::{Estimate, EstimateAsk, Reply, Request, feature, is_run_id};
use host_link::refusal::{Code, Reached, Refusal};
use std::fmt::Write as _;
use std::time::Duration;
use tokio::time::timeout;

/// How long the client waits for the estimate of each member, all of them asked at once
pub(crate) const ESTIMATE_WAIT: Duration = Duration::from_secs(3);

/// What a run of an unknown length before the run weighs in the rank
const UNKNOWN_RUN_MS: u64 = 10 * 60 * 1000;

/// What a member of a set told of a run
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
  Estimate(Estimate),
  /// Its agent predates estimates: no feature `estimate`
  Predates,
  /// No estimate: no answer within 3 s, a refusal, or no host file
  Missing(String),
}

/// A member of a set and what it told
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
  pub alias: String,
  pub idle_only: bool,
  pub answer: Answer,
}

impl Candidate {
  const fn estimate(&self) -> Option<&Estimate> {
    match &self.answer {
      Answer::Estimate(estimate) => Some(estimate),
      Answer::Predates | Answer::Missing(_) => None,
    }
  }

  /// Whether the member at `position` of its set may take the run: the first always, a heavy run only a member with a
  /// measure of its command, a run that is not heavy every member that answered. A member that takes runs only at once
  /// needs the start of `RunSpec::admit_now`, which an agent without estimates lacks
  fn eligible(&self, position: usize, heavy: bool) -> bool {
    !self.keeps_no_slot() && (position == 0 || self.estimate().is_some_and(|estimate| !heavy || estimate.expected_cores.is_some()))
  }

  /// Whether the member told that it keeps no slot for the runs of the repository
  fn keeps_no_slot(&self) -> bool {
    self.estimate().is_some_and(|estimate| estimate.slots == Some(0))
  }
}

/// The members of `candidates` that may take the run, in the order the run tries them: by their rank, a tie by the
/// order of the set
pub fn rank(candidates: &[Candidate], heavy: bool) -> Vec<usize> {
  let longest = candidates.iter().filter_map(|candidate| candidate.estimate()?.expected_wall_ms).max().unwrap_or(0);
  let mut order: Vec<usize> = (0..candidates.len()).filter(|&position| candidates[position].eligible(position, heavy)).collect();
  // a stable sort keeps the order of the set for a tie
  order.sort_by_key(|&position| candidates[position].estimate().map_or(u64::MAX, |estimate| rank_ms(estimate, longest)));
  order
}

/// The rank of an estimate: when the run would end there, `longest` for a wall that the member does not know
fn rank_ms(estimate: &Estimate, longest: u64) -> u64 {
  estimate
    .start_in_ms
    .saturating_add(u64::from(estimate.unknown_ahead).saturating_mul(UNKNOWN_RUN_MS))
    .saturating_add(estimate.expected_wall_ms.unwrap_or(longest))
}

/// The line of the plan: what each member told, then the member chosen: `brd-host: studio: starts in ~12 min, runs ~4
/// min; local: starts now, runs ~6 min; chose local`
pub fn plan_line(candidates: &[Candidate], heavy: bool, chosen: Option<usize>) -> String {
  let mut line = "brd-host:".to_owned();
  for (position, candidate) in candidates.iter().enumerate() {
    let told = match &candidate.answer {
      Answer::Estimate(_) if candidate.keeps_no_slot() => {
        "it keeps no slot for the runs of this repository (`branchdeck.hostSlots` or the limit of `brd-host slots --local --shrink` is 0 there), so no run goes there".to_owned()
      }
      Answer::Estimate(_) if !candidate.eligible(position, heavy) => "no measure of this command there, so a heavy run does not go there".to_owned(),
      Answer::Estimate(estimate) => {
        let starts = if estimate.start_in_ms == 0 {
          "starts now".to_owned()
        } else {
          format!("starts in {}", about(estimate.start_in_ms))
        };
        let unknown = match estimate.unknown_ahead {
          0 => String::new(),
          1 => " after a run of an unknown length".to_owned(),
          count => format!(" after {count} runs of an unknown length"),
        };
        let runs = estimate
          .expected_wall_ms
          .map_or_else(|| "no measure of this command".to_owned(), |wall| format!("runs {}", about(wall)));
        let busy = if candidate.idle_only { ", only at once" } else { "" };
        format!("{starts}{unknown}, {runs}{busy}")
      }
      Answer::Predates => format!("no estimate: its agent predates them (`brd-host upgrade {}`)", candidate.alias),
      Answer::Missing(why) => format!("no estimate: {why}"),
    };
    let separator = if position == 0 { " " } else { "; " };
    let _ = write!(line, "{separator}{}: {told}", candidate.alias);
  }
  match chosen {
    Some(chosen) => {
      let _ = write!(line, "; chose {}", candidates[chosen].alias);
    }
    None => line.push_str("; no member may take it"),
  }
  line
}

/// A duration of a plan, about: `~50 s`, `~12 min`, `~1 h 5 min`
fn about(ms: u64) -> String {
  let seconds = ms.div_ceil(1000);
  match seconds {
    0..60 => format!("~{seconds} s"),
    60..3600 => format!("~{} min", (seconds + 30) / 60),
    _ => format!("~{} h {} min", seconds / 3600, seconds % 3600 / 60),
  }
}

impl Client<'_> {
  /// The alias of the host of the run `run` for the name `name` of a command line (`wait`, `cancel`, `follow`, `fetch`,
  /// `forget`): `name` itself when it has a host file, else the member of the set `name` that `<run>.host` of the
  /// checkout of `dir` names (`crate::record`). A set without that file refuses with `usage` and names the members
  pub fn host_of_run(&self, name: &str, run: &str, dir: Option<&std::path::Path>) -> Result<String, Refusal> {
    let Some(set) = self.config.set_of(name)? else {
      return Ok(name.to_owned());
    };
    if !is_run_id(run) {
      return Err(Refusal::new(Code::Usage, format!("{run:?} cannot name a run")));
    }
    let host = match dir {
      Some(dir) => RunFiles::of(&Checkout::open(&self.git, dir)?)?.read_host(run)?,
      None => None,
    };
    host.ok_or_else(|| {
      let members: Vec<&str> = set.members.iter().map(|member| member.alias.as_str()).collect();
      Refusal::new(
        Code::Usage,
        format!(
          "{name} is a set of hosts ({}), and this checkout names no host of the run {run}: name its host, such as `brd-host wait {} {run}`",
          members.join(", "),
          members[0]
        ),
      )
    })
  }

  /// Runs a command of the checkout on the member of `set` that ranks first by the estimates of its members, and on the
  /// next one when a member that takes runs only at once refuses it with `busy`. Answers the alias of the member that
  /// took the run with the end of the run; when every member refused it with `busy`, the refusal of the first member of
  /// the set (else of the first one tried): nothing ran anywhere. `--exclusive` names one host: a
  /// measurement needs the quiet host it measures, so it refuses with `usage`
  pub async fn run_on_set(&self, set: &HostSet, request: &RunRequest<'_>, options: RunOptions) -> (String, Result<RunOutcome, RunFailure>) {
    let first = set.members[0].alias.clone();
    let before = |refusal: Refusal| (first.clone(), Err(RunFailure::at(refusal, Reached::Nothing, &crate::run::RunFacts::default())));
    if options.exclusive {
      return before(Refusal::new(
        Code::Usage,
        format!(
          "--exclusive names one host, not the set {}: a measurement needs the quiet host that it measures, such as `brd-host run {first} --exclusive`",
          set.name
        ),
      ));
    }
    let (ask, heavy) = match self.ask_of(request, options) {
      Ok(ask) => ask,
      Err(refusal) => return before(refusal),
    };
    let candidates = self.estimates(set, &ask, request, options).await;
    let order = rank(&candidates, heavy);
    (self.say)(&plan_line(&candidates, heavy, order.first().copied()));
    // the refusals of the members that answered `busy`, in the order of the tries
    let mut refused: Vec<(usize, RunFailure)> = Vec::new();
    for (step, &position) in order.iter().enumerate() {
      let candidate = &candidates[position];
      let admit_now = candidate.idle_only && candidate.estimate().is_some();
      let ran = self.run(&candidate.alias, request, RunOptions { admit_now, ..options }).await;
      match ran {
        Err(failure) if failure.refusal.code == Code::Busy => {
          if let Some(&next) = order.get(step + 1) {
            (self.say)(&format!("brd-host: {}; the run goes to {}", failure.refusal.message, candidates[next].alias));
          }
          refused.push((position, failure));
        }
        ran => return (candidate.alias.clone(), ran),
      }
    }
    // every member refused with `busy`: the refusal of the first member of the set, else of the first one tried
    let first_refused = refused.iter().position(|(position, _)| *position == 0).unwrap_or(0);
    if refused.is_empty() {
      return before(Refusal::new(Code::NoHost, format!("no member of the set {} may take the run", set.name)));
    }
    let (position, failure) = refused.swap_remove(first_refused);
    (candidates[position].alias.clone(), Err(failure))
  }

  /// What the members are asked of the run: its command and fetch, the class of its measures, its affinity, HEAD of the
  /// checkout, and whether it is heavy by `--heavy` or `--measure-as`, or by `heavy` of `[host]` of HEAD
  fn ask_of(&self, request: &RunRequest<'_>, options: RunOptions) -> Result<(EstimateAsk, bool), Refusal> {
    request.check()?;
    let checkout = Checkout::open(&self.git, request.checkout)?;
    let head = checkout.snapshot(Scan::Head, 1)?.head;
    let config = checkout.host_config(&head)?;
    let heavy = request.heavy_flag(options).is_some() || config.heavy_reason(request.command, &|path| script_in(checkout.root(), path)).is_some();
    let ask = EstimateAsk {
      repo: String::new(),
      affinity: request.affinity.to_owned(),
      commit: head,
      command: request.command.to_vec(),
      fetch: request.fetch.to_vec(),
      heavy,
      measure_as: request.measure_as.map(str::to_owned),
    };
    Ok((ask, heavy))
  }

  /// The estimates of the members of `set` for the run of `ask`, all asked at once, each within `ESTIMATE_WAIT`: in the
  /// repository of `request`, else in the one of the same name as the local one beside the repository of the host file
  /// when the host has it, else in that one
  async fn estimates(&self, set: &HostSet, ask: &EstimateAsk, request: &RunRequest<'_>, options: RunOptions) -> Vec<Candidate> {
    let name = if request.repo.is_none() && options.repo_by_name {
      Checkout::open(&self.git, request.checkout).ok().and_then(|checkout| checkout.repository_name())
    } else {
      None
    };
    let repos = |file: &HostFile| match request.repo {
      Some(repo) => vec![repo.to_owned()],
      None => name
        .as_deref()
        .and_then(|name| sibling(&file.repo, name))
        .filter(|repo| *repo != file.repo)
        .into_iter()
        .chain(std::iter::once(file.repo.clone()))
        .collect(),
    };
    let asking = set.members.iter().map(|member| async {
      let answer = match self.host(&member.alias) {
        Ok(mut host) => {
          let repos = repos(host.file());
          timeout(ESTIMATE_WAIT, self.estimate_on(&mut host, ask, &repos))
            .await
            .unwrap_or_else(|_| Answer::Missing(format!("no answer in {} s", ESTIMATE_WAIT.as_secs())))
        }
        Err(refusal) => Answer::Missing(refusal.message),
      };
      Candidate {
        alias: member.alias.clone(),
        idle_only: member.idle_only,
        answer,
      }
    });
    futures_util::future::join_all(asking).await
  }

  /// The estimate of `ask` on `host` in the first of `repos` that the host has, on one connection per repository whose
  /// hello names the feature `estimate`: an agent without it would close the connection on the request
  async fn estimate_on(&self, host: &mut Host, ask: &EstimateAsk, repos: &[String]) -> Answer {
    for (index, repo) in repos.iter().enumerate() {
      let (mut channel, agent) = match host.connect().await {
        Ok(connected) => connected,
        Err(refusal) => return Answer::Missing(refusal.message),
      };
      if !agent.features.iter().any(|known| known == feature::ESTIMATE) {
        channel.close().await;
        return Answer::Predates;
      }
      if ask.measure_as.is_some() && !agent.features.iter().any(|known| known == feature::MEASURE_AS) {
        channel.close().await;
        return Answer::Missing(format!(
          "its agent predates --measure-as, so it knows no measure of the class (`brd-host upgrade {}`)",
          host.alias()
        ));
      }
      let request = Request::Estimate(EstimateAsk {
        repo: repo.clone(),
        ..ask.clone()
      });
      if let Err(error) = channel.send(&request).await {
        return Answer::Missing(host.link_refusal(&error).message);
      }
      match host.recv(&mut channel, self.timing.read).await {
        Ok(Reply::Estimate(estimate)) => {
          channel.close().await;
          return Answer::Estimate(estimate);
        }
        Ok(Reply::Refused { code, .. }) if code == Code::RepoMissing.name() && index + 1 < repos.len() => {}
        Ok(Reply::Refused { code, message }) => return Answer::Missing(host.agent_refusal(&code, &message).message),
        Ok(reply) => return Answer::Missing(host.unexpected(&reply).message),
        Err(refusal) => return Answer::Missing(refusal.message),
      }
    }
    Answer::Missing("no repository to ask".to_owned())
  }
}

#[cfg(test)]
#[path = "choose_test.rs"]
mod choose_test;
