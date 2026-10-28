//! The identity of one end: a self-signed certificate and its key, and the fingerprint that the other end pins
//! (spec/cli/host.spec.md, "The link" and "The files"). The files are `identity.key` (PKCS#8 DER) and `identity.crt`
//! (DER) in the identity directory, the same files that yonder writes, so `bd-host setup --import-yonder` copies them
//! as they are

use crate::digest::Sha256;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::fs_retry;

/// The file of the private key, PKCS#8 DER, in the identity directory
pub const KEY_FILE: &str = "identity.key";

/// The file of the certificate, DER, in the identity directory
pub const CERT_FILE: &str = "identity.crt";

/// The identity directories of yonder below the home directory, which `bd-host setup --import-yonder` reads
pub mod yonder {
  /// The identity of the yonder client
  pub const CLIENT_DIR: &str = ".config/yonder";
  /// The identity of yonderd and its allow list `clients`
  pub const AGENT_DIR: &str = ".config/yonderd";
}

/// How long a call waits for the certificate of the call that wrote the key
const CREATE_WAIT: Duration = Duration::from_secs(10);

/// The SHA-256 of the DER of a certificate. It is the name of an end: a client pins the fingerprint of its host, and an
/// agent keeps the fingerprints of its clients in its allow list
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
  pub fn of(cert_der: &[u8]) -> Self {
    let mut digest = Sha256::default();
    digest.update(cert_der);
    Self(digest.bytes())
  }

  /// The first 12 hex digits, for a message
  pub fn short(&self) -> String {
    self.to_string()[..12].to_owned()
  }
}

impl fmt::Display for Fingerprint {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
  }
}

impl fmt::Debug for Fingerprint {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Fingerprint({self})")
  }
}

/// The refusal of a text that is not 64 hex digits
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("a fingerprint is 64 hex digits, not {0:?}")]
pub struct BadFingerprint(pub String);

impl FromStr for Fingerprint {
  type Err = BadFingerprint;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    let refused = || BadFingerprint(text.to_owned());
    if text.len() != 64 || !text.is_ascii() {
      return Err(refused());
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
      let Ok(parsed) = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16) else {
        return Err(refused());
      };
      *byte = parsed;
    }
    Ok(Self(bytes))
  }
}

impl Serialize for Fingerprint {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(self)
  }
}

impl<'de> Deserialize<'de> for Fingerprint {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let text = String::deserialize(deserializer)?;
    text.parse().map_err(serde::de::Error::custom)
  }
}

/// The certificate and the key of one end
#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
  cert_der: Vec<u8>,
  key_der: Vec<u8>,
}

impl fmt::Debug for Identity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Identity({})", self.fingerprint())
  }
}

/// What `Identity::import` did
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Imported {
  /// The identity was copied
  Copied,
  /// The target has the same identity already
  Same,
  /// The target has another identity, and it stays
  Kept,
  /// The source has no certificate
  Missing,
}

impl Identity {
  /// A new identity: an ECDSA P-256 key and a self-signed certificate for `tls::SERVER_NAME` (`tls::self_signed`)
  pub fn generate() -> io::Result<Self> {
    let (cert, key) = crate::tls::self_signed().map_err(io::Error::other)?;
    Ok(Self {
      cert_der: cert.to_vec(),
      key_der: key.secret_pkcs8_der().to_vec(),
    })
  }

  /// The identity in `dir`, created there when it is missing. The directory has the mode 0700 and the files 0600 on
  /// Unix
  pub fn load_or_create(dir: &Path) -> io::Result<Self> {
    if dir.join(CERT_FILE).exists() {
      return Self::load(dir);
    }
    Self::store(dir, &Self::generate()?)
  }

  /// Copies the identity in `from` (such as `~/.config/yonder`) to `to`, so the other end keeps its pin. An identity
  /// that `to` has already stays. The name in the certificate does not count: each end checks the fingerprint only
  pub fn import(from: &Path, to: &Path) -> io::Result<Imported> {
    if !from.join(CERT_FILE).exists() {
      return Ok(Imported::Missing);
    }
    let source = Self::load(from)?;
    let present = to.join(CERT_FILE).exists();
    let target = if present { Self::load(to)? } else { Self::store(to, &source)? };
    Ok(match (present, target == source) {
      (_, false) => Imported::Kept,
      (true, true) => Imported::Same,
      (false, true) => Imported::Copied,
    })
  }

