//! The files of the client in its directory, `~/.config/brd-host/` by default (openspec/specs/cli/host/spec.md, "The files"): the
//! identity of the client, one host file per alias in `hosts/<alias>.json`, one set of hosts per name in
//! `sets/<name>.json` (`brd-host set`), and the last base of each affinity in `bases/<alias>/<affinity>`. Each directory has the mode 0700 and each file 0600, written whole through a rename
//! (`sync_utils::private_file`), so a reader never sees a part

use crate::setup_hint;
use host_link::identity::{Fingerprint, Identity, create_private_dir};
use host_link::proto::is_run_id;
use host_link::refusal::{Code, Refusal};
use serde::{Deserialize, Serialize};
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// What the client knows of one host: the pin of its agent and where it answered last
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostFile {
  /// The ssh alias that `setup` used, and the name of this file
  pub alias: String,
  /// The fingerprint of the agent. A connection to an agent that shows another certificate fails
  pub fingerprint: Fingerprint,
  pub port: u16,
  /// The host name of the alias from `ssh -G`, which the client resolves when no cached address answers
  pub hostname: String,
  /// The addresses that answered, the last one that answered first
  pub addresses: Vec<SocketAddr>,
  /// The main repository on the host. A leading `~/` is the home directory of the agent
  pub repo: String,
}

/// A set of hosts, `sets/<name>.json`: a run to its name goes to the member that the estimates of the members choose
/// (`crate::choose`). The file of a set, not the host file, says how a member takes runs, as `write_host` writes the
/// host file whole and a client of an older build would drop a field there
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSet {
  /// The name of the set and of its file
  pub name: String,
  /// The members in the order of the fallback: a tie of the rank goes to the earlier one, and the first one may take
  /// any run
  pub members: Vec<SetMember>,
}

/// A member of a set of hosts
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMember {
  /// The alias of its host file
  pub alias: String,
  /// It takes a run only at once, or refuses it with `busy` and the run goes to the next member: a machine that a
  /// person works on, which keeps no queue (`RunSpec::admit_now`)
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub idle_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
  dir: PathBuf,
}

impl Config {
  pub const fn new(dir: PathBuf) -> Self {
    Self { dir }
  }

  /// `.config/brd-host` in the home directory `home`
  pub fn in_home(home: &Path) -> Self {
    Self::new(home.join(".config").join("brd-host"))
  }

  /// The configuration of the user of this process
  pub fn of_user() -> Result<Self, Refusal> {
    std::env::home_dir()
      .filter(|home| !home.as_os_str().is_empty())
      .map(|home| Self::in_home(&home))
      .ok_or_else(|| Refusal::new(Code::Config, "this process has no home directory for ~/.config/brd-host"))
  }

  pub fn dir(&self) -> &Path {
    &self.dir
  }

  /// The identity of this client, made at the first call
  pub fn identity(&self) -> Result<Identity, Refusal> {
    Identity::load_or_create(&self.dir).map_err(|error| Refusal::new(Code::Config, format!("cannot read or make the client identity in {}: {error}", self.dir.display())))
  }

  /// The aliases of the host files (`hosts/<alias>.json`), sorted: every host that `setup` made ready
  pub fn aliases(&self) -> Vec<String> {
    let mut aliases: Vec<String> = std::fs::read_dir(self.dir.join("hosts"))
      .into_iter()
      .flatten()
      .flatten()
      .filter_map(|entry| entry.file_name().to_str()?.strip_suffix(".json").map(str::to_owned))
      .filter(|alias| check_alias(alias).is_ok())
      .collect();
    aliases.sort();
    aliases
  }

  pub fn host_path(&self, alias: &str) -> PathBuf {
    self.dir.join("hosts").join(format!("{alias}.json"))
  }

  pub fn read_host(&self, alias: &str) -> Result<HostFile, Refusal> {
    check_alias(alias)?;
    let path = self.host_path(alias);
    let text = match std::fs::read_to_string(&path) {
      Ok(text) => text,
      Err(error) if error.kind() == io::ErrorKind::NotFound => {
        return Err(Refusal::new(
          Code::NoHost,
          format!(
            "no host file {} and no set of hosts {} for {alias}; {}, and `brd-host set {alias} <alias>…` makes a set of hosts of that name",
            path.display(),
            self.set_path(alias).display(),
            setup_hint(alias)
          ),
        ));
      }
      Err(error) => return Err(Refusal::new(Code::BadHostFile, format!("cannot read {}: {error}", path.display()))),
    };
    let host: HostFile = serde_json::from_str(&text).map_err(|error| Refusal::new(Code::BadHostFile, format!("{} is not a host file: {error}", path.display())))?;
    if host.alias != alias {
      return Err(Refusal::new(
        Code::BadHostFile,
        format!("{} names the alias {:?}, not {alias:?}", path.display(), host.alias),
      ));
    }
    Ok(host)
  }

