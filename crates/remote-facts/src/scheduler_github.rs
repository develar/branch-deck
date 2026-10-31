//! The pull requests of GitHub, the rules of their bases and the merge profiles, as the task of a session asks them
//! (the parent module's header)

use super::{InFlight, Input, Reply, SessionFacts};
use domain_policy::refresh::{
  FactTiming, ProfilesRead, ReadState, RefreshReason, ServiceState, branch_rules_due, merge_state_computing, profiles_due, pull_requests_due, pull_requests_follows_up,
  pull_requests_push_recheck_at,
};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::session::{FactRefreshReason, Version};
use repository_model::facts::FactResult;
use repository_model::pull_request_facts::PushedHead;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use sync_utils::timestamp::unix_now;
use tracing::Instrument;

/// What GitHub and Patronus are asked of a session besides the robots
#[derive(Default)]
pub(super) struct GitHubSchedule {
  pull_requests: PullRequestsSchedule,
  rules: RulesSchedule,
  profiles: ProfilesSchedule,
  /// The baseline of the committed table: the merge profiles are of its target
  baseline: Option<String>,
}

#[derive(Default)]
struct PullRequestsSchedule {
  list: FactTiming,
  /// A push or the refresh button during the list in flight asks one more after it
  in_flight: InFlight,
  /// The last answer said the branches are not on GitHub: only the refresh button asks again
  off_service: bool,
  /// GitHub refused the last list for its rate limit: only the refresh button asks before this
  retry_at: Option<Duration>,
  /// The heads of the pushes since the last list started, which the next list checks
  pushed: Vec<PushedHead>,
  /// When the list that looks again after a push starts
  recheck_at: Option<Duration>,
}

#[derive(Default)]
struct RulesSchedule {
  read: BTreeMap<String, ReadState>,
  reading: BTreeSet<String>,
  /// The refreshes that wait for reads in flight: each for the bases it asked, never for another read
  waiters: Vec<RulesWaiter>,
}

/// A refresh of the rules: the bases whose reads it waits for, and the version after the last of them
struct RulesWaiter {
  bases: BTreeSet<String>,
  version: Option<Version>,
  reply: Reply,
}

#[derive(Default)]
struct ProfilesSchedule {
  last: Option<ProfilesRead>,
  /// The baseline of the last read
  read_for: Option<String>,
  /// The reads in flight by baseline, each with the refreshes that wait for it: a refresh of another baseline waits for
  /// none of them
  reading: BTreeMap<String, Vec<Reply>>,
}

/// What a list of the pull requests answered, for its schedule
pub(super) struct PullRequestsListed {
  /// `None`: the session was gone
  pub availability: Option<PullRequestAvailability>,
  /// GitHub still shows another head of a branch the push before it pushed, or has not computed whether it can merge an
  /// open pull request (`refresh::merge_state_computing`): one more list looks a little later
  pub shows_old_head: bool,
  /// The version after its patch and the bases whose rules are read
  pub version: Option<Version>,
  pub rule_bases: Vec<String>,
}

impl GitHubSchedule {
  /// When the list that looks again after a push starts
  pub(super) const fn timer(&self) -> Option<Duration> {
    self.pull_requests.recheck_at
  }
}

impl SessionFacts {
  /// The table committed with `baseline`: the pull requests are listed by the rule of a sync
  pub(super) fn github_table_committed(&mut self, baseline: Option<String>) {
    if baseline.is_some() {
      self.github.baseline = baseline;
    }
    self.ask_pull_requests(RefreshReason::Sync, None);
  }

  /// A push ended that pushed `heads`: the pull requests are listed, and the list checks the heads
  pub(super) fn pull_requests_pushed(&mut self, heads: Vec<PushedHead>) {
    self.github.pull_requests.pushed.extend(heads);
    self.ask_pull_requests(RefreshReason::Push, None);
  }

  /// A window asks to read the pull requests (`Manual`), or after a write of it (`AfterWrite`, as after a push)
  pub(super) fn refresh_github(&mut self, family: GitHubFamily, reason: FactRefreshReason, reply: Reply) {
    let reason = match (family, reason) {
      (_, FactRefreshReason::Manual) => RefreshReason::Manual,
      (GitHubFamily::PullRequests, FactRefreshReason::AfterWrite) => RefreshReason::Push,
      (GitHubFamily::Rules | GitHubFamily::Profiles, FactRefreshReason::AfterWrite) => RefreshReason::Demand,
    };
    match family {
      GitHubFamily::PullRequests => self.ask_pull_requests(reason, Some(reply)),
      GitHubFamily::Rules => self.refresh_rules(reason, reply),
      GitHubFamily::Profiles => self.ask_profiles(reason, reply),
    }
  }

