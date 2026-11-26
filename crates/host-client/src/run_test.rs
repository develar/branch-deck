use super::*;
use crate::Timing;
use crate::testkit::{ALIAS, AgentStop, Answer, FakeProcesses, Hello, Said, Setup, calm, leak};
use git_ops::lease_slot::STATE_DIR;
use host_link::channel::{Channel, Frame};
use host_link::proto::{FileKind, Piece, is_run_id};
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use test_utils::git_test_utils::TestRepo;
use test_utils::history::HistoryCommit;

/// A checkout of two commits: the host has the first one, and the pack of a run holds the second one
struct Commits {
  repo: TestRepo,
  first: String,
  head: String,
}

fn two_commits() -> Commits {
  let repo = TestRepo::new();
  let first = repo.create_commit("Add a", "a.txt", "a\n");
  let head = repo.create_commit("Add b", "b.txt", "b\n");
  Commits { repo, first, head }
}

/// What the host has of the commits of a `Have`
type Has = dyn Fn(&[String]) -> Vec<String> + Send + Sync;

/// The commits of `asked` that are in `known`
fn known(known: Vec<String>) -> impl Fn(&[String]) -> Vec<String> + Send + Sync {
  move |asked| asked.iter().filter(|commit| known.contains(commit)).cloned().collect()
}

/// The bytes of the packs that reached the agent, with their targets
type Packed = Arc<Mutex<Vec<(PackTarget, String, Vec<u8>)>>>;

/// An agent for a whole run on `repo`. `script` answers each request first, and answers `true` when it did. Each other
/// request gets the replies of `answer_besides_follow`, with `has` for `Have`
fn agent(
  repo: TestRepo,
  has: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static,
  script: impl for<'c> Fn(&'c Request, &'c mut Channel) -> Answer<'c, bool> + Send + Sync + 'static,
) -> (Setup, Packed) {
  agent_with(repo, Hello::default(), has, script)
}

/// `agent` whose hello is `hello`
fn agent_with(
  repo: TestRepo,
  hello: Hello,
  has: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static,
  script: impl for<'c> Fn(&'c Request, &'c mut Channel) -> Answer<'c, bool> + Send + Sync + 'static,
) -> (Setup, Packed) {
  let packed = Packed::default();
  let has: Arc<Has> = Arc::new(has);
  let script = Arc::new(script);
  let setup = Setup::on(repo, hello, {
    let packed = Arc::clone(&packed);
    move |request, channel| {
      let (packed, has, script) = (Arc::clone(&packed), Arc::clone(&has), Arc::clone(&script));
      Box::pin(async move {
        if !script(request, &mut *channel).await {
          answer_besides_follow(request, channel, &packed, &*has).await;
        }
      })
    }
  });
  (setup, packed)
}

/// An agent for a whole run whose `Follow` runs `follow` with the offset of the request
fn agent_following(
  repo: TestRepo,
  has: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static,
  follow: impl for<'c> Fn(u64, &'c mut Channel) -> Answer<'c> + Send + Sync + 'static,
) -> (Setup, Packed) {
  let follow = Arc::new(follow);
  agent(repo, has, move |request, channel| {
    let follow = Arc::clone(&follow);
    Box::pin(async move {
      match request {
        Request::Follow { offset, .. } => {
          follow(*offset, channel).await;
          true
        }
        _ => false,
      }
    })
  })
}

/// `agent_following` of a checkout of two commits whose HEAD the host has
fn following_head(follow: impl for<'c> Fn(u64, &'c mut Channel) -> Answer<'c> + Send + Sync + 'static) -> Setup {
  let Commits { repo, head, .. } = two_commits();
  agent_following(repo, known(vec![head]), follow).0
}

/// The replies of an agent for a whole run, besides `Follow`
async fn answer_besides_follow(request: &Request, channel: &mut Channel, packed: &Mutex<Vec<(PackTarget, String, Vec<u8>)>>, has: &Has) {
  let _ = match request {
    Request::Have { commits, .. } => channel.send(&Reply::Have { commits: has(commits) }).await,
    Request::Pack { target, commit, .. } => {
      let mut bytes = Vec::new();
      let _ = channel.recv_stream(&mut bytes).await;
      packed.lock().unwrap_or_else(PoisonError::into_inner).push((target.clone(), commit.clone(), bytes));
      channel.send(&Reply::Packed).await
    }
    Request::Start(spec) => channel.send(&Reply::Started { run: spec.run.clone() }).await,
    Request::Fetch { .. } => {
      send_kind(channel, "out/bt-runs/0000000002000/test.log", b"hello", FileKind::Staged).await;
      channel.send(&Reply::Fetched { files: 1, bytes: 5 }).await
    }
    Request::Forget { .. } | Request::Cancel { .. } => channel.send(&Reply::Done { message: "removed".to_owned() }).await,
    _ => channel.send(&Reply::refused(Code::BadRequest, "not in this script")).await,
  };
}

fn command() -> Vec<String> {
  ["./community/tools/bt.cmd", "FooTest", "--json"].map(str::to_owned).to_vec()
}

fn fetch() -> Vec<String> {
  vec!["out/bt-runs".to_owned()]
}

/// The affinity of the runs of the tests
const AFFINITY: &str = "checkout-0123456789ab";

/// Runs the command of the tests in the checkout of `setup` with `fetch`, `options` and the short deadlines
#[expect(clippy::result_large_err, reason = "the answer of `Client::run`")]
async fn run_in(setup: &Setup, say: &(dyn Fn(&str) + Sync), presses: &(dyn Fn() -> u32 + Sync), fetch: &[String], options: RunOptions) -> Result<RunOutcome, RunFailure> {
  run_holding(setup, say, presses, fetch, options, Duration::ZERO).await
}

/// `run_in` without Ctrl+C or fetch directories, and the lines that it said
async fn run_saying(setup: &Setup, options: RunOptions) -> (Result<RunOutcome, RunFailure>, Vec<String>) {
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let ran = run_in(setup, &say, &calm, &[], options).await;
  (ran, said.lines())
}

/// `run_in` whose build of the pack waits `hold` before git starts
#[expect(clippy::result_large_err, reason = "the answer of `Client::run`")]
async fn run_holding(
  setup: &Setup,
  say: &(dyn Fn(&str) + Sync),
  presses: &(dyn Fn() -> u32 + Sync),
  fetch: &[String],
  options: RunOptions,
  hold: Duration,
) -> Result<RunOutcome, RunFailure> {
  let processes = FakeProcesses::default();
  run_by(&setup.client(&processes, say, presses).with_hold_pack(hold), setup, fetch, options).await
}

/// `run_in` without outputs, Ctrl+C or options, with the deadlines of `timing`
#[expect(clippy::result_large_err, reason = "the answer of `Client::run`")]
async fn run_timed(setup: &Setup, say: &(dyn Fn(&str) + Sync), timing: Timing) -> Result<RunOutcome, RunFailure> {
  let processes = FakeProcesses::default();
  run_by(&setup.client(&processes, say, &calm).with_timing(timing), setup, &[], RunOptions::default()).await
}

/// Runs the command of the tests with `client` in the checkout of `setup`
#[expect(clippy::result_large_err, reason = "the answer of `Client::run`")]
async fn run_by(client: &Client<'_>, setup: &Setup, fetch: &[String], options: RunOptions) -> Result<RunOutcome, RunFailure> {
  let command = command();
  let request = RunRequest {
    checkout: setup.repo.path(),
    repo: None,
    affinity: AFFINITY,
    command: &command,
    fetch,
    measure_as: None,
    group: None,
  };
  client.run(ALIAS, &request, options).await
}

fn kind(request: &Request) -> &'static str {
  match request {
    Request::Hello { .. } => "hello",
    Request::Have { .. } => "have",
    Request::Pack { .. } => "pack",
    Request::Start(_) => "start",
    Request::Follow { .. } => "follow",
    Request::Cancel { .. } => "cancel",
    Request::Fetch { .. } => "fetch",
    Request::Forget { .. } => "forget",
    Request::Runs { .. } => "runs",
    Request::Slots => "slots",
    Request::Disk { .. } => "disk",
    Request::Trim { .. } => "trim",
    Request::Stats { .. } => "stats",
    Request::Upgrade { .. } => "upgrade",
    Request::Estimate(_) => "estimate",
  }
}

/// The kinds of the requests that reached the agent, in their order
fn kinds(setup: &Setup) -> Vec<&'static str> {
  setup.agent.requests().iter().map(kind).collect()
}

/// The candidates of each `Have` that reached the agent, with its repository
fn haves(setup: &Setup) -> Vec<(String, Vec<String>)> {
  setup
    .agent
    .requests()
    .into_iter()
    .filter_map(|request| match request {
      Request::Have { repo, commits } => Some((repo, commits)),
      _ => None,
    })
    .collect()
}

/// The start of the run that reached the agent
fn start_spec(setup: &Setup) -> RunSpec {
  setup
    .agent
    .requests()
    .into_iter()
    .find_map(|request| match request {
      Request::Start(spec) => Some(spec),
      _ => None,
    })
    .unwrap()
}

/// Waits until `flag` is set, as a command of the host that waits for an event, up to a minute of the paused clock,
/// which no run reaches
async fn wait_for(flag: &AtomicBool) {
  let started = Instant::now();
  while !flag.load(Ordering::SeqCst) && started.elapsed() < Duration::from_mins(1) {
    sleep(Duration::from_millis(20)).await;
  }
}

async fn exit(channel: &mut Channel, code: i32) {
  let _ = channel.send(&Reply::Exit { code }).await;
}

/// The answer of a script to a follow whose run exits with `code`
async fn exited(channel: &mut Channel, code: i32) -> bool {
  exit(channel, code).await;
  true
}

async fn done(channel: &mut Channel, message: &str) {
  let _ = channel.send(&Reply::Done { message: message.to_owned() }).await;
}

fn step(name: &str, ms: u64) -> Phase {
  Phase { name: name.to_owned(), ms }
}

/// Sends the line `text` of `stream`
async fn output(channel: &mut Channel, stream: Stream, text: &str, end: u64) {
  send_piece(channel, stream, Piece::of(text.as_bytes(), false), end).await;
}

async fn send_piece(channel: &mut Channel, stream: Stream, piece: Piece, end: u64) {
  let _ = channel.send(&Reply::Output { stream, piece, end }).await;
}

async fn send_kind(channel: &mut Channel, path: &str, bytes: &[u8], kind: FileKind) {
  let _ = channel
    .send(&Reply::File {
      path: path.to_owned(),
      size: bytes.len() as u64,
      executable: false,
      kind,
    })
    .await;
  let _ = channel.send_bytes(bytes).await;
  let _ = channel.send_end().await;
}