  /// Writes `identity` to `dir`, and answers the identity that `dir` holds then.
  ///
  /// Two first calls can run at the same time, such as two first runs of a client. Each writes its key and its
  /// certificate to temporary files, then links the key into place. A link fails when the key exists, so one call wins.
  /// The winner then renames its certificate into place, and each other call waits for that certificate and loads the
  /// pair of the winner. So the key and the certificate always belong together
  fn store(dir: &Path, identity: &Self) -> io::Result<Self> {
    create_private_dir(dir)?;
    let key = Temporary(write_temporary(&dir.join(KEY_FILE), &identity.key_der)?);
    let cert = Temporary(write_temporary(&dir.join(CERT_FILE), &identity.cert_der)?);
    match fs::hard_link(&key.0, dir.join(KEY_FILE)) {
      Ok(()) => {
        fs_retry::rename(&cert.0, dir.join(CERT_FILE)).map_err(|error| named(&error, &dir.join(CERT_FILE)))?;
        Ok(identity.clone())
      }
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Self::load_when_written(dir),
      Err(error) => Err(named(&error, &dir.join(KEY_FILE))),
    }
  }

  /// The identity that another call of `store` writes in `dir`: its key is there, and its certificate comes within
  /// `CREATE_WAIT`
  fn load_when_written(dir: &Path) -> io::Result<Self> {
    let started = Instant::now();
    while !dir.join(CERT_FILE).exists() {
      if started.elapsed() >= CREATE_WAIT {
        return Err(io::Error::other(format!(
          "{} has a key without a certificate; remove {KEY_FILE} to make a new identity",
          dir.display()
        )));
      }
      thread::sleep(Duration::from_millis(10));
    }
    Self::load(dir)
  }

  /// The identity in `dir`
  pub fn load(dir: &Path) -> io::Result<Self> {
    let read = |name: &str| {
      let path = dir.join(name);
      fs::read(&path).map_err(|error| io::Error::new(error.kind(), format!("cannot read {}: {error}", path.display())))
    };
    Ok(Self {
      cert_der: read(CERT_FILE)?,
      key_der: read(KEY_FILE)?,
    })
  }

  pub fn fingerprint(&self) -> Fingerprint {
    Fingerprint::of(&self.cert_der)
  }

  pub(crate) fn cert_chain(&self) -> Vec<CertificateDer<'static>> {
    vec![CertificateDer::from(self.cert_der.clone())]
  }

  pub(crate) fn private_key(&self) -> PrivateKeyDer<'static> {
    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone()))
  }
}

/// Makes `dir` and its missing parents with the mode 0700 on Unix. A directory that exists keeps its mode
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
  let mut builder = fs::DirBuilder::new();
  builder.recursive(true);
  #[cfg(unix)]
  std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
  builder
    .create(dir)
    .map_err(|error| io::Error::new(error.kind(), format!("cannot create {}: {error}", dir.display())))
}

/// The count of the temporary files of this process, so two threads never share one
static TEMPORARIES: AtomicU64 = AtomicU64::new(0);

/// A temporary file, removed on every way out. A file that took its place by a rename is gone already
struct Temporary(PathBuf);

impl Drop for Temporary {
  fn drop(&mut self) {
    let _ = fs_retry::remove_file(&self.0);
  }
}

/// Writes `bytes` to a new file beside `path` that only its owner can read, and answers its path
fn write_temporary(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
  let temp = PathBuf::from(format!("{}.tmp-{}-{}", path.display(), std::process::id(), TEMPORARIES.fetch_add(1, Ordering::Relaxed)));
  let mut options = fs::OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
  let written = options.open(&temp).and_then(|mut file| {
    file.write_all(bytes)?;
    file.sync_all()
  });
  match written {
    Ok(()) => Ok(temp),
    Err(error) => {
      let _ = fs_retry::remove_file(&temp);
      Err(named(&error, path))
    }
  }
}

fn named(error: &io::Error, path: &Path) -> io::Error {
  io::Error::new(error.kind(), format!("cannot write {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "identity_test.rs"]
mod identity_test;
