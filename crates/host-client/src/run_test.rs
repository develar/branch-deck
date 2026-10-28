use super::*;
use crate::testkit::{ALIAS, AgentStop, FakeProcesses, Hello, Said, Setup, calm};
use host_link::channel::{Channel, Frame};
use host_link::proto::{FileKind, is_run_id};
use pretty_assertions::assert_eq;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;
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
  script: impl Fn(&Request, &mut Channel) -> bool + Send + Sync + 'static,
) -> (Setup, Packed) {
  let packed = Packed::default();
  let has: Arc<Has> = Arc::new(has);
  let setup = Setup::on(repo, Hello::default(), {
    let packed = Arc::clone(&packed);
    move |request, channel| {
      if !script(request, channel) {
        answer_besides_follow(request, channel, &packed, &*has);
      }
    }
  });
  (setup, packed)
}

/// An agent for a whole run whose `Follow` runs `follow` with the offset of the request
fn agent_following(repo: TestRepo, has: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static, follow: impl Fn(u64, &mut Channel) + Send + Sync + 'static) -> (Setup, Packed) {
  agent(repo, has, move |request, channel| match request {
    Request::Follow { offset, .. } => {
      follow(*offset, channel);
      true
    }
    _ => false,
  })
}

/// The replies of an agent for a whole run, besides `Follow`
fn answer_besides_follow(request: &Request, channel: &mut Channel, packed: &Mutex<Vec<(PackTarget, String, Vec<u8>)>>, has: &Has) {
  let _ = match request {
    Request::Have { commits, .. } => channel.send(&Reply::Have { commits: has(commits) }),
    Request::Pack { target, commit, .. } => {
      let mut bytes = Vec::new();
      let _ = channel.recv_stream(&mut bytes);
      packed.lock().unwrap_or_else(PoisonError::into_inner).push((target.clone(), commit.clone(), bytes));
      channel.send(&Reply::Packed)
    }
    Request::Start(spec) => channel.send(&Reply::Started { run: spec.run.clone() }),
    Request::Fetch { .. } => {
      send_kind(channel, "out/bt-runs/0000000002000/test.log", b"hello", FileKind::Staged);
      channel.send(&Reply::Fetched { files: 1, bytes: 5 })
    }
    Request::Forget { .. } | Request::Cancel { .. } => channel.send(&Reply::Done { message: "removed".to_owned() }),
    _ => channel.send(&Reply::refused(Code::BadRequest, "not in this script")),
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
fn run_in(setup: &Setup, say: &(dyn Fn(&str) + Sync), presses: &(dyn Fn() -> u32 + Sync), fetch: &[String], options: RunOptions) -> Result<RunOutcome, RunFailure> {
  run_holding(setup, say, presses, fetch, options, Duration::ZERO)
}

/// `run_in` whose build of the pack waits `hold` before git starts
#[expect(clippy::result_large_err, reason = "the answer of `Client::run`")]
fn run_holding(
  setup: &Setup,
  say: &(dyn Fn(&str) + Sync),
  presses: &(dyn Fn() -> u32 + Sync),
  fetch: &[String],
  options: RunOptions,
  hold: Duration,
) -> Result<RunOutcome, RunFailure> {
  let command = command();
  let processes = FakeProcesses::default();
  let request = RunRequest {
    checkout: setup.repo.path(),
    repo: None,
    affinity: AFFINITY,
    command: &command,
    fetch,
  };
  setup.client(&processes, say, presses).with_hold_pack(hold).run(ALIAS, &request, options)
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
    Request::Runs => "runs",
    Request::Slots => "slots",
    Request::Upgrade { .. } => "upgrade",
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

/// Waits until `flag` is set, up to 5 s, as a command of the host that waits for an event
fn wait_for(flag: &AtomicBool) {
  let started = Instant::now();
  while !flag.load(Ordering::SeqCst) && started.elapsed() < Duration::from_secs(5) {
    thread::sleep(Duration::from_millis(20));
  }
}

fn exit(channel: &mut Channel, code: i32) {
  let _ = channel.send(&Reply::Exit { code });
}

fn done(channel: &mut Channel, message: &str) {
  let _ = channel.send(&Reply::Done { message: message.to_owned() });
}

fn step(name: &str, ms: u64) -> Phase {
  Phase { name: name.to_owned(), ms }
}

fn output(channel: &mut Channel, stream: Stream, text: &str, end: u64) {
  let _ = channel.send(&Reply::Output {
    stream,
    text: text.to_owned(),
    end,
  });
}

fn send_kind(channel: &mut Channel, path: &str, bytes: &[u8], kind: FileKind) {
  let _ = channel.send(&Reply::File {
    path: path.to_owned(),
    size: bytes.len() as u64,
    executable: false,
    kind,
  });
  let _ = channel.send_bytes(bytes);
  let _ = channel.send_end();
}

/// Raises the Ctrl+C count to `presses` once the agent got a request of the kind `after`, as a person who presses
/// after the start. The thread gives up after 5 s
fn press_after(setup: &Setup, after: &'static str, presses: u32) -> (Arc<AtomicU32>, thread::JoinHandle<()>) {
  let count = Arc::new(AtomicU32::new(0));
  let (pressed, agent) = (Arc::clone(&count), setup.agent.requests_handle());
  let watcher = thread::spawn(move || {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
      if agent.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|request| kind(request) == after) {
        pressed.store(presses, Ordering::SeqCst);
        return;
      }
      thread::sleep(Duration::from_millis(20));
    }
  });
  (count, watcher)
}

/// The run id of an outcome
fn run_of(outcome: &RunOutcome) -> &str {
  outcome.facts.run.as_deref().unwrap()
}

/// The order of a run: `Have` and `Start` first, then the follow beside the pack, then the base sync of
/// `origin/master`, the fetch and the forget. The output streams, the steps of the client wrap the steps of the host,
/// and the record names what the fetch takes
#[test]
fn a_run_starts_follows_packs_syncs_the_base_and_fetches_in_order() {
  let Commits { repo, first, head } = two_commits_with_a_base();
  let base = repo.rev_parse("refs/remotes/origin/master").unwrap();
  let based = Arc::new(AtomicBool::new(false));
  let (setup, packed) = agent(repo, known(vec![first.clone()]), {
    let based = Arc::clone(&based);
    move |request, channel| match request {
      Request::Follow { .. } => {
        output(channel, Stream::Stderr, "progress", 30);
        let _ = channel.send(&Reply::Heartbeat { elapsed_ms: 15_000 });
        output(channel, Stream::Stdout, "the digest", 60);
        // A long command: it ends after the base came, or after 5 s
        wait_for(&based);
        let _ = channel.send(&Reply::Phases {
          phases: vec![step("slot", 0), step("command", 25_100)],
        });
        exit(channel, 3);
        true
      }
      Request::Pack { target: PackTarget::Base(_), .. } => {
        let _ = channel.recv_stream(&mut Vec::new());
        based.store(true, Ordering::SeqCst);
        let _ = channel.send(&Reply::Packed);
        true
      }
      _ => false,
    }
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &fetch(), RunOptions::default()).unwrap();

  assert_eq!((outcome.code, outcome.stdout.as_str()), (3, "the digest\n"));
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
  assert!(lines.contains(&format!("bd-host: run {} of {} on studio", run_of(&outcome), &head[..12])), "{lines:#?}");
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
    serde_json::from_str(&std::fs::read_to_string(setup.repo.path().join(format!(".git/branch-deck/host/runs/{}.json", run_of(&outcome)))).unwrap()).unwrap();
  assert_eq!(record, serde_json::json!({"commit": head, "fetch": ["out/bt-runs"], "outputs": [], "changes": false}));
}

/// The working tree ships as one commit of `bd-host` on HEAD, and the index stays. The same tree is the same commit,
/// so a host that has it gets no pack
#[test]
fn the_working_tree_ships_as_one_commit_and_the_same_tree_sends_no_pack() {
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
    |request, channel| match request {
      Request::Follow { .. } => {
        exit(channel, 0);
        true
      }
      _ => false,
    },
  );
  let options = RunOptions {
    working_tree: true,
    ..RunOptions::default()
  };
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let first = run_in(&setup, &say, &calm, &[], options).unwrap();
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
      "bd-host: run {} of {} on studio: the working tree, HEAD {} and 2 changed paths",
      run_of(&first),
      &commit[..12],
      &head[..12]
    )),
    "{:#?}",
    said.lines()
  );
  assert_eq!(packed.lock().unwrap_or_else(PoisonError::into_inner).len(), 1);

  has.lock().unwrap_or_else(PoisonError::into_inner).push(commit.clone());
  let second = run_in(&setup, &|_| {}, &calm, &[], options).unwrap();
  assert_eq!(second.facts.commit.as_deref(), Some(commit.as_str()), "the same tree on the same HEAD is the same commit");
  assert_eq!(packed.lock().unwrap_or_else(PoisonError::into_inner).len(), 1, "the second run sends no pack");
}