/// Raises the Ctrl+C count to `presses` once the agent got a request of the kind `after`, as a person who presses
/// after the start. A task of the test runtime looks for it, so the press comes at its moment of the paused clock; it
/// gives up after a minute of that clock, which no run reaches
fn press_after(setup: &Setup, after: &'static str, presses: u32) -> &'static AtomicU32 {
  let count = leak(AtomicU32::new(0));
  let agent = setup.agent.requests_handle();
  tokio::spawn(async move {
    let started = Instant::now();
    while started.elapsed() < Duration::from_mins(1) {
      if agent.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|request| kind(request) == after) {
        count.store(presses, Ordering::SeqCst);
        return;
      }
      sleep(Duration::from_millis(1)).await;
    }
  });
  count
}

/// The run id of an outcome
fn run_of(outcome: &RunOutcome) -> &str {
  outcome.facts.run.as_deref().unwrap()
}

/// The order of a run: `Have` and `Start` first, then the follow beside the pack, then the base sync of
/// `origin/master`, the fetch and the forget. The output streams, the steps of the client wrap the steps of the host,
/// and the record names what the fetch takes
#[tokio::test(start_paused = true)]
async fn a_run_starts_follows_packs_syncs_the_base_and_fetches_in_order() {
  let Commits { repo, first, head } = two_commits_with_a_base();
  let base = repo.rev_parse("refs/remotes/origin/master").unwrap();
  let based = leak(AtomicBool::new(false));
  let (setup, packed) = agent(repo, known(vec![first.clone()]), move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Follow { .. } => {
          output(channel, Stream::Stderr, "progress", 30).await;
          let _ = channel.send(&Reply::Heartbeat { elapsed_ms: 15_000 }).await;
          output(channel, Stream::Stdout, "the digest", 60).await;
          // A long command: it ends after the base came
          wait_for(based).await;
          let _ = channel
            .send(&Reply::Phases {
              phases: vec![step("slot", 0), step("command", 25_100)],
            })
            .await;
          exit(channel, 3).await;
          true
        }
        Request::Pack { target: PackTarget::Base(_), .. } => {
          let _ = channel.recv_stream(&mut Vec::new()).await;
          based.store(true, Ordering::SeqCst);
          let _ = channel.send(&Reply::Packed).await;
          true
        }
        _ => false,
      }
    })
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &fetch(), RunOptions::default()).await.unwrap();

  assert_eq!((outcome.code, outcome.stdout.as_slice()), (3, b"the digest\n".as_slice()));
  assert_eq!((outcome.facts.commit.as_deref(), outcome.facts.head.as_deref()), (Some(head.as_str()), Some(head.as_str())));
  assert_eq!(outcome.facts.repository.as_deref(), Some("~/projects/idea-1"));
  assert_eq!(outcome.facts.fetched, ["out/bt-runs/0000000002000/test.log"]);
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("out/bt-runs/0000000002000/test.log")).unwrap(), "hello");

  let mut kinds = kinds(&setup);
  assert_eq!(kinds[..2], ["have", "start"]);
  kinds.sort_unstable();
  assert_eq!(kinds, ["fetch", "follow", "forget", "have", "have", "pack", "pack", "start"]);
  let haves = haves(&setup);
  assert_eq!(
    haves[0],
    ("~/projects/idea-1".to_owned(), vec![head.clone(), first]),
    "HEAD, its parent and the merge base, once each"
  );
  assert!(haves[1].1.contains(&base), "the base sync asks about the base: {haves:?}");
  let spec = start_spec(&setup);
  assert_eq!(spec.run, run_of(&outcome));
  assert_eq!((spec.affinity.as_str(), spec.commit.as_str()), (AFFINITY, head.as_str()));
  assert_eq!((spec.command.clone(), spec.fetch.clone()), (command(), fetch()));
  assert!(is_run_id(run_of(&outcome)), "{}", run_of(&outcome));

  let packs = packed.lock().unwrap_or_else(PoisonError::into_inner).clone();
  assert_eq!(packs.len(), 1, "the base pack has its own answer");
  assert_eq!((packs[0].0.clone(), packs[0].1.as_str()), (PackTarget::Run(spec.run), head.as_str()));
  assert!(packs[0].2.starts_with(b"PACK"), "a git pack");
  assert_eq!(setup.config.base(ALIAS, AFFINITY), Some(base));
  assert!(
    setup
      .agent
      .requests()
      .iter()
      .any(|request| matches!(request, Request::Pack { target: PackTarget::Base(name), .. } if name == AFFINITY)),
    "the base goes to the base of the affinity"
  );

  let lines = said.lines();
  assert!(lines.contains(&"progress".to_owned()), "{lines:#?}");
  assert!(lines.contains(&format!("brd-host: run {} of {} on studio", run_of(&outcome), &head[..12])), "{lines:#?}");
  assert!(lines.contains(&"> studio: pack the commits that studio lacks".to_owned()), "{lines:#?}");
  assert!(lines.iter().any(|line| line.starts_with("> studio: pack of 1 commits, ")), "{lines:#?}");
  assert!(!lines.iter().any(|line| line.contains("warning")), "{lines:#?}");
  assert_eq!(
    outcome.facts.phases.iter().map(|phase| phase.name.as_str()).collect::<Vec<_>>(),
    ["pack", "upload", "slot", "command", "fetch", "total"],
    "the steps of the client wrap the steps of the host"
  );
  assert_eq!(outcome.facts.phases[3].ms, 25_100);
  let record: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(setup.repo.path().join(format!(".git/{STATE_DIR}/host/runs/{}.json", run_of(&outcome)))).unwrap()).unwrap();
  assert_eq!(record, serde_json::json!({"commit": head, "fetch": ["out/bt-runs"], "outputs": [], "changes": false}));
}

/// The working tree ships as one commit of `brd-host` on HEAD, and the index stays. The same tree is the same commit,
/// so a host that has it gets no pack
#[tokio::test(start_paused = true)]
async fn the_working_tree_ships_as_one_commit_and_the_same_tree_sends_no_pack() {
  let Commits { repo, head, .. } = two_commits();
  std::fs::write(repo.path().join("a.txt"), "a\nchanged\n").unwrap();
  std::fs::write(repo.path().join("new.txt"), "new\n").unwrap();
  let index = std::fs::read(repo.path().join(".git/index")).unwrap();
  let has = Arc::new(Mutex::new(vec![head.clone()]));
  let (setup, packed) = agent(
    repo,
    {
      let has = Arc::clone(&has);
      move |asked| known(has.lock().unwrap_or_else(PoisonError::into_inner).clone())(asked)
    },
    |request, channel| {
      Box::pin(async move {
        match request {
          Request::Follow { .. } => exited(channel, 0).await,
          _ => false,
        }
      })
    },
  );
  let options = RunOptions {
    working_tree: true,
    ..RunOptions::default()
  };
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let first = run_in(&setup, &say, &calm, &[], options).await.unwrap();
  let commit = first.facts.commit.clone().unwrap();
  assert_ne!(commit, head);
  assert!(first.facts.working_tree);
  assert_eq!(
    setup.repo.git(&["log", "-1", "--format=%P|%an <%ae>", &commit]),
    format!("{head}|bd-host <bd-host@bd-host.invalid>")
  );
  assert_eq!(std::fs::read(setup.repo.path().join(".git/index")).unwrap(), index, "the index stays");
  assert!(
    said.lines().contains(&format!(
      "brd-host: run {} of {} on studio: the working tree, HEAD {} and 2 changed paths",
      run_of(&first),
      &commit[..12],
      &head[..12]
    )),
    "{:#?}",
    said.lines()
  );
  assert_eq!(packed.lock().unwrap_or_else(PoisonError::into_inner).len(), 1);

  has.lock().unwrap_or_else(PoisonError::into_inner).push(commit.clone());
  let second = run_in(&setup, &|_| {}, &calm, &[], options).await.unwrap();
  assert_eq!(second.facts.commit.as_deref(), Some(commit.as_str()), "the same tree on the same HEAD is the same commit");
  assert_eq!(packed.lock().unwrap_or_else(PoisonError::into_inner).len(), 1, "the second run sends no pack");
}

/// `--head` refuses a checkout with local changes. The refusal stops the `Have` that goes on meanwhile, so it comes at
/// once even from a host that does not answer, and nothing starts. The host answers no request until the test ends: a
/// refusal that waited for the `Have` would be `timed_out`
#[tokio::test(start_paused = true)]
async fn head_refuses_a_checkout_with_local_changes() {
  let Commits { repo, .. } = two_commits();
  std::fs::write(repo.path().join("a.txt"), "a\nchanged\n").unwrap();
  let ended = leak(AtomicBool::new(false));
  let setup = Setup::on(repo, Hello::default(), move |_, _| Box::pin(wait_for(ended)));
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap_err();
  ended.store(true, Ordering::SeqCst);
  assert_eq!((failure.refusal.code, failure.exit), (Code::LocalChanges, Exit::NotRun), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains(" M a.txt"), "{}", failure.refusal.message);
  assert!(kinds(&setup).iter().all(|kind| *kind == "have"), "{:?}", kinds(&setup));
}

/// A host without a near commit gets the far candidates of the first-parent walk, and the pack builds on the commit
/// that it has from there. A host that has none of them refuses with `behind` and the count of the history
#[tokio::test(start_paused = true)]
async fn a_host_without_a_near_commit_gets_the_far_candidates_then_refuses_behind() {
  let commits: Vec<HistoryCommit> = (0..40).map(|n| HistoryCommit::new(format!("Commit {n}"), &[("n.txt", &format!("{n}\n"))])).collect();
  let (repo, ids) = TestRepo::with_history(&commits);
  let (setup, packed) = agent_following(repo, known(ids[..=7].to_vec()), |_, channel| Box::pin(exit(channel, 0)));
  run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap();
  let haves = haves(&setup);
  assert_eq!(haves.len(), 2, "{haves:#?}");
  assert_eq!(haves[0].1, [39, 38, 37, 35, 31, 23].map(|n| ids[n].clone()), "the distances up to 16");
  assert_eq!(haves[1].1, [39, 38, 37, 35, 31, 23, 7].map(|n| ids[n].clone()), "the distances up to 65536");
  assert_eq!(packed.lock().unwrap_or_else(PoisonError::into_inner).len(), 1);

  let (repo, ids) = TestRepo::with_history(&commits);
  let (setup, _) = agent(repo, |_| Vec::new(), |_, _| Box::pin(async { false }));
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Behind, Exit::NotRun));
  assert!(failure.refusal.message.contains("the whole history of 40 commits"), "{}", failure.refusal.message);
  assert_eq!(kinds(&setup), ["have", "have"], "the near candidates, then the long walk; no start");
  assert_eq!(failure.facts.commit.as_deref(), Some(ids[39].as_str()));
}

