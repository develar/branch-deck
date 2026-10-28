//! The git of a host that serves remote runs (`bd lease host git`, spec/cli/lease.spec.md, "Host objects"): its floor,
//! above `MIN_VERSION` of bd itself, and the Homebrew install or upgrade that `--ensure` runs below it. The runs of other
//! machines and the agent of the host use the git that their `PATH` finds first, with the `bin` directory of Homebrew
//! before `/usr/bin`, so a host keeps one floor that moves only on purpose instead of a check of each feature they use.
//! The floor is the newest release of git when the user set the policy (2026-10-27): "a minimum in the code, and `brew
//! upgrade git`". Homebrew has the newest release, so `brew upgrade git` reaches it. No git user may run while Homebrew
//! replaces git: the lend table of the host slots is the lock (`host_slots::ensure_host_git`). Ported from
//! `host_git.rs` of yonderd (IJAI-4792)

use crate::host_brew::run_with_deadline;
use anyhow::{Context, Result, anyhow, bail};
use git_executor::git_info::GitInfo;
use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The lowest git of a host
pub const HOST_MIN_VERSION: Version = Version::new(2, 56, 0);
/// The variable that names a fake Homebrew for a test, or `none` for a host without one
pub const TEST_BREW: &str = "BD_TEST_HOST_BREW";
/// The variable that sets another floor for a test, such as `0.0.0` or `99.0.0`
pub const TEST_MIN_VERSION: &str = "BD_TEST_HOST_MIN_VERSION";
/// The places of Homebrew on macOS beside the `PATH`: Apple silicon, then Intel
const BREW_PLACES: [&str; 2] = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];
/// The deadline of `brew install git` and `brew upgrade git`: an update of Homebrew, a download and an install
pub const BREW_DEADLINE: Duration = Duration::from_mins(30);
/// How often an upgrade tries again while another Homebrew process holds the lock of git, such as the Homebrew of a
/// call that was killed
const BREW_LOCK_POLL: Duration = Duration::from_secs(10);
/// The most bytes of the output of Homebrew in a refusal: its end, where Homebrew says what failed
const BREW_OUTPUT_TAIL: usize = 2000;

/// A version of git: `2.56.0`, or `2.56.0-rc1` for a release candidate, which comes before its release
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
  major: u32,
  minor: u32,
  patch: u32,
  rc: Option<u32>,
}

impl Version {
  pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
    Self { major, minor, patch, rc: None }
  }

  /// The version of a text such as `2.56.0`, `2.56.0.rc1`, `2.56.0-rc1`, `2.45.1.windows.1` or `2.50.1 (Apple
  /// Git-155)`. A part after the third number counts only when it names a release candidate
  pub fn parse(text: &str) -> Option<Self> {
    let text = text.split_whitespace().next()?;
    let parts: Vec<&str> = text.split(['.', '-']).collect();
    let number = |index: usize| parts.get(index).and_then(|part| part.parse::<u32>().ok());
    let rc = parts.iter().skip(2).find_map(|part| part.strip_prefix("rc").and_then(|rest| rest.parse().ok()));
    Some(Self {
      major: number(0)?,
      minor: number(1)?,
      patch: number(2).unwrap_or(0),
      rc,
    })
  }

  /// A release ranks after each of its release candidates
  const fn key(self) -> (u32, u32, u32, u32) {
    (
      self.major,
      self.minor,
      self.patch,
      match self.rc {
        Some(rc) => rc,
        None => u32::MAX,
      },
    )
  }
}

impl Ord for Version {
  fn cmp(&self, other: &Self) -> Ordering {
    self.key().cmp(&other.key())
  }
}

impl PartialOrd for Version {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl fmt::Display for Version {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
    if let Some(rc) = self.rc {
      write!(f, "-rc{rc}")?;
    }
    Ok(())
  }
}

/// The floor of this process: the one of `TEST_MIN_VERSION`, else `HOST_MIN_VERSION`
pub fn min_version() -> Version {
  std::env::var(TEST_MIN_VERSION).ok().and_then(|text| Version::parse(&text)).unwrap_or(HOST_MIN_VERSION)
}

/// The git that a `PATH` finds first
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundGit {
  pub path: PathBuf,
  /// `None` when its answer names no version
  pub version: Option<Version>,
  /// Its version as it answered it, such as `2.50.1 (Apple Git-155)`, or why it gave none
  pub text: String,
}