/// `--head` refuses a checkout with local changes. The refusal closes the `Have` that goes on meanwhile, so it comes at
/// once even from a host that does not answer, and nothing starts
#[test]
fn head_refuses_a_checkout_with_local_changes() {
  let Commits { repo, .. } = two_commits();
  std::fs::write(repo.path().join("a.txt"), "a\nchanged\n").unwrap();
  let setup = Setup::on(repo, Hello::default(), |_, _| thread::sleep(Duration::from_secs(10)));
  let started = Instant::now();
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::LocalChanges, Exit::NotRun));
  assert!(failure.refusal.message.contains(" M a.txt"), "{}", failure.refusal.message);
  assert!(started.elapsed() < Duration::from_secs(8), "the refusal waited for the host: {:?}", started.elapsed());
  assert!(kinds(&setup).iter().all(|kind| *kind == "have"), "{:?}", kinds(&setup));
}

/// A host without a near commit gets the far candidates of the first-parent walk, and the pack builds on the commit
/// that it has from there. A host that has none of them refuses with `behind` and the count of the history
#[test]
fn a_host_without_a_near_commit_gets_the_far_candidates_then_refuses_behind() {
  let commits: Vec<HistoryCommit> = (0..40).map(|n| HistoryCommit::new(format!("Commit {n}"), &[("n.txt", &format!("{n}\n"))])).collect();
  let (repo, ids) = TestRepo::with_history(&commits);
  let (setup, packed) = agent_following(repo, known(ids[..=7].to_vec()), |_, channel| exit(channel, 0));
  run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).unwrap();
  let haves = haves(&setup);
  assert_eq!(haves.len(), 2, "{haves:#?}");
  assert_eq!(haves[0].1, [39, 38, 37, 35, 31, 23].map(|n| ids[n].clone()), "the distances up to 16");
  assert_eq!(haves[1].1, [39, 38, 37, 35, 31, 23, 7].map(|n| ids[n].clone()), "the distances up to 65536");
  assert_eq!(packed.lock().unwrap_or_else(PoisonError::into_inner).len(), 1);

  let (repo, ids) = TestRepo::with_history(&commits);
  let (setup, _) = agent(repo, |_| Vec::new(), |_, _| false);
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Behind, Exit::NotRun));
  assert!(failure.refusal.message.contains("the whole history of 40 commits"), "{}", failure.refusal.message);
  assert_eq!(kinds(&setup), ["have", "have"], "the near candidates, then the long walk; no start");
  assert_eq!(failure.facts.commit.as_deref(), Some(ids[39].as_str()));
}

