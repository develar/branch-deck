use super::*;
use crate::testkit::{ALIAS, FakeProcesses, Said, Setup, calm, failed, ok};
use host_link::channel::Channel;
use host_link::proto::{Reply, Request};
use pretty_assertions::assert_eq;

/// The binary of the tests
const BINARY: &[u8] = b"a bd-host binary";

fn runs_reply(_: &Request, channel: &mut Channel) {
  let _ = channel.send(&Reply::Runs { runs: Vec::new() });
}

/// A host whose ssh answers each call of `setup`: the probe with `host_sum`, then the setup line of the agent
fn host(setup: &Setup, host_sum: &str) -> FakeProcesses {
  let line = format!(
    "{{\"fingerprint\":\"{}\",\"port\":{},\"launchd\":\"restarted\"}}\n",
    setup.agent.identity.fingerprint(),
    setup.agent.address.port()
  );
  FakeProcesses::new(&[
    ("shasum -a 256", ok(&format!("{host_sum}  .local/bin/bd-host\n"))),
    ("bd-host setup --agent", ok(&line)),
    ("ssh -G studio", ok("user develar\nhostname 127.0.0.1\nport 22\n")),
  ])
}

fn binary_file(setup: &Setup) -> PathBuf {
  let path = setup.dir.path().join("bd-host");
  std::fs::write(&path, BINARY).unwrap();
  path
}

fn request(binary: &Path) -> SetupRequest<'_> {
  SetupRequest {
    alias: ALIAS,
    binary,
    port: 7742,
    repo: "~/projects/idea-1",
    name: "bt on dev's Mac",
  }
}

#[test]
fn setup_copies_the_binary_runs_the_agent_setup_and_writes_the_host_file() {
  let setup = Setup::new(runs_reply);
  std::fs::remove_file(setup.config.host_path(ALIAS)).unwrap();
  let processes = host(&setup, "0000");
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let binary = binary_file(&setup);
  let prepared = setup.client(&processes, &say, &calm).setup(&request(&binary)).unwrap();

  assert!(prepared.copied);
  assert_eq!(prepared.launchd, "restarted");
  assert_eq!(prepared.agent.address, setup.agent.address);
  let file = setup.config.read_host(ALIAS).unwrap();
  assert_eq!(file.fingerprint, setup.agent.identity.fingerprint());
  assert_eq!((file.hostname.as_str(), file.port), ("127.0.0.1", setup.agent.address.port()));
  assert_eq!(file.addresses, [setup.agent.address]);

  let calls = processes.calls();
  let ssh_prefix: Vec<String> = SSH_OPTIONS.iter().copied().chain([ALIAS]).map(str::to_owned).collect();
  let copy = calls.iter().find(|call| call.text().contains("cat > .local/bin/bd-host.new")).unwrap();
  assert_eq!(copy.args[..=SSH_OPTIONS.len()], ssh_prefix[..]);
  assert_eq!((copy.input.as_slice(), copy.deadline), (BINARY, COPY_DEADLINE));
  let client_fingerprint = setup.config.identity().unwrap().fingerprint();
  assert!(
    processes.texts().iter().any(|text| text.ends_with(&format!(
      "studio .local/bin/bd-host setup --agent --client {client_fingerprint} --name 'bt on dev'\\''s Mac' --port 7742"
    ))),
    "{:#?}",
    processes.texts()
  );
  assert!(said.lines().iter().any(|line| line.starts_with("bd-host: copying bd-host (16 B)")), "{:#?}", said.lines());
}

#[test]
fn the_same_binary_is_not_copied_again() {
  let setup = Setup::new(runs_reply);
  let processes = host(&setup, &sha256_hex(BINARY));
  let binary = binary_file(&setup);
  let prepared = setup.client(&processes, &|_| {}, &calm).setup(&request(&binary)).unwrap();
  assert!(!prepared.copied);
  assert!(!processes.texts().iter().any(|text| text.contains("cat >")));
}

#[test]
fn a_host_that_ssh_cannot_reach_or_that_cannot_run_the_binary_is_refused() {
  let setup = Setup::new(runs_reply);
  let binary = binary_file(&setup);
  let processes = FakeProcesses::new(&[("shasum", failed(255, "ssh: connect to host studio port 22: Operation timed out\n"))]);
  let refusal = setup.client(&processes, &|_| {}, &calm).setup(&request(&binary)).unwrap_err();
  assert_eq!(refusal.code, Code::SshFailed);
  assert!(
    refusal.message.contains("Operation timed out") && refusal.message.contains("`ssh studio true`"),
    "{}",
    refusal.message
  );

  let processes = FakeProcesses::new(&[("shasum", ok("")), ("setup --agent", failed(126, "zsh: exec format error: .local/bin/bd-host\n"))]);
  let refusal = setup.client(&processes, &|_| {}, &calm).setup(&request(&binary)).unwrap_err();
  assert_eq!(refusal.code, Code::AgentFailed);
  assert!(refusal.message.contains("exec format error"), "{}", refusal.message);
}

#[test]
fn the_digest_is_lower_case_hex() {
  assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}
