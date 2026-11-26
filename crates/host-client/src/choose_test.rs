use super::*;
use crate::config::SetMember;
use crate::testkit::{ALIAS, FakeAgent, FakeProcesses, Hello, Said, Setup, calm, host_file_of};
use host_link::channel::Channel;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MIN: u64 = 60_000;

/// An estimate of a run that starts in `start` minutes after `unknown` runs of an unknown length, and runs `wall`
/// minutes by a measure of its command (`None`: no measure)
fn estimate(start: u64, unknown: u32, wall: Option<u64>) -> Estimate {
  Estimate {
    has_commit: true,
    warm: true,
    cores: 20,
    expected_cores: wall.map(|_| sync_utils::cores::Cores(60)),
    expected_wall_ms: wall.map(|wall| wall * MIN),
    start_in_ms: start * MIN,
    unknown_ahead: unknown,
    os: "macos".to_owned(),
    slots: Some(4),
  }
}

fn candidate(alias: &str, idle_only: bool, answer: Answer) -> Candidate {
  Candidate {
    alias: alias.to_owned(),
    idle_only,
    answer,
  }
}

/// The aliases of the members in the order that a run tries them
fn order(candidates: &[Candidate], heavy: bool) -> Vec<&str> {
  rank(candidates, heavy).into_iter().map(|position| candidates[position].alias.as_str()).collect()
}

/// The rank is when the run would end on a member: its start, 10 min for each run of an unknown length before it, and
/// its wall; a tie goes to the order of the set. The first member takes any run; a heavy run goes to another member
/// only with its measure of the command, and a member without an estimate ranks last
#[test]
fn the_members_of_a_set_rank_by_when_the_run_would_end_there() {
  let cases: Vec<(&str, Vec<Candidate>, bool, Vec<&str>)> = vec![
    (
      "the member where the run ends first",
      vec![
        candidate("studio", false, Answer::Estimate(estimate(12, 0, Some(4)))),
        candidate("local", true, Answer::Estimate(estimate(0, 0, Some(6)))),
      ],
      true,
      vec!["local", "studio"],
    ),
    (
      "a tie goes to the earlier member",
      vec![
        candidate("studio", false, Answer::Estimate(estimate(2, 0, Some(4)))),
        candidate("local", true, Answer::Estimate(estimate(0, 0, Some(6)))),
      ],
      true,
      vec!["studio", "local"],
    ),
    (
      "each run of an unknown length before it weighs 10 min",
      vec![
        candidate("studio", false, Answer::Estimate(estimate(0, 1, Some(4)))),
        candidate("local", true, Answer::Estimate(estimate(0, 0, Some(6)))),
      ],
      true,
      vec!["local", "studio"],
    ),
    (
      "a heavy run goes to no other member without its measure there",
      vec![
        candidate("studio", false, Answer::Estimate(estimate(30, 0, Some(4)))),
        candidate("local", true, Answer::Estimate(estimate(0, 0, None))),
      ],
      true,
      vec!["studio"],
    ),
    (
      "a run that is not heavy goes to a member without a measure, whose wall is the longest one known",
      vec![
        candidate("studio", false, Answer::Estimate(estimate(0, 0, Some(4)))),
        candidate("local", true, Answer::Estimate(estimate(0, 0, None))),
      ],
      false,
      vec!["studio", "local"],
    ),
    (
      "the first member without an estimate takes the run last, another one never",
      vec![
        candidate("studio", false, Answer::Missing("no answer in 3 s".to_owned())),
        candidate("local", true, Answer::Estimate(estimate(0, 0, Some(6)))),
        candidate("air", true, Answer::Predates),
      ],
      true,
      vec!["local", "studio"],
    ),
  ];
  for (case, candidates, heavy, expected) in &cases {
    assert_eq!(order(candidates, *heavy), *expected, "{case}");
  }
}

/// The plan of a run is one line: what each member told, then the member chosen
#[test]
fn the_plan_of_a_run_names_what_each_member_told_and_the_choice() {
  let candidates = vec![
    candidate("studio", false, Answer::Estimate(estimate(12, 2, Some(4)))),
    candidate("local", true, Answer::Estimate(estimate(0, 0, Some(6)))),
    candidate("linux", false, Answer::Estimate(estimate(0, 0, None))),
    candidate("air", false, Answer::Predates),
    candidate("mini", false, Answer::Missing("no answer in 3 s".to_owned())),
  ];
  assert_eq!(
    plan_line(&candidates, true, Some(1)),
    "brd-host: studio: starts in ~12 min after 2 runs of an unknown length, runs ~4 min; local: starts now, runs ~6 min, only at once; linux: no measure of this command there, so a heavy run does not go there; air: no estimate: its agent predates them (`brd-host upgrade air`); mini: no estimate: no answer in 3 s; chose local"
  );
  assert_eq!(about(45_000), "~45 s");
  assert_eq!(about(65 * MIN + 1), "~1 h 5 min");
}

