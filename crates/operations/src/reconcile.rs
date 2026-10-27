//! What a start whose answer was lost did (docs/design/git-operations.md, "Operations registry"; spec/branch/patronus.spec.md,
//! "A start whose answer was lost"): its record (`registry_file`, `RecordStatus::Unknown`, its `PendingStart`) holds what
//! it held for every process until a read of Patronus or GitHub tells, and is never sent again by itself.
//!
//! - Through the API: its robot is one of the user's robots on its source ref (the virtual branch, the stack branch of a
//!   whole stack) in its mode, started since it was sent (`PATRONUS_COMMENT_CLOCK_SLACK` for the clocks), on its commit
//!   when Patronus names one. It is proven absent only by a list of the user's robots since it was sent that the
//!   reconcile asks itself `SETTLED_AFTER` after the request at least (the time a start waits for its answer, and as
//!   long again): Patronus may still create the robot of a request it handles before then. A list a caller has (the
//!   session's) proves only that it started: the service may answer it from a full list it kept (`FULL_LIST_REUSE`),
//!   asked before the start settled however late it came, so without the robot the reconcile asks again.
//! - As a `/patronus` comment: its robot is one of its pull request in its mode started since; else the reply of the bot
//!   to its comment (one conditional request of its reactions) by its id, or, for a post whose answer was lost, the
//!   comment of a user with its text on the pull request since it was sent. Accepted is started (its robot comes with a
//!   later list), rejected failed, no comment found once settled failed (the post never arrived).
//!
//! `reconcile` runs at an admission that a start whose answer was lost refuses (`admit_settled`, the app's
//! `registry::Registry::admit` and `bd`), and after each list of the robots of a session (`Registry::reconcile_after_robots`):
//! settled, the record leaves the registry (`ProcessRegistry::settle`, while it is as it was read), and what it held is
//! free. Started at an admission refuses that admission as `plan_changed`: its plan was made without that robot.

use crate::land_execute::StartServices;
use crate::land_session::robots_since;
use crate::registry::OperationRequest;
use crate::registry_file::{Entry, PendingStart, ProcessRegistry, Record, Refused, busy};
use domain_policy::refresh::PATRONUS_COMMENT_CLOCK_SLACK;
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::error::{ApiError, ErrorCode, HolderTool};
use domain_types::landing::{Land, StartRoute};
use domain_types::operation::{Intent, JournalTransition, OperationId};
use domain_types::patronus::{PatronusCommandReply, PatronusCommandSubject, PatronusPushMode, PatronusRobot};
use github::service::GitHubService;
use patronus::service::PatronusService;
use remote_facts::patronus_command::{find_patronus_command, read_patronus_command_reply};
use remote_facts::service_sources::ServiceHost;
use repository_model::session::SessionKey;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tracing::{debug, instrument};

/// Seconds after its request when the robot of a start is in the list of Patronus if it started: the time a start waits
/// for its answer (`patronus::client::ACTION_TIMEOUT`, 30 s), and as long again for a request that Patronus still
/// handles after the client gave up on it
pub const SETTLED_AFTER: u32 = 60;

/// The starts whose answer was lost one admission settles before it gives up as `busy`
const SETTLE_ROUNDS: usize = 4;

/// What a start whose answer was lost did, as far as a read tells
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconciled {
  /// Its robot started: the one listed, or none yet (the bot accepted its comment)
  Started(Option<Box<PatronusRobot>>),
  /// No robot started: the record leaves, and the start can be asked again
  Failed,
  /// Not known yet: the record stays and holds what it held
  Unknown,
}

/// The mode of the start of `record`: a Safe Push unless its intent is a Dry Run
const fn mode_of(record: &Record) -> PatronusPushMode {
  match &record.intent {
    Intent::Land(Land { mode, .. }) => *mode,
    _ => PatronusPushMode::RebaseAutosquash,
  }
}

