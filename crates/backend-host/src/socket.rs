//! The local socket of the backend (docs/design/backend-protocol.md, "The socket"): one backend per key, 16 hex digits
//! of a hash of the data directory and the path of the executable ([`key`]), so a dev build beside a release has its
//! own. The directory of the key ([`dir_of`]) holds `backend.lock`, which the backend holds for the life of its process
//! ([`Listener`]), and on Unix the socket `backend.sock`.
//!
//! - **Unix**: `<base>/branch-deck-<key>/backend.sock`, the base `$TMPDIR` on macOS, `$XDG_RUNTIME_DIR` else, then the
//!   temporary directory, the path within the 104 bytes of `sun_path`. The directory is the owner's with mode 0700: the
//!   backend creates it so, and both sides refuse one that is not, with no fallback. Each side checks the uid of its
//!   peer and closes a connection of another user before it reads a message.
//! - **Windows**: the pipe `\\.\pipe\branch-deck-<key>`, every instance with a DACL of its owner alone and remote
//!   clients refused; the client checks the SID of the process of the server. The directory of the key is
//!   `%LOCALAPPDATA%\branch-deck-<key>\`.
//!
//! A second start of the app is a client: it sends `x-branch-deck/showMain` ([`show_main`]) and exits.

use crate::host::Host;
use crate::stream::serve_stream;
use serde_json::{Value, json};
use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::Instrument;

/// The file of the lock in the directory of the key
pub const LOCK_FILE: &str = "backend.lock";
/// The socket in the directory of the key (Unix)
pub const SOCKET_FILE: &str = "backend.sock";
/// The prefix of the directory of a key, and of the pipe on Windows
const PREFIX: &str = "branch-deck-";
/// How long a starting backend retries the lock that another holds
const LOCK_WAIT: Duration = Duration::from_secs(5);
/// The longest path of a Unix socket: `sun_path` of macOS, the shorter of the platforms
pub const SUN_PATH_MAX: usize = 104;

/// Why the socket cannot be served or reached
#[derive(Debug)]
pub enum SocketError {
  /// The directory of the key is not the owner's with mode 0700 (or not a directory)
  Directory {
    path: PathBuf,
    why: String,
  },
  /// The path of the socket passes `sun_path`
  PathTooLong {
    path: PathBuf,
    len: usize,
  },
  /// Another backend holds the lock of the key
  Held {
    lock: PathBuf,
  },
  /// The peer runs as another user
  OtherUser {
    what: String,
  },
  /// The backend answered with an error
  Refused {
    message: String,
  },
  Io {
    what: String,
    error: std::io::Error,
  },
}

impl fmt::Display for SocketError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Directory { path, why } => write!(f, "The directory of the backend {} is refused: {why}", path.display()),
      Self::PathTooLong { path, len } => write!(f, "The socket path {} is {len} bytes, past the {SUN_PATH_MAX} bytes of sun_path", path.display()),
      Self::Held { lock } => write!(f, "Another backend holds {}", lock.display()),
      Self::OtherUser { what } => write!(f, "The {what} runs as another user"),
      Self::Refused { message } => write!(f, "The backend refused: {message}"),
      Self::Io { what, error } => write!(f, "Cannot {what}: {error}"),
    }
  }
}

impl std::error::Error for SocketError {}

fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> SocketError {
  let what = what.into();
  move |error| SocketError::Io { what, error }
}

/// The key of a backend: 16 hex digits of the FNV-1a hash of the data directory and the path of the executable, the
/// same in every build (no hasher of the standard library, whose output may change between releases)
pub fn key(data_dir: &Path, executable: &Path) -> String {
  const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const PRIME: u64 = 0x0100_0000_01b3;
  let mut hash = OFFSET;
  let bytes = data_dir.as_os_str().as_encoded_bytes().iter().chain(&[0]).chain(executable.as_os_str().as_encoded_bytes());
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(PRIME);
  }
  format!("{hash:016x}")
}

/// The directory of the key `key`: `<base>/branch-deck-<key>` on Unix, `%LOCALAPPDATA%\branch-deck-<key>` on Windows
pub fn dir_of(key: &str) -> PathBuf {
  base().join(format!("{PREFIX}{key}"))
}

