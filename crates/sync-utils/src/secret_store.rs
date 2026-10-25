//! A secret of the app (the tokens of its sign-ins). On macOS, once the app or `bd` turned it on (`enable_keychain`), a
//! generic password of the login keychain, written and read through `/usr/bin/security` as the `gh` CLI does
//! (go-keyring): the item trusts `security`, which Apple signs and an update of Branch Deck never changes, so macOS asks
//! nothing after an update of an unsigned build, and `bd` reads it too. The keychain keeps it encrypted at rest; a
//! process of the same user can still read it through `security`, as it could read the file. Elsewhere, and while the
//! keychain can't be used (locked, no session), the 0600 file (`private_file`). A file the keychain does not have yet
//! is copied into it on the first read, and removed by the next write or sign-out.

use crate::child_process::{run_child, windowless_command};
use crate::private_file::{read_private, remove_private, write_private};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{info, warn};

/// A locked keychain may show a dialog: the secret falls back to the file meanwhile
const SECURITY_TIMEOUT: Duration = Duration::from_secs(10);
/// `security` exits with 44 (`errSecItemNotFound`) for an item it does not have
const ITEM_NOT_FOUND: i32 = 44;

static KEYCHAIN: AtomicBool = AtomicBool::new(false);

/// Keeps the secrets of this process in the login keychain on macOS: the app and `bd` call it at their start; the
/// tests never do, so they never touch the keychain of the developer
pub fn enable_keychain() {
  KEYCHAIN.store(cfg!(target_os = "macos"), Ordering::Relaxed);
}

/// A generic password: `service` is the app data directory's name (the bundle identifier), `account` the file's stem
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keychain {
  pub security: PathBuf,
  pub service: String,
  pub account: String,
}

/// The secret of `file`: in `keychain`, else in the file
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretStore {
  pub file: PathBuf,
  pub keychain: Option<Keychain>,
}

impl SecretStore {
  /// The keychain item of `file` when the keychain is on, and the names allow one
  pub fn of(file: &Path) -> Self {
    let keychain = KEYCHAIN.load(Ordering::Relaxed).then(|| keychain_of(file)).flatten();
    Self {
      file: file.to_path_buf(),
      keychain,
    }
  }

  /// The secret; None without one
  pub fn read(&self, what: &str) -> Option<String> {
    let Some(keychain) = &self.keychain else {
      return read_private(&self.file, what);
    };
    match keychain.find() {
      Ok(Some(secret)) => Some(secret),
      Ok(None) => {
        // the file of an earlier version is copied into the keychain; it stays for that version (an installed release
        // and a newer build share the app data directory) until this version writes or removes the secret
        let secret = read_private(&self.file, what)?;
        match keychain.add(&secret) {
          Ok(()) => info!(path = %self.file.display(), "copied {what} into the keychain"),
          Err(e) => warn!(error = %e, "cannot copy {what} into the keychain"),
        }
        Some(secret)
      }
      Err(e) => {
        warn!(error = %e, "cannot read {what} from the keychain: the file is used");
        read_private(&self.file, what)
      }
    }
  }

  /// Keeps the secret: in the keychain (and no file next to it), else in the file
  pub fn write(&self, secret: &str) -> std::io::Result<()> {
    if let Some(keychain) = &self.keychain {
      match keychain.add(secret) {
        Ok(()) => return remove_private(&self.file),
        Err(e) => warn!(error = %e, "cannot keep the secret in the keychain: a 0600 file keeps it"),
      }
    }
    write_private(&self.file, secret.as_bytes())
  }

  /// Removes the secret from the keychain and the file; none is fine
  pub fn remove(&self) -> std::io::Result<()> {
    if let Some(keychain) = &self.keychain {
      keychain.delete().map_err(std::io::Error::other)?;
    }
    remove_private(&self.file)
  }
}

