//! Sends a new binary to the agent of a host over the link, without ssh, so a background session can do it
//! (spec/cli/host.spec.md, "upgrade"):
//!
//! 1. The hello names the platform, the features and the build of the agent. A host that is not macOS, or an agent
//!    without `feature::UPGRADE`, refuses with `agent_failed`.
//! 2. The binary is `--binary`, or this executable. It must be of the platform of the host (its Mach-O or ELF header),
//!    and the build that the agent runs already is current and does not go, unless `force`.
//! 3. `Request::Upgrade` carries the binary.
//! 4. The client connects again until the hello names the new build, up to the drain of the old agent, the wait of the
//!    watchdog and 30 s for launchd. A hello of the previous build at the end means that the watchdog restored it

use crate::config::check_alias;
use crate::host::AgentInfo;
use crate::setup::sha256_hex;
use crate::{Client, short};
use host_link::proto::{Reply, Request, feature};
use host_link::refusal::{Code, Refusal};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::size::format_size;

/// The longest wait between two hellos after an upgrade
const MAX_POLL: Duration = Duration::from_secs(5);

/// What `upgrade` does
#[derive(Clone, Copy, Debug)]
pub struct UpgradeRequest<'a> {
  /// The alias of the host file
  pub alias: &'a str,
  /// The binary for the host: `--binary`, or this executable
  pub binary: &'a Path,
  /// Send the build that the agent runs already too, and let the agent take a binary of a lower version
  pub force: bool,
}

/// How the new binary came to the host
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpgradeWay {
  /// The agent runs the build already
  Current,
  /// `Request::Upgrade` over the link
  Request,
}

/// What `upgrade` did
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upgraded {
  pub way: UpgradeWay,
  /// The build that ran before, when the agent named it
  pub previous_sha256: Option<String>,
  pub sha256: String,
  pub version: String,
  /// The hello of the new agent
  pub agent: AgentInfo,
}

/// The fields of `Reply::Upgraded`
struct Installed {
  previous_sha256: String,
  sha256: String,
}