/// The failure of a run whose pack `on_pack` of the agent breaks. The agent knows the first commit, takes the cancel,
/// and sends nothing to a follow. Checks what every failed pack does: the client cancels the started run, says that the
/// cancel came, and sends no other request
fn failed_pack(commits: Commits, on_pack: impl Fn(&mut Channel) + Send + Sync + 'static) -> RunFailure {
  let Commits { repo, first, .. } = commits;
  let (setup, _) = agent(repo, known(vec![first]), move |request, channel| match request {
    Request::Pack { .. } => {
      on_pack(channel);
      true
    }
    Request::Cancel { .. } => {
      done(channel, "the host has the cancel");
      true
    }
    Request::Follow { .. } => true,
    _ => false,
  });
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).unwrap_err();
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
#[test]
fn a_failed_pack_cancels_the_run_and_refuses_with_its_code() {
  let failure = failed_pack(two_commits(), |channel| {
    let _ = channel.recv_stream(&mut Vec::new());
    let _ = channel.send(&Reply::refused(Code::PackFailed, "git index-pack failed"));
  });
  assert_eq!(failure.refusal.code, Code::PackFailed);
  assert!(
    failure.refusal.message.starts_with("the pack did not reach studio: studio: git index-pack failed"),
    "{}",
    failure.refusal.message
  );
}

/// The run goes to the repository of the same name as the local one beside the repository of the host file, and to
/// the one of the host file when the host lacks it
#[test]
fn the_repository_of_a_run_is_the_one_of_the_same_name_beside_the_host_file() {
  let by_name = RunOptions {
    repo_by_name: true,
    ..RunOptions::default()
  };
  let Commits { repo, head, .. } = two_commits();
  let name = repo.path().file_name().unwrap().to_string_lossy().into_owned();
  let sibling = format!("~/projects/{name}");
  let (setup, _) = agent_following(repo, known(vec![head]), |_, channel| exit(channel, 0));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], by_name).unwrap();
  assert_eq!(start_spec(&setup).repo, sibling);
  assert_eq!(outcome.facts.repository.as_deref(), Some(sibling.as_str()));
  assert!(said.lines().iter().any(|line| line.ends_with(&format!("on studio in {sibling}"))), "{:#?}", said.lines());

  let Commits { repo, head, .. } = two_commits();
  let missing = format!("~/projects/{}", repo.path().file_name().unwrap().to_string_lossy());
  let (setup, _) = agent(repo, known(vec![head]), move |request, channel| match request {
    Request::Have { repo, .. } if *repo == missing => {
      let _ = channel.send(&Reply::refused(Code::RepoMissing, "no git repository there"));
      true
    }
    Request::Follow { .. } => {
      exit(channel, 0);
      true
    }
    _ => false,
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  run_in(&setup, &say, &calm, &[], by_name).unwrap();
  assert_eq!(start_spec(&setup).repo, "~/projects/idea-1");
  assert!(
    said
      .lines()
      .iter()
      .any(|line| line.starts_with("bd-host: studio has no ~/projects/") && line.contains("the run goes to ~/projects/idea-1")),
    "{:#?}",
    said.lines()
  );
}

/// A lost connection follows again from the last offset, and a record that came twice shows once. A run that vanished
/// on the host after its start may have run its command, so it exits 255, and the client forgets nothing
#[test]
fn a_lost_follow_connects_again_from_its_offset_and_an_unknown_run_is_255() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, _) = agent_following(repo, known(vec![head]), |offset, channel| {
    output(channel, Stream::Stderr, "first", 10);
    if offset > 0 {
      output(channel, Stream::Stdout, "done", 20);
      exit(channel, 0);
    }
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], RunOptions::default()).unwrap();
  assert_eq!(outcome.stdout, "done\n");
  let lines = said.lines();
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

  let Commits { repo, head, .. } = two_commits();
  let (setup, _) = agent_following(repo, known(vec![head]), |_, channel| {
    output(channel, Stream::Stderr, "building", 10);
    let _ = channel.send(&Reply::refused(Code::UnknownRun, "the agent keeps no run"));
  });
  let failure = run_in(&setup, &|_| {}, &calm, &fetch(), RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::UnknownRun, Exit::Refused));
  assert!(failure.refusal.message.contains("whether its command ran is unknown"), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("`bd-host runs studio`"), "{}", failure.refusal.message);
  assert!(!kinds(&setup).contains(&"forget"), "{:?}", kinds(&setup));
}

