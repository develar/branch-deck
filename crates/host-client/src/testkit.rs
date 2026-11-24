//! The test doubles of the crate: scripted child processes, and an agent on a net in memory that answers each connection
//! from a script. The checkout of a run is a real repository (`TestRepo`). The tests of the client run on paused time
//! (`#[tokio::test(start_paused = true)]`): a deadline or a wait of the client passes when nothing else can go on, so
//! no test waits for the clock and none fails on a loaded machine

use crate::config::{Config, HostFile};
use crate::process::{Call, Processes, Ran};
use crate::{Build, Client, Net, Timing};
use host_link::allow::AllowList;
use host_link::channel::{Channel, Link, LinkError, ServerTls};
use host_link::identity::Identity;
use host_link::proto::{Reply, Request, SCHEMA_VERSION};
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use test_utils::git_test_utils::TestRepo;
use tokio::io::DuplexStream;
use tokio::sync::mpsc;
use tracing::Instrument as _;

/// The timing of the tests: deadlines that no run reaches, and short paces. On paused time a deadline passes only when
/// the fake says nothing, so only a test whose subject is a deadline sets its own (a lost link, a silent agent, the end
/// of an upgrade)
pub(crate) const UNHURRIED: Timing = Timing {
  connect: Duration::from_mins(1),
  resolve: Duration::from_mins(1),
  read: Duration::from_mins(1),
  reconnect_for: Duration::from_mins(1),
  first_backoff: Duration::from_millis(10),
  max_backoff: Duration::from_millis(50),
  agent_start: Duration::from_mins(1),
  keepalive: Duration::from_millis(50),
  upgrade: Duration::from_mins(1),
  interrupt_poll: Duration::from_millis(10),
  cancel_for: Duration::from_mins(1),
  cancel_grace: Duration::from_millis(500),
  state_poll: Duration::from_millis(10),
};

/// How long each step that the client of the tests measures takes (`Client::with_step_time`): the words of a step and of
/// the end of a run are the same on a loaded machine
pub(crate) const STEP_TIME: Duration = Duration::from_millis(400);

pub(crate) const ALIAS: &str = "studio";

/// The build of the client of the tests
pub(crate) fn build() -> Build {
  Build {
    version: "0.37.0".to_owned(),
    sha256: None,
  }
}

pub(crate) fn ok(stdout: &str) -> Ran {
  Ran {
    code: Some(0),
    stdout: stdout.as_bytes().to_vec(),
    ..Ran::default()
  }
}

pub(crate) fn failed(code: i32, stderr: &str) -> Ran {
  Ran {
    code: Some(code),
    stderr: stderr.to_owned(),
    ..Ran::default()
  }
}

/// Child processes that answer by rule: the first rule whose text the joined call holds. A call without a rule
/// answers exit 0 and no output
#[derive(Default)]
pub(crate) struct FakeProcesses {
  rules: Vec<(String, Ran)>,
  calls: Mutex<Vec<Call>>,
}

impl FakeProcesses {
  pub(crate) fn new(rules: &[(&str, Ran)]) -> Self {
    Self {
      rules: rules.iter().map(|(text, ran)| ((*text).to_owned(), ran.clone())).collect(),
      ..Self::default()
    }
  }

  pub(crate) fn calls(&self) -> Vec<Call> {
    self.calls.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }

  pub(crate) fn texts(&self) -> Vec<String> {
    self.calls().iter().map(Call::text).collect()
  }
}

impl Processes for FakeProcesses {
  fn run(&self, call: &Call, _stop: &dyn Fn() -> bool) -> Ran {
    self.calls.lock().unwrap_or_else(PoisonError::into_inner).push(call.clone());
    let text = call.text();
    self
      .rules
      .iter()
      .find(|(rule, _)| text.contains(rule.as_str()))
      .map_or_else(|| ok(""), |(_, ran)| ran.clone())
  }
}

/// What the agent does with the one request of a connection, after the hello: a future that borrows the request and the
/// channel. The connection closes after it
pub(crate) type Script = dyn for<'c> Fn(&'c Request, &'c mut Channel) -> Answer<'c> + Send + Sync;

/// The future of a script: `|request, channel| Box::pin(async move { … })`. Whatever else it needs it owns, as a clone of
/// an `Arc` or a value that `leak` made `'static`
pub(crate) type Answer<'c, T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'c>>;

/// `value` for the whole test process, so a script copies the reference into each of its futures
pub(crate) fn leak<T>(value: T) -> &'static T {
  Box::leak(Box::new(value))
}

/// What the agent says of itself in its hello
#[derive(Clone)]
pub(crate) struct Hello {
  pub(crate) schema: u32,
  pub(crate) features: Vec<String>,
  /// The build of the binary of the agent, which the script of an upgrade changes
  pub(crate) build: Arc<Mutex<Option<String>>>,
}