  /// A list of the pull requests for `reason`, if the rules say so now; a reason that follows up asks once more after
  /// the list in flight, any other joins it. `reply` hears the version after the list that serves it
  pub(super) fn ask_pull_requests(&mut self, reason: RefreshReason, reply: Option<Reply>) {
    let now = self.now();
    let schedule = &mut self.github.pull_requests;
    if schedule.in_flight.listing {
      schedule.in_flight.join(pull_requests_follows_up(reason), reply);
      return;
    }
    let service = ServiceState {
      off_service: schedule.off_service,
      retry_at: schedule.retry_at,
    };
    if (reply.is_none() && !self.watched) || !pull_requests_due(reason, now, schedule.list, service) {
      if reason == RefreshReason::Push {
        // the list that looks at the pushed heads is not asked
        schedule.pushed.clear();
      }
      if let Some(reply) = reply {
        let _ = reply.send(None);
      }
      return;
    }
    self.start_pull_requests(reply.into_iter().collect(), false);
  }

  /// The list that looks again after a push: once, as a push asks
  pub(super) fn recheck_pull_requests(&mut self) {
    self.github.pull_requests.recheck_at = None;
    if self.github.pull_requests.in_flight.listing {
      self.github.pull_requests.in_flight.follow_up = true;
    } else if self.watched {
      self.start_pull_requests(Vec::new(), true);
    }
  }