/// An agent that stops answering ends the run after the reconnect window with `timed_out` after the start (255), and
/// the refusal names `bd-host follow`
/// An agent that ends while the client follows its run refuses each new connection. The command can still run on the
/// host, so after the reconnect window the run ends with `timed_out` and exit 255, never with the 75 of `unreachable`
#[test]
fn an_agent_that_ends_during_the_follow_times_out_with_255_and_not_75() {
  let Commits { repo, head, .. } = two_commits();
  let stopper: Arc<OnceLock<AgentStop>> = Arc::default();
  let (setup, _) = agent_following(repo, known(vec![head]), {
    let stopper = Arc::clone(&stopper);
    move |_, channel| {
      output(channel, Stream::Stderr, "building", 10);
      stopper.get().expect("the stop of the agent").stop();
    }
  });
  assert!(stopper.set(setup.agent.stopper()).is_ok());
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let failure = run_in(&setup, &say, &calm, &[], RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::TimedOut, Exit::Refused), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("`bd-host follow studio "), "{}", failure.refusal.message);
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

#[test]
fn a_silent_agent_times_out_after_the_reconnect_window() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, _) = agent_following(repo, known(vec![head]), |_, _| thread::sleep(Duration::from_secs(4)));
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::TimedOut, Exit::Refused));
  assert!(failure.refusal.message.contains("`bd-host follow studio "), "{}", failure.refusal.message);
  assert!(!kinds(&setup).contains(&"forget"), "a run whose end the client did not see stays on the host");
}

const PATCH: &str = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+a formatted\n";

/// An agent of a run on `repo` whose host has every commit, whose command exits 0, and whose fetch sends `files`
fn fetching_agent(repo: TestRepo, files: Vec<(String, FileKind, Vec<u8>)>) -> Setup {
  agent(repo, <[String]>::to_vec, move |request, channel| match request {
    Request::Follow { .. } => {
      exit(channel, 0);
      true
    }
    Request::Fetch { .. } => {
      for (path, kind, bytes) in &files {
        send_kind(channel, path, bytes, kind.clone());
      }
      let _ = channel.send(&Reply::Fetched { files: 0, bytes: 0 });
      true
    }
    _ => false,
  })
  .0
}

/// The options of `bd-host run --apply`: the working tree, so a second run ships the changes of the first one
const APPLY: RunOptions = RunOptions {
  working_tree: true,
  nice: None,
  outputs: false,
  apply: true,
  repo_by_name: false,
};

/// The patch of the changes of a command applies to the files of the working tree, not to the index, and stays in the
/// git directory. A patch that does not apply refuses with `apply_failed` after the command (255), names the patch,
/// and the working tree stays as it was
#[test]
fn apply_brings_the_changes_back_all_or_nothing() {
  let changes = || ("changes.patch".to_owned(), FileKind::Changes, PATCH.as_bytes().to_vec());
  let Commits { repo, .. } = two_commits();
  let setup = fetching_agent(repo, vec![changes()]);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], APPLY).unwrap();
  assert_eq!(outcome.facts.applied_files, Some(1));
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("a.txt")).unwrap(), "a formatted\n");
  assert_eq!(setup.repo.git(&["diff", "--cached", "--name-only"]), "", "the index stays");
  let root = std::fs::canonicalize(setup.repo.path()).unwrap();
  let patch = root.join(format!(".git/branch-deck/host/runs/{}.patch", run_of(&outcome)));
  assert_eq!(std::fs::read_to_string(&patch).unwrap(), PATCH);
  // the line names the patch as git names the git directory: on Windows with `/` and `\` both
  let named = said
    .lines()
    .iter()
    .find_map(|line| {
      line
        .strip_prefix("bd-host: applied the changes of the command to 1 files of this working tree; `git apply -R ")?
        .strip_suffix("` undoes them")
        .map(str::to_owned)
    })
    .unwrap_or_else(|| panic!("no line of the apply: {:#?}", said.lines()));
  assert_eq!(std::fs::canonicalize(named).unwrap(), patch);

  // The working tree has `a formatted` now, so the same patch no longer applies
  let failure = run_in(&setup, &|_| {}, &calm, &[], APPLY).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit, failure.command_exit), (Code::ApplyFailed, Exit::Refused, Some(0)));
  assert!(failure.refusal.message.contains("exited with 0"), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("The patch is "), "{}", failure.refusal.message);
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("a.txt")).unwrap(), "a formatted\n", "nothing applied");

  let Commits { repo, .. } = two_commits();
  let quiet = fetching_agent(repo, Vec::new());
  let said = Said::default();
  let say = |line: &str| said.push(line);
  assert_eq!(run_in(&quiet, &say, &calm, &[], APPLY).unwrap().facts.applied_files, Some(0));
  assert!(said.lines().contains(&"bd-host: the command changed no file".to_owned()), "{:#?}", said.lines());
}