impl Default for Hello {
  fn default() -> Self {
    Self {
      schema: SCHEMA_VERSION,
      features: vec!["upgrade".to_owned()],
      build: Arc::default(),
    }
  }
}

/// How many bytes a pipe of the net in memory holds before a write waits for its reader
const PIPE: usize = 256 * 1024;

/// The network of a test: each agent listens on an address of its own, and a connect to another address is refused as
/// TCP refuses a port without a listener. A connection is a pipe in memory, so a test on paused time waits for no socket
/// of the system: a loopback connect completes in the kernel, and a paused clock would run past its deadline meanwhile
#[derive(Debug, Default)]
pub(crate) struct MemoryNet {
  listeners: Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<DuplexStream>>>,
}

impl MemoryNet {
  /// The connections to `address` from now on
  fn listen(&self, address: SocketAddr) -> mpsc::UnboundedReceiver<DuplexStream> {
    let (accepted, connections) = mpsc::unbounded_channel();
    self.listeners.lock().unwrap_or_else(PoisonError::into_inner).insert(address, accepted);
    connections
  }

  /// Ends the listener of `address`: each later connect is refused
  fn close(&self, address: SocketAddr) {
    self.listeners.lock().unwrap_or_else(PoisonError::into_inner).remove(&address);
  }

  /// A connection to the agent of `address`, or the refusal of an address where no agent listens
  pub(crate) fn dial(&self, address: SocketAddr) -> Result<Link, LinkError> {
    let refused = || LinkError::Unreachable(io::Error::new(io::ErrorKind::ConnectionRefused, "connection refused"));
    let listeners = self.listeners.lock().unwrap_or_else(PoisonError::into_inner);
    let accepted = listeners.get(&address).ok_or_else(refused)?;
    let (client, agent) = tokio::io::duplex(PIPE);
    // an agent whose accept loop ended takes no connection
    if accepted.send(agent).is_err() {
      return Err(refused());
    }
    Ok(Box::new(client))
  }
}

/// The addresses of the agents of the tests: a port of their own each, which no socket of the system has
static NEXT_PORT: AtomicU16 = AtomicU16::new(41_000);

/// An agent on an address of a net in memory. Each connection runs on a task of its own: the hello, then the script
pub(crate) struct FakeAgent {
  pub(crate) address: SocketAddr,
  pub(crate) identity: Identity,
  pub(crate) net: Arc<MemoryNet>,
  requests: Arc<Mutex<Vec<Request>>>,
  _dir: tempfile::TempDir,
}

impl FakeAgent {
  /// An agent that accepts `client`, says `hello`, and answers each request with `script`. It needs the runtime of the
  /// test, which runs its tasks
  pub(crate) fn start(client: &Identity, hello: Hello, script: Arc<Script>) -> Self {
    Self::start_on(Arc::new(MemoryNet::default()), client, hello, script)
  }

  /// `start` on the net `net` of another agent, so one client reaches both, as the members of a set of hosts
  pub(crate) fn start_on(net: Arc<MemoryNet>, client: &Identity, hello: Hello, script: Arc<Script>) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let allow = dir.path().join("clients");
    let mut list = AllowList::default();
    list.add(client.fingerprint(), "test").unwrap();
    std::fs::write(&allow, list.text()).unwrap();
    let address = SocketAddr::from(([127, 0, 0, 1], NEXT_PORT.fetch_add(1, Ordering::SeqCst)));
    let mut connections = net.listen(address);
    let identity = Identity::generate().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let tls = ServerTls::new(&identity, allow).unwrap();
    let served = Arc::clone(&requests);
    tokio::spawn(
      async move {
        while let Some(connection) = connections.recv().await {
          let (tls, requests, script, hello) = (tls.clone(), Arc::clone(&served), Arc::clone(&script), hello.clone());
          tokio::spawn(serve(Box::new(connection), tls, hello, requests, script).in_current_span());
        }
      }
      .in_current_span(),
    );
    Self {
      address,
      identity,
      net,
      requests,
      _dir: dir,
    }
  }

  /// The list of the requests, for a test that waits for one
  pub(crate) fn requests_handle(&self) -> Arc<Mutex<Vec<Request>>> {
    Arc::clone(&self.requests)
  }

  /// The requests after the hello, in the order they came
  pub(crate) fn requests(&self) -> Vec<Request> {
    self.requests.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// The stop of the agent, for a script that ends the agent in the middle of a run
  pub(crate) fn stopper(&self) -> AgentStop {
    AgentStop {
      net: Arc::clone(&self.net),
      address: self.address,
    }
  }
}

/// Ends the listener of a `FakeAgent`, so each new connection is refused
#[derive(Clone)]
pub(crate) struct AgentStop {
  net: Arc<MemoryNet>,
  address: SocketAddr,
}