/// The key of the backend of the app at `executable` on the data directory `data_dir`: `key` of the executable as the
/// file system resolves it (a `brd` symlink, a path with `..`), so the app and the bridge it starts name one key
pub fn app_key(data_dir: &Path, executable: &Path) -> String {
  let resolved = std::fs::canonicalize(executable).unwrap_or_else(|_| executable.to_owned());
  key(data_dir, &resolved)
}

/// Creates the directory of a key owner-only when it is missing, and refuses one that is not the owner's (the bridge
/// takes the lock of a start in it before the backend exists)
///
/// # Errors
/// The directory cannot be created, or is refused
pub fn prepare_dir(dir: &Path) -> Result<(), SocketError> {
  create_dir(dir)?;
  check_dir(dir)
}

/// Whether another process holds `backend.lock` of `dir` now: a backend lives, or one starts
pub fn lock_held(dir: &Path) -> bool {
  let Ok(file) = std::fs::OpenOptions::new().write(true).open(dir.join(LOCK_FILE)) else {
    return false;
  };
  matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

#[cfg(target_os = "macos")]
fn base() -> PathBuf {
  std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn base() -> PathBuf {
  std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from)
}

#[cfg(windows)]
fn base() -> PathBuf {
  std::env::var_os("LOCALAPPDATA").map_or_else(std::env::temp_dir, PathBuf::from)
}

/// The lock of the key, held while it lives
#[derive(Debug)]
pub struct BackendLock {
  _file: File,
}

/// Takes the lock of `dir`, retrying for `wait` while another holds it
///
/// # Errors
/// Another holds it after `wait`, or the file cannot be opened
pub async fn take_lock(dir: &Path, wait: Duration) -> Result<BackendLock, SocketError> {
  let path = dir.join(LOCK_FILE);
  let file = std::fs::OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(&path)
    .map_err(io(format!("open {}", path.display())))?;
  let deadline = tokio::time::Instant::now() + wait;
  loop {
    match file.try_lock() {
      Ok(()) => return Ok(BackendLock { _file: file }),
      Err(std::fs::TryLockError::WouldBlock) if tokio::time::Instant::now() < deadline => {
        tokio::time::sleep(Duration::from_millis(100)).await;
      }
      Err(std::fs::TryLockError::WouldBlock) => return Err(SocketError::Held { lock: path }),
      Err(std::fs::TryLockError::Error(error)) => return Err(io(format!("lock {}", path.display()))(error)),
    }
  }
}

/// Whether a peer of `peer` is of the user `own`: a connection of another user is closed before it is read
pub const fn same_user(peer: u32, own: u32) -> bool {
  peer == own
}

/// The listening backend of a key: the lock of the key and its socket
pub struct Listener {
  dir: PathBuf,
  _lock: BackendLock,
  #[cfg(unix)]
  socket: tokio::net::UnixListener,
  #[cfg(windows)]
  pipe: String,
  #[cfg(windows)]
  first: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl fmt::Debug for Listener {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Listener").field("dir", &self.dir).finish_non_exhaustive()
  }
}

impl Listener {
  /// The backend of the directory `dir`: creates it owner-only, takes its lock (retrying a few seconds), removes a
  /// stale socket under it and listens
  ///
  /// # Errors
  /// The directory is refused, another backend holds the lock, or the socket cannot be bound
  pub async fn bind(dir: &Path) -> Result<Self, SocketError> {
    Self::bind_waiting(dir, LOCK_WAIT).await
  }

  /// `bind`, retrying the lock for `wait`
  ///
  /// # Errors
  /// As `bind`
  pub async fn bind_waiting(dir: &Path, wait: Duration) -> Result<Self, SocketError> {
    create_dir(dir)?;
    check_dir(dir).inspect_err(|error| tracing::warn!(%error, "the backend opens no socket"))?;
    let lock = take_lock(dir, wait).await?;
    Self::listen(dir, lock)
  }

  #[cfg(unix)]
  fn listen(dir: &Path, lock: BackendLock) -> Result<Self, SocketError> {
    let path = socket_path(dir)?;
    // the lock is ours, so no backend listens on a socket file that is there: a stale one of a backend that died
    if std::fs::symlink_metadata(&path).is_ok() {
      sync_utils::fs_retry::remove_file(&path).map_err(io(format!("remove the stale socket {}", path.display())))?;
    }
    let listener = tokio::net::UnixListener::bind(&path).map_err(io(format!("listen on {}", path.display())))?;
    Ok(Self {
      dir: dir.to_owned(),
      _lock: lock,
      socket: listener,
    })
  }

  #[cfg(windows)]
  fn listen(dir: &Path, lock: BackendLock) -> Result<Self, SocketError> {
    let pipe = pipe_name(dir);
    let first = windows::create_instance(&pipe, true).map_err(io(format!("create the pipe {pipe}")))?;
    Ok(Self {
      dir: dir.to_owned(),
      _lock: lock,
      pipe,
      first,
    })
  }

  /// The directory of the key
  pub fn dir(&self) -> &Path {
    &self.dir
  }

  /// Serves each client that connects on a connection of `host` of its own, until the listener fails; a peer of another
  /// user is closed before it is read
  #[cfg(unix)]
  pub async fn serve(self, host: Arc<Host>) {
    let own = unix::euid();
    loop {
      let stream = match self.socket.accept().await {
        Ok((stream, _)) => stream,
        Err(error) => {
          tracing::warn!(%error, "the backend accepts no more connections");
          return;
        }
      };
      match stream.peer_cred() {
        Ok(peer) if same_user(peer.uid(), own) => {
          tokio::spawn(serve_stream(host.connect(), stream).in_current_span());
        }
        Ok(peer) => tracing::warn!(uid = peer.uid(), "a peer of another user is closed"),
        Err(error) => tracing::warn!(%error, "a peer whose user is unknown is closed"),
      }
    }
  }

  /// Serves each client that connects on a connection of `host` of its own, until the pipe fails
  #[cfg(windows)]
  pub async fn serve(self, host: Arc<Host>) {
    let mut server = self.first;
    loop {
      if let Err(error) = server.connect().await {
        tracing::warn!(%error, "the backend accepts no more connections");
        return;
      }
      let next = match windows::create_instance(&self.pipe, false) {
        Ok(next) => next,
        Err(error) => {
          tracing::warn!(%error, "the backend creates no more instances of its pipe");
          return;
        }
      };
      let connected = std::mem::replace(&mut server, next);
      tokio::spawn(serve_stream(host.connect(), connected).in_current_span());
    }
  }
}

/// The stream of a client to the backend
#[cfg(unix)]
pub type Stream = tokio::net::UnixStream;
/// The stream of a client to the backend
#[cfg(windows)]
pub type Stream = tokio::net::windows::named_pipe::NamedPipeClient;

/// Connects to the backend of `dir`: its directory checked, its server of this user
///
/// # Errors
/// The directory is refused, no backend listens, or it runs as another user
#[cfg(unix)]
pub async fn connect(dir: &Path) -> Result<Stream, SocketError> {
  // no directory is no backend yet, which a bridge then starts: a refused one is told by the backend that binds it
  check_dir(dir).inspect_err(|error| tracing::debug!(%error, "the backend is not reached"))?;
  let path = socket_path(dir)?;
  let stream = tokio::net::UnixStream::connect(&path).await.map_err(io(format!("connect to {}", path.display())))?;
  let peer = stream.peer_cred().map_err(io("read the user of the backend"))?;
  if !same_user(peer.uid(), unix::euid()) {
    return Err(SocketError::OtherUser { what: "backend".to_owned() });
  }
  Ok(stream)
}

/// Connects to the backend of `dir`: its server of this user
///
/// # Errors
/// No backend listens, or it runs as another user
#[cfg(windows)]
pub async fn connect(dir: &Path) -> Result<Stream, SocketError> {
  let pipe = pipe_name(dir);
  let client = windows::open(&pipe).await.map_err(io(format!("connect to {pipe}")))?;
  if !windows::server_is_own(&client).map_err(io("read the user of the backend"))? {
    return Err(SocketError::OtherUser { what: "backend".to_owned() });
  }
  Ok(client)
}

/// A second start of the app: connects to the backend of `dir`, initializes as the app of `build` and asks it to show
/// the main window (`x-branch-deck/showMain`); answers once the backend answered
///
/// # Errors
/// No backend is reached, or it refused (`otherBuild`: another build)
pub async fn show_main(dir: &Path, build: &str) -> Result<(), SocketError> {
  let stream = connect(dir).await?;
  let (read, mut write) = tokio::io::split(stream);
  let initialize = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {
      "channel": crate::wire::ROOT,
      "protocolVersions": ["1.0.0"],
      "clientId": "branch-deck-app",
      "clientInfo": { "name": "branch-deck-app", "build": build },
      "_meta": { "x-branch-deck": { "version": crate::wire::VOCABULARY_VERSION, "capabilities": {} } },
    },
  });
  let show = json!({ "jsonrpc": "2.0", "id": 2, "method": "x-branch-deck/showMain", "params": { "channel": crate::wire::ROOT } });
  let mut lines = BufReader::new(read).lines();
  for request in [initialize, show] {
    let id = request["id"].clone();
    let mut line = request.to_string();
    line.push('\n');
    write.write_all(line.as_bytes()).await.map_err(io("write to the backend"))?;
    write.flush().await.map_err(io("write to the backend"))?;
    loop {
      let Some(answer) = lines.next_line().await.map_err(io("read the backend"))? else {
        return Err(SocketError::Refused {
          message: "the backend closed the connection".to_owned(),
        });
      };
      let Ok(answer) = serde_json::from_str::<Value>(&answer) else {
        continue;
      };
      if answer["id"] != id {
        continue;
      }
      if let Some(error) = answer.get("error") {
        return Err(SocketError::Refused {
          message: error["message"].as_str().unwrap_or("no reason").to_owned(),
        });
      }
      break;
    }
  }
  Ok(())
}

