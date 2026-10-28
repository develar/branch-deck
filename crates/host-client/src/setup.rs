//! Makes a host ready for runs over ssh, while the person is at the terminal (spec/cli/host.spec.md, "setup"):
//!
//! 1. Copies the binary, `--binary` or this executable, to `~/.local/bin/bd-host` on the host when the host has another
//!    build there (the SHA-256 differs).
//! 2. Runs `bd-host setup --agent` there: the agent identity, this client in the allow list, and the `LaunchAgent`.
//! 3. Reads the host name of the alias from `ssh -G`, and resolves it.
//! 4. Writes the host file with the fingerprint of the agent, then checks the hello over the link.
//!
//! ssh runs in batch mode with a connect timeout, and each call has a deadline, so a host that does not answer fails in
//! bounded time. After `setup`, no run needs ssh

use crate::config::{HostFile, check_alias};
use crate::host::{AgentInfo, resolve};
use crate::process::{Call, Ran};
use crate::{Client, shell_quote};
use host_link::digest::Sha256;
use host_link::identity::Fingerprint;
use host_link::refusal::{Code, Refusal};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::size::format_size;

/// The ssh options of every call: no prompt, a bounded connect, a dead link noticed in 20 s, and a connection of its
/// own, because a multiplexed client ignores its own keepalive options and waits for its master
pub const SSH_OPTIONS: [&str; 14] = [
  "-o",
  "BatchMode=yes",
  "-o",
  "ConnectTimeout=10",
  "-o",
  "ServerAliveInterval=10",
  "-o",
  "ServerAliveCountMax=2",
  "-o",
  "ControlMaster=no",
  "-o",
  "ControlPath=none",
  "-o",
  "LogLevel=ERROR",
];

/// The deadline of a short ssh call, the key agent included
const SSH_DEADLINE: Duration = Duration::from_secs(30);

/// The deadline of the copy of the binary, some MiB
const COPY_DEADLINE: Duration = Duration::from_secs(120);

/// The binary on the host, relative to the home directory, where ssh starts
pub const AGENT_PATH: &str = ".local/bin/bd-host";

/// How often `setup` asks for the hello of an agent that starts
const HELLO_RETRY: Duration = Duration::from_millis(500);

/// What `setup <alias>` does
#[derive(Clone, Copy, Debug)]
pub struct SetupRequest<'a> {
  /// The ssh alias of the host, and the name of its host file
  pub alias: &'a str,
  /// The local binary for the host: `--binary`, or this executable
  pub binary: &'a Path,
  pub port: u16,
  /// The main repository on the host, for the host file
  pub repo: &'a str,
  /// The name of this client in the allow list of the agent, for a person
  pub name: &'a str,
}

/// What `setup <alias>` did
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
  /// Whether the binary went to the host
  pub copied: bool,
  /// What `bd-host setup --agent` did with launchd: `bootstrapped`, `restarted`, or `none` on Linux
  pub launchd: String,
  pub agent: AgentInfo,
  pub host_file: PathBuf,
}

/// The line that `bd-host setup --agent` prints
#[derive(Deserialize)]
struct AgentSetup {
  fingerprint: Fingerprint,
  port: u16,
  launchd: String,
}