/// The failure of a run whose pack `on_pack` of the agent breaks. The agent knows the first commit, takes the cancel,
/// and sends nothing to a follow. Checks what every failed pack does: the client cancels the started run, says that the
/// cancel came, and sends no other request
async fn failed_pack(commits: Commits, on_pack: impl for<'c> Fn(&'c mut Channel) -> Answer<'c> + Send + Sync + 'static) -> RunFailure {
  let Commits { repo, first, .. } = commits;
  let on_pack = Arc::new(on_pack);
  let (setup, _) = agent(repo, known(vec![first]), move |request, channel| {
    let on_pack = Arc::clone(&on_pack);
    Box::pin(async move {
      match request {
        Request::Pack { .. } => {
          on_pack(channel).await;
          true
        }
        Request::Cancel { .. } => {
          done(channel, "the host has the cancel").await;
          true
        }
        Request::Follow { .. } => true,
        _ => false,
      }
    })
  });
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap_err();
  assert_eq!(failure.exit, Exit::NotRun);
  assert!(failure.refusal.message.starts_with("the pack did not reach studio: "), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("; studio has the cancel of the run "), "{}", failure.refusal.message);
  // The follow starts with the run, beside the pack, so its requests come in any order
  let kinds: Vec<&str> = kinds(&setup).into_iter().filter(|kind| *kind != "follow").collect();
  assert_eq!(kinds, ["have", "start", "pack", "cancel"]);
  failure
}

/// A pack that the agent refuses cancels the started run, stops the follow, and refuses with the code of the pack;
/// the checkout of the run waits for the pack, so the command did not run
#[tokio::test(start_paused = true)]
async fn a_failed_pack_cancels_the_run_and_refuses_with_its_code() {
  let failure = failed_pack(two_commits(), |channel| {
    Box::pin(async move {
      let _ = channel.recv_stream(&mut Vec::new()).await;
      let _ = channel.send(&Reply::refused(Code::PackFailed, "git index-pack failed")).await;
    })
  })
  .await;
  assert_eq!(failure.refusal.code, Code::PackFailed);
  assert!(
    failure.refusal.message.starts_with("the pack did not reach studio: studio: git index-pack failed"),
    "{}",
    failure.refusal.message
  );
}

/// The run goes to the repository of the same name as the local one beside the repository of the host file, and to
/// the one of the host file when the host lacks it
#[tokio::test(start_paused = true)]
async fn the_repository_of_a_run_is_the_one_of_the_same_name_beside_the_host_file() {
  let by_name = RunOptions {
    repo_by_name: true,
    ..RunOptions::default()
  };
  let Commits { repo, head, .. } = two_commits();
  let name = repo.path().file_name().unwrap().to_string_lossy().into_owned();
  let sibling = format!("~/projects/{name}");
  let (setup, _) = agent_following(repo, known(vec![head]), |_, channel| Box::pin(exit(channel, 0)));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], by_name).await.unwrap();
  assert_eq!(start_spec(&setup).repo, sibling);
  assert_eq!(outcome.facts.repository.as_deref(), Some(sibling.as_str()));
  assert!(said.lines().iter().any(|line| line.ends_with(&format!("on studio in {sibling}"))), "{:#?}", said.lines());

  let Commits { repo, head, .. } = two_commits();
  let missing: &'static str = leak(format!("~/projects/{}", repo.path().file_name().unwrap().to_string_lossy()));
  let (setup, _) = agent(repo, known(vec![head]), move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Have { repo, .. } if repo == missing => {
          let _ = channel.send(&Reply::refused(Code::RepoMissing, "no git repository there")).await;
          true
        }
        Request::Follow { .. } => exited(channel, 0).await,
        _ => false,
      }
    })
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  run_in(&setup, &say, &calm, &[], by_name).await.unwrap();
  assert_eq!(start_spec(&setup).repo, "~/projects/idea-1");
  assert!(
    said
      .lines()
      .iter()
      .any(|line| line.starts_with("brd-host: studio has no ~/projects/") && line.contains("the run goes to ~/projects/idea-1")),
    "{:#?}",
    said.lines()
  );
}

/// A lost connection follows again from the last offset, and a record that came twice shows once. A run that vanished
/// on the host after its start may have run its command, so it exits 255, and the client forgets nothing
#[tokio::test(start_paused = true)]
async fn a_lost_follow_connects_again_from_its_offset_and_an_unknown_run_is_255() {
  let setup = following_head(|offset, channel| {
    Box::pin(async move {
      output(channel, Stream::Stderr, "first", 10).await;
      if offset > 0 {
        output(channel, Stream::Stdout, "done", 20).await;
        exit(channel, 0).await;
      }
    })
  });
  let (ran, lines) = run_saying(&setup, RunOptions::default()).await;
  let outcome = ran.unwrap();
  assert_eq!(outcome.stdout, b"done\n");
  assert_eq!(lines.iter().filter(|line| *line == "first").count(), 1, "{lines:#?}");
  assert!(
    lines
      .iter()
      .any(|line| line.contains(&format!("following the run {} again from byte 10", run_of(&outcome)))),
    "{lines:#?}"
  );
  let follows: Vec<u64> = setup
    .agent
    .requests()
    .iter()
    .filter_map(|request| match request {
      Request::Follow { offset, .. } => Some(*offset),
      _ => None,
    })
    .collect();
  assert_eq!(follows, [0, 10]);

  let setup = following_head(|_, channel| {
    Box::pin(async move {
      output(channel, Stream::Stderr, "building", 10).await;
      let _ = channel.send(&Reply::refused(Code::UnknownRun, "the agent keeps no run")).await;
    })
  });
  let failure = run_in(&setup, &|_| {}, &calm, &fetch(), RunOptions::default()).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::UnknownRun, Exit::Refused));
  assert!(failure.refusal.message.contains("whether its command ran is unknown"), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("`brd-host runs studio`"), "{}", failure.refusal.message);
  assert!(!kinds(&setup).contains(&"forget"), "{:?}", kinds(&setup));
}

/// An agent whose `Follow` sends the pieces of two lines of each stream: the first connection breaks inside both lines,
/// and the second one goes on from the offset of the last piece
fn agent_with_pieces() -> Setup {
  let Commits { repo, head, .. } = two_commits();
  agent_following(repo, known(vec![head]), |offset, channel| {
    Box::pin(async move {
      if offset == 0 {
        send_piece(channel, Stream::Stdout, Piece::of(b"a long ", true), 10).await;
        send_piece(channel, Stream::Stderr, Piece::of(b"warn", true), 20).await;
        return;
      }
      send_piece(channel, Stream::Stdout, Piece::of(b"line\r", false), 30).await;
      send_piece(channel, Stream::Stderr, Piece::of(b"ing", false), 40).await;
      send_piece(channel, Stream::Stdout, Piece::of(b"\xff\xfe", false), 50).await;
      send_piece(channel, Stream::Stderr, Piece::of(b"no end", true), 60).await;
      send_piece(channel, Stream::Stdout, Piece::of(b"last", true), 70).await;
      exit(channel, 0).await;
    })
  })
  .0
}

/// The sink of the output gets the bytes of each stream as the command wrote them: no line end after a partial piece,
/// none between the pieces of a line that a lost connection cut, and the bytes of a piece that is not UTF-8
#[tokio::test(start_paused = true)]
async fn the_output_sink_gets_the_bytes_of_each_stream() {
  let setup = agent_with_pieces();
  let got: Mutex<(Vec<u8>, Vec<u8>)> = Mutex::default();
  let output = |stream: Stream, bytes: &[u8]| {
    let mut got = got.lock().unwrap_or_else(PoisonError::into_inner);
    match stream {
      Stream::Stdout => got.0.extend_from_slice(bytes),
      Stream::Stderr => got.1.extend_from_slice(bytes),
    }
  };
  let processes = FakeProcesses::default();
  let quiet = |_: &str| {};
  let client = setup.client(&processes, &quiet, &calm).with_output(&output);
  let outcome = run_by(&client, &setup, &[], RunOptions::default()).await.unwrap();
  assert_eq!(outcome.stdout, b"", "the sink has the stdout");
  let (stdout, stderr) = got.into_inner().unwrap_or_else(PoisonError::into_inner);
  assert_eq!(stdout, b"a long line\r\n\xff\xfe\nlast");
  assert_eq!(stderr, b"warning\nno end");
}

/// Without an output sink the stdout collects as bytes, and `say` gets each stderr line: a line without its end before
/// the next line of the client, and at the end of the follow
#[tokio::test(start_paused = true)]
async fn without_a_sink_the_stdout_collects_and_say_gets_each_stderr_line() {
  let setup = agent_with_pieces();
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], RunOptions::default()).await.unwrap();
  assert_eq!(outcome.stdout, b"a long line\r\n\xff\xfe\nlast");
  let lines = said.lines();
  let at = |line: &str| lines.iter().position(|said| said == line).unwrap_or_else(|| panic!("{line:?} in {lines:#?}"));
  let again = lines.iter().position(|line| line.contains("following the run")).unwrap_or_else(|| panic!("{lines:#?}"));
  assert!(at("warn") < again && again < at("ing"), "the line of the client comes after the open line: {lines:#?}");
  assert!(at("ing") < at("no end"), "{lines:#?}");
}

/// A piece whose base64 does not decode is an error of the protocol: the run refuses with `link`, exit 255 after the
/// start
#[tokio::test(start_paused = true)]
async fn a_piece_whose_base64_does_not_decode_refuses_with_link() {
  let setup = following_head(|_, channel| {
    Box::pin(async move {
      let piece = Piece {
        text: "x".to_owned(),
        base64: Some("not base64!".to_owned()),
        ..Piece::default()
      };
      send_piece(channel, Stream::Stdout, piece, 10).await;
      exit(channel, 0).await;
    })
  });
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Link, Exit::Refused), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("base64 does not decode"), "{}", failure.refusal.message);
}

/// An agent that ends while the client follows its run refuses each new connection. The command can still run on the
/// host, so after the reconnect window the run ends with `timed_out` and exit 255, never with the 75 of `unreachable`
#[tokio::test(start_paused = true)]
async fn an_agent_that_ends_during_the_follow_times_out_with_255_and_not_75() {
  let Commits { repo, head, .. } = two_commits();
  let stopper: &'static OnceLock<AgentStop> = leak(OnceLock::new());
  let (setup, _) = agent_following(repo, known(vec![head]), move |_, channel| {
    Box::pin(async move {
      output(channel, Stream::Stderr, "building", 10).await;
      stopper.get().expect("the stop of the agent").stop();
    })
  });
  assert!(stopper.set(setup.agent.stopper()).is_ok());
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let timing = Timing {
    reconnect_for: Duration::from_millis(300),
    ..crate::testkit::UNHURRIED
  };
  let failure = run_timed(&setup, &say, timing).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::TimedOut, Exit::Refused), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("`brd-host follow studio "), "{}", failure.refusal.message);
  assert!(said.lines().iter().any(|line| line.contains("following the run")), "{:#?}", said.lines());
  assert!(!kinds(&setup).contains(&"forget"), "a run whose end the client did not see stays on the host");
}