/// Changes that did not come, because the fetch was cut, refuse with `apply_failed` and name the fetch that copies
/// them; a cut fetch without the apply refuses with `fetch_failed`, leaves no part and keeps the run on the host
#[test]
fn a_cut_fetch_refuses_after_the_command_and_keeps_the_run() {
  let cut = |kind: FileKind| {
    move |request: &Request, channel: &mut Channel| match request {
      Request::Follow { .. } => {
        exit(channel, 3);
        true
      }
      Request::Fetch { .. } => {
        let _ = channel.send(&Reply::File {
          path: "out/bt-runs/1/big.log".to_owned(),
          size: 10,
          executable: false,
          kind: kind.clone(),
        });
        let _ = channel.send_bytes(b"four");
        true
      }
      _ => false,
    }
  };
  let Commits { repo, .. } = two_commits();
  let (setup, _) = agent(repo, <[String]>::to_vec, cut(FileKind::Staged));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let failure = run_in(&setup, &say, &calm, &fetch(), RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit, failure.command_exit), (Code::FetchFailed, Exit::Refused, Some(3)));
  assert!(failure.refusal.message.contains("`bd-host fetch studio "), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("`bd-host forget studio "), "{}", failure.refusal.message);
  let root = setup.repo.path();
  assert!(!root.join("out/bt-runs/1/big.log").exists());
  assert!(!root.join("out/bt-runs/1/big.log.bd-host-part").exists());
  assert!(!kinds(&setup).contains(&"forget"));
  assert!(failure.facts.phases.iter().any(|phase| phase.name == "fetch"), "{:?}", failure.facts.phases);

  let Commits { repo, .. } = two_commits();
  let (setup, _) = agent(repo, <[String]>::to_vec, cut(FileKind::Changes));
  let failure = run_in(&setup, &|_| {}, &calm, &[], APPLY).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::ApplyFailed, Exit::Refused));
  assert!(failure.refusal.message.contains("did not come"), "{}", failure.refusal.message);
}

/// An agent whose `Follow` waits for the cancel of the run, up to 5 s, then ends the run with 130 and the line `said`.
/// The answer is the flag of the cancel
fn agent_that_waits_for_the_cancel(repo: TestRepo, has: impl Fn(&[String]) -> Vec<String> + Send + Sync + 'static, said: &'static str) -> (Setup, Arc<AtomicBool>) {
  let cancelled = Arc::new(AtomicBool::new(false));
  let (setup, _) = agent(repo, has, {
    let cancelled = Arc::clone(&cancelled);
    move |request, channel| match request {
      Request::Follow { .. } => {
        wait_for(&cancelled);
        output(channel, Stream::Stderr, said, 20);
        exit(channel, 130);
        true
      }
      Request::Cancel { .. } => {
        cancelled.store(true, Ordering::SeqCst);
        done(channel, "the supervisor has the cancel request");
        true
      }
      _ => false,
    }
  });
  (setup, cancelled)
}

/// Runs to the end with one Ctrl+C after the first `Follow`, and answers the outcome and the said lines. Ctrl+C before
/// the start would refuse the run
fn run_with_ctrl_c_after_the_follow(setup: &Setup) -> (RunOutcome, Vec<String>) {
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let (count, watcher) = press_after(setup, "follow", 1);
  let presses = || count.load(Ordering::SeqCst);
  let outcome = run_in(setup, &say, &presses, &[], RunOptions::default()).unwrap();
  watcher.join().unwrap();
  (outcome, said.lines())
}

/// Ctrl+C after the start sends `Cancel` on its own connection, and the output follows up to the exit
#[test]
fn ctrl_c_cancels_the_run_and_follows_it_to_its_end() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, cancelled) = agent_that_waits_for_the_cancel(repo, known(vec![head]), "interrupted");
  let (outcome, lines) = run_with_ctrl_c_after_the_follow(&setup);
  assert_eq!(outcome.code, 130);
  assert!(cancelled.load(Ordering::SeqCst));
  assert!(lines.iter().any(|line| line.starts_with("bd-host: Ctrl+C: cancelling the run")), "{lines:#?}");
  assert!(lines.contains(&"interrupted".to_owned()), "{lines:#?}");
}

/// Ctrl+C before the start runs nothing on the host and exits 130
#[test]
fn ctrl_c_before_the_start_reaches_no_host() {
  let Commits { repo, first, .. } = two_commits();
  let pressed = Arc::new(AtomicBool::new(false));
  let (setup, _) = agent(repo, known(vec![first]), {
    let pressed = Arc::clone(&pressed);
    move |request, _| {
      if let Request::Have { .. } = request {
        pressed.store(true, Ordering::SeqCst);
      }
      false
    }
  });
  let presses = || u32::from(pressed.load(Ordering::SeqCst));
  let failure = run_in(&setup, &|_| {}, &presses, &[], RunOptions::default()).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Interrupted, Exit::Interrupted));
  assert_eq!(kinds(&setup), ["have"]);
}

/// Ctrl+C while the pack builds cancels the run on the host, and the output follows up to its end
#[test]
fn ctrl_c_during_the_build_of_the_pack_cancels_the_run_on_the_host() {
  let Commits { repo, first, .. } = two_commits();
  let (setup, cancelled) = agent_that_waits_for_the_cancel(repo, known(vec![first]), "bd-host: the run was cancelled before its command");
  let (count, watcher) = press_after(&setup, "pack", 1);
  let presses = || count.load(Ordering::SeqCst);
  let started = Instant::now();
  let outcome = run_holding(&setup, &|_| {}, &presses, &[], RunOptions::default(), Duration::from_secs(5)).unwrap();
  watcher.join().unwrap();
  assert_eq!(outcome.code, 130, "the run ends with the code of the host");
  assert!(cancelled.load(Ordering::SeqCst));
  assert!(started.elapsed() < Duration::from_secs(5), "the build was not stopped: {:?}", started.elapsed());
}

