//! The robots of a repository, the user's and, whoever started them, those of the pushed branches of the user, kept in
//! the object cache (`patronus_robots`) so that `brd status` (`landed_by`) asks Patronus as the app does ("Load on
//! Patronus" in `openspec/specs/branch/patronus/spec.md`), not on every call:
//!
//! - A kept list holds for a minute while a robot of it runs (the app polls a running robot once a minute) and for five
//!   minutes while none does (a sync of the app lists at most every five minutes): only a running robot can merge.
//! - Then the robots started since two minutes before the kept list was asked are listed, and the running robots it
//!   knows are polled (`fetch-updates`: only the changed ones come back), both merged into it; the first list asks for
//!   the last 7 days, and a robot started before them drops out. A branch pushed since the kept list is asked for the
//!   last 7 days once.
//! - A token Patronus rejected is not sent again until the token file changes (another token is another key); after a
//!   failure nothing is asked for 1 s, doubled up to 10 minutes, reset by the next answer, and the kept robots stay.
//! - `brd push`, `brd safe-push` and `brd robot` decide on the robots: they ask for the full list every time (through
//!   their session, `session_facts`), of the branches that may hold them, and keep it (`keep_listed`) beside the robots
//!   of someone else on the other branches.
//!
//! Off Space, without a Space token or with `--no-cache` nothing is kept.

use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::services::patronus_service;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusRobot, RobotsAsk};
use domain_types::session::{Fact, FactState, RobotsHead};
use object_cache::cache::ObjectCache;
use patronus::service::{FULL_LIST_DAYS, PatronusService};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use sync_core::patronus::SpaceRepository;
use sync_utils::timestamp::unix_now;
use tokio::runtime::Runtime;

/// A kept list holds this long while a robot of it runs, seconds: the poll interval of the app
pub const RUNNING_TTL_SECONDS: u32 = 60;
/// A kept list holds this long while no robot of it runs, seconds: a sync of the app lists at most this often
pub const IDLE_TTL_SECONDS: u32 = 300;
/// A list asks for the robots started since this long before the kept list was asked, seconds, as the app's lists do
pub const LIST_OVERLAP_SECONDS: u32 = 120;
const BACKOFF_FIRST_SECONDS: u32 = 1;
const BACKOFF_MAX_SECONDS: u32 = 600;
const FULL_LIST_SECONDS: u32 = FULL_LIST_DAYS * 86_400;

/// What is kept of the robots of a key
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptRobots {
  /// The last answer of a list, with what the polls since changed of it; `None` before one
  pub listed: Option<ListedRobots>,
  /// When the last request was sent, Unix seconds
  pub asked_at: u32,
  /// Why nothing is asked now; `None` while the kept list decides
  pub hold: Option<RobotsHold>,
}

/// The robots a list answered
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedRobots {
  /// When the list was asked, Unix seconds: the next list asks for the robots started since shortly before
  pub at: u32,
  /// Newest first
  pub robots: Vec<PatronusRobot>,
  /// The pushed branches listed for the whole window whoever started their robots; one pushed since is asked for it
  #[serde(default)]
  pub sources: BTreeSet<String>,
}

/// Why Patronus is not asked now
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RobotsHold {
  /// It rejected the token: a renewed token is another key
  Rejected,
  /// It failed: nothing is asked for `delay_seconds` after the request
  Backoff { delay_seconds: u32 },
}

/// What a read of the robots does now (`next_request`)
#[derive(Debug)]
pub enum NextRequest {
  /// The kept answer holds
  Kept,
  /// A list of the robots `ask` (since `None`: the last 7 days), and a poll of `poll`, the running robots the kept list
  /// knows
  Ask { ask: RobotsAsk, poll: Vec<KnownRobotStatus> },
}

/// What a read at `now` asks of Patronus, given what is kept, of the pushed branches `sources`
pub fn next_request(kept: Option<&KeptRobots>, sources: &[String], now: u32) -> NextRequest {
  let full = NextRequest::Ask {
    ask: RobotsAsk::full(sources.to_vec()),
    poll: Vec::new(),
  };
  let Some(kept) = kept else {
    return full;
  };
  match kept.hold {
    Some(RobotsHold::Rejected) => return NextRequest::Kept,
    Some(RobotsHold::Backoff { delay_seconds }) if now < kept.asked_at.saturating_add(delay_seconds) => return NextRequest::Kept,
    _ => {}
  }
  let Some(listed) = &kept.listed else {
    return full;
  };
  let poll: Vec<KnownRobotStatus> = listed
    .robots
    .iter()
    .filter(|robot| !robot.status.is_finished())
    .map(|robot| KnownRobotStatus {
      id: robot.id.clone(),
      status: robot.status,
    })
    .collect();
  let ttl = if poll.is_empty() { IDLE_TTL_SECONDS } else { RUNNING_TTL_SECONDS };
  if kept.hold.is_none() && now.saturating_sub(listed.at) < ttl {
    return NextRequest::Kept;
  }
  let since = listed.at.saturating_sub(LIST_OVERLAP_SECONDS);
  if since <= now.saturating_sub(FULL_LIST_SECONDS) {
    return full;
  }
  let ask = RobotsAsk {
    since: Some(since),
    sources: sources.to_vec(),
    new_sources: sources.iter().filter(|source| !listed.sources.contains(*source)).cloned().collect(),
  };
  NextRequest::Ask { ask, poll }
}