  /// Starts a list; `recheck`: it looks again after a push, and asks no further look
  fn start_pull_requests(&mut self, waiters: Vec<Reply>, recheck: bool) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let now = self.now();
    let schedule = &mut self.github.pull_requests;
    schedule.in_flight.start(waiters);
    schedule.list.started_at = Some(now);
    let pushed = std::mem::take(&mut schedule.pushed);
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let listed = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.pull_requests(&key).await;
            // GitHub shows an old head of a pushed branch, or has not computed whether it can merge a pull request
            let shows_old_head = !recheck && (pushed.iter().any(|head| head.shows_old_head(&result)) || merge_state_computing(&result));
            let availability = Some(result.availability.clone());
            let applied = scheduler.port.apply(&key, ticket, FactResult::PullRequests(result)).await;
            let moved = applied.as_ref().map_or_default(|applied| applied.moved_remotes.clone());
            if !moved.is_empty() {
              // someone pushed to these: their fetch runs beside the listing, which answers its waiters now
              let (scheduler, key) = (scheduler.clone(), key.clone());
              tokio::spawn(async move { scheduler.sources.fetch_moved_remotes(&key, moved).await }.in_current_span());
            }
            PullRequestsListed {
              availability,
              shows_old_head,
              version: applied.as_ref().map(|applied| applied.version),
              rule_bases: applied.map_or_default(|applied| applied.rule_bases),
            }
          }
          None => PullRequestsListed {
            availability: None,
            shows_old_head: false,
            version: None,
            rule_bases: Vec::new(),
          },
        };
        let _ = inbox.send(Input::PullRequests(listed));
      }
      .in_current_span(),
    );
  }

  pub(super) fn pull_requests_listed(&mut self, listed: &PullRequestsListed) {
    let now = self.now();
    let schedule = &mut self.github.pull_requests;
    schedule.list.answered_at = Some(now);
    let follow_up = schedule.in_flight.end(listed.version);
    match &listed.availability {
      Some(PullRequestAvailability::Available { owner, name }) => {
        schedule.off_service = false;
        schedule.retry_at = None;
        // the review requests are of the same repository: it need not be resolved again
        self.review_requests.github_repository = Some(format!("{owner}/{name}"));
      }
      Some(PullRequestAvailability::NotGitHub) => schedule.off_service = true,
      // the time of the rate limit on the session clock
      Some(PullRequestAvailability::RateLimited { retry_at }) => schedule.retry_at = Some(now + Duration::from_secs(u64::from(retry_at.saturating_sub(unix_now())))),
      Some(_) | None => {}
    }
    if let Some(at) = pull_requests_push_recheck_at(now, !listed.shows_old_head) {
      schedule.recheck_at = Some(at);
    }
    if listed.availability.is_some() && self.watched {
      self.read_rules(&listed.rule_bases, RefreshReason::Sync);
    }
    if let Some(waiters) = follow_up {
      self.start_pull_requests(waiters, false);
    }
  }

  /// Reads the rules of `bases` that `reason` asks (`branch_rules_due`), those in flight aside
  fn read_rules(&mut self, bases: &[String], reason: RefreshReason) {
    let rules = &self.github.rules;
    let due: Vec<String> = bases
      .iter()
      .filter(|base| !rules.reading.contains(*base) && branch_rules_due(reason, rules.read.get(*base).copied().unwrap_or(ReadState::Never)))
      .cloned()
      .collect();
    for branch in due {
      self.read_rule(branch);
    }
  }

  fn read_rule(&mut self, branch: String) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    self.github.rules.reading.insert(branch.clone());
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let (known, version) = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.branch_rules(&key, branch.clone()).await;
            let known = result.is_ok();
            let applied = scheduler.port.apply(&key, ticket, FactResult::Rules { branch: branch.clone(), result }).await;
            (Some(known), applied.map(|applied| applied.version))
          }
          None => (None, None),
        };
        let _ = inbox.send(Input::Rules { branch, known, version });
      }
      .in_current_span(),
    );
  }

  /// A start or a form asks the rules GitHub didn't give again (`reason`); `reply` hears the version after the last of
  /// those reads, `None` at once when none runs: the read of another base (a listing's) is not waited for
  fn refresh_rules(&mut self, reason: RefreshReason, reply: Reply) {
    let failed: Vec<String> = self
      .github
      .rules
      .read
      .iter()
      .filter(|(_, read)| **read == ReadState::Failed)
      .map(|(base, _)| base.clone())
      .collect();
    self.read_rules(&failed, reason);
    let rules = &mut self.github.rules;
    let bases: BTreeSet<String> = failed.into_iter().filter(|base| rules.reading.contains(base)).collect();
    if bases.is_empty() {
      let _ = reply.send(None);
    } else {
      rules.waiters.push(RulesWaiter { bases, version: None, reply });
    }
  }

  pub(super) fn rule_read(&mut self, branch: &str, known: Option<bool>, version: Option<Version>) {
    let rules = &mut self.github.rules;
    rules.reading.remove(branch);
    if let Some(known) = known {
      rules.read.insert(branch.to_owned(), if known { ReadState::Known } else { ReadState::Failed });
    }
    for waiter in &mut rules.waiters {
      if waiter.bases.remove(branch) {
        waiter.version = version.or(waiter.version);
      }
    }
    let (answered, waiting): (Vec<RulesWaiter>, Vec<RulesWaiter>) = std::mem::take(&mut rules.waiters).into_iter().partition(|waiter| waiter.bases.is_empty());
    rules.waiters = waiting;
    for waiter in answered {
      let _ = waiter.reply.send(waiter.version);
    }
  }

  /// The merge profiles of the target of the baseline, when `profiles_due` says so or the baseline changed since the
  /// last read; a read of the same baseline in flight answers the refresh too, one of another baseline does not. `None`
  /// at once without a committed table or a read
  fn ask_profiles(&mut self, reason: RefreshReason, reply: Reply) {
    let now = self.now();
    let Some(baseline) = self.github.baseline.clone() else {
      let _ = reply.send(None);
      return;
    };
    let schedule = &mut self.github.profiles;
    if let Some(waiters) = schedule.reading.get_mut(&baseline) {
      waiters.push(reply);
      return;
    }
    if !profiles_due(reason, now, schedule.last) && schedule.read_for.as_ref() == Some(&baseline) {
      let _ = reply.send(None);
      return;
    }
    let Some(inbox) = self.inbox.upgrade() else {
      let _ = reply.send(None);
      return;
    };
    schedule.reading.insert(baseline.clone(), vec![reply]);
    schedule.last = Some(ProfilesRead { started_at: now, failed: false });
    schedule.read_for = Some(baseline.clone());
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let (failed, version) = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.profiles(&key, baseline.clone()).await;
            let failed = result.is_err();
            let applied = scheduler
              .port
              .apply(
                &key,
                ticket,
                FactResult::Profiles {
                  baseline: baseline.clone(),
                  result,
                },
              )
              .await;
            (Some(failed), applied.map(|applied| applied.version))
          }
          None => (None, None),
        };
        let _ = inbox.send(Input::Profiles { baseline, failed, version });
      }
      .in_current_span(),
    );
  }

  pub(super) fn profiles_read(&mut self, baseline: &str, failed: Option<bool>, version: Option<Version>) {
    let schedule = &mut self.github.profiles;
    let waiters = schedule.reading.remove(baseline).unwrap_or_default();
    // the last read is of the baseline asked last
    if let (Some(failed), Some(last), true) = (failed, schedule.last.as_mut(), schedule.read_for.as_deref() == Some(baseline)) {
      last.failed = failed;
    }
    for waiter in waiters {
      let _ = waiter.send(version);
    }
  }

  /// No window shows the session: the look again after a push waits for none
  pub(super) const fn github_unwatched(&mut self) {
    self.github.pull_requests.recheck_at = None;
  }
}

/// The families of this module, as a window asks them again
#[derive(Clone, Copy)]
pub(super) enum GitHubFamily {
  PullRequests,
  Rules,
  Profiles,
}