impl FoundGit {
  pub fn is_new_enough(&self, floor: Version) -> bool {
    self.version.is_some_and(|version| version >= floor)
  }
}

impl fmt::Display for FoundGit {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "git {} at {}", self.text, self.path.display())
  }
}

/// The git that `path` finds first, with its version (`GitInfo::from_path`: one `git version`); `None` when no
/// directory of `path` has one
pub fn find_git(path: &OsStr) -> Option<FoundGit> {
  let git = which("git", path)?;
  let (version, text) = match GitInfo::from_path(&git.to_string_lossy()) {
    Ok(info) => (Version::parse(&info.version), info.version),
    Err(error) => (None, format!("that fails `version`: {error}")),
  };
  Some(FoundGit { path: git, version, text })
}

/// The first executable file `name` in a directory of `path`; an empty entry counts as none
pub fn which(name: &str, path: &OsStr) -> Option<PathBuf> {
  std::env::split_paths(path)
    .filter(|dir| !dir.as_os_str().is_empty())
    .map(|dir| dir.join(name))
    .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
  use std::os::unix::fs::PermissionsExt;
  std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
  path.is_file()
}

/// The `brew` program: the one of `TEST_BREW`, else on macOS the first of `path`, else one of `BREW_PLACES`; none
/// elsewhere, where Homebrew is not the tool of the system
pub fn brew(path: &OsStr) -> Option<PathBuf> {
  if let Some(named) = std::env::var_os(TEST_BREW) {
    return (!named.is_empty() && named != "none").then(|| PathBuf::from(named));
  }
  if !cfg!(target_os = "macos") {
    return None;
  }
  which("brew", path).or_else(|| BREW_PLACES.iter().map(PathBuf::from).find(|place| is_executable(place)))
}

/// The directory of the programs of Homebrew, `$(brew --prefix)/bin`: Homebrew keeps `brew` itself there
pub fn brew_bin(brew: &Path) -> Option<&Path> {
  brew.parent().filter(|bin| !bin.as_os_str().is_empty())
}

/// `path` with `dir` before `/usr/bin`. A `path` that has `dir` before `/usr/bin` already stays as it is. Else each
/// `dir` goes, and one `dir` comes right before the first `/usr/bin`, or at the end when there is none
pub fn with_first(path: &OsStr, dir: &Path) -> OsString {
  if path.is_empty() {
    return dir.as_os_str().to_owned();
  }
  let system = Path::new("/usr/bin");
  let entries: Vec<PathBuf> = std::env::split_paths(path).collect();
  let first_system = entries.iter().position(|entry| entry == system);
  let first_own = entries.iter().position(|entry| entry == dir);
  if first_own.is_some_and(|own| first_system.is_none_or(|system| own < system)) {
    return path.to_owned();
  }
  let mut entries: Vec<PathBuf> = entries.into_iter().filter(|entry| entry != dir).collect();
  let at = entries.iter().position(|entry| entry == system).unwrap_or(entries.len());
  entries.insert(at, dir.to_path_buf());
  std::env::join_paths(entries).unwrap_or_else(|_| path.to_owned())
}

/// The `PATH` of the runs: `path` with the `bin` directory of Homebrew before `/usr/bin`, and the `brew` program
pub fn run_path(path: &OsStr) -> (OsString, Option<PathBuf>) {
  let brew = brew(path);
  let path = brew.as_deref().and_then(brew_bin).map_or_else(|| path.to_owned(), |bin| with_first(path, bin));
  (path, brew)
}

/// What `--ensure` does about the git of the `PATH` of the runs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitPlan {
  /// The git is new enough
  Ready,
  /// `brew install git`: Homebrew has no git yet
  Install,
  /// `brew upgrade git`: the git of Homebrew is too old
  Upgrade,
  /// bd cannot get a new git; the message says what the user does
  Refuse(String),
}

impl GitPlan {
  /// The arguments of `brew` for an install or an upgrade
  pub const fn brew_args(&self) -> Option<[&'static str; 2]> {
    match self {
      Self::Install => Some(["install", "git"]),
      Self::Upgrade => Some(["upgrade", "git"]),
      Self::Ready | Self::Refuse(_) => None,
    }
  }
}