/// The host takes the slot while git builds the pack, and empty chunks keep the connection of the pack alive
/// meanwhile. The follow starts with the run, so the lines of the host show during the build
#[test]
fn the_pack_builds_after_the_start_and_keeps_its_connection_alive() {
  let Commits { repo, first, .. } = two_commits();
  let keepalives = Arc::new(AtomicU32::new(0));
  let (setup, _) = agent(repo, known(vec![first]), {
    let keepalives = Arc::clone(&keepalives);
    move |request, channel| match request {
      Request::Pack { .. } => {
        while let Ok(Frame::Bytes(chunk)) = channel.recv_frame() {
          if chunk.is_empty() {
            keepalives.fetch_add(1, Ordering::SeqCst);
          }
        }
        let _ = channel.send(&Reply::Packed);
        true
      }
      Request::Follow { .. } => {
        output(channel, Stream::Stderr, "bd-host: slot /pool/1 (warm)", 30);
        exit(channel, 0);
        true
      }
      _ => false,
    }
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_holding(&setup, &say, &calm, &[], RunOptions::default(), Duration::from_millis(400)).unwrap();
  assert_eq!(outcome.code, 0);
  let sent = keepalives.load(Ordering::SeqCst);
  assert!(sent >= 3, "{sent} empty chunks");
  let lines = said.lines();
  let at = |prefix: &str| lines.iter().position(|line| line.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix} in {lines:#?}"));
  assert!(at("bd-host: slot /pool/1 (warm)") < at("> studio: pack of 1 commits"), "{lines:#?}");
  assert_eq!(
    outcome.facts.phases.iter().map(|phase| phase.name.as_str()).collect::<Vec<_>>(),
    ["pack", "upload", "total"]
  );
}

/// A second Ctrl+C stops the follow at once, while the agent only sends heartbeats. The run stays on the host
#[test]
fn a_second_ctrl_c_stops_the_follow_at_once() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, _) = agent_following(repo, known(vec![head]), |_, channel| {
    for _ in 0..50 {
      thread::sleep(Duration::from_millis(200));
      if channel.send(&Reply::Heartbeat { elapsed_ms: 200 }).is_err() {
        return;
      }
    }
  });
  let (count, watcher) = press_after(&setup, "follow", 2);
  let presses = || count.load(Ordering::SeqCst);
  let failure = run_in(&setup, &|_| {}, &presses, &[], RunOptions::default()).unwrap_err();
  watcher.join().unwrap();
  assert_eq!((failure.refusal.code, failure.exit), (Code::Interrupted, Exit::Interrupted));
  assert!(failure.refusal.message.contains("`bd-host follow studio "), "{}", failure.refusal.message);
  assert!(!kinds(&setup).contains(&"forget"));
}

/// A refusal of the agent keeps its code; before the command it exits 75, after it 255. A run that failed on the host
/// is removed there all the same, and the client says the steps of the host
#[test]
fn a_refusal_of_the_agent_exits_by_how_far_the_run_came() {
  let refused_after = |phases: Vec<Phase>, code: Code| {
    let Commits { repo, head, .. } = two_commits();
    let (setup, _) = agent_following(repo, known(vec![head]), move |_, channel| {
      let _ = channel.send(&Reply::Phases { phases: phases.clone() });
      let _ = channel.send(&Reply::refused(code, "the supervisor failed"));
    });
    let said = Said::default();
    let say = |line: &str| said.push(line);
    let failure = run_in(&setup, &say, &calm, &[], RunOptions::default()).unwrap_err();
    assert_eq!(kinds(&setup).last(), Some(&"forget"));
    (failure, said.lines())
  };
  let (before, lines) = refused_after(vec![step("slot", 0), step("prepare", 3_909_000)], Code::AgentError);
  assert_eq!((before.refusal.code, before.exit), (Code::AgentError, Exit::NotRun));
  assert_eq!(before.refusal.message, "studio: the supervisor failed");
  assert!(lines.contains(&"bd-host: phases: slot 0.0s, prepare 1h05m09s".to_owned()), "{lines:#?}");
  let (after, _) = refused_after(vec![step("prepare", 10), step("command", 10)], Code::AgentError);
  assert_eq!(after.exit, Exit::Refused);
  let (lease, _) = refused_after(vec![step("slot", 10)], Code::LeaseFailed);
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
  };
  let no_host = Client::new(crate::config::Config::new(dir.path().join("config")), crate::testkit::build(), &processes, &|_| {}, &calm)
    .run(ALIAS, &request, RunOptions::default())
    .unwrap_err();
  assert_eq!((no_host.refusal.code, no_host.exit.code()), (Code::NoHost, 75));
}

/// A start whose reply does not come can leave a run on the agent, which would wait for its pack. The client cancels it
#[test]
fn a_start_without_its_reply_is_cancelled() {
  let Commits { repo, head, .. } = two_commits();
  let (setup, _) = agent(repo, known(vec![head]), |request, _| matches!(request, Request::Start(_)));
  let failure = run_in(&setup, &|_| {}, &calm, &[], RunOptions::default()).unwrap_err();
  assert!(matches!(failure.refusal.code, Code::Link | Code::TimedOut), "{failure:?}");
  assert_eq!(failure.exit, Exit::NotRun);
  assert!(failure.refusal.message.starts_with("the start of the run "), "{}", failure.refusal.message);
  assert!(failure.refusal.message.contains("; studio has the cancel of the run "), "{}", failure.refusal.message);
  assert_eq!(kinds(&setup), ["have", "start", "cancel"]);
}

/// A commit whose `[host]` does not parse starts no run that asks for its outputs
#[test]
fn a_config_that_does_not_parse_starts_no_run_with_outputs() {
  let repo = TestRepo::new();
  repo.create_commit("Add the config", ".branchdeck.toml", "[host]\noutputs = \"results\"\n");
  let setup = fetching_agent(repo, Vec::new());
  let outputs = RunOptions {
    outputs: true,
    ..RunOptions::default()
  };
  let failure = run_in(&setup, &|_| {}, &calm, &[], outputs).unwrap_err();
  assert_eq!((failure.refusal.code, failure.exit), (Code::BadConfig, Exit::NotRun));
  assert!(!kinds(&setup).contains(&"start"), "{:?}", kinds(&setup));
}