/// A member that keeps no slot for the runs of the repository (`Estimate::slots` 0) takes no run, the first member of the
/// set neither, and the plan says why; an agent before the field (`None`) ranks as before
#[test]
fn a_member_without_a_slot_for_the_repository_takes_no_run_the_first_neither() {
  let none = || Estimate {
    slots: Some(0),
    ..estimate(0, 0, Some(2))
  };
  let candidates = vec![
    candidate("local", false, Answer::Estimate(none())),
    candidate("studio", false, Answer::Estimate(estimate(12, 0, Some(4)))),
    candidate(
      "old",
      false,
      Answer::Estimate(Estimate {
        slots: None,
        ..estimate(30, 0, Some(4))
      }),
    ),
  ];
  for heavy in [true, false] {
    assert_eq!(order(&candidates, heavy), ["studio", "old"], "heavy {heavy}");
  }
  assert_eq!(
    plan_line(&candidates, true, Some(1)),
    "brd-host: local: it keeps no slot for the runs of this repository (`branchdeck.hostSlots` or the limit of `brd-host slots --local --shrink` is 0 there), so no run goes there; studio: starts in ~12 min, runs ~4 min; old: starts in ~30 min, runs ~4 min; chose studio"
  );
  assert_eq!(
    order(&[candidate("local", false, Answer::Estimate(none()))], false),
    Vec::<&str>::new(),
    "a set of one such member"
  );
}

/// What a member of the tests does: its estimate (`None`: it never answers one), and its start, which it refuses with
/// `busy` when `busy` says so of a start that admits at once; its run exits 0
#[derive(Clone)]
struct Member {
  estimate: Option<Estimate>,
  busy: Arc<dyn Fn() -> bool + Send + Sync>,
}

/// The script of `member`
fn script(member: Member) -> impl for<'c> Fn(&'c Request, &'c mut Channel) -> crate::testkit::Answer<'c> + Send + Sync + 'static {
  move |request, channel| {
    let member = member.clone();
    Box::pin(async move {
      let _ = match request {
        Request::Estimate(_) => match member.estimate {
          Some(estimate) => channel.send(&Reply::Estimate(estimate)).await,
          None => std::future::pending().await,
        },
        Request::Have { commits, .. } => channel.send(&Reply::Have { commits: commits.clone() }).await,
        Request::Start(spec) if spec.admit_now && (member.busy)() => {
          channel
            .send(&Reply::refused(
              Code::Busy,
              "<host> takes runs only at once (`idle_only` of the set), and its heavy turn has no room for it now",
            ))
            .await
        }
        Request::Start(spec) => channel.send(&Reply::Started { run: spec.run.clone() }).await,
        Request::Follow { .. } => {
          let _ = channel.send(&Reply::Phases { phases: Vec::new() }).await;
          channel.send(&Reply::Exit { code: 0 }).await
        }
        Request::Pack { .. } => {
          let _ = channel.recv_stream(&mut std::io::sink()).await;
          channel.send(&Reply::Packed).await
        }
        Request::Forget { .. } | Request::Cancel { .. } => channel.send(&Reply::Done { message: "done".to_owned() }).await,
        _ => channel.send(&Reply::refused(Code::BadRequest, "not in this script")).await,
      };
    })
  }
}

/// The hello of an agent with estimates and classes of measures
fn estimating() -> Hello {
  Hello {
    features: ["estimate", "heavy", "measure_as"].map(str::to_owned).to_vec(),
    ..Hello::default()
  }
}

/// A set `any` of `studio`, the agent of the setup, and the members `others` of their aliases on its net, each with its
/// hello
struct Hosts {
  setup: Setup,
  others: Vec<FakeAgent>,
  set: HostSet,
}