/// A refusal of the client after the start, such as `untrusted` from a new connection, exits 255 as `link` and keeps
/// its code in the message. A second Ctrl+C keeps its own code
#[test]
fn a_refusal_of_the_client_after_the_start_never_exits_75() {
  let untrusted = unseen_after_start(Refusal::new(Code::Untrusted, "another certificate"));
  assert_eq!(
    (untrusted.code, untrusted.code.exit(Reached::Started), untrusted.message.as_str()),
    (Code::Link, Exit::Refused, "untrusted: another certificate")
  );
  let timed_out = unseen_after_start(Refusal::new(Code::TimedOut, "no reply"));
  assert_eq!((timed_out.code, timed_out.message.as_str()), (Code::TimedOut, "no reply"));
  assert_eq!(unseen_after_start(Refusal::new(Code::Interrupted, "a second Ctrl+C")).code, Code::Interrupted);
}

/// An agent that stops answering ends the run after the reconnect window with `timed_out` after the start (255), and
/// the refusal names `brd-host follow`. Each `Follow` of the agent stays silent until the run has ended; the read
/// deadline ends each connection, and the second one ends past the reconnect window. On paused time each deadline
/// passes exactly
#[tokio::test(start_paused = true)]
async fn a_silent_agent_times_out_after_the_reconnect_window() {
  let Commits { repo, head, .. } = two_commits();
  let ended = leak(AtomicBool::new(false));
  let (setup, _) = agent_following(repo, known(vec![head]), move |_, _| Box::pin(wait_for(ended)));
  let timing = Timing {
    read: Duration::from_millis(250),
    reconnect_for: Duration::from_millis(500),
    ..crate::testkit::UNHURRIED
  };
  let failure = run_timed(&setup, &|_| {}, timing).await.unwrap_err();
  ended.store(true, Ordering::SeqCst);
  assert_eq!((failure.refusal.code, failure.exit), (Code::TimedOut, Exit::Refused));
  assert!(failure.refusal.message.contains("`brd-host follow studio "), "{}", failure.refusal.message);
  assert!(!kinds(&setup).contains(&"forget"), "a run whose end the client did not see stays on the host");
}

const PATCH: &str = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+a formatted\n";

/// An agent of a run on `repo` whose host has every commit, whose command exits 0, and whose fetch sends `files`
fn fetching_agent(repo: TestRepo, files: Vec<(String, FileKind, Vec<u8>)>) -> Setup {
  fetching_agent_with(repo, Hello::default(), files)
}

/// `fetching_agent` whose hello is `hello`
fn fetching_agent_with(repo: TestRepo, hello: Hello, files: Vec<(String, FileKind, Vec<u8>)>) -> Setup {
  let files: &'static [(String, FileKind, Vec<u8>)] = leak(files);
  agent_with(repo, hello, <[String]>::to_vec, move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Follow { .. } => exited(channel, 0).await,
        Request::Fetch { .. } => {
          for (path, kind, bytes) in files {
            send_kind(channel, path, bytes, kind.clone()).await;
          }
          let _ = channel.send(&Reply::Fetched { files: 0, bytes: 0 }).await;
          true
        }
        _ => false,
      }
    })
  })
  .0
}

/// The options of `brd-host run --apply`: the working tree, so a second run ships the changes of the first one
const APPLY: RunOptions = RunOptions {
  working_tree: true,
  nice: None,
  outputs: false,
  apply: true,
  repo_by_name: false,
  heavy: false,
  exclusive: false,
  max: None,
  admit_now: false,
};

/// The patch of the changes of a command applies to the files of the working tree, not to the index, and stays in the
/// git directory. A patch that does not apply refuses with `apply_failed` after the command (255), names the patch,
/// and the working tree stays as it was
#[tokio::test(start_paused = true)]
async fn apply_brings_the_changes_back_all_or_nothing() {
  let changes = || ("changes.patch".to_owned(), FileKind::Changes, PATCH.as_bytes().to_vec());
  let Commits { repo, .. } = two_commits();
  let setup = fetching_agent(repo, vec![changes()]);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], APPLY).await.unwrap();
  assert_eq!(outcome.facts.applied_files, Some(1));
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("a.txt")).unwrap(), "a formatted\n");
  assert_eq!(setup.repo.git(&["diff", "--cached", "--name-only"]), "", "the index stays");
  let root = std::fs::canonicalize(setup.repo.path()).unwrap();
  let patch = root.join(format!(".git/{STATE_DIR}/host/runs/{}.patch", run_of(&outcome)));
  assert_eq!(std::fs::read_to_string(&patch).unwrap(), PATCH);
  // the line names the patch as git names the git directory: on Windows with `/` and `\` both
  let named = said
    .lines()
    .iter()
    .find_map(|line| {
      line
        .strip_prefix("brd-host: applied the changes of the command to 1 files of this working tree; `git apply -R ")?
        .strip_suffix("` undoes them")
        .map(str::to_owned)
    })
    .unwrap_or_else(|| panic!("no line of the apply: {:#?}", said.lines()));
  assert_eq!(std::fs::canonicalize(named).unwrap(), patch);

  // The working tree has `a formatted` now, so the same patch no longer applies
  let failure = run_in(&setup, &|_| {}, &calm, &[], APPLY).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit, failure.command_exit), (Code::ApplyFailed, Exit::Refused, Some(0)));
  assert!(failure.refusal.message.contains("exited with 0"), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("The patch is "), "{}", failure.refusal.message);
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("a.txt")).unwrap(), "a formatted\n", "nothing applied");

  let Commits { repo, .. } = two_commits();
  let quiet = fetching_agent(repo, Vec::new());
  let said = Said::default();
  let say = |line: &str| said.push(line);
  assert_eq!(run_in(&quiet, &say, &calm, &[], APPLY).await.unwrap().facts.applied_files, Some(0));
  assert!(said.lines().contains(&"brd-host: the command changed no file".to_owned()), "{:#?}", said.lines());
}

/// An agent whose fetch cuts the stream of its one file of `kind`, and whose command exits 3
fn cut_fetch(repo: TestRepo, kind: FileKind) -> Setup {
  let kind: &'static FileKind = leak(kind);
  agent(repo, <[String]>::to_vec, move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Follow { .. } => exited(channel, 3).await,
        Request::Fetch { .. } => {
          let _ = channel
            .send(&Reply::File {
              path: "out/bt-runs/1/big.log".to_owned(),
              size: 10,
              executable: false,
              kind: kind.clone(),
            })
            .await;
          let _ = channel.send_bytes(b"four").await;
          true
        }
        _ => false,
      }
    })
  })
  .0
}

/// Changes that did not come, because the fetch was cut, refuse with `apply_failed` and name the fetch that copies
/// them; a cut fetch without the apply refuses with `fetch_failed`, leaves no part and keeps the run on the host
#[tokio::test(start_paused = true)]
async fn a_cut_fetch_refuses_after_the_command_and_keeps_the_run() {
  let Commits { repo, .. } = two_commits();
  let setup = cut_fetch(repo, FileKind::Staged);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let failure = run_in(&setup, &say, &calm, &fetch(), RunOptions::default()).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit, failure.command_exit), (Code::FetchFailed, Exit::Refused, Some(3)));
  assert!(failure.refusal.message.contains("`brd-host fetch studio "), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("`brd-host forget studio "), "{}", failure.refusal.message);
  let root = setup.repo.path();
  assert!(!root.join("out/bt-runs/1/big.log").exists());
  assert!(!root.join("out/bt-runs/1/big.log.bd-host-part").exists());
  assert!(!kinds(&setup).contains(&"forget"));
  assert!(failure.facts.phases.iter().any(|phase| phase.name == "fetch"), "{:?}", failure.facts.phases);

  let Commits { repo, .. } = two_commits();
  let setup = cut_fetch(repo, FileKind::Changes);
  let failure = run_in(&setup, &|_| {}, &calm, &[], APPLY).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::ApplyFailed, Exit::Refused));
  assert!(failure.refusal.message.contains("did not come"), "{}", failure.refusal.message);
}

/// An agent whose `Follow` waits for the cancel of the run, up to a minute of the paused clock, then ends the run with
/// 130 and the line `said`. The answer is the flag of the cancel
fn agent_that_waits_for_the_cancel(repo: TestRepo, has: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static, said: &'static str) -> (Setup, &'static AtomicBool) {
  let cancelled = leak(AtomicBool::new(false));
  let (setup, _) = agent(repo, has, move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Follow { .. } => {
          wait_for(cancelled).await;
          output(channel, Stream::Stderr, said, 20).await;
          exit(channel, 130).await;
          true
        }
        Request::Cancel { .. } => {
          cancelled.store(true, Ordering::SeqCst);
          done(channel, "the supervisor has the cancel request").await;
          true
        }
        _ => false,
      }
    })
  });
  (setup, cancelled)
}

/// Runs to the end with one Ctrl+C after the first `Follow`, and answers the outcome and the said lines. Ctrl+C before
/// the start would refuse the run
async fn run_with_ctrl_c_after_the_follow(setup: &Setup) -> (RunOutcome, Vec<String>) {
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let count = press_after(setup, "follow", 1);
  let presses = || count.load(Ordering::SeqCst);
  let outcome = run_in(setup, &say, &presses, &[], RunOptions::default()).await.unwrap();
  (outcome, said.lines())
}

/// Ctrl+C after the start sends `Cancel` on its own connection, and the output follows up to the exit
#[tokio::test(start_paused = true)]
async fn ctrl_c_cancels_the_run_and_follows_it_to_its_end() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, cancelled) = agent_that_waits_for_the_cancel(repo, known(vec![head]), "interrupted");
  let (outcome, lines) = run_with_ctrl_c_after_the_follow(&setup).await;
  assert_eq!(outcome.code, 130);
  assert!(cancelled.load(Ordering::SeqCst));
  assert!(
    lines.iter().any(|line| line.starts_with("brd-host: Ctrl+C, SIGTERM or SIGHUP: cancelling the run")),
    "{lines:#?}"
  );
  assert!(lines.contains(&"interrupted".to_owned()), "{lines:#?}");
}

/// Ctrl+C before the start runs nothing on the host and exits 130
#[tokio::test(start_paused = true)]
async fn ctrl_c_before_the_start_reaches_no_host() {
  let Commits { repo, first, .. } = two_commits();
  let pressed = leak(AtomicBool::new(false));
  let (setup, _) = agent(repo, known(vec![first]), move |request, _| {
    if let Request::Have { .. } = request {
      pressed.store(true, Ordering::SeqCst);
    }
    Box::pin(async { false })
  });
  let presses = || u32::from(pressed.load(Ordering::SeqCst));
  let failure = run_in(&setup, &|_| {}, &presses, &[], RunOptions::default()).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Interrupted, Exit::Interrupted));
  assert_eq!(kinds(&setup), ["have"]);
}