/// The outputs of `[host]` of the shipped commit go into the record, so the fetch takes their files
#[test]
fn the_outputs_of_the_commit_come_back_in_place_of_their_local_copies() {
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
  let outcome = run_in(&setup, &|_| {}, &calm, &[], outputs).unwrap();
  assert_eq!(outcome.facts.fetched, ["results/new.txt"]);
  assert!(!setup.repo.path().join("results/old.txt").exists());
  assert!(start_spec(&setup).outputs);
}

#[test]
fn runs_cancel_and_slots_answer_the_replies_of_the_agent() {
  let setup = Setup::new(|request, channel| {
    let _ = match request {
      Request::Runs => channel.send(&Reply::Runs { runs: Vec::new() }),
      Request::Slots => channel.send(&Reply::Slots {
        repositories: vec![host_link::proto::RepositorySlots {
          repo: "~/projects/idea-1".to_owned(),
          table: serde_json::json!({"slots": []}),
        }],
      }),
      _ => channel.send(&Reply::refused(Code::BadRequest, "no")),
    };
  });
  let processes = FakeProcesses::default();
  let client = setup.client(&processes, &|_| {}, &calm);
  assert_eq!(client.runs(ALIAS).unwrap().len(), 0);
  assert_eq!(client.slots(ALIAS).unwrap()[0].repo, "~/projects/idea-1");
  assert_eq!(client.cancel(ALIAS, "r1").unwrap_err().code, Code::BadRequest);
}

/// `Reply::Phases` belongs to `Follow` only. Anywhere else it is a reply out of turn
#[test]
fn the_steps_of_a_run_outside_a_follow_are_out_of_turn() {
  let setup = Setup::new(|_, channel| {
    let _ = channel.send(&Reply::Phases { phases: Vec::new() });
  });
  let processes = FakeProcesses::default();
  assert_eq!(setup.client(&processes, &|_| {}, &calm).runs(ALIAS).unwrap_err().code, Code::Link);
}

/// A request that waits gets heartbeats before its reply, and each heartbeat starts the read deadline again. The wait
/// here is twice the deadline of the short timing
#[test]
fn heartbeats_keep_a_waiting_request_alive() {
  let setup = Setup::new(|request, channel| {
    if matches!(request, Request::Runs) {
      for _ in 0..4 {
        thread::sleep(Duration::from_millis(1000));
        let _ = channel.send(&Reply::Heartbeat { elapsed_ms: 1000 });
      }
      let _ = channel.send(&Reply::Runs { runs: Vec::new() });
    }
  });
  let processes = FakeProcesses::default();
  assert_eq!(setup.client(&processes, &|_| {}, &calm).runs(ALIAS).unwrap().len(), 0);
}

/// Two commits whose `origin/master` is a commit beside HEAD on the first one, so a base sync sends a pack
fn two_commits_with_a_base() -> Commits {
  let commits = two_commits();
  let tree = commits.repo.git(&["rev-parse", &format!("{}^{{tree}}", commits.first)]);
  let base = commits.repo.git(&["commit-tree", &tree, "-p", &commits.first, "-m", "Side"]);
  commits.repo.git(&["update-ref", "refs/remotes/origin/master", &base]);
  commits
}

/// Ctrl+C after the end of the run closes the connection of the base sync, also while the host indexes the base, and
/// says no warning
#[test]
fn ctrl_c_stops_the_base_sync_while_the_host_indexes_it() {
  let Commits { repo, first, .. } = two_commits_with_a_base();
  let indexing = Arc::new(AtomicBool::new(false));
  let (setup, _) = agent(repo, known(vec![first]), {
    let indexing = Arc::clone(&indexing);
    move |request, channel| match request {
      Request::Pack { target: PackTarget::Base(_), .. } => {
        let _ = channel.recv_stream(&mut Vec::new());
        indexing.store(true, Ordering::SeqCst);
        for _ in 0..50 {
          thread::sleep(Duration::from_millis(200));
          if channel.send(&Reply::Heartbeat { elapsed_ms: 200 }).is_err() {
            break;
          }
        }
        let _ = channel.send(&Reply::Packed);
        true
      }
      Request::Follow { .. } => {
        exit(channel, 0);
        true
      }
      _ => false,
    }
  });
  let presses = || u32::from(indexing.load(Ordering::SeqCst));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let started = Instant::now();
  assert_eq!(run_in(&setup, &say, &presses, &[], RunOptions::default()).unwrap().code, 0);
  assert!(started.elapsed() < Duration::from_secs(8), "the sync held the end: {:?}", started.elapsed());
  assert!(!said.lines().iter().any(|line| line.contains("warning")), "{:#?}", said.lines());
}

