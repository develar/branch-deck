//! The relay of a connection through `/usr/bin/nc` on macOS (openspec/specs/cli/host/spec.md, "The link").
//!
//! macOS refuses a connection to the local network with `EHOSTUNREACH` when the responsible process of the client has
//! no Local Network permission. An app without a team ID, such as a local build of the IDE, gets no permission even
//! when its switch is on. A process that is responsible for itself, such as a `launchd` job, gets none either, unless it
//! is a binary of the system. So the client starts the system `nc` responsible for itself (see the `disclaim` module),
//! connects it to the agent, and talks TLS over its stdin and stdout, which the client reads and writes as one tokio
//! stream. TLS and the pinned certificates stay end to end: `nc` carries only the encrypted bytes.
//!
//! The process remembers each address whose direct connect got the refusal, so each later connect to it goes through
//! the relay at once. A run opens several connections, and each of them would fail a direct connect first.

use crate::channel::LinkError;
use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;
use sync_utils::child_process::windowless_command;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, AsyncWrite, BufReader, Lines, ReadBuf};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::time::timeout;
use tracing::Instrument as _;

/// The system `nc`.
pub(crate) const NC: &str = "/usr/bin/nc";

/// The addresses of this process whose direct connect got the refusal of the Local Network permission.
static RELAYED: Mutex<BTreeSet<IpAddr>> = Mutex::new(BTreeSet::new());

/// Remembers that a direct connect to `ip` got the refusal of the Local Network permission.
pub(crate) fn remember(ip: IpAddr) {
  RELAYED.lock().unwrap_or_else(PoisonError::into_inner).insert(ip);
}

/// Whether a direct connect to `ip` got the refusal of the Local Network permission in this process.
pub(crate) fn is_remembered(ip: IpAddr) -> bool {
  RELAYED.lock().unwrap_or_else(PoisonError::into_inner).contains(&ip)
}

/// Whether a failed connect is the refusal of the Local Network permission, which the relay can pass.
pub(crate) fn is_local_network_refusal(error: &io::Error) -> bool {
  cfg!(target_os = "macos") && error.kind() == io::ErrorKind::HostUnreachable && Path::new(NC).exists()
}

/// Why macOS Local Network privacy can be the cause of `error` on a connect to `address`, or `None`. It can be when the
/// connect failed with `EHOSTUNREACH` on an address of the local network, and the relay cannot pass the refusal.
pub fn local_network_cause(error: &LinkError, address: SocketAddr) -> Option<String> {
  match error {
    LinkError::Unreachable(error) if cfg!(target_os = "macos") => cause(error.kind(), address.ip(), unavailable()),
    _ => None,
  }
}

fn cause(kind: io::ErrorKind, ip: IpAddr, unavailable: Option<String>) -> Option<String> {
  if kind != io::ErrorKind::HostUnreachable || !is_local(ip) {
    return None;
  }
  unavailable.map(|reason| {
        format!(
            "macOS Local Network privacy can refuse {ip}, because {reason}. Then the app that started this process needs the Local Network permission (System Settings, Privacy & Security, Local Network)"
        )
    })
}

/// Why the relay cannot pass a refusal of the Local Network permission, or `None` when it can.
fn unavailable() -> Option<String> {
  if !Path::new(NC).exists() {
    return Some(format!("there is no {NC}"));
  }
  #[cfg(target_os = "macos")]
  let disclaim = crate::disclaim::unavailable();
  #[cfg(not(target_os = "macos"))]
  let disclaim = None::<String>;
  disclaim.map(|reason| format!("{NC} cannot be responsible for itself: {reason}"))
}

/// Whether `ip` is an address of the local network: a private or a link-local one.
const fn is_local(ip: IpAddr) -> bool {
  match ip {
    IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
    IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
  }
}

/// The pipes of `nc` connected to `address`, as one stream: its stdout is what the agent sends, its stdin what it gets.
/// Answers when `nc` has connected, or the error that `nc` gave. `deadline` bounds the connect of `nc`. The drop of the
/// stream ends `nc`
pub(crate) async fn connect(address: SocketAddr, deadline: Duration) -> io::Result<Relay> {
  connect_with(NC, address, deadline).await
}

pub(crate) async fn connect_with(nc: &str, address: SocketAddr, deadline: Duration) -> io::Result<Relay> {
  let host = match address {
    SocketAddr::V6(v6) if v6.scope_id() != 0 => format!("{}%{}", v6.ip(), v6.scope_id()),
    _ => address.ip().to_string(),
  };
  let seconds = deadline.as_secs().max(1).to_string();
  let mut command = windowless_command(nc).into_command();
  command
    .args(["-v", "-G", &seconds, &host, &address.port().to_string()])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  // Without the disclaim, `nc` keeps the responsible process of the client, and `local_network_cause` names why.
  #[cfg(target_os = "macos")]
  let _ = crate::disclaim::disclaim(&mut command);
  let mut command = tokio::process::Command::from(command);
  command.kill_on_drop(true);
  let mut child = command.spawn().map_err(|error| io::Error::new(error.kind(), format!("cannot start {nc}: {error}")))?;
  let (Some(stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take()) else {
    return Err(io::Error::other(format!("{nc} has no pipes")));
  };
  // `nc -G` ends its own connect at the deadline; one more second lets it say so.
  let mut lines = BufReader::new(stderr).lines();
  let said = timeout(deadline + Duration::from_secs(1), connect_line(&mut lines))
    .await
    .unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "nc did not connect in time")));
  said?;
  // the rest of the stderr is read and dropped, so `nc` never blocks on a full pipe
  tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} }.in_current_span());
  Ok(Relay { stdin, stdout, _child: child })
}

/// Reads the stderr of `nc -v` up to its connect line
async fn connect_line(lines: &mut Lines<BufReader<ChildStderr>>) -> io::Result<()> {
  let mut first = String::new();
  loop {
    match lines.next_line().await? {
      Some(line) if line.contains("succeeded") => return Ok(()),
      Some(line) => {
        if first.is_empty() {
          first = line;
        }
      }
      None => {
        return Err(io::Error::new(io::ErrorKind::HostUnreachable, format!("nc: {}", first.trim().trim_start_matches("nc: "))));
      }
    }
  }
}

/// The connection through `nc`: the bytes from the agent come on its stdout, the bytes to it go on its stdin. The drop
/// kills `nc`, which ends its connection
#[derive(Debug)]
pub struct Relay {
  stdin: ChildStdin,
  stdout: ChildStdout,
  _child: Child,
}

impl AsyncRead for Relay {
  fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.stdout).poll_read(cx, buf)
  }
}

impl AsyncWrite for Relay {
  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
    Pin::new(&mut self.stdin).poll_write(cx, buf)
  }

  fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.stdin).poll_flush(cx)
  }

  /// Closes the stdin of `nc`, which ends the half of the connection to the agent
  fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.stdin).poll_shutdown(cx)
  }
}

#[cfg(test)]
#[path = "relay_test.rs"]
mod relay_test;
