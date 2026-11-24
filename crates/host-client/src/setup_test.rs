use super::*;
use crate::testkit::{ALIAS, FakeProcesses, Said, Setup, calm, failed, ok, runs_reply};
use host_link::proto::Request;
use pretty_assertions::assert_eq;

/// The binary of the tests
const BINARY: &[u8] = b"a brd-host binary";

/// A host whose ssh answers each call of `setup`: the probe with `host_sum`, then the setup line of the agent
fn host(setup: &Setup, host_sum: &str) -> FakeProcesses {
  let line = format!(
    "{{\"fingerprint\":\"{}\",\"port\":{},\"launchd\":\"restarted\"}}\n",
    setup.agent.identity.fingerprint(),
    setup.agent.address.port()
  );
  FakeProcesses::new(&[
    ("shasum -a 256", ok(&format!("{host_sum}  .local/bin/brd-host\n"))),
    ("brd-host setup --agent", ok(&line)),
    ("ssh -G studio", ok("user develar\nhostname 127.0.0.1\nport 22\n")),
  ])
}

fn binary_file(setup: &Setup) -> PathBuf {
  let path = setup.dir.path().join("brd-host");
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

#[tokio::test(start_paused = true)]
async fn setup_copies_the_binary_runs_the_agent_setup_and_writes_the_host_file() {
  let setup = Setup::new(runs_reply);
  std::fs::remove_file(setup.config.host_path(ALIAS)).unwrap();
  let processes = host(&setup, "0000");
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let binary = binary_file(&setup);
  let prepared = setup.client(&processes, &say, &calm).setup(&request(&binary)).await.unwrap();

  assert!(prepared.copied);
  assert_eq!(prepared.launchd, "restarted");
  assert_eq!(prepared.agent.address, setup.agent.address);
  let file = setup.config.read_host(ALIAS).unwrap();
  assert_eq!(file.fingerprint, setup.agent.identity.fingerprint());
  assert_eq!((file.hostname.as_str(), file.port), ("127.0.0.1", setup.agent.address.port()));
  assert_eq!(file.addresses, [setup.agent.address]);

  let calls = processes.calls();
  let ssh_prefix: Vec<String> = SSH_OPTIONS.iter().copied().chain([ALIAS]).map(str::to_owned).collect();
  let copy = calls.iter().find(|call| call.text().contains("cat > .local/bin/brd-host.new")).unwrap();
  assert_eq!(copy.args[..=SSH_OPTIONS.len()], ssh_prefix[..]);
  assert_eq!((copy.input.as_slice(), copy.deadline), (BINARY, COPY_DEADLINE));
  let client_fingerprint = setup.config.identity().unwrap().fingerprint();
  assert!(
    processes.texts().iter().any(|text| text.ends_with(&format!(
      "studio .local/bin/brd-host setup --agent --client {client_fingerprint} --name 'bt on dev'\\''s Mac' --port 7742"
    ))),
    "{:#?}",
    processes.texts()
  );
  assert!(said.lines().iter().any(|line| line.starts_with("brd-host: copying brd-host (17 B)")), "{:#?}", said.lines());
}

#[tokio::test(start_paused = true)]
async fn the_same_binary_is_not_copied_again() {
  let setup = Setup::new(runs_reply);
  let processes = host(&setup, &sha256_hex(BINARY));
  let binary = binary_file(&setup);
  let prepared = setup.client(&processes, &|_| {}, &calm).setup(&request(&binary)).await.unwrap();
  assert!(!prepared.copied);
  assert!(!processes.texts().iter().any(|text| text.contains("cat >")));
}

#[tokio::test(start_paused = true)]
async fn a_host_that_ssh_cannot_reach_or_that_cannot_run_the_binary_is_refused() {
  let setup = Setup::new(runs_reply);
  let binary = binary_file(&setup);
  let processes = FakeProcesses::new(&[("shasum", failed(255, "ssh: connect to host studio port 22: Operation timed out\n"))]);
  let refusal = setup.client(&processes, &|_| {}, &calm).setup(&request(&binary)).await.unwrap_err();
  assert_eq!(refusal.code, Code::SshFailed);
  assert!(
    refusal.message.contains("Operation timed out") && refusal.message.contains("`ssh studio true`"),
    "{}",
    refusal.message
  );

  let processes = FakeProcesses::new(&[("shasum", ok("")), ("setup --agent", failed(126, "zsh: exec format error: .local/bin/brd-host\n"))]);
  let refusal = setup.client(&processes, &|_| {}, &calm).setup(&request(&binary)).await.unwrap_err();
  assert_eq!(refusal.code, Code::AgentFailed);
  assert!(refusal.message.contains("exec format error"), "{}", refusal.message);
}

#[test]
fn the_digest_is_lower_case_hex() {
  assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

/// The agent setup of a local host for the fake agent of `setup`: the line that `brd-host setup --agent` prints for it,
/// once the fingerprint of this client is the one of the allow list of the fake
fn local_agent(setup: &Setup) -> impl Fn(Fingerprint) -> Result<LocalAgent, Refusal> + '_ {
  move |client| {
    assert_eq!(client, setup.config.identity().unwrap().fingerprint(), "the agent setup allows this client");
    Ok(LocalAgent {
      copied: true,
      line: format!(
        "{{\"fingerprint\":\"{}\",\"port\":{},\"launchd\":\"bootstrapped\"}}\n",
        setup.agent.identity.fingerprint(),
        setup.agent.address.port()
      ),
    })
  }
}

#[tokio::test(start_paused = true)]
async fn a_local_setup_pins_the_agent_of_this_machine_on_the_loopback_without_ssh_and_a_request_goes_there() {
  let setup = Setup::new(runs_reply);
  let processes = FakeProcesses::default();
  let client = setup.client(&processes, &|_| {}, &calm);
  let request = LocalSetupRequest {
    alias: LOCAL_ALIAS,
    repo: "~/projects/idea-1",
  };
  let prepared = client.setup_local(&request, &local_agent(&setup)).await.unwrap();

  assert!(prepared.copied);
  assert_eq!((prepared.launchd.as_str(), prepared.agent.address), ("bootstrapped", setup.agent.address));
  assert_eq!(prepared.host_file, setup.config.host_path(LOCAL_ALIAS));
  let file = setup.config.read_host(LOCAL_ALIAS).unwrap();
  assert_eq!(
    file,
    HostFile {
      alias: LOCAL_ALIAS.to_owned(),
      fingerprint: setup.agent.identity.fingerprint(),
      port: setup.agent.address.port(),
      hostname: "127.0.0.1".to_owned(),
      addresses: vec![SocketAddr::from((LOCAL_BIND, setup.agent.address.port()))],
      repo: "~/projects/idea-1".to_owned(),
    }
  );
  assert!(processes.calls().is_empty(), "no ssh, no child");

  client.runs(LOCAL_ALIAS).await.unwrap();
  assert_eq!(
    setup.agent.requests().last(),
    Some(&Request::Runs { run: None }),
    "a request over the loopback with the pins of the setup"
  );
}

#[tokio::test(start_paused = true)]
async fn a_local_setup_refuses_a_bad_alias_before_the_agent_setup_and_an_agent_setup_without_its_line() {
  let setup = Setup::new(runs_reply);
  let processes = FakeProcesses::default();
  let client = setup.client(&processes, &|_| {}, &calm);
  let never = |_: Fingerprint| -> Result<LocalAgent, Refusal> { panic!("the agent setup runs after the check of the alias") };
  let refusal = client.setup_local(&LocalSetupRequest { alias: "../x", repo: "~/r" }, &never).await.unwrap_err();
  assert_eq!(refusal.code, Code::Usage);

  let silent = |_: Fingerprint| {
    Ok(LocalAgent {
      copied: false,
      line: "launchd said nothing\n".to_owned(),
    })
  };
  let refusal = client.setup_local(&LocalSetupRequest { alias: LOCAL_ALIAS, repo: "~/r" }, &silent).await.unwrap_err();
  assert_eq!(refusal.code, Code::AgentFailed);
  assert!(refusal.message.contains("printed no setup line: launchd said nothing"), "{}", refusal.message);
  assert!(setup.config.read_host(LOCAL_ALIAS).is_err(), "no host file");
}
