use super::*;
use crate::testkit::{FakeProcesses, Hello, Said, Setup, calm};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

/// The head of a Mach-O binary of arm64, the platform of the fake agent, and a body
const MACHO_ARM64: &[u8] = b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01 a bd-host of arm64";

/// The build that the fake agent runs before an upgrade
const OLD: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// An agent of `OLD` that takes an upgrade. With `restarts`, the hellos after the upgrade name the new build, as a new
/// `serve` would; without it, the old build answers, as after the rollback of the watchdog
fn upgradable(restarts: bool) -> Setup {
  let build = Arc::new(Mutex::new(Some(OLD.to_owned())));
  let hello = Hello {
    build: Arc::clone(&build),
    ..Hello::default()
  };
  Setup::with_hello(hello, move |request, channel| {
    let Request::Upgrade { size, sha256, .. } = request else {
      let _ = channel.send(&Reply::refused(Code::BadRequest, "not in this script"));
      return;
    };
    let mut bytes = Vec::new();
    channel.recv_stream(&mut bytes).unwrap();
    assert_eq!((bytes.len() as u64, sha256_hex(&bytes)), (*size, sha256.clone()));
    if restarts {
      *build.lock().unwrap_or_else(PoisonError::into_inner) = Some(sha256.clone());
    }
    let _ = channel.send(&Reply::Upgraded {
      previous_sha256: OLD.to_owned(),
      sha256: sha256.clone(),
      version: "0.37.0".to_owned(),
    });
  })
}

fn binary_file(setup: &Setup, bytes: &[u8]) -> PathBuf {
  let path = setup.dir.path().join("bd-host");
  std::fs::write(&path, bytes).unwrap();
  path
}

fn request(binary: &Path) -> UpgradeRequest<'_> {
  UpgradeRequest {
    alias: "studio",
    binary,
    force: false,
  }
}

/// The binary goes once to an agent with the feature, and the upgrade ends at the hello of the new build; the build
/// that the agent runs already does not go without `--force`; a binary of another platform never goes
#[test]
fn upgrade_sends_this_executable_once_and_refuses_another_platform() {
  let setup = upgradable(true);
  let binary = binary_file(&setup, MACHO_ARM64);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let processes = FakeProcesses::default();
  let upgraded = setup.client(&processes, &say, &calm).upgrade(&request(&binary)).unwrap();
  let sha256 = sha256_hex(MACHO_ARM64);
  assert_eq!(upgraded.way, UpgradeWay::Request);
  assert_eq!(upgraded.previous_sha256.as_deref(), Some(OLD));
  assert_eq!((upgraded.sha256.as_str(), upgraded.version.as_str()), (sha256.as_str(), "0.37.0"));
  assert_eq!(
    setup.agent.requests(),
    [Request::Upgrade {
      size: MACHO_ARM64.len() as u64,
      sha256: sha256.clone(),
      force: false,
    }]
  );
  assert_eq!(
    said.lines(),
    [
      format!("bd-host: sending bd-host ({} B) to studio", MACHO_ARM64.len()),
      format!("bd-host: studio installed bd-host {}; waiting for the new agent to answer", &sha256[..12]),
    ]
  );

  let current = setup.client(&processes, &|_| {}, &calm).upgrade(&request(&binary)).unwrap();
  assert_eq!(current.way, UpgradeWay::Current);
  assert_eq!(setup.agent.requests().len(), 1, "the build that the agent runs does not go again");

  let mut elf = b"\x7fELF".to_vec();
  elf.resize(18, 0);
  elf.extend_from_slice(&0xb7_u16.to_le_bytes());
  let linux = binary_file(&setup, &elf);
  let refusal = setup.client(&processes, &|_| {}, &calm).upgrade(&request(&linux)).unwrap_err();
  assert_eq!(refusal.code, Code::AgentFailed);
  assert!(
    refusal.message.ends_with("is a binary of linux aarch64, and the host runs macos aarch64"),
    "{}",
    refusal.message
  );
  assert_eq!(setup.agent.requests().len(), 1);
}

#[test]
fn the_previous_build_that_answers_at_the_end_is_a_rollback() {
  let setup = upgradable(false);
  let binary = binary_file(&setup, MACHO_ARM64);
  let refusal = setup.client(&FakeProcesses::default(), &|_| {}, &calm).upgrade(&request(&binary)).unwrap_err();
  assert_eq!(refusal.code, Code::RolledBack);
  assert!(refusal.message.contains(&OLD[..12]), "{}", refusal.message);
  assert!(refusal.message.contains("~/.cache/bd-host/upgrade.json on studio"), "{}", refusal.message);
}

/// An agent without the feature cannot read an upgrade request, so the client refuses before the binary goes
#[test]
fn an_agent_without_the_feature_is_refused_before_the_binary_goes() {
  let hello = Hello {
    features: Vec::new(),
    ..Hello::default()
  };
  let setup = Setup::with_hello(hello, |_, _| {});
  let binary = binary_file(&setup, MACHO_ARM64);
  let refusal = setup.client(&FakeProcesses::default(), &|_| {}, &calm).upgrade(&request(&binary)).unwrap_err();
  assert_eq!(refusal.code, Code::AgentFailed);
  assert_eq!(
    refusal.message,
    "the agent on studio has no feature upgrade: `bd-host setup studio` installs a new one over ssh"
  );
  assert_eq!(setup.agent.requests().len(), 0);
}

#[test]
fn the_header_names_the_platform_of_a_binary() {
  let elf = |machine: u16| {
    let mut bytes = b"\x7fELF".to_vec();
    bytes.resize(18, 0);
    bytes.extend_from_slice(&machine.to_le_bytes());
    bytes
  };
  assert_eq!(binary_platform(MACHO_ARM64), Platform::Of("macos", "aarch64"));
  assert_eq!(binary_platform(b"\xcf\xfa\xed\xfe\x07\x00\x00\x01"), Platform::Of("macos", "x86_64"));
  assert_eq!(binary_platform(&elf(0x3e)), Platform::Of("linux", "x86_64"));
  assert_eq!(binary_platform(&elf(0xb7)), Platform::Of("linux", "aarch64"));
  assert_eq!(binary_platform(b"\xca\xfe\xba\xbe\x00\x00\x00\x02"), Platform::Universal);
  for unknown in [&b"#!/bin/sh"[..], b"", &elf(0x28), b"\xcf\xfa\xed\xfe\x12\x00\x00\x00"] {
    assert_eq!(binary_platform(unknown), Platform::Unknown, "{unknown:?}");
  }
}
