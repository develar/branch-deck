//! The connection to the agent of one host: the address choice, and the hello of each connection (openspec/specs/cli/host/spec.md,
//! "The link", "hello").
//!
//! The client tries the cached addresses of the host file first, in their order, each with a connect deadline of 5 s.
//! Then it resolves the host name on a thread under a deadline of 5 s, IPv4 first, and tries each new address. The
//! address that answers goes first in the host file. A cached address that failed goes after the ones that did not, so
//! the next connection tries it last. When only an address of the host name answers, every cached address failed, and
//! the host file keeps the new address alone. No step waits without a deadline, so a `.local` name that does not
//! resolve costs 5 s. When macOS Local Network privacy can be what refused an address, the refusal names it once.
//!
//! A caller that stops the work of a host drops the future that uses its connection, which ends the connection

use crate::config::{Config, HostFile};
use crate::{Net, Timing, setup_hint};
use host_link::channel::{Channel, ClientTls, LinkError};
use host_link::proto::{Reply, Request, SCHEMA_VERSION};
use host_link::refusal::{Code, Refusal, on_host};
use host_link::relay::local_network_cause;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;

/// How many addresses the host file keeps
const KEPT_ADDRESSES: usize = 4;

/// What the hello of the agent told
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentInfo {
  /// The version of Branch Deck of the agent
  pub version: String,
  pub schema: u32,
  pub os: String,
  pub arch: String,
  pub features: Vec<String>,
  /// The SHA-256 of the binary of the agent; `None` when the agent cannot read its binary
  pub build: Option<String>,
  /// The address that answered
  pub address: SocketAddr,
}

/// One host: its host file and the identity of this client
#[derive(Clone)]
pub struct Host {
  config: Config,
  file: HostFile,
  /// The version of this client, for the hello
  version: String,
  /// The TLS configuration of the pin of the host and the identity of the client, built once for all connections
  tls: ClientTls,
  timing: Timing,
  /// How the connections reach the addresses
  net: Net,
}

/// The failures of the addresses that one connection tried
#[derive(Default)]
struct Tries {
  /// Each address with what it said
  tried: Vec<String>,
  /// What the first address with another certificate said
  untrusted: Option<String>,
  failed: Vec<SocketAddr>,
  /// Why macOS Local Network privacy can be what refused the connections, from the first address where it can be
  local_network: Option<String>,
}

/// Why one address gave no channel
enum Attempt {
  /// Try the next address. The second value tells why macOS Local Network privacy can be the cause
  Failed(String, Option<String>),
  /// Another certificate: try the next address, and name the pin when none answers
  Untrusted(String),
  /// The agent answered and refused: no other address can do better
  Refused(Refusal),
}

impl Host {
  pub(crate) fn open(config: &Config, alias: &str, version: &str, timing: Timing, net: Net) -> Result<Self, Refusal> {
    let file = config.read_host(alias)?;
    let identity = config.identity()?;
    let tls = ClientTls::new(file.fingerprint, &identity).map_err(|error| Refusal::new(Code::Config, format!("cannot set up TLS with the client identity: {error}")))?;
    Ok(Self {
      config: config.clone(),
      file,
      version: version.to_owned(),
      tls,
      timing,
      net,
    })
  }

  pub fn alias(&self) -> &str {
    &self.file.alias
  }

  pub const fn file(&self) -> &HostFile {
    &self.file
  }

  /// A new connection after its hello
  pub async fn connect(&mut self) -> Result<(Channel, AgentInfo), Refusal> {
    let mut tries = Tries::default();
    if let Some(answered) = self.try_each(self.file.addresses.clone(), &mut tries).await {
      return answered;
    }
    let resolved = resolve(&self.file.hostname, self.file.port, self.timing.resolve).await;
    let lookup = match &resolved {
      Ok(addresses) if addresses.is_empty() => format!("{} resolved to no address", self.file.hostname),
      Ok(addresses) => format!(
        "{} resolved to {}",
        self.file.hostname,
        addresses.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
      ),
      Err(said) => format!("{} did not resolve: {said}", self.file.hostname),
    };
    let fresh: Vec<SocketAddr> = resolved.unwrap_or_default().into_iter().filter(|address| !self.file.addresses.contains(address)).collect();
    if let Some(answered) = self.try_each(fresh, &mut tries).await {
      return answered;
    }
    let Tries {
      tried, untrusted, local_network, ..
    } = tries;
    let alias = self.alias().to_owned();
    if let Some(said) = untrusted {
      return Err(Refusal::new(
        Code::Untrusted,
        format!(
          "the agent on {alias} is not the one that the host file pins ({}), or it does not accept this client: {said}; {}",
          self.file.fingerprint.short(),
          setup_hint(&alias)
        ),
      ));
    }
    let tried = if tried.is_empty() { "no cached address".to_owned() } else { tried.join(", ") };
    let local_network = local_network.map_or_else(String::new, |cause| format!(" {cause}."));
    Err(Refusal::new(
      Code::Unreachable,
      format!(
        "no agent answers on {alias}: tried {tried}; {lookup}.{local_network} The Mac can sleep, or the agent is not installed: {}",
        setup_hint(&alias)
      ),
    ))
  }

