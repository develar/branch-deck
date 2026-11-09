//! The running IDEs with Air and the IDE of a directory. Each IDE process writes one instance file `<pid>.json` at its
//! start in the directory of the user (`instance_dir`), which names its socket. A file of a process that no longer runs,
//! one that does not parse and one of another schema are skipped. Each running IDE is asked over its socket for the
//! launch directories it hosts (`ask_hosts`, the request `hosts`), within `HOSTS_TIMEOUT`; an IDE that does not answer
//! in time, fails or refuses is skipped. The IDE of a directory is the one with a host that holds it, the deepest host
//! first, then the IDE whose window got the focus last, then the newest file (`pick`).

use crate::client::Client;
use crate::protocol::{Hosts, Refusal};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use sync_utils::process_scan::is_running;
use sync_utils::span::in_current_span;
use tracing::debug;

/// The schema of the instance files this client reads; a file of another schema is skipped
pub const SCHEMA_VERSION: u32 = 1;

/// How long the discovery waits for the answer of one IDE to `hosts`: the IDE asks no consent for it, so a longer wait
/// is an IDE that hangs
pub const HOSTS_TIMEOUT: Duration = Duration::from_secs(2);

/// A running IDE as its instance file names it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
  /// The instance file
  pub file: PathBuf,
  pub pid: u32,
  /// The product code (`IU`)
  pub product: String,
  /// The build (`IU-263.1234`)
  pub build: String,
  pub socket: PathBuf,
  /// When the IDE wrote the file
  pub modified: SystemTime,
}

/// A running IDE and its answer to `hosts`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostedInstance {
  pub instance: Instance,
  /// The launch directories of the IDE, absolute
  pub hosts: Vec<PathBuf>,
  /// Unix milliseconds of the last focus of a window of the IDE; none when the IDE does not say
  pub focused_at: Option<u64>,
}

/// The IDE of a directory and its host that holds it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match<'a> {
  pub instance: &'a HostedInstance,
  /// The launch directory of the IDE: the directory or one of its parents
  pub host: &'a Path,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceFile {
  schema_version: u32,
  pid: u32,
  #[serde(default)]
  product: String,
  #[serde(default)]
  build: String,
  socket: PathBuf,
}

/// The directory of the instance files: `~/Library/Application Support/JetBrains/air/launch` on macOS,
/// `%APPDATA%\JetBrains\air\launch` on Windows, `$XDG_DATA_HOME/JetBrains/air/launch` (else
/// `~/.local/share/JetBrains/air/launch`) on Linux; none without a home directory
pub fn instance_dir() -> Option<PathBuf> {
  let data = if cfg!(target_os = "macos") {
    std::env::home_dir().map(|home| home.join("Library/Application Support"))
  } else if cfg!(windows) {
    std::env::var_os("APPDATA").map(PathBuf::from)
  } else {
    std::env::var_os("XDG_DATA_HOME")
      .map(PathBuf::from)
      .filter(|dir| dir.is_absolute())
      .or_else(|| std::env::home_dir().map(|home| home.join(".local/share")))
  };
  data.map(|dir| dir.join("JetBrains").join("air").join("launch"))
}

/// The instances of the files in `dir` whose process `alive` says runs; none when the directory is missing
pub fn read_instances(dir: &Path, alive: impl Fn(u32) -> bool) -> Vec<Instance> {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return Vec::new();
  };
  let mut instances = Vec::new();
  for entry in entries.flatten() {
    let file = entry.path();
    if file.extension().is_none_or(|extension| extension != "json") {
      continue;
    }
    match read_instance(&file) {
      // pid 0 is the kernel or the idle process, never an IDE
      Some(instance) if instance.pid != 0 && alive(instance.pid) => instances.push(instance),
      Some(instance) => debug!(file = %file.display(), pid = instance.pid, "the process of an instance file is gone"),
      None => debug!(file = %file.display(), "skipped an instance file that does not parse"),
    }
  }
  instances
}

fn read_instance(file: &Path) -> Option<Instance> {
  let text = std::fs::read_to_string(file).ok()?;
  let parsed: InstanceFile = serde_json::from_str(&text).ok()?;
  if parsed.schema_version != SCHEMA_VERSION {
    return None;
  }
  let modified = std::fs::metadata(file).and_then(|metadata| metadata.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
  Some(Instance {
    file: file.to_path_buf(),
    pid: parsed.pid,
    product: parsed.product,
    build: parsed.build,
    socket: parsed.socket,
    modified,
  })
}

/// The answers of `instances` to `hosts`, each asked on its own thread within `timeout`; an IDE that does not answer in
/// time, fails or refuses is skipped
pub fn ask_hosts(instances: Vec<Instance>, timeout: Duration) -> Vec<HostedInstance> {
  std::thread::scope(|scope| {
    // every ask starts before the first join
    let mut asks = Vec::with_capacity(instances.len());
    for instance in instances {
      asks.push(scope.spawn(in_current_span(move || ask(instance, timeout))));
    }
    asks
      .into_iter()
      .filter_map(|ask| ask.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
      .collect()
  })
}

fn ask(instance: Instance, timeout: Duration) -> Option<HostedInstance> {
  match Client::new(&instance.socket).with_timeout(timeout).hosts() {
    Ok(Hosts::Listed { hosts, focused_at }) => Some(HostedInstance { instance, hosts, focused_at }),
    Ok(Hosts::Refused(Refusal { code, message })) => {
      debug!(pid = instance.pid, socket = %instance.socket.display(), %code, ?message, "skipped an IDE that refused to name its hosts");
      None
    }
    Err(error) => {
      debug!(pid = instance.pid, socket = %instance.socket.display(), %error, "skipped an IDE that did not name its hosts");
      None
    }
  }
}

/// The IDE of `directory`: the instance with a host that is `directory` or one of its parents, the deepest host first,
/// then the newest focus (an IDE that names none is older than any), then the newest file. The paths compare by their
/// components, without a read of the file system
pub fn pick<'a>(instances: &'a [HostedInstance], directory: &Path) -> Option<Match<'a>> {
  instances
    .iter()
    .flat_map(|instance| {
      instance
        .hosts
        .iter()
        .filter(|host| host.is_absolute() && directory.starts_with(host))
        .map(move |host| Match { instance, host })
    })
    .max_by(|a, b| {
      let depth = |it: &Match<'_>| it.host.components().count();
      depth(a)
        .cmp(&depth(b))
        .then_with(|| a.instance.focused_at.cmp(&b.instance.focused_at))
        .then_with(|| a.instance.instance.modified.cmp(&b.instance.instance.modified))
    })
}

/// The IDE of `directory` among the running processes of the instance files in `dir`, and its host that holds it
pub fn find_instance(dir: &Path, directory: &Path) -> Option<(Instance, PathBuf)> {
  let instances = ask_hosts(read_instances(dir, |pid| is_running(pid, None)), HOSTS_TIMEOUT);
  pick(&instances, directory).map(|found| (found.instance.instance.clone(), found.host.to_path_buf()))
}