/// Creates `dir` owner-only (mode 0700 on Unix) when it is missing
fn create_dir(dir: &Path) -> Result<(), SocketError> {
  #[cfg(unix)]
  let created = {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(dir)
  };
  #[cfg(not(unix))]
  let created = std::fs::create_dir_all(dir);
  match created {
    Ok(()) => Ok(()),
    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
    Err(error) => Err(io(format!("create {}", dir.display()))(error)),
  }
}

/// Refuses a directory of the key that is no directory of this user with mode 0700 (Unix); a link is refused too
///
/// # Errors
/// Why the directory is refused
#[cfg(unix)]
pub fn check_dir(dir: &Path) -> Result<(), SocketError> {
  use std::os::unix::fs::MetadataExt;
  let refused = |why: String| SocketError::Directory { path: dir.to_owned(), why };
  let metadata = std::fs::symlink_metadata(dir).map_err(|error| refused(error.to_string()))?;
  if !metadata.is_dir() {
    return Err(refused("not a directory".to_owned()));
  }
  if metadata.uid() != unix::euid() {
    return Err(refused(format!("owned by the uid {}", metadata.uid())));
  }
  let mode = metadata.mode() & 0o7777;
  if mode != 0o700 {
    return Err(refused(format!("mode {mode:o}, not 700")));
  }
  Ok(())
}