/// Ctrl+C while the pack builds cancels the run on the host, and the output follows up to its end. The build holds two
/// minutes of the paused clock unless Ctrl+C stops it, so a run that ends within a minute was stopped
#[tokio::test(start_paused = true)]
async fn ctrl_c_during_the_build_of_the_pack_cancels_the_run_on_the_host() {
  let Commits { repo, first, .. } = two_commits();
  let (setup, cancelled) = agent_that_waits_for_the_cancel(repo, known(vec![first]), "brd-host: the run was cancelled before its command");
  let count = press_after(&setup, "pack", 1);
  let presses = || count.load(Ordering::SeqCst);
  let started = Instant::now();
  let outcome = run_holding(&setup, &|_| {}, &presses, &[], RunOptions::default(), Duration::from_mins(2)).await.unwrap();
  assert_eq!(outcome.code, 130, "the run ends with the code of the host");
  assert!(cancelled.load(Ordering::SeqCst));
  assert!(started.elapsed() < Duration::from_mins(1), "the build was not stopped: {:?}", started.elapsed());
}

/// The host takes the slot while git builds the pack, and empty chunks keep the connection of the pack alive
/// meanwhile, one per keepalive time of the paused clock. The follow starts with the run, so the lines of the host show
/// during the build
#[tokio::test(start_paused = true)]
async fn the_pack_builds_after_the_start_and_keeps_its_connection_alive() {
  let keepalives = leak(AtomicU32::new(0));
  let counted = |chunk: &[u8]| {
    if chunk.is_empty() {
      keepalives.fetch_add(1, Ordering::SeqCst);
    }
    Duration::ZERO
  };
  let setup = agent_reading_the_pack(two_commits(), counted, Some("brd-host: slot /pool/1 (warm)"));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_holding(&setup, &say, &calm, &[], RunOptions::default(), Duration::from_millis(400)).await.unwrap();
  assert_eq!(outcome.code, 0);
  // the hold of 400 ms at a keepalive every 50 ms
  assert_eq!(keepalives.load(Ordering::SeqCst), 8, "the empty chunks of the hold");
  let lines = said.lines();
  let at = |prefix: &str| lines.iter().position(|line| line.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix} in {lines:#?}"));
  assert!(at("brd-host: slot /pool/1 (warm)") < at("> studio: pack of 1 commits"), "{lines:#?}");
  assert_eq!(
    outcome.facts.phases.iter().map(|phase| phase.name.as_str()).collect::<Vec<_>>(),
    ["pack", "upload", "total"]
  );
}

/// An agent whose host has the first commit of `commits`, whose pack gives each chunk to `chunk`, which answers how long
/// the agent then waits on the paused clock, as a slow reader, and whose follow says `line`, if any, and exits 0
fn agent_reading_the_pack(commits: Commits, chunk: impl Fn(&[u8]) -> Duration + Send + Sync + 'static, line: Option<&'static str>) -> Setup {
  let Commits { repo, first, .. } = commits;
  let chunk = Arc::new(chunk);
  agent(repo, known(vec![first]), move |request, channel| {
    let chunk = Arc::clone(&chunk);
    Box::pin(async move {
      match request {
        Request::Pack { .. } => {
          while let Ok(Frame::Bytes(bytes)) = channel.recv_frame().await {
            sleep(chunk(&bytes)).await;
          }
          let _ = channel.send(&Reply::Packed).await;
          true
        }
        Request::Follow { .. } => {
          if let Some(line) = line {
            output(channel, Stream::Stderr, line, 30).await;
          }
          exited(channel, 0).await
        }
        _ => false,
      }
    })
  })
  .0
}

/// A follow of a run that goes on: a heartbeat every 200 ms of the paused clock for 10 s, until the client closes the
/// connection
fn heartbeats_only(_: u64, channel: &mut Channel) -> Answer<'_> {
  Box::pin(async move {
    for _ in 0..50 {
      sleep(Duration::from_millis(200)).await;
      if channel.send(&Reply::Heartbeat { elapsed_ms: 200 }).await.is_err() {
        return;
      }
    }
  })
}

/// A second Ctrl+C stops the follow at once, while the agent only sends heartbeats. The run stays on the host
#[tokio::test(start_paused = true)]
async fn a_second_ctrl_c_stops_the_follow_at_once() {
  let setup = following_head(heartbeats_only);
  let count = press_after(&setup, "follow", 2);
  let presses = || count.load(Ordering::SeqCst);
  let failure = run_in(&setup, &|_| {}, &presses, &[], RunOptions::default()).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Interrupted, Exit::Interrupted));
  assert!(failure.refusal.message.contains("`brd-host follow studio "), "{}", failure.refusal.message);
  assert!(!kinds(&setup).contains(&"forget"));
}

/// One Ctrl+C (or SIGTERM) leaves a follow at once, while the agent only sends heartbeats: no cancel reaches the host,
/// and the run goes on there
#[tokio::test(start_paused = true)]
async fn ctrl_c_leaves_a_follow_and_the_run_goes_on() {
  let (setup, _) = agent_following(TestRepo::new(), |_| Vec::new(), heartbeats_only);
  let count = press_after(&setup, "follow", 1);
  let presses = || count.load(Ordering::SeqCst);
  let processes = FakeProcesses::default();
  let refusal = setup.client(&processes, &|_| {}, &presses).follow_run(ALIAS, "1-2").await.unwrap_err();
  assert_eq!(refusal.code, Code::Interrupted);
  assert_eq!(
    refusal.message,
    format!("Ctrl+C, SIGTERM or SIGHUP left the follow of the run 1-2, which goes on on {ALIAS}: `brd-host follow {ALIAS} 1-2` follows it again")
  );
  assert!(!kinds(&setup).contains(&"cancel"), "{:?}", kinds(&setup));
}

/// A run whose follow ends with the steps `phases` and the refusal `code` of the agent, and the lines that it said
async fn refused_after(phases: Vec<Phase>, code: Code) -> (RunFailure, Vec<String>) {
  let Commits { repo, head, .. } = two_commits();
  let phases: &'static [Phase] = leak(phases);
  let (setup, _) = agent_following(repo, known(vec![head]), move |_, channel| {
    Box::pin(async move {
      let _ = channel.send(&Reply::Phases { phases: phases.to_vec() }).await;
      let _ = channel.send(&Reply::refused(code, "the supervisor failed")).await;
    })
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let failure = run_in(&setup, &say, &calm, &[], RunOptions::default()).await.unwrap_err();
  assert_eq!(kinds(&setup).last(), Some(&"forget"));
  (failure, said.lines())
}

/// A refusal of the agent keeps its code; before the command it exits 75, after it 255. A run that failed on the host
/// is removed there all the same, and the client says the steps of the host
#[tokio::test(start_paused = true)]
async fn a_refusal_of_the_agent_exits_by_how_far_the_run_came() {
  let (before, lines) = refused_after(vec![step("slot", 0), step("prepare", 3_909_000)], Code::AgentError).await;
  assert_eq!((before.refusal.code, before.exit), (Code::AgentError, Exit::NotRun));
  assert_eq!(before.refusal.message, "studio: the supervisor failed");
  assert!(lines.contains(&"brd-host: phases: slot 0.0s, prepare 1h05m09s".to_owned()), "{lines:#?}");
  let (after, _) = refused_after(vec![step("prepare", 10), step("command", 10)], Code::AgentError).await;
  assert_eq!(after.exit, Exit::Refused);
  let (lease, _) = refused_after(vec![step("slot", 10)], Code::LeaseFailed).await;
  assert_eq!((lease.refusal.code, lease.exit), (Code::LeaseFailed, Exit::NotRun));

  let dir = tempfile::tempdir().unwrap();
  let processes = FakeProcesses::default();
  let command = command();
  let request = RunRequest {
    checkout: dir.path(),
    repo: None,
    affinity: AFFINITY,
    command: &command,
    fetch: &[],
    measure_as: None,
    group: None,
  };
  let no_host = Client::new(crate::config::Config::new(dir.path().join("config")), crate::testkit::build(), &processes, &|_| {}, &calm)
    .run(ALIAS, &request, RunOptions::default())
    .await
    .unwrap_err();
  assert_eq!((no_host.refusal.code, no_host.exit.code()), (Code::NoHost, 75));
}

/// A start whose reply does not come can leave a run on the agent, which would wait for its pack. The client cancels it
#[tokio::test(start_paused = true)]
async fn a_start_without_its_reply_is_cancelled() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, _) = agent(repo, known(vec![head]), |request, _| {
    let started = matches!(request, Request::Start(_));
    Box::pin(async move { started })
  });
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap_err();
  assert!(matches!(failure.refusal.code, Code::Link | Code::TimedOut), "{failure:?}");
  assert_eq!(failure.exit, Exit::NotRun);
  assert!(failure.refusal.message.starts_with("the start of the run "), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("; studio has the cancel of the run "), "{}", failure.refusal.message);
  assert_eq!(kinds(&setup), ["have", "start", "cancel"]);
}

/// A commit whose `[host]` does not parse starts no run, one that asks for its outputs or not (bt): the client reads
/// `[host]` of every run for its heavy patterns
#[tokio::test(start_paused = true)]
async fn a_config_that_does_not_parse_starts_no_run() {
  for options in [
    RunOptions {
      outputs: true,
      ..RunOptions::default()
    },
    RunOptions::default(),
  ] {
    let repo = TestRepo::new();
    repo.create_commit("Add the config", ".branchdeck.toml", "[host]\noutputs = \"results\"\n");
    let setup = fetching_agent(repo, Vec::new());
    let failure = run_in(&setup, &|_| {}, &calm, &[], options).await.unwrap_err();
    assert_eq!((failure.refusal.code, failure.exit), (Code::BadConfig, Exit::NotRun), "{options:?}");
    assert!(!kinds(&setup).contains(&"start"), "{:?}", kinds(&setup));
  }
}

/// A run of the command of the tests on an agent of `hello`, in a checkout whose commit has `config` as its
/// `.branchdeck.toml`: whether its start is heavy, and the lines that name a heavy run
async fn heavy_run(config: Option<&str>, hello: &Hello, options: RunOptions) -> (bool, Vec<String>) {
  let repo = TestRepo::new();
  match config {
    Some(config) => repo.create_commit("Add the config", ".branchdeck.toml", config),
    None => repo.create_commit("Add a", "a.txt", "a\n"),
  };
  let setup = fetching_agent_with(repo, hello.clone(), Vec::new());
  let said = Said::default();
  let say = |line: &str| said.push(line);
  run_in(&setup, &say, &calm, &[], options).await.unwrap();
  let lines: Vec<String> = said.lines().into_iter().filter(|line| line.contains("heavy")).collect();
  (start_spec(&setup).heavy, lines)
}

