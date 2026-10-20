use crate::landed_test::bd;
use crate::remote_fixture_test::{Space, robot};
use crate::robot_list::{IDLE_TTL_SECONDS, LIST_OVERLAP_SECONDS, RUNNING_TTL_SECONDS, RobotSource, read_robots_at};
use object_cache::cache::{CacheConfig, ObjectCache};
use patronus::client::PatronusClient;
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use sync_core::patronus::SpaceRepository;
use sync_utils::timestamp::{format_utc_timestamp, unix_now};

/// A Patronus that answers for `ultimate`, the token of the user, which the test replaces, and an object cache of its
/// own
struct Fixture {
  fake: Arc<FakePatronus>,
  client: PatronusClient,
  token: Arc<Mutex<TokenLookup>>,
  cache: ObjectCache,
  space: SpaceRepository,
  runtime: tokio::runtime::Runtime,
  _dir: tempfile::TempDir,
}

impl Fixture {
  fn new() -> Self {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let (fake, client) = runtime.block_on(start_fake());
    let dir = tempfile::tempdir().unwrap();
    Self {
      fake,
      client,
      token: Arc::new(Mutex::new(token("t"))),
      cache: ObjectCache::open_dir(dir.path(), &CacheConfig::default()),
      space: SpaceRepository {
        project_key: "ij".to_string(),
        name: "ultimate".to_string(),
      },
      runtime,
      _dir: dir,
    }
  }

  /// The robots a read at `now` gives, `id status` each, newest first; a service of its own, as each `bd` process has
  fn read(&self, now: u32) -> Vec<String> {
    let token = Arc::clone(&self.token);
    let service = PatronusService::new(self.client.clone(), move |_| token.lock().unwrap().clone());
    let source = RobotSource {
      runtime: &self.runtime,
      service: &service,
      space: &self.space,
      prefix: "test-user",
      cache: &self.cache,
    };
    read_robots_at(&source, now).iter().map(|robot| format!("{} {:?}", robot.id, robot.status)).collect()
  }

  /// The list and poll requests so far
  fn requests(&self) -> (u32, u32) {
    (self.fake.list_calls.load(Ordering::SeqCst), self.fake.update_calls.load(Ordering::SeqCst))
  }

  fn list(&self, robots: Vec<Value>) {
    *self.fake.robots.lock().unwrap() = Some(robots);
  }

  fn poll_answers(&self, robots: Vec<Value>) {
    *self.fake.updates.lock().unwrap() = Some(robots);
  }

  /// The `start` of the last list
  fn listed_since(&self) -> String {
    self.fake.last_query.lock().unwrap()["start"].clone()
  }

  fn renew_token(&self, value: &str) {
    *self.token.lock().unwrap() = token(value);
  }
}

fn token(value: &str) -> TokenLookup {
  TokenLookup::Found(Token {
    value: value.to_string(),
    stamp: TokenStamp::Env,
  })
}

/// A robot of auth started `ago` seconds before `now`
fn robot_of(id: &str, status: &str, now: u32, ago: u32) -> Value {
  robot_json(id, status, "refs/heads/test-user/virtual/auth", &format_utc_timestamp(now - ago))
}

/// The request budget of `bd status` on Patronus: a list kept a minute while a robot of it runs and five while none
/// does, then a list of the robots started since shortly before it and a poll of the running ones; the first list is of
/// the last 7 days
#[test_log::test]
fn the_robots_are_kept_while_they_hold_then_the_new_ones_are_listed_and_the_running_ones_polled() {
  let fixture = Fixture::new();
  let start = unix_now();
  fixture.list(vec![robot_of("running", "RUNNING", start, 3600), robot_of("failed", "FAILURE", start, 7200)]);
  assert_eq!(fixture.read(start), ["running Running", "failed Failure"]);
  assert_eq!(fixture.requests(), (1, 0));
  // the last 7 days, to the hour
  assert_eq!(fixture.listed_since()[..13], format_utc_timestamp(start - 7 * 86_400)[..13]);

  // kept for a minute while a robot of it runs: a status asks nothing
  assert_eq!(fixture.read(start + RUNNING_TTL_SECONDS - 1), ["running Running", "failed Failure"]);
  assert_eq!(fixture.requests(), (1, 0));

  // then the robots started since two minutes before the kept list, and a poll of the one it knows running
  fixture.list(vec![robot_of("new", "RUNNING", start, 60)]);
  fixture.poll_answers(vec![robot_of("running", "SUCCESS", start, 3600)]);
  let at = start + RUNNING_TTL_SECONDS;
  assert_eq!(fixture.read(at), ["new Running", "running Success", "failed Failure"]);
  assert_eq!(fixture.requests(), (2, 1));
  assert_eq!(fixture.listed_since(), format_utc_timestamp(start - LIST_OVERLAP_SECONDS));
  assert_eq!(
    *fixture.fake.last_updates.lock().unwrap(),
    json!({"robots": [{"id": "running", "lastKnownStatus": "RUNNING"}]})
  );

  // nothing runs once the new one finished: kept for five minutes, then a list and no poll
  fixture.list(Vec::new());
  fixture.poll_answers(vec![robot_of("new", "SUCCESS", start, 60)]);
  let finished = at + RUNNING_TTL_SECONDS;
  assert_eq!(fixture.read(finished), ["new Success", "running Success", "failed Failure"]);
  assert_eq!(fixture.requests(), (3, 2));
  assert_eq!(fixture.read(finished + IDLE_TTL_SECONDS - 1).len(), 3);
  assert_eq!(fixture.requests(), (3, 2));
  assert_eq!(fixture.read(finished + IDLE_TTL_SECONDS).len(), 3);
  assert_eq!(fixture.requests(), (4, 2));

  // another token is another key: its first list is of the last 7 days
  fixture.renew_token("renewed");
  fixture.list(vec![robot_of("failed", "FAILURE", start, 7200)]);
  assert_eq!(fixture.read(finished + IDLE_TTL_SECONDS + 1), ["failed Failure"]);
  assert_eq!(fixture.requests(), (5, 2));
}