fn hosts(studio: Member, others: Vec<(&str, bool, Hello, Member)>) -> Hosts {
  let setup = Setup::with_hello(estimating(), script(studio));
  let identity = setup.config.identity().expect("the identity");
  let mut members = vec![SetMember {
    alias: ALIAS.to_owned(),
    idle_only: false,
  }];
  let others = others
    .into_iter()
    .map(|(alias, idle_only, hello, member)| {
      let agent = FakeAgent::start_on(Arc::clone(&setup.agent.net), &identity, hello, Arc::new(script(member)));
      setup.config.write_host(&host_file_of(alias, &agent, vec![agent.address])).expect("a host file");
      members.push(SetMember {
        alias: alias.to_owned(),
        idle_only,
      });
      agent
    })
    .collect();
  let set = HostSet { name: "any".to_owned(), members };
  setup.config.write_set(&set).expect("the set");
  Hosts { setup, others, set }
}

fn member(estimate: Option<Estimate>, busy: bool) -> Member {
  Member {
    estimate,
    busy: Arc::new(move || busy),
  }
}

fn command() -> Vec<String> {
  ["pnpm", "e2e"].map(str::to_owned).to_vec()
}

const AFFINITY: &str = "checkout-0123456789ab";

/// The request of a run of `command` from `checkout`
fn request_of<'a>(checkout: &'a std::path::Path, command: &'a [String]) -> RunRequest<'a> {
  RunRequest {
    checkout,
    repo: None,
    affinity: AFFINITY,
    command,
    fetch: &[],
    measure_as: None,
    group: None,
    priority: None,
  }
}

/// A heavy run of the command of the tests to the set of `hosts`, and the lines it said
async fn run_on(hosts: &Hosts, options: RunOptions) -> ((String, Result<RunOutcome, RunFailure>), Vec<String>) {
  run_in(hosts, &hosts.set, hosts.setup.repo.path(), options).await
}

/// `run_on` to `set` from the checkout `checkout`
async fn run_in(hosts: &Hosts, set: &HostSet, checkout: &std::path::Path, options: RunOptions) -> ((String, Result<RunOutcome, RunFailure>), Vec<String>) {
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let processes = FakeProcesses::default();
  let client = hosts.setup.client(&processes, &say, &calm);
  let command = command();
  let request = request_of(checkout, &command);
  let ran = client.run_on_set(set, &request, RunOptions { heavy: true, ..options }).await;
  (ran, said.lines())
}

/// The starts that reached an agent, as whether each admits at once
fn starts(requests: &[Request]) -> Vec<bool> {
  requests
    .iter()
    .filter_map(|request| match request {
      Request::Start(spec) => Some(spec.admit_now),
      _ => None,
    })
    .collect()
}

/// What each member told, by its alias
fn answers(candidates: &[Candidate]) -> Vec<(&str, Answer)> {
  candidates.iter().map(|candidate| (candidate.alias.as_str(), candidate.answer.clone())).collect()
}

/// The first request of the agent of the setup, an estimate
fn first_ask(hosts: &Hosts) -> EstimateAsk {
  let Some(Request::Estimate(asked)) = hosts.setup.agent.requests().first().cloned() else {
    panic!("no estimate: {:?}", hosts.setup.agent.requests());
  };
  asked
}

/// The client asks every member at once, each within 3 s, and reads the feature from the hello of the same connection:
/// an agent without estimates gets no request it would close the connection on, and a silent one costs 3 s
#[tokio::test(start_paused = true)]
async fn the_members_are_asked_at_once_and_one_without_estimates_or_an_answer_has_none() {
  let hosts = hosts(
    member(Some(estimate(12, 0, Some(4))), false),
    vec![
      ("local", true, Hello::default(), member(Some(estimate(0, 0, Some(6))), false)),
      ("mini", false, estimating(), member(None, false)),
    ],
  );
  let processes = FakeProcesses::default();
  let client = hosts.setup.client(&processes, &|_| {}, &calm);
  let command = command();
  let request = request_of(hosts.setup.repo.path(), &command);
  let (ask, heavy) = client.ask_of(&request, RunOptions::default()).expect("the ask");
  assert!(!heavy, "neither --heavy nor [host] makes the run heavy");
  let started = tokio::time::Instant::now();
  let candidates = client.estimates(&hosts.set, &ask, &request, RunOptions::default()).await;
  assert_eq!(started.elapsed(), ESTIMATE_WAIT, "the members are asked at once");
  assert_eq!(
    answers(&candidates),
    [
      ("studio", Answer::Estimate(estimate(12, 0, Some(4)))),
      ("local", Answer::Predates),
      ("mini", Answer::Missing("no answer in 3 s".to_owned())),
    ]
  );
  let asked = first_ask(&hosts);
  assert_eq!((asked.repo.as_str(), asked.affinity.as_str(), asked.command), ("~/projects/idea-1", AFFINITY, command));
  assert_eq!(hosts.others[0].requests(), Vec::<Request>::new(), "an agent without estimates gets no request");
}

