use super::*;
use crate::testkit::{ALIAS, FAST, Hello, Setup, host_file};
use host_link::identity::Identity;
use pretty_assertions::assert_eq;
use std::net::TcpListener;

fn runs_reply(_: &Request, channel: &mut Channel) {
  let _ = channel.send(&Reply::Runs { runs: Vec::new() });
}

/// An address where nothing listens: a port that was free a moment ago
fn dead_address() -> SocketAddr {
  TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

fn open(setup: &Setup) -> Host {
  Host::open(&setup.config, ALIAS, "0.37.0", FAST).unwrap()
}

#[test]
fn the_cached_address_answers_with_the_hello() {
  let setup = Setup::new(runs_reply);
  let (channel, info) = open(&setup).connect().unwrap();
  channel.close();
  assert_eq!(
    info,
    AgentInfo {
      version: "0.37.0".to_owned(),
      schema: SCHEMA_VERSION,
      os: "macos".to_owned(),
      arch: "aarch64".to_owned(),
      features: vec!["upgrade".to_owned()],
      build: None,
      address: setup.agent.address,
    }
  );
}

/// A cached address that no longer answers costs one connect; the host name gives the new one, and the host file
/// keeps the new one alone, because every cached address failed
#[test]
fn a_dead_cached_address_falls_back_to_the_host_name_and_the_file_keeps_the_new_one() {
  let setup = Setup::new(runs_reply);
  setup.config.write_host(&host_file(&setup.agent, vec![dead_address()])).unwrap();
  let (channel, info) = open(&setup).connect().unwrap();
  channel.close();
  assert_eq!(info.address, setup.agent.address);
  assert_eq!(setup.config.read_host(ALIAS).unwrap().addresses, [setup.agent.address]);
}

/// A cached address that answers after a dead one goes first, so the next connection does not try the dead one first
#[test]
fn a_cached_address_that_answers_goes_before_a_cached_one_that_failed() {
  let setup = Setup::new(runs_reply);
  let (dead, other) = (dead_address(), dead_address());
  setup.config.write_host(&host_file(&setup.agent, vec![dead, setup.agent.address, other])).unwrap();
  let (channel, info) = open(&setup).connect().unwrap();
  channel.close();
  assert_eq!(info.address, setup.agent.address);
  assert_eq!(
    setup.config.read_host(ALIAS).unwrap().addresses,
    [setup.agent.address, other, dead],
    "the untried address keeps its place before the failed one"
  );
}

#[test]
fn no_answer_names_each_address_the_host_name_and_the_setup_command() {
  let setup = Setup::new(runs_reply);
  let dead = dead_address();
  let mut file = host_file(&setup.agent, vec![dead]);
  file.port = dead.port();
  setup.config.write_host(&file).unwrap();
  let refusal = open(&setup).connect().unwrap_err();
  assert_eq!(refusal.code, Code::Unreachable);
  assert!(
    refusal.message.starts_with(&format!("no agent answers on studio: tried {dead} (no connection: ")),
    "{}",
    refusal.message
  );
  assert!(refusal.message.contains(&format!("127.0.0.1 resolved to {dead}")), "{}", refusal.message);
  assert!(refusal.message.contains("`bd-host setup studio`"), "{}", refusal.message);
}

#[test]
fn an_agent_with_another_certificate_is_untrusted() {
  let setup = Setup::new(runs_reply);
  let mut file = host_file(&setup.agent, vec![setup.agent.address]);
  file.fingerprint = Identity::generate().unwrap().fingerprint();
  setup.config.write_host(&file).unwrap();
  assert_eq!(open(&setup).connect().unwrap_err().code, Code::Untrusted);
}

#[test]
fn an_agent_that_does_not_allow_this_client_is_untrusted() {
  let setup = Setup::new(runs_reply);
  let stranger = Config::new(setup.dir.path().join("stranger"));
  stranger.write_host(&host_file(&setup.agent, vec![setup.agent.address])).unwrap();
  let refusal = Host::open(&stranger, ALIAS, "0.37.0", FAST).unwrap().connect().unwrap_err();
  assert_eq!(refusal.code, Code::Untrusted, "{}", refusal.message);
}

/// Another schema refuses with `schema`; another build of the same schema runs, with one line that names the upgrade
#[test]
fn another_schema_refuses_and_another_build_runs_with_a_hint() {
  let hello = Hello {
    schema: SCHEMA_VERSION + 1,
    ..Hello::default()
  };
  let setup = Setup::with_hello(hello, runs_reply);
  let refusal = open(&setup).connect().unwrap_err();
  assert_eq!(refusal.code, Code::Schema);
  assert!(refusal.message.contains(&format!("speaks schema {}", SCHEMA_VERSION + 1)), "{}", refusal.message);
  assert!(refusal.message.contains("`bd-host setup studio`"), "{}", refusal.message);

  let agent = AgentInfo {
    version: "0.37.0".to_owned(),
    schema: SCHEMA_VERSION,
    os: "macos".to_owned(),
    arch: "aarch64".to_owned(),
    features: Vec::new(),
    build: Some("a".repeat(64)),
    address: dead_address(),
  };
  let hint = another_build_hint("studio", &agent, Some(&"b".repeat(64))).unwrap();
  assert!(hint.contains("`bd-host upgrade studio`"), "{hint}");
  assert_eq!(another_build_hint("studio", &agent, Some(&"a".repeat(64))), None, "the same build");
  assert_eq!(another_build_hint("studio", &agent, None), None, "a client that cannot read its own binary");
}

#[test]
fn a_lookup_answers_ipv4_first_and_each_address_once() {
  let found = resolve("127.0.0.1", 7742, Duration::from_secs(2)).unwrap();
  assert_eq!(found, ["127.0.0.1:7742".parse::<SocketAddr>().unwrap()]);
  assert_ne!(resolve("no-such-host.invalid", 7742, Duration::from_secs(5)).unwrap_err(), "");
}