  /// One request on a new connection, and its first reply. `deadline` bounds the wait for each frame of the reply
  pub(crate) async fn request(&mut self, request: &Request, deadline: Duration) -> Result<(Channel, Reply), Refusal> {
    let (mut channel, _) = self.connect().await?;
    channel.send(request).await.map_err(|error| self.link_refusal(&error))?;
    let reply = self.recv(&mut channel, deadline).await?;
    Ok((channel, reply))
  }

  /// The next reply on `channel` that is not a heartbeat. `deadline` bounds the wait for each frame, so each heartbeat
  /// starts the deadline again
  pub(crate) async fn recv(&self, channel: &mut Channel, deadline: Duration) -> Result<Reply, Refusal> {
    self.recv_beating(channel, deadline, &mut || {}).await
  }

  /// `recv` that calls `heartbeat` at each heartbeat it skips
  pub(crate) async fn recv_beating(&self, channel: &mut Channel, deadline: Duration, heartbeat: &mut (dyn FnMut() + Send)) -> Result<Reply, Refusal> {
    channel.set_read_deadline(deadline);
    loop {
      match channel.recv::<Reply>().await.map_err(|error| self.link_refusal(&error))? {
        Reply::Heartbeat { .. } => heartbeat(),
        reply => return Ok(reply),
      }
    }
  }

  /// The refusal of a failed channel: `unreachable`, `untrusted`, `timed_out` or `link`
  pub(crate) fn link_refusal(&self, error: &LinkError) -> Refusal {
    let alias = self.alias();
    let message = match error {
      LinkError::TimedOut => format!("the agent on {alias} did not answer in time"),
      LinkError::Untrusted(said) => format!("the link to {alias} is not trusted: {said}"),
      LinkError::Unreachable(said) => format!("the link to {alias} broke: {said}"),
      LinkError::Protocol(said) => format!("the link to {alias} failed: {said}"),
      LinkError::Io(said) => format!("the link to {alias} failed: {said}"),
    };
    Refusal::new(error.code(), message)
  }

  /// The refusal for a reply that the protocol does not allow there
  pub(crate) fn unexpected(&self, reply: &Reply) -> Refusal {
    Refusal::new(Code::Link, format!("the agent on {} sent a reply out of turn: {reply:?}", self.alias()))
  }

  /// A refusal of the agent, with its own code and the alias for the host that it names (`<host>`). A code that this
  /// build does not know is `agent_error`
  pub(crate) fn agent_refusal(&self, code: &str, message: &str) -> Refusal {
    Refusal::of_agent(code, &format!("{}: {}", self.alias(), on_host(message, self.alias())))
  }

  /// Tries each of `addresses` in turn, and notes each failure in `tries`. Answers the first channel, the refusal of
  /// an agent that answered and refused, or `None` when no address answered
  async fn try_each(&mut self, addresses: Vec<SocketAddr>, tries: &mut Tries) -> Option<Result<(Channel, AgentInfo), Refusal>> {
    for address in addresses {
      match self.attempt(address).await {
        Ok(connected) => {
          self.remember(address, &tries.failed);
          return Some(Ok(connected));
        }
        Err(Attempt::Refused(refusal)) => return Some(Err(refusal)),
        Err(Attempt::Failed(said, local_network)) => {
          tries.tried.push(format!("{address} ({said})"));
          tries.local_network = tries.local_network.take().or(local_network);
        }
        Err(Attempt::Untrusted(said)) => {
          tries.tried.push(format!("{address} (untrusted)"));
          tries.untrusted.get_or_insert(said);
        }
      }
      tries.failed.push(address);
    }
    None
  }

