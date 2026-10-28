//! The environment and the limits of the prepare steps and the command of a run (spec/cli/host.spec.md, "The
//! environment of a run and `[host]`").
//!
//! The environment is the one of the agent, then the login environment of the user on top, then the directory of the
//! git of the host before `/usr/bin` in `PATH`, then the variables of the run (`BD_HOST_RUN`, `BD_HOST_AFFINITY`,
//! `BD_HOST_COMMIT`, `BD_HOST_SLOT`), then `[host.env]`. The open-files limit goes up to [`OPEN_FILES`], because
//! launchd starts a job with 256: too few for a parallel test run

use crate::home::Home;
use host_link::config::EnvValue;
use host_link::proto::RunSpec;
use host_link::refusal::{Code, Refusal};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

/// The open-files limit of a run, or the hard limit when it is lower
pub(crate) const OPEN_FILES: u64 = 65_536;

/// How many ports after the first one a port variable tries
const PORT_TRIES: u16 = 100;

/// How long a probe of a port waits for a connection
const PORT_PROBE: Duration = Duration::from_millis(200);

/// The variables of a run on top of `env`: the variables of the run, then `[host.env]` in the order of the names
pub(crate) fn run_env(
  mut env: BTreeMap<OsString, OsString>,
  config: &[(String, EnvValue)],
  home: &Home,
  spec: &RunSpec,
  slot: &Path,
) -> Result<BTreeMap<OsString, OsString>, Refusal> {
  let number = slot_number(slot);
  for (name, value) in [
    ("BD_HOST_RUN", spec.run.as_str()),
    ("BD_HOST_AFFINITY", spec.affinity.as_str()),
    ("BD_HOST_COMMIT", spec.commit.as_str()),
    ("BD_HOST_SLOT", &number.to_string()),
  ] {
    env.insert(OsString::from(name), OsString::from(value));
  }
  for (name, value) in config {
    let key = OsString::from(name);
    match value {
      EnvValue::Text(text) => {
        env.insert(key, OsString::from(text));
      }
      EnvValue::List { prepend, append } => {
        let expand = |entry: &String| list_entry(entry, home, slot);
        let inherited = env.get(&key).filter(|value| !value.is_empty()).cloned();
        let entries: Vec<OsString> = prepend.iter().map(expand).chain(inherited).chain(append.iter().map(expand)).collect();
        env.insert(key, join(&entries));
      }
      EnvValue::Port(base) => {
        let first = base.saturating_add(number);
        let port = free_port(first).ok_or_else(|| {
          Refusal::new(
            Code::AgentError,
            format!("[host.env] {name}: no free TCP port from {first} to {}", first.saturating_add(PORT_TRIES - 1)),
          )
        })?;
        env.insert(key, OsString::from(port.to_string()));
      }
      EnvValue::Unset => {
        env.remove(&key);
      }
    }
  }
  Ok(env)
}

/// One entry of a list variable: `~/` is the home directory, an absolute path stays, and a relative path is below the
/// slot root, such as `node_modules/.bin`
fn list_entry(entry: &str, home: &Home, slot: &Path) -> OsString {
  if entry == "~" || entry.starts_with("~/") || Path::new(entry).is_absolute() {
    home.expand(entry).into_os_string()
  } else {
    slot.join(entry).into_os_string()
  }
}

fn join(entries: &[OsString]) -> OsString {
  let mut joined = OsString::new();
  for (index, entry) in entries.iter().enumerate() {
    if index > 0 {
      joined.push(":");
    }
    joined.push(entry);
  }
  joined
}

/// The number of a slot: the name of its parent directory when that is a number, as in the pool of `bd lease`
/// (`<pool>/<n>/<repo>`), else a number below 100 from the path
pub(crate) fn slot_number(slot: &Path) -> u16 {
  if let Some(number) = slot
    .parent()
    .and_then(Path::file_name)
    .and_then(|name| name.to_str())
    .and_then(|name| name.parse::<u16>().ok())
    .filter(|number| *number < 1000)
  {
    return number;
  }
  let hash = slot
    .to_string_lossy()
    .bytes()
    .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
  u16::try_from(hash % 100).unwrap_or(0)
}

/// The first free TCP port from `first`, of [`PORT_TRIES`] ports
pub(crate) fn free_port(first: u16) -> Option<u16> {
  (0..PORT_TRIES).filter_map(|offset| first.checked_add(offset)).find(|port| *port != 0 && is_free(*port))
}

/// Whether nothing listens on the TCP port of the loopback addresses and this process can bind it
fn is_free(port: u16) -> bool {
  let answers = |address: SocketAddr| TcpStream::connect_timeout(&address, PORT_PROBE).is_ok();
  TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok() && !answers(SocketAddr::from((Ipv4Addr::LOCALHOST, port))) && !answers(SocketAddr::from((Ipv6Addr::LOCALHOST, port)))
}

/// Raises the soft open-files limit of this process to [`OPEN_FILES`], or to the hard limit when it is lower. The
/// children of the process inherit it. Answers the soft limit after the call
pub(crate) fn raise_open_files() -> io::Result<u64> {
  let (soft, hard) = crate::sys::open_files()?;
  let target = OPEN_FILES.min(hard).min(crate::sys::max_files_per_process());
  if soft >= target {
    return Ok(soft);
  }
  crate::sys::set_open_files(target, hard)?;
  Ok(target)
}

/// The load average of the last minute, or `None` when the system does not tell
pub(crate) fn load() -> Option<f64> {
  crate::sys::load()
}

#[cfg(test)]
#[path = "run_env_test.rs"]
mod run_env_test;