/// A run is heavy by `--heavy`, else by the first pattern of `heavy` of `[host]` of the shipped commit that matches its
/// command: the start carries `heavy`, and a line says why. A command that no pattern matches is no heavy run. An agent
/// without the feature `heavy` gets the run with a line that names the upgrade
#[tokio::test(start_paused = true)]
async fn a_run_is_heavy_by_a_pattern_of_its_commit_or_by_heavy_and_says_why() {
  let heavy_agent = Hello {
    features: vec!["upgrade".to_owned(), "heavy".to_owned()],
    ..Hello::default()
  };
  let heavy = RunOptions {
    heavy: true,
    ..RunOptions::default()
  };
  // the command of the tests is `./community/tools/bt.cmd FooTest --json`
  let patterns = "[host]\nheavy = [\"pnpm e2e*\", \"./community/tools/bt.cmd*\", \"*\"]\n";
  assert_eq!(
    heavy_run(Some(patterns), &heavy_agent, RunOptions::default()).await,
    (
      true,
      vec![format!(
        "brd-host: a heavy run by `./community/tools/bt.cmd*` of [host]: on {ALIAS} it waits until the cores that its command used before are free beside the other heavy runs, or the whole host without a measure"
      )]
    ),
    "the first pattern that matches names the run"
  );
  assert_eq!(
    heavy_run(Some("[host]\nheavy = [\"pnpm e2e*\"]\n"), &heavy_agent, RunOptions::default()).await,
    (false, vec![])
  );
  assert_eq!(heavy_run(None, &heavy_agent, RunOptions::default()).await, (false, vec![]), "no [host], no heavy run");
  assert_eq!(
    heavy_run(None, &heavy_agent, heavy).await,
    (
      true,
      vec![format!(
        "brd-host: a heavy run by --heavy: on {ALIAS} it waits until the cores that its command used before are free beside the other heavy runs, or the whole host without a measure"
      )]
    )
  );
  assert_eq!(
    heavy_run(None, &Hello::default(), heavy).await,
    (
      true,
      vec![
        format!("brd-host: the agent on {ALIAS} predates heavy runs, so this one runs beside the others: `brd-host upgrade {ALIAS}` sends this build"),
        format!(
          "brd-host: a heavy run by --heavy: on {ALIAS} it waits until the cores that its command used before are free beside the other heavy runs, or the whole host without a measure"
        ),
      ]
    )
  );
}

/// `--exclusive` starts a heavy run that is exclusive and says what it waits for and what it holds; an agent without the
/// feature `exclusive` would run the measurement beside the others, so the client refuses before the start with
/// `bad_config` (exit 75) and names the upgrade
#[tokio::test(start_paused = true)]
async fn an_exclusive_run_is_a_heavy_one_and_an_agent_before_it_refuses_before_the_start() {
  let exclusive = RunOptions {
    exclusive: true,
    ..RunOptions::default()
  };
  let setup = agent_of(&["upgrade", "heavy", "exclusive"]);
  let said = Said::default();
  run_in(&setup, &|line: &str| said.push(line), &calm, &[], exclusive).await.unwrap();
  let spec = start_spec(&setup);
  assert_eq!((spec.heavy, spec.exclusive), (true, true));
  assert!(
    said.lines().contains(&format!(
      "brd-host: an exclusive run by --exclusive: on {ALIAS} it waits for a quiet host without another heavy run, and while it runs no heavy run is admitted and each new run that is not heavy waits for its end"
    )),
    "{:?}",
    said.lines()
  );

  let setup = agent_of(&["upgrade", "heavy"]);
  let failure = run_in(&setup, &|_| {}, &calm, &[], exclusive).await.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::BadConfig, Exit::NotRun));
  assert_eq!(
    failure.refusal.message,
    format!("the agent on {ALIAS} predates exclusive runs, so it would run this measurement beside the others: `brd-host upgrade {ALIAS}` sends this build")
  );
  assert!(!kinds(&setup).contains(&"start"), "{:?}", kinds(&setup));
}

/// A fetching agent whose hello names `features`, for a checkout of one commit
fn agent_of(features: &[&str]) -> Setup {
  let repo = TestRepo::new();
  repo.create_commit("Add a", "a.txt", "a\n");
  let hello = Hello {
    features: features.iter().map(|feature| (*feature).to_owned()).collect(),
    ..Hello::default()
  };
  fetching_agent_with(repo, hello, Vec::new())
}

/// A run with the command of the tests in the checkout of `setup` of the class `measure_as` and the group `group`, and
/// the lines it said
async fn run_named(setup: &Setup, measure_as: Option<&str>, group: Option<&str>, options: RunOptions) -> (Result<RunOutcome, RunFailure>, Vec<String>) {
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let processes = FakeProcesses::default();
  let command = command();
  let request = RunRequest {
    checkout: setup.repo.path(),
    repo: None,
    affinity: AFFINITY,
    command: &command,
    fetch: &[],
    measure_as,
    group,
  };
  let ran = setup.client(&processes, &say, &calm).run(ALIAS, &request, options).await;
  (ran, said.lines())
}

/// A run of `--measure-as` with the command of the tests in the checkout of `setup`, and the lines it said
async fn run_of_class(setup: &Setup, class: &str) -> (Result<RunOutcome, RunFailure>, Vec<String>) {
  run_named(setup, Some(class), None, RunOptions::default()).await
}

/// A run of `--group` with the command of the tests in the checkout of `setup`, heavy by `--heavy`, and the lines it said
async fn run_of_group(setup: &Setup, group: &str) -> (Result<RunOutcome, RunFailure>, Vec<String>) {
  let heavy = RunOptions {
    heavy: true,
    ..RunOptions::default()
  };
  run_named(setup, None, Some(group), heavy).await
}

/// `--group` starts the run with its group; an agent without the feature `group` queues every heavy run first come
/// first, so the run goes on there without the field and its line names the upgrade, as nothing else of it changes; a
/// group that no name may be refuses with `usage` before the agent
#[tokio::test(start_paused = true)]
async fn a_run_of_a_group_names_it_in_the_start_and_an_agent_before_groups_queues_it_first_come_first() {
  let setup = agent_of(&["heavy", "exclusive", "measure_as", "group"]);
  let (ran, said) = run_of_group(&setup, "typing-session").await;
  ran.unwrap();
  assert_eq!(start_spec(&setup).group.as_deref(), Some("typing-session"));
  assert!(!said.iter().any(|line| line.contains("predates the groups")), "{said:?}");

  let setup = agent_of(&["heavy", "exclusive", "measure_as"]);
  let (ran, said) = run_of_group(&setup, "typing-session").await;
  ran.unwrap();
  assert_eq!(start_spec(&setup).group, None, "an older agent gets no group");
  assert!(
    said.contains(&format!(
      "brd-host: the agent on {ALIAS} predates the groups of the heavy queue, so this run of the group typing-session waits first come first: `brd-host upgrade {ALIAS}` sends this build"
    )),
    "{said:?}"
  );

  let setup = agent_of(&["heavy", "exclusive", "measure_as", "group"]);
  let failure = run_of_group(&setup, "a b").await.0.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Usage, Exit::Usage));
  assert!(
    failure.refusal.message.starts_with(r#"--group "a b" (or BRD_HOST_GROUP) cannot name a group"#),
    "{}",
    failure.refusal.message
  );
  assert_eq!(kinds(&setup), Vec::<&str>::new());
}

/// `--measure-as` starts a heavy run with its class and says that the cores of the runs of the class count; an agent
/// without the feature `measure_as` would measure the run by its command, so the client refuses before the start with
/// `bad_config` (exit 75) and names the upgrade; a class that no name may be refuses with `usage` before the agent
#[tokio::test(start_paused = true)]
async fn a_run_of_a_class_is_heavy_and_an_agent_before_it_refuses_before_the_start() {
  let setup = agent_of(&["heavy", "exclusive", "measure_as"]);
  let (ran, said) = run_of_class(&setup, "accio-typing-gate").await;
  ran.unwrap();
  let spec = start_spec(&setup);
  assert_eq!((spec.heavy, spec.measure_as.as_deref()), (true, Some("accio-typing-gate")));
  assert!(
    said.contains(&format!(
      "brd-host: a heavy run by --measure-as: on {ALIAS} it waits until the cores that the runs of the class accio-typing-gate used before are free beside the other heavy runs, or the whole host without a measure"
    )),
    "{said:?}"
  );

  let setup = agent_of(&["heavy", "exclusive"]);
  let failure = run_of_class(&setup, "accio-typing-gate").await.0.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::BadConfig, Exit::NotRun));
  assert_eq!(
    failure.refusal.message,
    format!(
      "the agent on {ALIAS} predates --measure-as, so it would measure this run by its command and give it the whole host every time: `brd-host upgrade {ALIAS}` sends this build"
    )
  );
  assert!(!kinds(&setup).contains(&"start"), "{:?}", kinds(&setup));

  let setup = agent_of(&["heavy", "exclusive", "measure_as"]);
  let failure = run_of_class(&setup, "gate/2").await.0.unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Usage, Exit::Usage));
  assert!(
    failure.refusal.message.starts_with(r#"--measure-as "gate/2" cannot name a class of measures"#),
    "{}",
    failure.refusal.message
  );
  assert_eq!(kinds(&setup), Vec::<&str>::new());
}

/// An agent without the feature `heavy` refuses every run of a commit whose `[host]` has the key, a light one too, so
/// the client refuses before the start with `bad_config` (exit 75) and names the upgrade; nothing reaches the agent but
/// the `Have`
#[tokio::test(start_paused = true)]
async fn a_commit_with_heavy_on_an_agent_before_heavy_runs_refuses_before_the_start() {
  for config in ["[host]\nheavy = [\"pnpm e2e*\"]\n", "[host]\nheavy = []\n"] {
    let repo = TestRepo::new();
    repo.create_commit("Add the config", ".branchdeck.toml", config);
    let setup = fetching_agent(repo, Vec::new());
    let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).await.unwrap_err();
    assert_eq!((failure.refusal.code, failure.exit), (Code::BadConfig, Exit::NotRun), "{config}");
    assert_eq!(
      failure.refusal.message,
      format!("the agent on {ALIAS} predates `heavy` of [host] of .branchdeck.toml, so it refuses every run of this commit: `brd-host upgrade {ALIAS}` sends this build")
    );
    assert!(!kinds(&setup).contains(&"start"), "{:?}", kinds(&setup));
  }
}

/// The outputs of `[host]` of the shipped commit go into the record, so the fetch takes their files
#[tokio::test(start_paused = true)]
async fn the_outputs_of_the_commit_come_back_in_place_of_their_local_copies() {
  let repo = TestRepo::new();
  repo.create_commit("Add the config", ".branchdeck.toml", "[host]\noutputs = [\"results\"]\n");
  let results = FileKind::Output { root: "results".to_owned() };
  let setup = fetching_agent(repo, vec![("results/new.txt".to_owned(), results, b"new".to_vec())]);
  std::fs::create_dir_all(setup.repo.path().join("results")).unwrap();
  std::fs::write(setup.repo.path().join("results/old.txt"), "old").unwrap();
  std::fs::create_dir_all(setup.repo.path().join(".git/info")).unwrap();
  std::fs::write(setup.repo.path().join(".git/info/exclude"), "results/\n").unwrap();
  let outputs = RunOptions {
    outputs: true,
    ..RunOptions::default()
  };
  let outcome = run_in(&setup, &|_| {}, &calm, &[], outputs).await.unwrap();
  assert_eq!(outcome.facts.fetched, ["results/new.txt"]);
  assert!(!setup.repo.path().join("results/old.txt").exists());
  assert!(start_spec(&setup).outputs);
}