/// The estimate of a run of `--group` names its group, so a member places it by the turns of the groups; an older agent
/// ignores the field and estimates the end of its queue, where it queues the run
#[tokio::test(start_paused = true)]
async fn the_estimate_of_a_run_of_a_group_names_its_group() {
  let hosts = hosts(member(Some(estimate(12, 0, Some(4))), false), Vec::new());
  let processes = FakeProcesses::default();
  let client = hosts.setup.client(&processes, &|_| {}, &calm);
  let command = command();
  let request = RunRequest {
    group: Some("typing-session"),
    priority: None,
    ..request_of(hosts.setup.repo.path(), &command)
  };
  let (ask, _) = client.ask_of(&request, RunOptions::default()).expect("the ask");
  assert_eq!(ask.group.as_deref(), Some("typing-session"));
  let (ask, _) = client.ask_of(&request_of(hosts.setup.repo.path(), &command), RunOptions::default()).expect("the ask");
  assert_eq!(ask.group, None);
}

/// A run of `--measure-as` is heavy and asks each member by its class, whose measure there decides; a member whose
/// agent predates classes knows no measure of one, so it has no estimate and names the upgrade
#[tokio::test(start_paused = true)]
async fn a_run_of_a_class_asks_by_its_class_and_a_member_before_classes_has_no_estimate() {
  let before = Hello {
    features: ["estimate", "heavy"].map(str::to_owned).to_vec(),
    ..Hello::default()
  };
  let hosts = hosts(
    member(Some(estimate(12, 0, Some(4))), false),
    vec![("local", true, before, member(Some(estimate(0, 0, Some(6))), false))],
  );
  let processes = FakeProcesses::default();
  let client = hosts.setup.client(&processes, &|_| {}, &calm);
  let command = command();
  let request = RunRequest {
    measure_as: Some("accio-gate"),
    ..request_of(hosts.setup.repo.path(), &command)
  };
  let (ask, heavy) = client.ask_of(&request, RunOptions::default()).expect("the ask");
  assert!(heavy, "--measure-as makes the run heavy");
  assert_eq!(ask.measure_as.as_deref(), Some("accio-gate"));
  let candidates = client.estimates(&hosts.set, &ask, &request, RunOptions::default()).await;
  assert_eq!(
    answers(&candidates),
    [
      ("studio", Answer::Estimate(estimate(12, 0, Some(4)))),
      (
        "local",
        Answer::Missing("its agent predates --measure-as, so it knows no measure of the class (`brd-host upgrade local`)".to_owned())
      ),
    ]
  );
  assert_eq!(first_ask(&hosts).measure_as.as_deref(), Some("accio-gate"));
  assert_eq!(hosts.others[0].requests(), Vec::<Request>::new(), "an agent before classes gets no request");
  assert_eq!(rank(&candidates, heavy), [0], "the member without an estimate may not take a heavy run");
}

/// A member that takes runs only at once gets the start of `admit_now`; its `busy` sends the run to the next member by
/// the rank, and `<run>.host` of the checkout names the member that took it, so a wait to the name of the set goes there
#[tokio::test(start_paused = true)]
async fn a_busy_member_that_takes_runs_only_at_once_sends_the_run_to_the_next_one() {
  let hosts = hosts(
    member(Some(estimate(12, 0, Some(4))), false),
    vec![("local", true, estimating(), member(Some(estimate(0, 0, Some(6))), true))],
  );
  let ((alias, ran), said) = run_on(&hosts, RunOptions::default()).await;
  let outcome = ran.expect("the run");
  assert_eq!((alias.as_str(), outcome.code), ("studio", 0));
  assert_eq!(
    said[0],
    "brd-host: studio: starts in ~12 min, runs ~4 min; local: starts now, runs ~6 min, only at once; chose local"
  );
  assert!(
    said
      .iter()
      .any(|line| line == "brd-host: local: local takes runs only at once (`idle_only` of the set), and its heavy turn has no room for it now; the run goes to studio"),
    "{said:#?}"
  );
  assert_eq!((starts(&hosts.others[0].requests()), starts(&hosts.setup.agent.requests())), (vec![true], vec![false]));

  let run = outcome.facts.run.expect("the run id");
  let processes = FakeProcesses::default();
  let client = hosts.setup.client(&processes, &|_| {}, &calm);
  assert_eq!(client.host_of_run("any", &run, Some(hosts.setup.repo.path())), Ok("studio".to_owned()));
  assert_eq!(
    client.host_of_run("local", &run, Some(hosts.setup.repo.path())),
    Ok("local".to_owned()),
    "a host file comes first"
  );
  let unknown = client.host_of_run("any", "1-2", Some(hosts.setup.repo.path())).expect_err("no host of the run");
  assert_eq!(unknown.code, Code::Usage);
  assert!(
    unknown
      .message
      .starts_with("any is a set of hosts (studio, local), and this checkout names no host of the run 1-2"),
    "{}",
    unknown.message
  );
}

