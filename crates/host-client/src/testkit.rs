//! The test doubles of the crate: scripted child processes, and an agent on a loopback port that answers each
//! connection from a script. The checkout of a run is a real repository (`TestRepo`)

use crate::config::{Config, HostFile};
use crate::process::{Call, Processes, Ran};
use crate::{Build, Client, Timing};
use host_link::allow::AllowList;
use host_link::channel::{Channel, ServerTls};
use host_link::identity::Identity;
use host_link::proto::{Reply, Request, SCHEMA_VERSION};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;
use sync_utils::span::in_current_span;
use test_utils::git_test_utils::TestRepo;

/// Short deadlines, so a test of a lost link ends in seconds
pub(crate) const FAST: Timing = Timing {
  connect: Duration::from_secs(2),
  resolve: Duration::from_secs(2),
  read: Duration::from_secs(2),
  reconnect_for: Duration::from_secs(3),
  first_backoff: Duration::from_millis(10),
  max_backoff: Duration::from_millis(50),
  agent_start: Duration::from_secs(1),
  keepalive: Duration::from_millis(50),
  upgrade: Duration::from_millis(500),
};

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

/// What the agent does with the one request of a connection, after the hello
pub(crate) type Script = dyn Fn(&Request, &mut Channel) + Send + Sync;

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

/// An agent on a loopback port. Each connection runs on a thread of its own: the hello, then the script
pub(crate) struct FakeAgent {
  pub(crate) address: SocketAddr,
  pub(crate) identity: Identity,
  requests: Arc<Mutex<Vec<Request>>>,
  stop: Arc<AtomicBool>,
  _dir: tempfile::TempDir,
}

impl FakeAgent {
  /// An agent that accepts `client`, says `hello`, and answers each request with `script`
  pub(crate) fn start(client: &Identity, hello: Hello, script: impl Fn(&Request, &mut Channel) + Send + Sync + 'static) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let allow = dir.path().join("clients");
    let mut list = AllowList::default();
    list.add(client.fingerprint(), "test").unwrap();
    std::fs::write(&allow, list.text()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let identity = Identity::generate().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let script: Arc<Script> = Arc::new(script);
    let tls = ServerTls::new(&identity, allow).unwrap();
    {
      let (requests, stop) = (Arc::clone(&requests), Arc::clone(&stop));
      thread::spawn(in_current_span(move || {
        for socket in listener.incoming() {
          if stop.load(Ordering::SeqCst) {
            return;
          }
          let Ok(socket) = socket else { continue };
          let (tls, requests, script, hello) = (tls.clone(), Arc::clone(&requests), Arc::clone(&script), hello.clone());
          thread::spawn(in_current_span(move || serve(socket, &tls, &hello, &requests, &*script)));
        }
      }));
    }
    Self {
      address,
      identity,
      requests,
      stop,
      _dir: dir,
    }
  }

  /// The list of the requests, for a test thread that waits for one
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
      stop: Arc::clone(&self.stop),
      address: self.address,
    }
  }
}

impl Drop for FakeAgent {
  fn drop(&mut self) {
    self.stopper().stop();
  }
}

/// Ends the accept loop of a `FakeAgent` and closes its port, so each new connection is refused
pub(crate) struct AgentStop {
  stop: Arc<AtomicBool>,
  address: SocketAddr,
}

impl AgentStop {
  pub(crate) fn stop(&self) {
    self.stop.store(true, Ordering::SeqCst);
    // Wakes the accept loop, which then sees the stop
    let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
  }
}

fn serve(socket: TcpStream, tls: &ServerTls, hello: &Hello, requests: &Mutex<Vec<Request>>, script: &Script) {
  let Ok(mut channel) = Channel::accept(socket, tls, Duration::from_secs(5)) else {
    return;
  };
  let Ok(Request::Hello { .. }) = channel.recv::<Request>() else {
    return;
  };
  let reply = Reply::Hello {
    schema_version: hello.schema,
    agent_version: "0.37.0".to_owned(),
    os: "macos".to_owned(),
    arch: "aarch64".to_owned(),
    features: hello.features.clone(),
    binary_sha256: hello.build.lock().unwrap_or_else(PoisonError::into_inner).clone(),
  };
  if channel.send(&reply).is_err() || hello.schema != SCHEMA_VERSION {
    return;
  }
  let Ok(request) = channel.recv::<Request>() else {
    return;
  };
  requests.lock().unwrap_or_else(PoisonError::into_inner).push(request.clone());
  script(&request, &mut channel);
  channel.close();
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
  pub(crate) fn new(script: impl Fn(&Request, &mut Channel) + Send + Sync + 'static) -> Self {
    Self::with_hello(Hello::default(), script)
  }

  pub(crate) fn with_hello(hello: Hello, script: impl Fn(&Request, &mut Channel) + Send + Sync + 'static) -> Self {
    let repo = TestRepo::new();
    repo.create_commit("Add a", "a.txt", "a\n");
    Self::on(repo, hello, script)
  }

  /// A setup whose checkout is `repo`, so the script can name its commits
  pub(crate) fn on(repo: TestRepo, hello: Hello, script: impl Fn(&Request, &mut Channel) + Send + Sync + 'static) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::new(dir.path().join("config"));
    let client = config.identity().unwrap();
    let agent = FakeAgent::start(&client, hello, script);
    config.write_host(&host_file(&agent, vec![agent.address])).unwrap();
    Self { dir, config, agent, repo }
  }

  /// A client of the setup with the short deadlines
  pub(crate) fn client<'a>(&self, processes: &'a dyn Processes, say: &'a (dyn Fn(&str) + Sync), presses: &'a (dyn Fn() -> u32 + Sync)) -> Client<'a> {
    Client::new(self.config.clone(), build(), processes, say, presses).with_timing(FAST)
  }
}

/// The host file of `ALIAS` for `agent`, with `addresses` cached. The host name resolves to the loopback
pub(crate) fn host_file(agent: &FakeAgent, addresses: Vec<SocketAddr>) -> HostFile {
  HostFile {
    alias: ALIAS.to_owned(),
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