/// The disk report, the trim and the summary of the runs come after the heartbeats of the agent, as the JSON it sent,
/// and `fresh`, `max`, `plan` and `days` reach it
#[tokio::test(start_paused = true)]
async fn runs_cancel_slots_disk_trim_and_stats_answer_the_replies_of_the_agent() {
  let hello = Hello {
    features: vec!["upgrade".to_owned(), "disk".to_owned(), "trim".to_owned(), "stats".to_owned()],
    ..Hello::default()
  };
  let setup = Setup::with_hello(hello, |request, channel| {
    Box::pin(async move {
      let replies = match request {
        Request::Runs { .. } => vec![Reply::Runs { runs: Vec::new(), heavy: None }],
        Request::Disk { fresh: true } => vec![
          Reply::Heartbeat { elapsed_ms: 100 },
          Reply::Disk {
            report: serde_json::json!({"bytes": 7}),
          },
        ],
        Request::Trim { max: Some(6), plan: true } => vec![
          Reply::Heartbeat { elapsed_ms: 100 },
          Reply::Trimmed {
            report: serde_json::json!({"plan": true}),
          },
        ],
        Request::Stats { days: 3 } => vec![
          Reply::Heartbeat { elapsed_ms: 100 },
          Reply::Stats {
            report: serde_json::json!({"runs": 2}),
          },
        ],
        Request::Slots => vec![Reply::Slots {
          repositories: vec![host_link::proto::RepositorySlots {
            repo: "~/projects/idea-1".to_owned(),
            table: serde_json::json!({"slots": []}),
            abandoned: Vec::new(),
          }],
        }],
        _ => vec![Reply::refused(Code::BadRequest, "no")],
      };
      for reply in replies {
        let _ = channel.send(&reply).await;
      }
    })
  });
  let processes = FakeProcesses::default();
  let client = setup.client(&processes, &|_| {}, &calm);
  assert_eq!(client.runs(ALIAS).await.unwrap().runs.len(), 0);
  assert_eq!(client.slots(ALIAS).await.unwrap()[0].repo, "~/projects/idea-1");
  assert_eq!(client.cancel(ALIAS, "r1").await.unwrap_err().code, Code::BadRequest);
  assert_eq!(client.disk(ALIAS, true).await.unwrap(), serde_json::json!({"bytes": 7}));
  assert_eq!(kinds(&setup).last(), Some(&"disk"));
  assert_eq!(client.disk(ALIAS, false).await.unwrap_err().code, Code::BadRequest);
  assert_eq!(client.trim(ALIAS, Some(6), true).await.unwrap(), serde_json::json!({"plan": true}));
  assert_eq!(kinds(&setup).last(), Some(&"trim"));
  assert_eq!(client.stats(ALIAS, 3).await.unwrap(), serde_json::json!({"runs": 2}));
  assert_eq!(kinds(&setup).last(), Some(&"stats"));
}

/// An agent without the feature `disk`, `trim` or `stats` gets no such request: it would close the connection on one it
/// does not know
#[tokio::test(start_paused = true)]
async fn disk_trim_and_stats_of_an_agent_without_the_feature_send_no_request() {
  let older = Setup::new(|_, channel| {
    Box::pin(async move {
      let _ = channel.send(&Reply::refused(Code::BadRequest, "no")).await;
    })
  });
  let processes = FakeProcesses::default();
  let client = older.client(&processes, &|_| {}, &calm);
  for refused in [
    client.disk(ALIAS, false).await.unwrap_err(),
    client.trim(ALIAS, None, false).await.unwrap_err(),
    client.stats(ALIAS, 3).await.unwrap_err(),
  ] {
    assert_eq!(refused.code, Code::AgentFailed, "{}", refused.message);
    assert!(refused.message.contains("brd-host upgrade studio"), "{}", refused.message);
  }
  assert_eq!(kinds(&older), Vec::<&str>::new());
}

/// `Reply::Phases` belongs to `Follow` only. Anywhere else it is a reply out of turn
#[tokio::test(start_paused = true)]
async fn the_steps_of_a_run_outside_a_follow_are_out_of_turn() {
  let setup = Setup::new(|_, channel| {
    Box::pin(async move {
      let _ = channel.send(&Reply::Phases { phases: Vec::new() }).await;
    })
  });
  let processes = FakeProcesses::default();
  assert_eq!(setup.client(&processes, &|_| {}, &calm).runs(ALIAS).await.unwrap_err().code, Code::Link);
}

/// A request that waits gets heartbeats before its reply, and each heartbeat starts the read deadline again: on paused
/// time the agent sends a heartbeat every half of the deadline, five times, so the request takes two and a half
/// deadlines and its reads never wait one. A deadline of the whole request would end it
#[tokio::test(start_paused = true)]
async fn heartbeats_keep_a_waiting_request_alive() {
  const READ: Duration = Duration::from_secs(1);
  let setup = Setup::new(|request, channel| {
    let runs = matches!(request, Request::Runs { .. });
    Box::pin(async move {
      if !runs {
        return;
      }
      for _ in 0..5 {
        sleep(READ / 2).await;
        if channel.send(&Reply::Heartbeat { elapsed_ms: 0 }).await.is_err() {
          return;
        }
      }
      let _ = channel.send(&Reply::Runs { runs: Vec::new(), heavy: None }).await;
    })
  });
  let processes = FakeProcesses::default();
  let timing = Timing {
    read: READ,
    ..crate::testkit::UNHURRIED
  };
  let started = Instant::now();
  assert_eq!(setup.client(&processes, &|_| {}, &calm).with_timing(timing).runs(ALIAS).await.unwrap().runs.len(), 0);
  assert_eq!(started.elapsed(), READ * 5 / 2, "the request outlived its read deadline");
}

/// Two commits whose `origin/master` is a commit beside HEAD on the first one, so a base sync sends a pack
fn two_commits_with_a_base() -> Commits {
  let commits = two_commits();
  let tree = commits.repo.git(&["rev-parse", &format!("{}^{{tree}}", commits.first)]);
  let base = commits.repo.git(&["commit-tree", &tree, "-p", &commits.first, "-m", "Side"]);
  commits.repo.git(&["update-ref", "refs/remotes/origin/master", &base]);
  commits
}

/// An agent of a checkout with a base, whose host has the first commit: `base` answers the pack of the base after its
/// stream came, and the follow waits `command` of the paused clock, then exits with `code`
fn agent_of_a_base(base: impl for<'c> Fn(&'c mut Channel) -> Answer<'c, ()> + Send + Sync + 'static, command: Duration, code: i32) -> Setup {
  let Commits { repo, first, .. } = two_commits_with_a_base();
  let base = Arc::new(base);
  agent(repo, known(vec![first]), move |request, channel| {
    let base = Arc::clone(&base);
    Box::pin(async move {
      match request {
        Request::Pack { target: PackTarget::Base(_), .. } => {
          let _ = channel.recv_stream(&mut Vec::new()).await;
          base(channel).await;
          true
        }
        Request::Follow { .. } => {
          sleep(command).await;
          exited(channel, code).await
        }
        _ => false,
      }
    })
  })
  .0
}

/// Ctrl+C after the end of the run stops the base sync, and its connection with it, also while the host indexes the
/// base, and says no warning. The host indexes until the client closes, for up to two minutes of the paused clock: its
/// heartbeats end by a failed send, so the client closed the connection and did not wait for the index
#[tokio::test(start_paused = true)]
async fn ctrl_c_stops_the_base_sync_while_the_host_indexes_it() {
  let indexing = leak(AtomicBool::new(false));
  let closed = leak(AtomicBool::new(false));
  let setup = agent_of_a_base(
    move |channel| {
      Box::pin(async move {
        indexing.store(true, Ordering::SeqCst);
        let started = Instant::now();
        while started.elapsed() < Duration::from_mins(2) {
          sleep(Duration::from_millis(200)).await;
          if channel.send(&Reply::Heartbeat { elapsed_ms: 200 }).await.is_err() {
            closed.store(true, Ordering::SeqCst);
            return;
          }
        }
        let _ = channel.send(&Reply::Packed).await;
      })
    },
    Duration::ZERO,
    0,
  );
  let presses = || u32::from(indexing.load(Ordering::SeqCst));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  assert_eq!(run_in(&setup, &say, &presses, &[], RunOptions::default()).await.unwrap().code, 0);
  wait_for(closed).await;
  assert!(closed.load(Ordering::SeqCst), "the sync held the end: the host indexed to its end");
  assert!(!said.lines().iter().any(|line| line.contains("warning")), "{:#?}", said.lines());
}

/// An agent that refuses the base gives one warning line, and the run ends with the code of its command
#[tokio::test(start_paused = true)]
async fn a_failed_base_sync_only_warns() {
  let setup = agent_of_a_base(
    |channel| {
      Box::pin(async move {
        let _ = channel.send(&Reply::refused(Code::BadRequest, "no base here")).await;
      })
    },
    Duration::from_millis(300),
    4,
  );
  let (ran, lines) = run_saying(&setup, RunOptions::default()).await;
  assert_eq!(ran.unwrap().code, 4);
  let warnings: Vec<String> = lines.into_iter().filter(|line| line.contains("warning")).collect();
  assert_eq!(warnings.len(), 1, "{warnings:#?}");
  assert!(
    warnings[0].starts_with("brd-host: warning: cannot sync the base of the next packs to studio (bad_request): "),
    "{}",
    warnings[0]
  );
  assert_eq!(setup.config.base(ALIAS, AFFINITY), None);
}

/// Text of `size` bytes that zlib cannot make much smaller: 6 random bits in each byte
fn noise(size: usize) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
  (0..size)
    .map(|_| {
      state ^= state << 13;
      state ^= state >> 7;
      state ^= state << 17;
      char::from(ALPHABET[usize::try_from(state >> 58).unwrap()])
    })
    .collect()
}

/// The smallest pack of `two_commits_with_a_large_file`. It is much larger than the pipes of the net of the tests
/// (256 KiB), so the writes of the client wait while the agent does not read
const LARGE_PACK: usize = 1024 * 1024;

/// Two commits whose second one adds a file of noise, so the pack of that commit holds more than `LARGE_PACK` bytes
fn two_commits_with_a_large_file() -> Commits {
  let repo = TestRepo::new();
  let first = repo.create_commit("Add a", "a.txt", "a\n");
  let head = repo.create_commit("Add large", "large.txt", &noise(LARGE_PACK * 3 / 2));
  Commits { repo, first, head }
}