/// A rejected token is not sent again until it changes; after a failure nothing is asked for 1 s, doubled, and the kept
/// robots stay
#[test_log::test]
fn a_rejected_token_and_a_failure_hold_the_requests_and_keep_the_robots() {
  let fixture = Fixture::new();
  let start = unix_now();
  let lists = || fixture.requests().0;
  fixture.list(vec![robot_of("merged", "SUCCESS", start, 3600)]);
  assert_eq!(fixture.read(start), ["merged Success"]);

  // Patronus fails: a GET is asked again once, then nothing for a second, then for two; the robots stay
  fixture.fake.set_status(503);
  let failed = start + IDLE_TTL_SECONDS;
  assert_eq!((fixture.read(failed), lists()), (vec!["merged Success".to_string()], 3));
  assert_eq!((fixture.read(failed), lists()), (vec!["merged Success".to_string()], 3));
  assert_eq!((fixture.read(failed + 1), lists()), (vec!["merged Success".to_string()], 5));
  assert_eq!((fixture.read(failed + 2), lists()), (vec!["merged Success".to_string()], 5));
  // the next answer ends the backoff and is kept
  fixture.fake.set_status(200);
  assert_eq!((fixture.read(failed + 3), lists()), (vec!["merged Success".to_string()], 6));
  assert_eq!((fixture.read(failed + 4), lists()), (vec!["merged Success".to_string()], 6));

  // a rejected token is sent once; the kept robots stay until a new token asks again
  fixture.fake.set_status(401);
  let rejected = failed + 3 + IDLE_TTL_SECONDS;
  assert_eq!((fixture.read(rejected), lists()), (vec!["merged Success".to_string()], 7));
  assert_eq!((fixture.read(rejected + 86_400), lists()), (vec!["merged Success".to_string()], 7));
  fixture.fake.set_status(200);
  fixture.renew_token("renewed");
  assert_eq!((fixture.read(rejected + 86_400), lists()), (vec!["merged Success".to_string()], 8));
}

/// `bd status` keeps the robots and asks Patronus once while they hold; `bd push`, which decides on them, asks and keeps
/// its list for the statuses after it; `--no-cache` asks every time
#[test_log::test]
fn status_asks_patronus_once_while_the_kept_robots_hold() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output) = bd(&space, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  space.set_robots(vec![robot("merged", "SUCCESS", "auth", &space.local_tip("test-user/virtual/auth"))]);
  let lists = || space.fake.list_calls.load(Ordering::SeqCst);
  let (_, first) = bd(&space, &["status"]);
  assert_eq!((lists(), &first["branches"][0]["landed_by"]["branch"]), (1, &json!("auth")));
  let (_, second) = bd(&space, &["status"]);
  assert_eq!((lists(), &second["branches"][0]["landed_by"]), (1, &first["branches"][0]["landed_by"]));
  // a push decides on the robots: it asks, and its list serves the next status
  let (exit_code, _) = bd(&space, &["push", "auth"]);
  assert_eq!((exit_code, lists()), (4, 2));
  bd(&space, &["status"]);
  assert_eq!(lists(), 2);
  bd(&space, &["status", "--no-cache"]);
  assert_eq!(lists(), 3);
}