/// Two clients race for one free member that takes runs only at once: its admission takes one of them, and the other
/// goes to the next member, so they never meet on one host
#[tokio::test(start_paused = true)]
async fn two_clients_that_race_for_a_free_member_get_one_admission() {
  let admitted = Arc::new(AtomicBool::new(false));
  let taken = Arc::clone(&admitted);
  let local = Member {
    estimate: Some(estimate(0, 0, Some(6))),
    busy: Arc::new(move || taken.swap(true, Ordering::SeqCst)),
  };
  let hosts = hosts(member(Some(estimate(12, 0, Some(4))), false), vec![("local", true, estimating(), local)]);
  // a checkout each, as two clients have
  let other = test_utils::git_test_utils::TestRepo::new();
  other.create_commit("Add a", "a.txt", "a\n");
  let (first, second) = tokio::join!(run_on(&hosts, RunOptions::default()), run_in(&hosts, &hosts.set, other.path(), RunOptions::default()));
  let mut ended: Vec<&str> = [&first.0, &second.0].iter().map(|(alias, _)| alias.as_str()).collect();
  ended.sort_unstable();
  assert_eq!(ended, ["local", "studio"]);
  assert_eq!(starts(&hosts.others[0].requests()), [true, true], "both asked the member that takes runs only at once");
}

/// A heavy run goes to no member but the first without its measure of the command there, so the scripts of a measurement
/// stay on the first member; `--exclusive` names one host and refuses with a set
#[tokio::test(start_paused = true)]
async fn a_heavy_run_without_a_measure_stays_on_the_first_member_and_an_exclusive_one_refuses() {
  let hosts = hosts(
    member(Some(estimate(30, 0, Some(4))), false),
    vec![("local", true, estimating(), member(Some(estimate(0, 0, None)), false))],
  );
  let ((alias, ran), said) = run_on(&hosts, RunOptions::default()).await;
  assert_eq!((alias.as_str(), ran.expect("the run").code), ("studio", 0));
  assert_eq!(
    said[0],
    "brd-host: studio: starts in ~30 min, runs ~4 min; local: no measure of this command there, so a heavy run does not go there; chose studio"
  );
  assert_eq!(starts(&hosts.others[0].requests()), Vec::<bool>::new());

  let ((_, exclusive), _) = run_on(
    &hosts,
    RunOptions {
      exclusive: true,
      ..RunOptions::default()
    },
  )
  .await;
  let failure = exclusive.expect_err("an exclusive run to a set");
  assert_eq!((failure.refusal.code, failure.exit.code()), (Code::Usage, 2));
}

/// When every member refuses the run with `busy`, the run ends with the refusal of the first member of the set: nothing
/// ran anywhere, exit 75
#[tokio::test(start_paused = true)]
async fn a_run_that_every_member_refuses_with_busy_ends_with_the_refusal_of_the_first_member() {
  let hosts = hosts(
    member(Some(estimate(12, 0, Some(4))), true),
    vec![("local", true, estimating(), member(Some(estimate(0, 0, Some(6))), true))],
  );
  let mut set = hosts.set.clone();
  set.members[0].idle_only = true;
  let ((alias, ran), said) = run_in(&hosts, &set, hosts.setup.repo.path(), RunOptions::default()).await;
  let failure = ran.expect_err("busy everywhere");
  assert_eq!((alias.as_str(), failure.refusal.code, failure.exit.code()), (ALIAS, Code::Busy, 75));
  assert!(failure.refusal.message.starts_with("studio: "), "{}", failure.refusal.message);
  assert_eq!(
    said[1],
    "brd-host: local: local takes runs only at once (`idle_only` of the set), and its heavy turn has no room for it now; the run goes to studio"
  );
  assert_eq!((starts(&hosts.others[0].requests()), starts(&hosts.setup.agent.requests())), (vec![true], vec![true]));
}