/// The robot of `start` in `mode` among `robots` of the user under `prefix`: started since it was sent, on its source ref
/// (on its commit when Patronus names one), or of its pull request for a comment; the newest of them
#[must_use]
pub fn robot_of(start: &PendingStart, mode: PatronusPushMode, prefix: &str, robots: &[PatronusRobot]) -> Option<PatronusRobot> {
  let dry_run = mode == PatronusPushMode::DryRun;
  let since = start.sent_at.saturating_sub(PATRONUS_COMMENT_CLOCK_SLACK);
  let name = start.source_ref.strip_prefix("refs/heads/").unwrap_or_default();
  let virtual_branch = strip_branch_namespace(name, prefix, BranchKind::Virtual).map(str::to_owned);
  let stack_root = strip_branch_namespace(name, prefix, BranchKind::Stack).map(str::to_owned);
  robots
    .iter()
    .filter(|robot| robot.start_time >= since && (robot.push_mode == PatronusPushMode::DryRun) == dry_run)
    .filter(|robot| match &start.route {
      StartRoute::Api { .. } => {
        let on_ref = (virtual_branch.is_some() && robot.branch_name == virtual_branch) || (stack_root.is_some() && robot.stack_root == stack_root);
        on_ref && robot.source_revision.as_ref().is_none_or(|revision| start.tip.is_empty() || *revision == start.tip)
      }
      StartRoute::Comment { number } => robot.pull_request.as_ref().is_some_and(|link| link.number == *number),
    })
    .max_by_key(|robot| robot.start_time)
    .cloned()
}

/// Whether a read asked at `read_at` (Unix seconds) comes late enough after the request of `start` to prove a robot absent
#[must_use]
pub const fn settled(start: &PendingStart, read_at: u32) -> bool {
  read_at >= start.sent_at.saturating_add(SETTLED_AFTER)
}

/// What the robots of the user since `start` was sent tell of it in `mode` under `prefix`, listed by a request at
/// `asked_at`: its robot started; through the API, a list asked once it settled without its robot proves that none
/// started; else unknown (a comment the bot accepted may start a robot Patronus counts as someone else's)
#[must_use]
pub fn by_robots(start: &PendingStart, mode: PatronusPushMode, prefix: &str, robots: &[PatronusRobot], asked_at: u32) -> Reconciled {
  if let Some(robot) = robot_of(start, mode, prefix, robots) {
    return Reconciled::Started(Some(Box::new(robot)));
  }
  match start.route {
    StartRoute::Api { .. } if settled(start, asked_at) => Reconciled::Failed,
    _ => Reconciled::Unknown,
  }
}

/// What a reconcile reads with: the Patronus of the session of `key` with its Space repository and whether it asks the
/// new API, its GitHub with the repository of the branches (none: not asked), at `now` (Unix seconds)
pub struct ReconcileSources {
  pub patronus: Option<(Arc<PatronusService>, String, bool)>,
  pub github: Option<(Arc<GitHubService>, GitHubRepository)>,
  pub key: SessionKey,
  pub now: u32,
}

impl ReconcileSources {
  /// The services `host` has for the session of `key` (the app's, the E2E server's): what it can't resolve is not asked
  pub async fn of(host: &dyn ServiceHost, key: &SessionKey, now: u32) -> Self {
    let space = host.space_repository(key).await.ok().flatten();
    let patronus = match (space, host.patronus(key)) {
      (Some(space), Ok(patronus)) => Some((patronus, space.name, host.new_api(key))),
      _ => None,
    };
    let github = match (host.github(key), host.github_repository(key).await) {
      (Ok(github), Ok(Some(repository))) => Some((github, repository)),
      _ => None,
    };
    Self {
      patronus,
      github,
      key: key.clone(),
      now,
    }
  }

  /// The services a run of `bd` runs on (`StartServices`), its Patronus asking the new API when `new_api`
  #[must_use]
  pub fn of_start(services: &StartServices, key: &SessionKey, new_api: bool, now: u32) -> Self {
    Self {
      patronus: services.patronus.clone().map(|patronus| (patronus, services.space.clone(), new_api)),
      github: services.github.clone(),
      key: key.clone(),
      now,
    }
  }
}

/// How an admission settles a start whose answer was lost in its way
pub enum SettleWith<'a> {
  /// Over the services of the session, resolved once one is in the way, at Unix seconds `.1`
  Host(&'a dyn ServiceHost, u32),
  /// Over what the caller resolved (`bd`, from the services of its run)
  Sources(ReconcileSources),
}