/// An agent that refuses the base gives one warning line, and the run ends with the code of its command
#[test]
fn a_failed_base_sync_only_warns() {
  let Commits { repo, first, .. } = two_commits_with_a_base();
  let (setup, _) = agent(repo, known(vec![first]), |request, channel| match request {
    Request::Pack { target: PackTarget::Base(_), .. } => {
      let _ = channel.recv_stream(&mut Vec::new());
      let _ = channel.send(&Reply::refused(Code::BadRequest, "no base here"));
      true
    }
    Request::Follow { .. } => {
      thread::sleep(Duration::from_millis(300));
      exit(channel, 4);
      true
    }
    _ => false,
  });
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let outcome = run_in(&setup, &say, &calm, &[], RunOptions::default()).unwrap();
  assert_eq!(outcome.code, 4);
  let warnings: Vec<String> = said.lines().into_iter().filter(|line| line.contains("warning")).collect();
  assert_eq!(warnings.len(), 1, "{warnings:#?}");
  assert!(
    warnings[0].starts_with("bd-host: warning: cannot sync the base of the next packs to studio (bad_request): "),
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

/// The smallest pack of `two_commits_with_a_large_file`. It is much larger than the first socket buffers of the
/// loopback (128 KiB on macOS), so the writes of the client wait while the agent does not read
const LARGE_PACK: usize = 1024 * 1024;

/// Two commits whose second one adds a file of noise, so the pack of that commit holds more than `LARGE_PACK` bytes
fn two_commits_with_a_large_file() -> Commits {
  let repo = TestRepo::new();
  let first = repo.create_commit("Add a", "a.txt", "a\n");
  let head = repo.create_commit("Add large", "large.txt", &noise(LARGE_PACK * 3 / 2));
  Commits { repo, first, head }
}

/// An agent that reads the pack late: each write of the pack waits up to the read deadline, not up to the short
/// connect deadline. The stall of 2 s is four times the connect deadline, so a write with that deadline fails
#[test]
fn a_slow_agent_reader_takes_a_large_pack() {
  let Commits { repo, first, .. } = two_commits_with_a_large_file();
  let packed = Arc::new(Mutex::new(Vec::new()));
  let (setup, _) = agent(repo, known(vec![first]), {
    let packed = Arc::clone(&packed);
    move |request, channel| match request {
      // The first chunk of the pack comes after the build, so the wait holds the upload and not the build
      Request::Pack { .. } => {
        let mut bytes = Vec::new();
        while let Ok(Frame::Bytes(chunk)) = channel.recv_frame() {
          if bytes.is_empty() && !chunk.is_empty() {
            thread::sleep(Duration::from_secs(2));
          }
          bytes.extend_from_slice(&chunk);
        }
        *packed.lock().unwrap_or_else(PoisonError::into_inner) = bytes;
        let _ = channel.send(&Reply::Packed);
        true
      }
      Request::Follow { .. } => {
        exit(channel, 0);
        true
      }
      _ => false,
    }
  });
  let timing = Timing {
    connect: Duration::from_millis(500),
    read: Duration::from_secs(5),
    ..crate::testkit::FAST
  };
  let (command, processes) = (command(), FakeProcesses::default());
  let request = RunRequest {
    checkout: setup.repo.path(),
    repo: None,
    affinity: AFFINITY,
    command: &command,
    fetch: &[],
  };
  let outcome = setup
    .client(&processes, &|_| {}, &calm)
    .with_timing(timing)
    .run(ALIAS, &request, RunOptions::default())
    .unwrap();
  assert_eq!(outcome.code, 0);
  let bytes = packed.lock().unwrap_or_else(PoisonError::into_inner);
  assert!(bytes.len() > LARGE_PACK, "{} bytes", bytes.len());
  assert!(bytes.starts_with(b"PACK"));
}

/// An agent that drops the connection in the middle of the pack is a lost link, not a git failure. The run started
/// already, so the client cancels it, and nothing follows it. The checkout of the run waits for the pack, so the command
/// did not run (exit 75)
#[test]
fn a_link_lost_during_the_pack_cancels_the_started_run_with_a_link_refusal() {
  // The empty chunks of the build, then one chunk of the pack, then the connection closes
  let failure = failed_pack(two_commits_with_a_large_file(), |channel| {
    while let Ok(Frame::Bytes(chunk)) = channel.recv_frame() {
      if !chunk.is_empty() {
        break;
      }
    }
  });
  assert!(matches!(failure.refusal.code, Code::Link | Code::TimedOut), "{:?}", failure.refusal);
}

/// A cancel whose first connection fails connects again, and the run still ends with its own exit
#[test]
fn a_failed_cancel_connects_again() {
  let Commits { repo, head, .. } = two_commits();
  let cancels = Arc::new(AtomicU32::new(0));
  let (setup, _) = agent(repo, known(vec![head]), {
    let cancels = Arc::clone(&cancels);
    move |request, channel| match request {
      Request::Follow { .. } => {
        let started = Instant::now();
        while cancels.load(Ordering::SeqCst) < 2 && started.elapsed() < Duration::from_secs(5) {
          thread::sleep(Duration::from_millis(100));
          let _ = channel.send(&Reply::Heartbeat { elapsed_ms: 100 });
        }
        exit(channel, 130);
        true
      }
      // The first cancel gets no reply: the connection closes
      Request::Cancel { .. } => {
        if cancels.fetch_add(1, Ordering::SeqCst) > 0 {
          done(channel, "the host has the cancel");
        }
        true
      }
      _ => false,
    }
  });
  let (outcome, lines) = run_with_ctrl_c_after_the_follow(&setup);
  assert_eq!(outcome.code, 130);
  assert_eq!(cancels.load(Ordering::SeqCst), 2);
  assert!(lines.iter().any(|line| line.starts_with("bd-host: cannot send the cancel of the run ")), "{lines:#?}");
  assert!(lines.contains(&"bd-host: the host has the cancel".to_owned()), "{lines:#?}");
}