impl Keychain {
  /// The secret of the item; None when it has none
  pub fn find(&self) -> Result<Option<String>, String> {
    let output = self.run(&["find-generic-password", "-s", &self.service, "-a", &self.account, "-w"], None)?;
    match output.status.code() {
      Some(0) => decode(String::from_utf8_lossy(&output.stdout).trim()).map(Some),
      Some(ITEM_NOT_FOUND) => Ok(None),
      _ => Err(output.failure_message("security")),
    }
  }

  /// Adds or replaces the item. The secret goes through stdin (`security -i`), never into the arguments a process
  /// listing shows; hex, so neither quoting nor the hex that `security` prints for a secret with other bytes applies.
  pub fn add(&self, secret: &str) -> Result<(), String> {
    let command = format!("add-generic-password -U -s {} -a {} -w {}\n", self.service, self.account, encode(secret));
    let output = self.run(&["-i"], Some(command.into_bytes()))?;
    if output.status.success() { Ok(()) } else { Err(output.failure_message("security")) }
  }

  pub fn delete(&self) -> Result<(), String> {
    let output = self.run(&["delete-generic-password", "-s", &self.service, "-a", &self.account], None)?;
    match output.status.code() {
      Some(0 | ITEM_NOT_FOUND) => Ok(()),
      _ => Err(output.failure_message("security")),
    }
  }

  fn run(&self, args: &[&str], stdin: Option<Vec<u8>>) -> Result<crate::child_process::ChildOutput, String> {
    run_child(windowless_command(&self.security).args(args), stdin, SECURITY_TIMEOUT, &|| false).map_err(|e| format!("security: {e:?}"))
  }
}

/// The item of `file`: service and account of letters, digits, `.`, `-` and `_` only, as `security -i` takes them
/// without quoting
fn keychain_of(file: &Path) -> Option<Keychain> {
  let service = file.parent()?.file_name()?.to_str()?;
  let account = file.file_stem()?.to_str()?;
  let plain = |name: &str| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
  (plain(service) && plain(account)).then(|| Keychain {
    security: PathBuf::from("/usr/bin/security"),
    service: service.to_string(),
    account: account.to_string(),
  })
}

fn encode(secret: &str) -> String {
  const DIGITS: &[u8; 16] = b"0123456789abcdef";
  let mut hex = String::with_capacity(secret.len() * 2);
  for byte in secret.bytes() {
    hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
    hex.push(char::from(DIGITS[usize::from(byte & 0xf)]));
  }
  hex
}

fn decode(hex: &str) -> Result<String, String> {
  let invalid = || "the keychain item is not a secret of Branch Deck".to_string();
  if !hex.len().is_multiple_of(2) {
    return Err(invalid());
  }
  let bytes = (0..hex.len())
    .step_by(2)
    .map(|index| hex.get(index..index + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
    .collect::<Option<Vec<u8>>>()
    .ok_or_else(invalid)?;
  String::from_utf8(bytes).map_err(|_utf8| invalid())
}

/// The JSON of the secret of `file` (`SecretStore::of`); None also for one that does not parse, with a warning
pub fn read_secret_json<T: DeserializeOwned>(file: &Path, what: &str) -> Option<T> {
  parse_json(&SecretStore::of(file).read(what)?, file, what)
}

pub fn parse_json<T: DeserializeOwned>(text: &str, file: &Path, what: &str) -> Option<T> {
  match serde_json::from_str(text) {
    Ok(value) => Some(value),
    Err(e) => {
      warn!(path = %file.display(), error = %e, "{what} is not valid");
      None
    }
  }
}

pub fn write_secret_json<T: Serialize>(file: &Path, value: &T) -> std::io::Result<()> {
  SecretStore::of(file).write(&serde_json::to_string_pretty(value).map_err(std::io::Error::other)?)
}

pub fn remove_secret(file: &Path) -> std::io::Result<()> {
  SecretStore::of(file).remove()
}