/// What the start of `record` did, read now: the user's robots, `listed` (a list the caller has, which proves only that
/// it started) or asked now (at `sources.now`, since it was sent: through the API when no list tells yet, or once it
/// settled, as only such a list proves it absent), then for a comment the reply of the bot. A read that fails tells
/// nothing: unknown
#[instrument(skip_all, fields(op = %record.id))]
pub async fn reconcile(record: &Record, sources: &ReconcileSources, listed: Option<&[PatronusRobot]>) -> Reconciled {
  let Some(start) = record.pending_start() else {
    // nothing was sent: nothing can run
    return Reconciled::Failed;
  };
  let (mode, prefix) = (mode_of(record), record.session.prefix.as_str());
  if let Some(robot) = listed.and_then(|robots| robot_of(start, mode, prefix, robots)) {
    return Reconciled::Started(Some(Box::new(robot)));
  }
  let api = matches!(start.route, StartRoute::Api { .. });
  if listed.is_none() || (api && settled(start, sources.now)) {
    let asked = list_robots(sources, start.sent_at.saturating_sub(PATRONUS_COMMENT_CLOCK_SLACK)).await;
    if let Some(robots) = asked {
      let told = by_robots(start, mode, prefix, &robots, sources.now);
      if told != Reconciled::Unknown || api {
        return told;
      }
    } else if api {
      return Reconciled::Unknown;
    }
  } else if api {
    // not settled yet: a list asked now could not prove it absent either
    return Reconciled::Unknown;
  }
  let StartRoute::Comment { number } = start.route else {
    return Reconciled::Unknown;
  };
  by_comment(start, number, mode, sources).await
}

/// The robots of the user since `since`, asked now; none when Patronus can't be asked
async fn list_robots(sources: &ReconcileSources, since: u32) -> Option<Vec<PatronusRobot>> {
  let (patronus, space, new_api) = sources.patronus.as_ref()?;
  robots_since(patronus, space, &sources.key.prefix, *new_api, since)
    .await
    .inspect_err(|error| debug!(error = %error.message, "the robots can't be listed: the start stays unknown"))
    .ok()
}

/// What the bot answered the comment of `start` on pull request `number`, found by its id or by its text since it was
/// sent
async fn by_comment(start: &PendingStart, number: u32, mode: PatronusPushMode, sources: &ReconcileSources) -> Reconciled {
  let Some((github, repository)) = &sources.github else {
    return Reconciled::Unknown;
  };
  let path = sources.key.worktree.as_str();
  let id = if let Some(id) = start.comment_id {
    id.to_string()
  } else {
    let since = start.sent_at.saturating_sub(PATRONUS_COMMENT_CLOCK_SLACK);
    match find_patronus_command(github, path, repository, number, mode, PatronusCommandSubject::OwnBranch, since).await {
      Ok(Some(comment)) => comment.id,
      // the post never arrived
      Ok(None) if settled(start, sources.now) => return Reconciled::Failed,
      Ok(None) | Err(_) => return Reconciled::Unknown,
    }
  };
  match read_patronus_command_reply(github, path, repository, number, &id).await {
    Ok(PatronusCommandReply::Accepted) => Reconciled::Started(None),
    Ok(PatronusCommandReply::Rejected { .. }) => Reconciled::Failed,
    Ok(PatronusCommandReply::Pending) | Err(_) => Reconciled::Unknown,
  }
}

/// How an admission went that settled the starts whose answer was lost in its way: its record, or why not; and the robot
/// of one that did start, which the caller writes into its facts
pub struct SettledAdmission {
  pub entry: Result<Entry, ApiError>,
  pub started: Option<PatronusRobot>,
}