/// The key of the robots of a repository: who asks (the server and the token, `PatronusService::identity`), the Space
/// repository and the branch prefix, which names the branches of the robots
pub fn robots_key(identity: &str, space: &SpaceRepository, prefix: &str) -> [u8; 32] {
  let mut hasher = Sha256::new();
  for part in ["patronus_robots", identity, &space.project_key, &space.name, prefix] {
    hasher.update(part.as_bytes());
    hasher.update([0]);
  }
  hasher.finalize().into()
}

/// Where the robots of a repository are asked and kept
pub struct RobotSource<'a> {
  pub runtime: &'a Runtime,
  pub service: &'a PatronusService,
  pub space: &'a SpaceRepository,
  pub prefix: &'a str,
  pub cache: &'a ObjectCache,
  /// `RepoContext::patronus_new_api`: the list names the merge profiles
  pub new_api: bool,
  /// The pushed branches of the table (`BranchTable::robot_sources`), whose robots count whoever started them
  pub sources: &'a [String],
}

/// The robots `brd status` read, newest first, with how Patronus answered for the repository, as the head of the robots
/// of a session holds it (`RobotsView::head`): the next steps read one answer
pub struct ReadRobots {
  pub robots: Vec<PatronusRobot>,
  pub head: Fact<RobotsHead>,
}

impl ReadRobots {
  /// `robots` with the answer `availability`; a failure after a list keeps the robots it listed (`Stale`), one before
  /// any is `Unavailable`
  fn answered(robots: Vec<PatronusRobot>, availability: PatronusAvailability, listed_before: bool, now: u32) -> Self {
    let state = match &availability {
      PatronusAvailability::Available | PatronusAvailability::NotPatronus => FactState::Fresh,
      failed => {
        let reason = match failed {
          PatronusAvailability::NotLoggedIn => ApiError::new(ErrorCode::NotLoggedIn, "Run ./safePush.cmd once to log in to Patronus"),
          PatronusAvailability::Error { message } => ApiError::new(ErrorCode::PatronusError, message.clone()),
          PatronusAvailability::Available | PatronusAvailability::NotPatronus => unreachable!("an answer that is no failure"),
        };
        if listed_before {
          FactState::Stale { reason: Some(reason) }
        } else {
          FactState::Unavailable { reason }
        }
      }
    };
    Self {
      robots,
      head: Fact {
        value: Some(RobotsHead {
          availability,
          github_repository: None,
        }),
        state,
        seq: 1,
        observed_at: now,
      },
    }
  }
}

/// The robots of the repository for `brd status`, newest first, the user's and those of `source.sources`: the kept ones
/// while they hold, else what Patronus answers now merged into them (`next_request`), kept; the kept ones, or none,
/// while it can't be asked
pub fn read_robots_at(source: &RobotSource<'_>, now: u32) -> ReadRobots {
  let Some(identity) = source.service.identity() else {
    return ReadRobots::answered(Vec::new(), PatronusAvailability::NotLoggedIn, false, now);
  };
  let key = robots_key(&identity, source.space, source.prefix);
  let kept: Option<KeptRobots> = source.cache.get_patronus_robots(&key);
  let (ask, poll) = match next_request(kept.as_ref(), source.sources, now) {
    NextRequest::Kept => {
      let robots = kept.and_then(|kept| kept.listed).map_or_default(|listed| listed.robots);
      return ReadRobots::answered(robots, PatronusAvailability::Available, true, now);
    }
    NextRequest::Ask { ask, poll } => (ask, poll),
  };
  let answer = source
    .runtime
    .block_on(source.service.robots(&source.space.name, source.prefix, &ask, true, source.new_api));
  let base = kept.as_ref().and_then(|kept| kept.listed.as_ref());
  let listed_before = base.is_some();
  let availability = answer.availability.clone();
  let next = match answer.availability {
    PatronusAvailability::Available => {
      let (mut robots, mut sources) = if ask.since.is_some() {
        base.map_or_default(|listed| (listed.robots.clone(), listed.sources.clone()))
      } else {
        (Vec::new(), BTreeSet::new())
      };
      sources.extend(ask.sources);
      merge_robots(&mut robots, answer.robots);
      if !poll.is_empty() {
        // a failed poll keeps what the list says
        let polled = source.runtime.block_on(source.service.poll(source.prefix, &poll));
        if polled.availability == PatronusAvailability::Available {
          merge_robots(&mut robots, polled.robots);
        }
      }
      robots.retain(|robot| robot.start_time > now.saturating_sub(FULL_LIST_SECONDS));
      robots.sort_by_key(|robot| std::cmp::Reverse(robot.start_time));
      KeptRobots {
        listed: Some(ListedRobots { at: now, robots, sources }),
        asked_at: now,
        hold: None,
      }
    }
    availability => {
      // the token was found: Patronus rejected it
      let hold = if availability == PatronusAvailability::NotLoggedIn {
        RobotsHold::Rejected
      } else {
        let delay_seconds = match kept.as_ref().and_then(|kept| kept.hold) {
          Some(RobotsHold::Backoff { delay_seconds }) => delay_seconds.saturating_mul(2).min(BACKOFF_MAX_SECONDS),
          _ => BACKOFF_FIRST_SECONDS,
        };
        RobotsHold::Backoff { delay_seconds }
      };
      tracing::debug!(?hold, "Patronus didn't list the robots");
      KeptRobots {
        listed: base.cloned(),
        asked_at: now,
        hold: Some(hold),
      }
    }
  };
  source.cache.put_patronus_robots(&key, &next);
  ReadRobots::answered(next.listed.map_or_default(|listed| listed.robots), availability, listed_before, now)
}