  /// Writes the host file whole, through a rename
  pub fn write_host(&self, host: &HostFile) -> Result<(), Refusal> {
    check_alias(&host.alias)?;
    let path = self.host_path(&host.alias);
    let mut text = serde_json::to_string_pretty(host).map_err(|error| Refusal::new(Code::Config, format!("cannot write {}: {error}", path.display())))?;
    text.push('\n');
    write_private(&path, &text)
  }

  pub fn set_path(&self, name: &str) -> PathBuf {
    self.dir.join("sets").join(format!("{name}.json"))
  }

  /// The set of hosts `name`, or `None` when it has no file
  pub fn read_set(&self, name: &str) -> Result<Option<HostSet>, Refusal> {
    check_alias(name)?;
    let path = self.set_path(name);
    let text = match std::fs::read_to_string(&path) {
      Ok(text) => text,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(Refusal::new(Code::BadHostFile, format!("cannot read {}: {error}", path.display()))),
    };
    let set: HostSet = serde_json::from_str(&text).map_err(|error| Refusal::new(Code::BadHostFile, format!("{} is not a set of hosts: {error}", path.display())))?;
    if set.name != name || set.members.is_empty() {
      return Err(Refusal::new(
        Code::BadHostFile,
        format!("{} names the set {:?} or no member, not the members of {name:?}", path.display(), set.name),
      ));
    }
    Ok(Some(set))
  }

  /// Writes the set of hosts whole, through a rename: its name is no alias of a host file, and each member is one, once
  pub fn write_set(&self, set: &HostSet) -> Result<(), Refusal> {
    check_alias(&set.name)?;
    if self.host_path(&set.name).exists() {
      return Err(Refusal::new(
        Code::Usage,
        format!(
          "{} is the alias of a host file ({}), so it cannot name a set",
          set.name,
          self.host_path(&set.name).display()
        ),
      ));
    }
    if set.members.is_empty() {
      return Err(Refusal::new(Code::Usage, format!("the set {} needs a member", set.name)));
    }
    for (index, member) in set.members.iter().enumerate() {
      self.read_host(&member.alias)?;
      if set.members[..index].iter().any(|earlier| earlier.alias == member.alias) {
        return Err(Refusal::new(Code::Usage, format!("the set {} names {} twice", set.name, member.alias)));
      }
    }
    let path = self.set_path(&set.name);
    let mut text = serde_json::to_string_pretty(set).map_err(|error| Refusal::new(Code::Config, format!("cannot write {}: {error}", path.display())))?;
    text.push('\n');
    write_private(&path, &text)
  }

  /// The set of hosts that `name` of a command line names: none when `name` has a host file, which comes first, or no
  /// set; a name of neither then refuses as an alias does, by its host file
  pub fn set_of(&self, name: &str) -> Result<Option<HostSet>, Refusal> {
    if check_alias(name).is_err() || self.host_path(name).exists() {
      return Ok(None);
    }
    self.read_set(name)
  }

  /// The file of the base that the last base sync of `affinity` sent to the host of `alias`
  fn base_path(&self, alias: &str, affinity: &str) -> PathBuf {
    self.dir.join("bases").join(alias).join(affinity)
  }

  /// The commit that `write_base` recorded, or `None` when the file is missing or holds no commit
  pub fn base(&self, alias: &str, affinity: &str) -> Option<String> {
    if !is_run_id(alias) || !is_run_id(affinity) {
      return None;
    }
    let text = std::fs::read_to_string(self.base_path(alias, affinity)).ok()?;
    let commit = text.trim();
    let hex = commit.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    (hex && matches!(commit.len(), 40 | 64)).then(|| commit.to_owned())
  }

  /// Records the commit of the base of `affinity` on the host of `alias`, so the next run asks the host about it
  pub fn write_base(&self, alias: &str, affinity: &str, commit: &str) -> Result<(), Refusal> {
    check_alias(alias)?;
    if !is_run_id(affinity) {
      return Err(Refusal::new(Code::Usage, format!("the affinity {affinity:?} cannot name a file")));
    }
    write_private(&self.base_path(alias, affinity), &format!("{commit}\n"))
  }
}

/// Writes a file of the client: its directories 0700, the file 0600 through a rename. A failure is `config`
pub(crate) fn write_private(path: &Path, text: &str) -> Result<(), Refusal> {
  let cannot = |error: io::Error| Refusal::new(Code::Config, format!("cannot write {}: {error}", path.display()));
  let dir = path.parent().ok_or_else(|| cannot(io::Error::other("the path has no parent directory")))?;
  create_private_dir(dir).map_err(cannot)?;
  sync_utils::private_file::write_private(path, text.as_bytes()).map_err(cannot)
}

/// Refuses an alias that cannot name a host file: `[0-9A-Za-z._-]+`, at most 128 bytes, a letter or a digit first
pub fn check_alias(alias: &str) -> Result<(), Refusal> {
  if is_run_id(alias) {
    Ok(())
  } else {
    Err(Refusal::new(
      Code::Usage,
      format!("the alias {alias:?} cannot name a host file: use letters, digits, `.`, `_` and `-`"),
    ))
  }
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;