  async fn attempt(&self, address: SocketAddr) -> Result<(Channel, AgentInfo), Attempt> {
    let connected = match self.net.dial(address, self.timing.connect).await {
      Ok(link) => Channel::connect_over(link, &self.tls, self.timing.connect).await,
      Err(error) => Err(error),
    };
    let mut channel = match connected {
      Ok(channel) => channel,
      Err(LinkError::Untrusted(said)) => return Err(Attempt::Untrusted(said)),
      Err(error) => {
        let local_network = local_network_cause(&error, address);
        return Err(Attempt::Failed(error.to_string(), local_network));
      }
    };
    let hello = channel.exchange::<_, Reply>(&Request::hello(&self.version)).await;
    // The connect deadline is short, for a dead address. A write of a large pack to a slow link takes longer, so each
    // later write has the read deadline of a reply
    channel.set_write_deadline(self.timing.read);
    match hello {
      Ok(Reply::Hello {
        schema_version,
        agent_version,
        os,
        arch,
        features,
        binary_sha256,
      }) if schema_version == SCHEMA_VERSION => Ok((
        channel,
        AgentInfo {
          version: agent_version,
          schema: schema_version,
          os,
          arch,
          features,
          build: binary_sha256,
          address,
        },
      )),
      Ok(Reply::Hello {
        schema_version, agent_version, ..
      }) => Err(Attempt::Refused(self.schema_refusal(&format!("brd-host {agent_version} speaks schema {schema_version}")))),
      Ok(Reply::Refused { code, message }) if code == Code::Schema.name() => Err(Attempt::Refused(self.schema_refusal(&message))),
      Ok(Reply::Refused { code, message }) => Err(Attempt::Refused(self.agent_refusal(&code, &message))),
      Ok(reply) => Err(Attempt::Refused(self.unexpected(&reply))),
      Err(LinkError::Untrusted(said)) => Err(Attempt::Untrusted(said)),
      Err(error) => Err(Attempt::Failed(error.to_string(), None)),
    }
  }

  fn schema_refusal(&self, said: &str) -> Refusal {
    Refusal::new(
      Code::Schema,
      format!(
        "the agent on {} and this client speak two schemas of host-link ({said}; this client speaks {SCHEMA_VERSION}); {}",
        self.alias(),
        setup_hint(self.alias())
      ),
    )
  }

  /// Puts the address that answered first in the host file, then the cached addresses that did not fail, then the
  /// ones in `failed`. An address of the host name answers only after every cached address failed, and the cached ones
  /// then go. A host file that cannot be written keeps the old addresses
  fn remember(&mut self, address: SocketAddr, failed: &[SocketAddr]) {
    let cached = self.file.addresses.contains(&address);
    let mut addresses = vec![address];
    addresses.extend(self.file.addresses.iter().copied().filter(|known| *known != address && !failed.contains(known)));
    if cached {
      addresses.extend(failed.iter().copied().filter(|known| *known != address));
    }
    addresses.truncate(KEPT_ADDRESSES);
    if addresses != self.file.addresses {
      self.file.addresses = addresses;
      let _ = self.config.write_host(&self.file);
    }
  }
}

/// The addresses of a host name, IPv4 first, under a deadline. The lookup of tokio runs on the blocking pool, because
/// the resolver of the system has no deadline. A lookup that ends late ends on its own
pub(crate) async fn resolve(hostname: &str, port: u16, deadline: Duration) -> Result<Vec<SocketAddr>, String> {
  let Ok(found) = timeout(deadline, tokio::net::lookup_host((hostname, port))).await else {
    return Err(format!("no answer in {} s", deadline.as_secs()));
  };
  let mut found: Vec<SocketAddr> = found.map_err(|error| error.to_string())?.collect();
  found.sort_by_key(SocketAddr::is_ipv6);
  let mut addresses = Vec::with_capacity(found.len());
  for address in found {
    if !addresses.contains(&address) {
      addresses.push(address);
    }
  }
  Ok(addresses)
}

/// The hello of a host and a hint when it runs another build than this client: the run goes on, and one line names
/// `brd-host upgrade <alias>`, that runs work across builds, and that an upgrade waits for the user, as it deploys a
/// new agent on a machine that the user shares
pub(crate) fn another_build_hint(alias: &str, agent: &AgentInfo, client: Option<&str>) -> Option<String> {
  match (agent.build.as_deref(), client) {
    (Some(agent_build), Some(client_build)) if agent_build != client_build => Some(format!(
      "brd-host: {alias} runs another build of brd-host ({} {}, this client {}); runs work across builds, so this is no error: `brd-host upgrade {alias}` sends this one, only after the user agrees",
      agent.version,
      crate::short(agent_build),
      crate::short(client_build)
    )),
    _ => None,
  }
}

#[cfg(test)]
#[path = "host_test.rs"]
mod host_test;