impl AgentStop {
  pub(crate) fn stop(&self) {
    self.net.close(self.address);
  }
}

async fn serve(link: Link, tls: ServerTls, hello: Hello, requests: Arc<Mutex<Vec<Request>>>, script: Arc<Script>) {
  let Ok(mut channel) = Channel::accept_over(link, &tls, Duration::from_secs(5)).await else {
    return;
  };
  let Ok(Request::Hello { .. }) = channel.recv::<Request>().await else {
    return;
  };
  let build = hello.build.lock().unwrap_or_else(PoisonError::into_inner).clone();
  let reply = Reply::Hello {
    schema_version: hello.schema,
    agent_version: "0.37.0".to_owned(),
    os: "macos".to_owned(),
    arch: "aarch64".to_owned(),
    features: hello.features.clone(),
    binary_sha256: build,
  };
  if channel.send(&reply).await.is_err() || hello.schema != SCHEMA_VERSION {
    return;
  }
  let Ok(request) = channel.recv::<Request>().await else {
    return;
  };
  requests.lock().unwrap_or_else(PoisonError::into_inner).push(request.clone());
  script(&request, &mut channel).await;
  channel.close().await;
}

/// A script that answers each request with no runs
pub(crate) fn runs_reply<'c>(_: &'c Request, channel: &'c mut Channel) -> Answer<'c> {
  Box::pin(async move {
    let _ = channel.send(&Reply::Runs { runs: Vec::new(), heavy: None }).await;
  })
}

/// A client directory with a host file of `ALIAS` that names the agent of the script, and a checkout
pub(crate) struct Setup {
  pub(crate) dir: tempfile::TempDir,
  pub(crate) config: Config,
  pub(crate) agent: FakeAgent,
  /// The local checkout of the runs: a repository with one commit
  pub(crate) repo: TestRepo,
}

impl Setup {
  /// A setup whose agent answers with `script`. Its agent runs on the runtime of the test
  pub(crate) fn new(script: impl for<'c> Fn(&'c Request, &'c mut Channel) -> Answer<'c> + Send + Sync + 'static) -> Self {
    Self::with_hello(Hello::default(), script)
  }

  pub(crate) fn with_hello(hello: Hello, script: impl for<'c> Fn(&'c Request, &'c mut Channel) -> Answer<'c> + Send + Sync + 'static) -> Self {
    let repo = TestRepo::new();
    repo.create_commit("Add a", "a.txt", "a\n");
    Self::on(repo, hello, script)
  }

  /// A setup whose checkout is `repo`, so the script can name its commits
  pub(crate) fn on(repo: TestRepo, hello: Hello, script: impl for<'c> Fn(&'c Request, &'c mut Channel) -> Answer<'c> + Send + Sync + 'static) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::new(dir.path().join("config"));
    let client = config.identity().unwrap();
    let agent = FakeAgent::start(&client, hello, Arc::new(script));
    config.write_host(&host_file(&agent, vec![agent.address])).unwrap();
    Self { dir, config, agent, repo }
  }

  /// A client of the setup with the short deadlines, on the net of its agent
  pub(crate) fn client<'a>(&self, processes: &'a dyn Processes, say: &'a (dyn Fn(&str) + Sync), presses: &'a (dyn Fn() -> u32 + Sync)) -> Client<'a> {
    Client::new(self.config.clone(), build(), processes, say, presses)
      .with_timing(UNHURRIED)
      .with_step_time(STEP_TIME)
      .with_net(self.net())
  }

  /// The net of the agent, for a client that the test makes itself
  pub(crate) fn net(&self) -> Net {
    Net::Memory(Arc::clone(&self.agent.net))
  }
}

/// The host file of `ALIAS` for `agent`, with `addresses` cached. The host name resolves to the loopback address, whose
/// port is the one of the agent on its net
pub(crate) fn host_file(agent: &FakeAgent, addresses: Vec<SocketAddr>) -> HostFile {
  host_file_of(ALIAS, agent, addresses)
}

/// `host_file` of the alias `alias`
pub(crate) fn host_file_of(alias: &str, agent: &FakeAgent, addresses: Vec<SocketAddr>) -> HostFile {
  HostFile {
    alias: alias.to_owned(),
    fingerprint: agent.identity.fingerprint(),
    port: agent.address.port(),
    hostname: "127.0.0.1".to_owned(),
    addresses,
    repo: "~/projects/idea-1".to_owned(),
  }
}

/// The lines that a client said, collected for a test
#[derive(Default)]
pub(crate) struct Said(Mutex<Vec<String>>);

impl Said {
  pub(crate) fn push(&self, line: &str) {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).push(line.to_owned());
  }

  pub(crate) fn lines(&self) -> Vec<String> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }
}

/// Never pressed
pub(crate) const fn calm() -> u32 {
  0
}