/// The answered robots replace the kept ones of their ids; the others are added
fn merge_robots(robots: &mut Vec<PatronusRobot>, answered: Vec<PatronusRobot>) {
  for robot in answered {
    match robots.iter_mut().find(|known| known.id == robot.id) {
      Some(known) => *known = robot,
      None => robots.push(robot),
    }
  }
}

/// The robots of the repository for `brd status`, the user's and those of the pushed branches `sources`, best effort
/// (`read_robots_at`): none without Space, a Space token or an answer of Patronus, the head saying which
pub fn read_robots(context: &RepoContext, environment: &Environment, sources: &[String]) -> ReadRobots {
  let read = || -> Result<ReadRobots, CliError> {
    let Some(space) = context.space_repository()? else {
      return Ok(ReadRobots::answered(Vec::new(), PatronusAvailability::NotPatronus, false, unix_now()));
    };
    let service = patronus_service(environment)?;
    let cache = context.cache();
    let source = RobotSource {
      runtime: context.runtime()?,
      service: &service,
      space: &space,
      prefix: &context.prefix,
      cache: &cache,
      new_api: context.patronus_new_api,
      sources,
    };
    Ok(read_robots_at(&source, unix_now()))
  };
  read().unwrap_or_else(|error| {
    tracing::debug!(error = %error.message, "no robots: Patronus is not available");
    ReadRobots::answered(Vec::new(), PatronusAvailability::Error { message: error.message }, false, unix_now())
  })
}

/// The robots a command listed to decide on (the full list of the last 7 days, asked now through its session,
/// `session_facts`): the user's and those of the pushed branches `sources` it asked, kept for `brd status` with the kept
/// robots of someone else on the other branches, which the narrower list did not ask
pub fn keep_listed(context: &RepoContext, service: &PatronusService, space: &SpaceRepository, listed: &[PatronusRobot], sources: &[String]) {
  let Some(identity) = service.identity() else {
    return;
  };
  let (cache, key, now) = (context.cache(), robots_key(&identity, space, &context.prefix), unix_now());
  let kept: Option<KeptRobots> = cache.get_patronus_robots(&key);
  let listed = merge_listed(kept.and_then(|kept| kept.listed), listed, sources, &context.prefix, now);
  let kept = KeptRobots {
    listed: Some(listed),
    asked_at: now,
    hold: None,
  };
  cache.put_patronus_robots(&key, &kept);
}

/// `listed`, the full list of the user's robots and of `sources`, over the kept list `before`: of it the robots of
/// someone else on another branch stay, and the branches it listed, while it is of the last 7 days
pub fn merge_listed(before: Option<ListedRobots>, listed: &[PatronusRobot], sources: &[String], prefix: &str, now: u32) -> ListedRobots {
  let mut robots = listed.to_vec();
  let mut kept_sources: BTreeSet<String> = sources.iter().cloned().collect();
  if let Some(before) = before.filter(|before| before.at > now.saturating_sub(FULL_LIST_SECONDS)) {
    let others = before
      .robots
      .into_iter()
      .filter(|robot| robot.is_someone_elses() && source_of(robot, prefix).is_none_or(|source| !sources.contains(&source)) && !listed.iter().any(|known| known.id == robot.id));
    robots.extend(others);
    kept_sources.extend(before.sources);
  }
  robots.sort_by_key(|robot| std::cmp::Reverse(robot.start_time));
  ListedRobots {
    at: now,
    robots,
    sources: kept_sources,
  }
}

/// The pushed branch a robot was started on, `{prefix}/virtual/<name>` or `{prefix}/stack/<root>`
fn source_of(robot: &PatronusRobot, prefix: &str) -> Option<String> {
  match (&robot.branch_name, &robot.stack_root) {
    (Some(name), _) => to_final_branch_name(prefix, name).ok(),
    (None, Some(root)) => to_stack_branch_name(prefix, root).ok(),
    (None, None) => None,
  }
}