impl Client<'_> {
  pub fn setup(&self, request: &SetupRequest<'_>) -> Result<Prepared, Refusal> {
    let alias = request.alias;
    check_alias(alias)?;
    let identity = self.config.identity()?;
    let binary = std::fs::read(request.binary).map_err(|error| Refusal::new(Code::AgentFailed, format!("cannot read the binary {}: {error}", request.binary.display())))?;

    let probe = self.ssh(
      alias,
      &format!("shasum -a 256 {AGENT_PATH} 2>/dev/null || sha256sum {AGENT_PATH} 2>/dev/null || true"),
      Vec::new(),
      SSH_DEADLINE,
    );
    if !probe.ok() {
      return Err(ssh_refusal(alias, "cannot read the SHA-256 of the binary", &probe));
    }
    let probed = probe.stdout_text();
    let host_sum = probed.split_whitespace().next().unwrap_or_default();
    let copied = host_sum != sha256_hex(&binary);
    if copied {
      (self.say)(&format!("bd-host: copying bd-host ({}) to {alias}:~/{AGENT_PATH}", format_size(binary.len() as u64)));
      let staged = format!("{AGENT_PATH}.new");
      let copy = self.ssh(
        alias,
        &format!("mkdir -p .local/bin && cat > {staged} && chmod 755 {staged} && mv -f {staged} {AGENT_PATH}"),
        binary,
        COPY_DEADLINE,
      );
      if !copy.ok() {
        return Err(ssh_refusal(alias, "cannot copy the binary", &copy));
      }
    }

    let setup = self.ssh(
      alias,
      &format!(
        "{AGENT_PATH} setup --agent --client {} --name {} --port {}",
        identity.fingerprint(),
        shell_quote(request.name),
        request.port
      ),
      Vec::new(),
      SSH_DEADLINE,
    );
    if !setup.ok() {
      return Err(Refusal::new(
        Code::AgentFailed,
        format!(
          "`bd-host setup --agent` failed on {alias}: {}. A binary of another platform does not run there",
          setup.said()
        ),
      ));
    }
    let setup_text = setup.stdout_text();
    let answered: AgentSetup = setup_text
      .lines()
      .rev()
      .find(|line| line.trim_start().starts_with('{'))
      .and_then(|line| serde_json::from_str(line).ok())
      .ok_or_else(|| {
        Refusal::new(
          Code::AgentFailed,
          format!("`bd-host setup --agent` on {alias} printed no setup line: {}", setup_text.trim()),
        )
      })?;

    let hostname = self.ssh_hostname(alias);
    let addresses: Vec<SocketAddr> = resolve(&hostname, answered.port, self.timing.resolve).unwrap_or_default();
    let file = HostFile {
      alias: alias.to_owned(),
      fingerprint: answered.fingerprint,
      port: answered.port,
      hostname,
      addresses,
      repo: request.repo.to_owned(),
    };
    self.config.write_host(&file)?;

    let agent = self.wait_for_hello(alias, &answered.launchd)?;
    Ok(Prepared {
      copied,
      launchd: answered.launchd,
      agent,
      host_file: self.config.host_path(alias),
    })
  }

  /// The hello of the agent that `bd-host setup --agent` started, which needs a moment to listen
  fn wait_for_hello(&self, alias: &str, launchd: &str) -> Result<AgentInfo, Refusal> {
    let started = Instant::now();
    loop {
      let mut host = self.host(alias)?;
      match host.connect() {
        Ok((channel, info)) => {
          channel.close();
          return Ok(info);
        }
        Err(refusal) if refusal.code == Code::Unreachable && started.elapsed() < self.timing.agent_start => thread::sleep(HELLO_RETRY),
        Err(refusal) if launchd == "none" => {
          return Err(Refusal::new(
            refusal.code,
            format!(
              "{}. `bd-host setup --agent` starts no agent on this platform: run `~/{AGENT_PATH} serve` on {alias}",
              refusal.message
            ),
          ));
        }
        Err(refusal) => return Err(refusal),
      }
    }
  }

  /// The host name of an alias from `ssh -G`, which reads only the local configuration. Without it, the alias
  fn ssh_hostname(&self, alias: &str) -> String {
    let config = self.child(&Call::new("ssh", &["-G", alias], SSH_DEADLINE));
    config
      .stdout_text()
      .lines()
      .find_map(|line| line.strip_prefix("hostname ").map(|name| name.trim().to_owned()))
      .filter(|name| !name.is_empty() && config.ok())
      .unwrap_or_else(|| alias.to_owned())
  }

  fn ssh(&self, alias: &str, command: &str, input: Vec<u8>, deadline: Duration) -> Ran {
    let args: Vec<&str> = SSH_OPTIONS.iter().copied().chain([alias, command]).collect();
    self.child(&Call::new("ssh", &args, deadline).with_input(input))
  }

  /// Runs a local child, which Ctrl+C stops
  fn child(&self, call: &Call) -> Ran {
    self.processes.run(call, &|| self.interrupted())
  }
}

fn ssh_refusal(alias: &str, what: &str, ran: &Ran) -> Refusal {
  Refusal::new(
    Code::SshFailed,
    format!("{what} on {alias} over ssh: {}. Check that `ssh {alias} true` answers in batch mode", ran.said()),
  )
}

/// The SHA-256 of `bytes` as 64 lower-case hex digits
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
  let mut digest = Sha256::default();
  digest.update(bytes);
  digest.hex()
}

#[cfg(test)]
#[path = "setup_test.rs"]
mod setup_test;