/// The user's `%LOCALAPPDATA%` is the user's: the pipe has a DACL of its own
///
/// # Errors
/// The directory is missing
#[cfg(windows)]
pub fn check_dir(dir: &Path) -> Result<(), SocketError> {
  if dir.is_dir() {
    Ok(())
  } else {
    Err(SocketError::Directory {
      path: dir.to_owned(),
      why: "not a directory".to_owned(),
    })
  }
}

/// The socket of `dir`, within `sun_path`
#[cfg(unix)]
fn socket_path(dir: &Path) -> Result<PathBuf, SocketError> {
  let path = dir.join(SOCKET_FILE);
  let len = path.as_os_str().len();
  // the NUL that ends it takes the last byte
  if len >= SUN_PATH_MAX {
    return Err(SocketError::PathTooLong { path, len });
  }
  Ok(path)
}

/// The pipe of `dir`: `\\.\pipe\` and the name of the directory of the key
#[cfg(windows)]
pub fn pipe_name(dir: &Path) -> String {
  let name = dir.file_name().map_or_else(|| PREFIX.to_owned(), |name| name.to_string_lossy().into_owned());
  format!(r"\\.\pipe\{name}")
}

#[cfg(unix)]
#[expect(unsafe_code, reason = "geteuid(2), which std does not wrap")]
mod unix {
  /// The effective uid of this process
  pub(super) fn euid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail
    unsafe { libc::geteuid() }
  }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "the security descriptor of a pipe and the token of a process, which std and tokio do not wrap")]
mod windows {
  //! The pipe of the backend: each instance with a DACL of its owner alone, remote clients refused; the SID of the
  //! process of the server, compared with the own one by the client