/// Admits the operation `id` of `tool` in the session `key` that asks `request` into `registry`, as
/// `ProcessRegistry::try_admit` does, on the blocking pool; a start whose answer was lost that holds what it needs is
/// reconciled first, as `settle` says (none: never, it stays `busy`): failed, it leaves and the admission is tried again;
/// started, it leaves and the admission is refused as `plan_changed`; unknown, `busy` names it
pub async fn admit_settled(
  registry: &ProcessRegistry,
  id: &OperationId,
  tool: HolderTool,
  key: &SessionKey,
  request: &OperationRequest,
  settle: Option<SettleWith<'_>>,
) -> SettledAdmission {
  let mut settle = settle;
  let mut sources: Option<ReconcileSources> = None;
  for _ in 0..SETTLE_ROUNDS {
    let (admitting, entering, session, asked) = (registry.clone(), id.clone(), key.clone(), request.clone());
    let admitted = tokio::task::spawn_blocking(in_current_span(move || admitting.try_admit(&entering, tool, &session, &asked))).await;
    let holder = match admitted {
      Ok(Ok(entry)) => return admitted_with(Ok(entry), None),
      Ok(Err(Refused::Failed(error))) => return admitted_with(Err(error), None),
      Ok(Err(Refused::Busy(holder))) => holder,
      Err(error) => return admitted_with(Err(ApiError::internal(format!("The admission into the operations registry failed: {error}"))), None),
    };
    if holder.pending_start().is_none() || !holder.holder(0).unknown {
      return admitted_with(Err(busy(&holder, unix_now())), None);
    }
    if sources.is_none() {
      sources = match settle.take() {
        Some(SettleWith::Host(host, now)) => Some(ReconcileSources::of(host, key, now).await),
        Some(SettleWith::Sources(given)) => Some(given),
        None => None,
      };
    }
    let Some(sources) = &sources else {
      return admitted_with(Err(busy(&holder, unix_now())), None);
    };
    let reconciled = reconcile(&holder, sources, None).await;
    let transition = match &reconciled {
      Reconciled::Unknown => return admitted_with(Err(busy(&holder, sources.now)), None),
      Reconciled::Started(robot) => JournalTransition::Started { robot: robot.clone() },
      Reconciled::Failed => JournalTransition::NotStarted,
    };
    let settling = (registry.clone(), (*holder).clone());
    let settled = tokio::task::spawn_blocking(in_current_span(move || settling.0.settle(&settling.1, transition))).await;
    if let Ok(Err(error)) = settled {
      return admitted_with(Err(error), None);
    }
    if let Reconciled::Started(robot) = reconciled {
      let robot = robot.map(|robot| *robot);
      return admitted_with(Err(started_meanwhile(&holder, robot.as_ref(), sources.now)), robot);
    }
    debug!(op = %holder.id, "the start whose answer was lost started no robot: it left the registry");
  }
  admitted_with(
    Err(ApiError::new(ErrorCode::Busy, "Starts whose answer was lost hold what the operation needs: retry")),
    None,
  )
}

const fn admitted_with(entry: Result<Entry, ApiError>, started: Option<PatronusRobot>) -> SettledAdmission {
  SettledAdmission { entry, started }
}

/// The refusal of an admission whose plan did not know the robot that the start of `holder`, whose answer was lost,
/// started: "The Safe Push of auth that `bd` (pid 4242) sent 40 s ago started (robot …): decide again"
fn started_meanwhile(holder: &Record, robot: Option<&PatronusRobot>, now: u32) -> ApiError {
  let named = holder.holder(now);
  let what = match (&holder.intent, named.subject) {
    (
      Intent::Land(Land {
        mode: PatronusPushMode::DryRun, ..
      }),
      Some(subject),
    ) => format!("Dry Run of {subject}"),
    (_, Some(subject)) => format!("Safe Push of {subject}"),
    (_, None) => "start".to_string(),
  };
  let tool = match named.tool {
    HolderTool::Bd => "`bd`",
    HolderTool::App => "Branch Deck",
  };
  let sent = holder.pending_start().map_or(0, |start| now.saturating_sub(start.sent_at));
  let robot = robot.map_or_else(|| "Patronus accepted its comment".to_string(), |robot| format!("robot {}", robot.url));
  ApiError::new(
    ErrorCode::PlanChanged,
    format!(
      "The {what} that {tool} (pid {}) sent {sent} s ago, whose answer was lost, started ({robot}): decide again",
      named.pid
    ),
  )
}

#[cfg(test)]
#[path = "reconcile_test.rs"]
mod reconcile_test;