/// After the hello each write of a connection waits up to the read deadline, not up to the short connect deadline,
/// which is for a dead address, so a large pack to an agent that reads it slowly does not fail at the connect deadline.
/// On paused time the agent reads a chunk every 2 s, so the upload takes longer than the connect deadline of 5 s, and
/// the pack, larger than the pipes, comes whole
#[tokio::test(start_paused = true)]
async fn after_its_hello_a_connection_writes_with_the_read_deadline_and_a_large_pack_comes_whole() {
  let packed = leak(Mutex::new(Vec::new()));
  let slow = |chunk: &[u8]| {
    packed.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(chunk);
    if chunk.is_empty() { Duration::ZERO } else { Duration::from_secs(2) }
  };
  let setup = agent_reading_the_pack(two_commits_with_a_large_file(), slow, None);
  let timing = Timing {
    connect: Duration::from_secs(5),
    read: Duration::from_secs(70),
    ..crate::testkit::UNHURRIED
  };
  let processes = FakeProcesses::default();
  let (channel, _) = setup.client(&processes, &|_| {}, &calm).with_timing(timing).host(ALIAS).unwrap().connect().await.unwrap();
  assert_eq!(channel.write_deadline(), timing.read, "the write deadline of a connection after its hello");
  drop(channel);

  let started = Instant::now();
  let outcome = run_timed(&setup, &|_| {}, timing).await.unwrap();
  assert_eq!(outcome.code, 0);
  assert!(started.elapsed() > timing.connect, "the upload outlived the connect deadline: {:?}", started.elapsed());
  let bytes = packed.lock().unwrap_or_else(PoisonError::into_inner);
  assert!(bytes.len() > LARGE_PACK, "{} bytes", bytes.len());
  assert!(bytes.starts_with(b"PACK"));
}

/// An agent that drops the connection in the middle of the pack is a lost link, not a git failure. The run started
/// already, so the client cancels it, and nothing follows it. The checkout of the run waits for the pack, so the command
/// did not run (exit 75)
#[tokio::test(start_paused = true)]
async fn a_link_lost_during_the_pack_cancels_the_started_run_with_a_link_refusal() {
  // The empty chunks of the build, then one chunk of the pack, then the connection closes
  let failure = failed_pack(two_commits_with_a_large_file(), |channel| {
    Box::pin(async move {
      while let Ok(Frame::Bytes(chunk)) = channel.recv_frame().await {
        if !chunk.is_empty() {
          break;
        }
      }
    })
  })
  .await;
  assert!(matches!(failure.refusal.code, Code::Link | Code::TimedOut), "{:?}", failure.refusal);
}

/// A cancel whose first connection fails connects again, and the run still ends with its own exit
#[tokio::test(start_paused = true)]
async fn a_failed_cancel_connects_again() {
  let Commits { repo, head, .. } = two_commits();
  let cancels = leak(AtomicU32::new(0));
  let (setup, _) = agent(repo, known(vec![head]), move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Follow { .. } => {
          let started = Instant::now();
          while cancels.load(Ordering::SeqCst) < 2 && started.elapsed() < Duration::from_mins(1) {
            sleep(Duration::from_millis(100)).await;
            let _ = channel.send(&Reply::Heartbeat { elapsed_ms: 100 }).await;
          }
          exit(channel, 130).await;
          true
        }
        // The first cancel gets no reply: the connection closes
        Request::Cancel { .. } => {
          if cancels.fetch_add(1, Ordering::SeqCst) > 0 {
            done(channel, "the host has the cancel").await;
          }
          true
        }
        _ => false,
      }
    })
  });
  let (outcome, lines) = run_with_ctrl_c_after_the_follow(&setup).await;
  assert_eq!(outcome.code, 130);
  assert_eq!(cancels.load(Ordering::SeqCst), 2);
  assert!(lines.iter().any(|line| line.starts_with("brd-host: cannot send the cancel of the run ")), "{lines:#?}");
  assert!(lines.contains(&"brd-host: the host has the cancel".to_owned()), "{lines:#?}");
}

/// A run whose end a `brd-host wait` of the checkout finished first, while this client followed it, reports that end: the
/// files that the wait copied, and no second fetch, apply or forget. A run that finished itself writes its end, so a
/// later wait answers from it
#[tokio::test(start_paused = true)]
async fn a_run_that_a_wait_finished_first_reports_its_end_and_fetches_nothing() {
  let Commits { repo, head, .. } = two_commits();
  let runs_dir: &'static PathBuf = leak(repo.path().join(".git").join(STATE_DIR).join("host/runs"));
  let (setup, _) = agent(repo, known(vec![head]), move |request, channel| {
    Box::pin(async move {
      match request {
        Request::Follow { run, .. } => {
          let end = RunEnd {
            exit: 0,
            command_exit: Some(0),
            phases: vec![step("command", 5)],
            fetched: vec!["out/bt-runs/0000000002000/test.log".to_owned()],
            forgotten: true,
            ..RunEnd::default()
          };
          RunFiles::new(runs_dir).unwrap().write_end(run, &end).unwrap();
          let _ = channel.send(&Reply::Phases { phases: vec![step("command", 5)] }).await;
          let _ = channel.send(&Reply::Exit { code: 0 }).await;
          true
        }
        _ => false,
      }
    })
  });
  let outcome = run_in(&setup, &|_| {}, &calm, &fetch(), RunOptions::default()).await.unwrap();
  assert_eq!(outcome.facts.fetched, ["out/bt-runs/0000000002000/test.log"]);
  assert!(!kinds(&setup).contains(&"fetch") && !kinds(&setup).contains(&"forget"), "{:?}", kinds(&setup));

  let setup = following_head(|_, channel| {
    Box::pin(async move {
      let _ = channel.send(&Reply::Phases { phases: vec![step("command", 5)] }).await;
      let _ = channel.send(&Reply::Exit { code: 4 }).await;
    })
  });
  let outcome = run_in(&setup, &|_| {}, &calm, &fetch(), RunOptions::default()).await.unwrap();
  let run = outcome.facts.run.expect("the run");
  let files = RunFiles::new(&setup.repo.path().join(".git").join(STATE_DIR).join("host/runs")).unwrap();
  let end = files.read_end(&run).unwrap().expect("the end of the run");
  assert_eq!((end.exit, end.command_exit, end.forgotten), (4, Some(4), true));
  assert_eq!(end.fetched, outcome.facts.fetched);
}

/// The answer of an agent to `Runs`: the run of the start that reached it, with `phases` ended and `exit`
fn runs_reply(setup_requests: &Mutex<Vec<Request>>, phases: &[&str], exit: Option<i32>) -> Reply {
  let run = setup_requests
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .iter()
    .find_map(|request| match request {
      Request::Start(spec) => Some(spec.run.clone()),
      _ => None,
    })
    .unwrap_or_default();
  Reply::Runs {
    runs: vec![RunInfo {
      run,
      affinity: AFFINITY.to_owned(),
      commit: String::new(),
      command: command(),
      started_ms: 0,
      ended_ms: None,
      exit,
      phases: phases.iter().map(|name| step(name, 1)).collect(),
      fetch: fetch(),
      forgotten: false,
      refused: None,
      cores: None,
    }],
    heavy: None,
  }
}

/// An agent whose run answers `Runs` with the ended steps `phases` and `exit`, for a run with `--max`
fn agent_polled(phases: &'static [&'static str], exit: Option<i32>) -> Setup {
  let Commits { repo, first, .. } = two_commits();
  let seen: &'static Mutex<Vec<Request>> = leak(Mutex::default());
  let (setup, _) = agent(repo, known(vec![first]), move |request, channel| {
    seen.lock().unwrap_or_else(PoisonError::into_inner).push(request.clone());
    Box::pin(async move {
      match request {
        Request::Runs { .. } => {
          let _ = channel.send(&runs_reply(seen, phases, exit)).await;
          true
        }
        Request::Follow { .. } => {
          output(channel, Stream::Stdout, "ran", 3).await;
          let _ = channel.send(&Reply::Phases { phases: vec![step("command", 7)] }).await;
          let _ = channel.send(&Reply::Exit { code: exit.unwrap_or_default() }).await;
          true
        }
        _ => false,
      }
    })
  });
  setup
}

/// A run with `--max` follows no output: past its max it leaves the run going with `still_running` (124) and sends no
/// cancel and no follow; the pack reached the host before it left. The max has passed at the first look, so the wait
/// asks the host once
#[tokio::test(start_paused = true)]
async fn run_max_leaves_past_its_max_without_a_cancel_and_never_before_its_pack() {
  let setup = agent_polled(&["slot", "prepare"], None);
  let past = || true;
  let processes = FakeProcesses::default();
  let options = RunOptions {
    max: Some(Duration::from_mins(9)),
    ..RunOptions::default()
  };
  let failure = run_by(&setup.client(&processes, &|_| {}, &calm).with_past_max(&past), &setup, &fetch(), options)
    .await
    .unwrap_err();
  assert_eq!(
    (failure.refusal.code, failure.exit),
    (Code::StillRunning, Exit::StillRunning),
    "{}",
    failure.refusal.message
  );
  assert!(failure.refusal.message.contains("it runs its command"), "{}", failure.refusal.message);
  let kinds = kinds(&setup);
  assert!(kinds.contains(&"pack") && kinds.contains(&"runs"), "{kinds:?}");
  assert!(!kinds.contains(&"cancel") && !kinds.contains(&"follow") && !kinds.contains(&"forget"), "{kinds:?}");
  assert!(failure.facts.phases.iter().any(|phase| phase.name == "upload"), "{:?}", failure.facts.phases);
}

/// A run with `--max` whose run ends in time finishes it as a wait does: its output into the log of the run and not to
/// the stdout, its files, its end and its forget, and the exit code of the command
#[tokio::test(start_paused = true)]
async fn run_max_finishes_a_run_that_ends_in_time_as_a_wait_does() {
  let setup = agent_polled(&["slot", "prepare", "command"], Some(5));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let options = RunOptions {
    max: Some(Duration::from_mins(9)),
    ..RunOptions::default()
  };
  let outcome = run_in(&setup, &say, &calm, &fetch(), options).await.unwrap();
  assert_eq!(outcome.code, 5);
  assert_eq!(outcome.stdout, b"", "the output goes to the log");
  assert_eq!(outcome.facts.fetched, ["out/bt-runs/0000000002000/test.log"]);
  let run = outcome.facts.run.expect("the run");
  let files = RunFiles::new(&setup.repo.path().join(".git").join(STATE_DIR).join("host/runs")).unwrap();
  assert_eq!(std::fs::read_to_string(files.log(&run)).unwrap(), "ran\n");
  assert!(files.read_end(&run).unwrap().expect("the end").forgotten);
  assert_eq!(kinds(&setup).last(), Some(&"forget"));
  assert!(said.lines().iter().any(|line| line.ends_with(&format!("run {run} ended on studio"))), "{:#?}", said.lines());
}