  use std::ffi::c_void;
  use std::io;
  use std::os::windows::io::AsRawHandle;
  use std::time::Duration;
  use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};
  use windows_sys::Win32::Foundation::{CloseHandle, ERROR_PIPE_BUSY, HANDLE, LocalFree};
  use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
  use windows_sys::Win32::Security::{GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
  use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
  use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION};

  /// A handle closed on drop
  struct Owned(HANDLE);

  impl Drop for Owned {
    fn drop(&mut self) {
      // SAFETY: the handle was opened by this module and is closed once
      unsafe { CloseHandle(self.0) };
    }
  }

  /// The SID of the user of the process `process`, as its string (`S-1-5-21-…`)
  fn sid_of(process: HANDLE) -> io::Result<String> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: a valid process handle, a pointer to a handle
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut token) } == 0 {
      return Err(io::Error::last_os_error());
    }
    let token = Owned(token);
    let mut len = 0u32;
    // SAFETY: the first call asks the size of the buffer
    unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &raw mut len) };
    let mut buffer = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: a buffer of `len` bytes, aligned for TOKEN_USER
    if unsafe { GetTokenInformation(token.0, TokenUser, buffer.as_mut_ptr().cast(), len, &raw mut len) } == 0 {
      return Err(io::Error::last_os_error());
    }
    // SAFETY: the buffer holds a TOKEN_USER now
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut text: *mut u16 = std::ptr::null_mut();
    // SAFETY: the SID of the token, a pointer to the string the call allocates
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut text) } == 0 {
      return Err(io::Error::last_os_error());
    }
    // SAFETY: a NUL-terminated string of the call, freed once read
    let sid = unsafe {
      let mut len = 0;
      while *text.add(len) != 0 {
        len += 1;
      }
      let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
      LocalFree(text.cast());
      sid
    };
    Ok(sid)
  }

  /// The SID of the user of this process
  fn own_sid() -> io::Result<String> {
    // SAFETY: the pseudo handle of this process needs no close
    sid_of(unsafe { GetCurrentProcess() })
  }

  /// A new instance of the pipe `name`, its DACL the owner's alone and remote clients refused; `first` refuses a name
  /// another server holds
  pub(super) fn create_instance(name: &str, first: bool) -> io::Result<NamedPipeServer> {
    let sddl: Vec<u16> = format!("D:P(A;;GA;;;{})", own_sid()?).encode_utf16().chain([0]).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: a NUL-terminated SDDL, a pointer to the descriptor the call allocates
    if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &raw mut descriptor, std::ptr::null_mut()) } == 0 {
      return Err(io::Error::last_os_error());
    }
    let mut attributes = SECURITY_ATTRIBUTES {
      nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
      lpSecurityDescriptor: descriptor,
      bInheritHandle: 0,
    };
    // SAFETY: the attributes and their descriptor live until the call returned
    let created = unsafe {
      ServerOptions::new()
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .create_with_security_attributes_raw(name, (&raw mut attributes).cast::<c_void>())
    };
    // SAFETY: the descriptor of the call above, freed once
    unsafe { LocalFree(descriptor) };
    created
  }

  /// A client of the pipe `name`, retrying a busy one for a second
  pub(super) async fn open(name: &str) -> io::Result<NamedPipeClient> {
    for _ in 0..20 {
      match ClientOptions::new().open(name) {
        Err(error) if error.raw_os_error() == i32::try_from(ERROR_PIPE_BUSY).ok() => {
          tokio::time::sleep(Duration::from_millis(50)).await;
        }
        opened => return opened,
      }
    }
    ClientOptions::new().open(name)
  }

  /// The process of the server of `client` runs as this user
  pub(super) fn server_is_own(client: &NamedPipeClient) -> io::Result<bool> {
    let mut pid = 0u32;
    // SAFETY: the handle of an open pipe, a pointer to the id
    if unsafe { GetNamedPipeServerProcessId(client.as_raw_handle(), &raw mut pid) } == 0 {
      return Err(io::Error::last_os_error());
    }
    // SAFETY: the id of a process; the handle is closed on drop
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
      return Err(io::Error::last_os_error());
    }
    let process = Owned(process);
    Ok(sid_of(process.0)? == own_sid()?)
  }
}

#[cfg(test)]
#[path = "socket_test.rs"]
mod socket_test;