/// The plan for the git `found` that the `PATH` of the runs finds first, with the `brew` program and whether the `bin`
/// directory of Homebrew has a git; `macos` selects the hint of a host without Homebrew
pub fn plan(found: Option<&FoundGit>, floor: Version, brew: Option<&Path>, brew_has_git: bool, macos: bool) -> GitPlan {
  if found.is_some_and(|found| found.is_new_enough(floor)) {
    return GitPlan::Ready;
  }
  let what = found.map_or_else(|| "missing: the PATH of the host finds no git".to_string(), ToString::to_string);
  let Some(brew) = brew else {
    return GitPlan::Refuse(if macos {
      format!(
        "The git of this host is {what}, older than {floor}, the floor of a host of remote runs. bd upgrades git with Homebrew, and this host has no Homebrew. Install Homebrew once at the host (it asks for the password of an administrator): /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\". Then run `brew install git`"
      )
    } else {
      format!(
        "The git of this host is {what}, older than {floor}, the floor of a host of remote runs. Install git {floor} or later, so that the PATH of the host finds it first. The git of a distribution is often older, such as git 2.43 of Ubuntu 24.04: take a newer source of packages, such as the git-core PPA on Ubuntu (`sudo add-apt-repository ppa:git-core/ppa && sudo apt install git`), or build git from its source"
      )
    });
  };
  let brew_git = brew_bin(brew).map(|bin| bin.join("git"));
  match found {
    Some(found) if brew_has_git && brew_git.as_ref() != Some(&found.path) => GitPlan::Refuse(format!(
      "The git of this host is {what}, older than {floor}, the floor of a host of remote runs, and the PATH of the host finds it before the git of Homebrew in {}. Remove it, or put a newer git first",
      brew_bin(brew).unwrap_or(brew).display()
    )),
    _ if brew_has_git => GitPlan::Upgrade,
    _ => GitPlan::Install,
  }
}

/// The plan of the git of `path`, as the runs find it: the git of the `PATH` of the runs (`run_path`), the `brew`
/// program and whether its `bin` has a git
pub fn plan_of(path: &OsStr, floor: Version) -> (GitPlan, Option<FoundGit>, OsString, Option<PathBuf>) {
  let (path, brew) = run_path(path);
  let found = find_git(&path);
  let brew_has_git = brew.as_deref().and_then(brew_bin).is_some_and(|bin| is_executable(&bin.join("git")));
  let plan = plan(found.as_ref(), floor, brew.as_deref(), brew_has_git, cfg!(target_os = "macos"));
  (plan, found, path, brew)
}

/// Whether Homebrew failed because another Homebrew process holds a lock: of the formula (`has already locked`) or of
/// its update (`already in progress`)
pub fn is_homebrew_lock(said: &str) -> bool {
  said.contains("has already locked") || said.contains("already in progress")
}

/// Runs `brew <args>` with `path` as its `PATH`, then answers the git of `path` when it is at least `floor`, else the
/// error says why. While another Homebrew process holds the lock of git (one that a killed call left), it waits
/// `BREW_LOCK_POLL`, takes a new enough git as the end of that process, and else tries again, up to `BREW_DEADLINE` in
/// all. `progress` hears each try
pub fn run_brew(brew: &Path, args: [&str; 2], path: &OsStr, floor: Version, progress: &dyn Fn(&str)) -> Result<FoundGit> {
  let command = format!("brew {}", args.join(" "));
  let started = Instant::now();
  loop {
    progress(&format!("`{command}` runs"));
    let deadline = BREW_DEADLINE.saturating_sub(started.elapsed());
    let (success, said) = run_with_deadline(brew, &args, path, deadline).with_context(|| format!("`{command}` failed"))?;
    if success {
      break;
    }
    if is_homebrew_lock(&said) && started.elapsed() + BREW_LOCK_POLL < BREW_DEADLINE {
      progress(&format!("another Homebrew process works on git; `{command}` tries again in {} s", BREW_LOCK_POLL.as_secs()));
      std::thread::sleep(BREW_LOCK_POLL);
      if let Some(found) = find_git(path).filter(|found| found.is_new_enough(floor)) {
        return Ok(found);
      }
      continue;
    }
    let tail = said.char_indices().rev().nth(BREW_OUTPUT_TAIL).map_or(said.as_str(), |(at, _)| &said[at..]);
    bail!("`{command}` failed: {tail}");
  }
  match find_git(path) {
    Some(found) if found.is_new_enough(floor) => Ok(found),
    Some(found) => Err(anyhow!(
      "`{command}` ended, and the PATH of the host still finds {found}, older than {floor}, the floor of a host of remote runs: Homebrew has no newer git, or another git comes first"
    )),
    None => Err(anyhow!("`{command}` ended, and the PATH of the host still finds no git")),
  }
}
