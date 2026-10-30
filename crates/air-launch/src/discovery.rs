//! The instance files of the running IDEs with Air: one `<pid>.json` per IDE process in the directory of the user
//! (`instance_dir`), which names its socket and the base directories of its open projects. A file of a process that
//! no longer runs, one that does not parse and one of another schema are skipped. The IDE of a directory is the one
//! whose project holds it, the deepest project first, then the newest file (`pick`).

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use sync_utils::process_scan::is_running;
use tracing::debug;

/// The schema of the instance files this client reads; a file of another schema is skipped
pub const SCHEMA_VERSION: u32 = 1;

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
  /// The base directories of the open projects, absolute and normalized
  pub projects: Vec<PathBuf>,
  /// When the IDE wrote the file last
  pub modified: SystemTime,
}

/// The IDE of a directory and its project that holds it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match<'a> {
  pub instance: &'a Instance,
  /// The base directory of the project: the directory or one of its parents
  pub project: &'a Path,
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
  projects: Vec<PathBuf>,
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
    projects: parsed.projects,
    modified,
  })
}

/// The IDE of `directory`: the instance with a project that is `directory` or one of its parents, the deepest project
/// first, then the newest file. The paths compare by their components, without a read of the file system
pub fn pick<'a>(instances: &'a [Instance], directory: &Path) -> Option<Match<'a>> {
  instances
    .iter()
    .flat_map(|instance| {
      instance
        .projects
        .iter()
        .filter(|project| project.is_absolute() && directory.starts_with(project))
        .map(move |project| Match { instance, project })
    })
    .max_by(|a, b| {
      let depth = |it: &Match<'_>| it.project.components().count();
      depth(a).cmp(&depth(b)).then_with(|| a.instance.modified.cmp(&b.instance.modified))
    })
}

/// The IDE of `directory` among the running processes of the instance files in `dir`
pub fn find_instance(dir: &Path, directory: &Path) -> Option<(Instance, PathBuf)> {
  let instances = read_instances(dir, |pid| is_running(pid, None));
  pick(&instances, directory).map(|found| (found.instance.clone(), found.project.to_path_buf()))
}