impl Client<'_> {
  pub fn upgrade(&self, request: &UpgradeRequest<'_>) -> Result<Upgraded, Refusal> {
    let alias = request.alias;
    check_alias(alias)?;
    let mut host = self.host(alias)?;
    let (channel, info) = host.connect()?;
    channel.close();
    if info.os != "macos" {
      return Err(Refusal::new(
        Code::AgentFailed,
        format!(
          "the agent on {alias} runs on {}, and only launchd on macOS starts an upgraded agent: `bd-host setup {alias}` installs a new one",
          info.os
        ),
      ));
    }
    let binary = std::fs::read(request.binary).map_err(|error| Refusal::new(Code::AgentFailed, format!("cannot read the binary {}: {error}", request.binary.display())))?;
    check_platform(&binary, request.binary, &info)?;
    let sha256 = sha256_hex(&binary);
    if !request.force && info.build.as_deref() == Some(sha256.as_str()) {
      return Ok(Upgraded {
        way: UpgradeWay::Current,
        previous_sha256: Some(sha256.clone()),
        sha256,
        version: info.version.clone(),
        agent: info,
      });
    }
    if !info.features.iter().any(|known| known == feature::UPGRADE) {
      return Err(Refusal::new(
        Code::AgentFailed,
        format!(
          "the agent on {alias} has no feature {}: `bd-host setup {alias}` installs a new one over ssh",
          feature::UPGRADE
        ),
      ));
    }
    (self.say)(&format!("bd-host: sending bd-host ({}) to {alias}", format_size(binary.len() as u64)));
    let (mut channel, _) = host.connect()?;
    let upgrade = Request::Upgrade {
      size: binary.len() as u64,
      sha256,
      force: request.force,
    };
    channel
      .send(&upgrade)
      .and_then(|()| channel.send_stream(&mut binary.as_slice(), &mut |_| {}))
      .map_err(|error| host.link_refusal(&error))?;
    let installed = match host.recv(&mut channel, self.timing.read)? {
      Reply::Upgraded { previous_sha256, sha256, .. } => Installed { previous_sha256, sha256 },
      Reply::Refused { code, message } => return Err(host.agent_refusal(&code, &message)),
      reply => return Err(host.unexpected(&reply)),
    };
    channel.close();
    self.wait_for_new(alias, installed)
  }

  /// Connects again until the hello names the new build, up to the upgrade time. Refuses with `rolled_back` when the
  /// previous build answers at the end
  fn wait_for_new(&self, alias: &str, installed: Installed) -> Result<Upgraded, Refusal> {
    (self.say)(&format!(
      "bd-host: {alias} installed bd-host {}; waiting for the new agent to answer",
      short(&installed.sha256)
    ));
    let started = Instant::now();
    let mut backoff = self.timing.first_backoff;
    let mut answered: Option<Option<String>> = None;
    let mut lost = None;
    loop {
      let mut host = self.host(alias)?;
      match host.connect() {
        Ok((channel, info)) => {
          channel.close();
          if info.build.as_deref() == Some(installed.sha256.as_str()) {
            return Ok(Upgraded {
              way: UpgradeWay::Request,
              previous_sha256: Some(installed.previous_sha256),
              sha256: installed.sha256,
              version: info.version.clone(),
              agent: info,
            });
          }
          answered = Some(info.build);
        }
        Err(refusal) if matches!(refusal.code, Code::Unreachable | Code::Link | Code::TimedOut) => lost = Some(refusal),
        Err(refusal) => return Err(refusal),
      }
      if started.elapsed() >= self.timing.upgrade {
        break;
      }
      thread::sleep(backoff);
      backoff = (backoff * 2).min(MAX_POLL.min(self.timing.max_backoff));
    }
    let waited = self.timing.upgrade.as_secs();
    let log = format!("~/.cache/bd-host/upgrade.json on {alias} has the outcome of the watchdog, and ~/Library/Logs/bd-host.log its lines");
    Err(match answered {
      Some(Some(binary)) if binary == installed.previous_sha256 => Refusal::new(
        Code::RolledBack,
        format!(
          "the new agent {} did not listen, so the watchdog brought back the previous agent {}; {log}",
          short(&installed.sha256),
          short(&binary)
        ),
      ),
      Some(binary) => Refusal::new(
        Code::AgentFailed,
        format!(
          "the agent on {alias} answers with the build {}, not the new {}, {waited} s after the upgrade; {log}",
          binary.as_deref().map_or("that it does not name", short),
          short(&installed.sha256)
        ),
      ),
      None => {
        let said = lost.map_or_else(String::new, |refusal| format!(": {}", refusal.message));
        Refusal::new(Code::Unreachable, format!("no agent answers on {alias} {waited} s after the upgrade{said}; {log}"))
      }
    })
  }
}

/// Refuses a binary that is not of the platform of the host. A universal Mach-O binary runs on both architectures
fn check_platform(binary: &[u8], path: &Path, host: &AgentInfo) -> Result<(), Refusal> {
  match binary_platform(binary) {
    Platform::Universal => Ok(()),
    Platform::Of(os, arch) if os == host.os && arch == host.arch => Ok(()),
    Platform::Of(os, arch) => Err(Refusal::new(
      Code::AgentFailed,
      format!("{} is a binary of {os} {arch}, and the host runs {} {}", path.display(), host.os, host.arch),
    )),
    Platform::Unknown => Err(Refusal::new(
      Code::AgentFailed,
      format!("{} is no Mach-O or ELF binary of aarch64 or x86_64", path.display()),
    )),
  }
}

/// The platform of a binary from its header, in the names of `std::env::consts`
#[derive(Debug, PartialEq, Eq)]
enum Platform {
  Of(&'static str, &'static str),
  Universal,
  Unknown,
}

fn binary_platform(binary: &[u8]) -> Platform {
  let u32_at = |at: usize| binary.get(at..at + 4).map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
  let u16_at = |at: usize| binary.get(at..at + 2).map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]));
  match binary.get(..4) {
    Some([0xcf, 0xfa, 0xed, 0xfe]) => match u32_at(4) {
      Some(0x0100_000c) => Platform::Of("macos", "aarch64"),
      Some(0x0100_0007) => Platform::Of("macos", "x86_64"),
      _ => Platform::Unknown,
    },
    Some([0xca, 0xfe, 0xba, 0xbe]) => Platform::Universal,
    Some([0x7f, b'E', b'L', b'F']) => match u16_at(18) {
      Some(0xb7) => Platform::Of("linux", "aarch64"),
      Some(0x3e) => Platform::Of("linux", "x86_64"),
      _ => Platform::Unknown,
    },
    _ => Platform::Unknown,
  }
}

#[cfg(test)]
#[path = "upgrade_test.rs"]
mod upgrade_test;
